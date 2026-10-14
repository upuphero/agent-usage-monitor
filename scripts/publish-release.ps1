param(
    [Parameter(Mandatory = $true)][string]$ArtifactDirectory,
    [Parameter(Mandatory = $true)][string]$Commit,
    [Parameter(Mandatory = $true)][string]$Repository,
    [switch]$VerifyOnly
)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$artifactRoot = [IO.Path]::GetFullPath($ArtifactDirectory)
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
if ($version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z.-]+)?$') { throw 'Invalid package version' }
if ($Commit -notmatch '^[0-9a-f]{40}$' -or $Repository -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') { throw 'Invalid release target' }
$tag = "v$version"
$installerName = "Agent-Usage-Monitor_${version}_x64-setup.exe"
$portableName = "Agent-Usage-Monitor_${version}_x64-portable.zip"
$macArmName = "Agent-Usage-Monitor_${version}_macos-arm64-collector.tar.gz"
$macIntelName = "Agent-Usage-Monitor_${version}_macos-x64-collector.tar.gz"
$packageNames = @($installerName, $portableName, $macArmName, $macIntelName)
$manifestPath = Join-Path $artifactRoot 'SHA256SUMS.txt'
$expectedHashes = @{}
foreach ($line in Get-Content -LiteralPath $manifestPath) {
    if ($line -notmatch '^([0-9a-f]{64})  (.+)$') { throw 'Invalid SHA-256 manifest' }
    $hash = $Matches[1]; $name = $Matches[2]
    if ($name -notin $packageNames -or $expectedHashes.ContainsKey($name)) { throw 'Unexpected or duplicate package in manifest' }
    $expectedHashes[$name] = $hash
}
if ($expectedHashes.Count -ne 4) { throw 'Windows installer, portable and both Mac collectors are required' }
foreach ($name in $packageNames) {
    $path = Join-Path $artifactRoot $name
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedHashes[$name]) { throw "Package SHA-256 mismatch: $name" }
}
if ($VerifyOnly) { Write-Output "Verified release packages for $tag"; return }

function Get-TagCommit {
    $errorPath = Join-Path $artifactRoot 'tag-lookup-error.txt'
    $output = & gh api "repos/$Repository/git/ref/tags/$tag" 2> $errorPath
    if ($LASTEXITCODE -ne 0) {
        if ((Get-Content -LiteralPath $errorPath -Raw) -match '404') { return $null }
        throw 'Could not inspect the remote version tag'
    }
    $object = ($output | ConvertFrom-Json).object
    for ($depth = 0; $depth -lt 8 -and $object.type -eq 'tag'; $depth++) {
        $output = & gh api "repos/$Repository/git/tags/$($object.sha)"
        if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the annotated version tag' }
        $object = ($output | ConvertFrom-Json).object
    }
    if ($object.type -ne 'commit') { throw 'Version tag does not resolve to a commit' }
    return $object.sha
}
$tagCommit = Get-TagCommit
if ($tagCommit -and $tagCommit -ne $Commit) { throw 'This version tag belongs to another commit. Increment the version; existing tags are never moved.' }
if (-not $tagCommit) {
    $output = & gh api "repos/$Repository/git/refs" -f "ref=refs/tags/$tag" -f "sha=$Commit"
    if ($LASTEXITCODE -ne 0 -or ($output | ConvertFrom-Json).object.sha -ne $Commit) { throw 'Could not create the exact-commit version tag' }
    $tagCommit = $Commit
}

$releaseOutput = & gh release view $tag --repo $Repository --json isDraft,assets,url 2> $null
$releaseExists = $LASTEXITCODE -eq 0
if ($releaseExists) {
    $release = $releaseOutput | ConvertFrom-Json
    if (-not $tagCommit) { throw 'Existing release has no matching version tag' }
    if (-not $release.isDraft) {
        $names = @($release.assets | ForEach-Object name)
        foreach ($name in @($packageNames + 'SHA256SUMS.txt')) {
            if ($name -notin $names) { throw 'Published release is incomplete; existing release assets are never replaced' }
        }
        Write-Output "Release $tag already exists for this commit: $($release.url)"
        return
    }
}

$notesPath = Join-Path $artifactRoot 'release-notes.md'
@"
Agent Usage Monitor $version — Windows desktop + Mac log collectors

- **安装版：** $installerName
- **便携版：** $portableName，解压后运行 monitor-desktop.exe。
- **Mac Apple Silicon 采集器：** $macArmName
- **Mac Intel 采集器：** $macIntelName
- **校验：** 下载后可用 SHA256SUMS.txt 核对文件。

账号额度与设备 Token 分开显示。Windows 同步本机日志；Mac 解压采集器，在终端运行 ./monitor-collector collect --output mac-usage.json，再在 Windows 设置中导入该报告。报告更新按设备替换，相同日志记录去重，冲突不计入汇总。没有 Mac 报告时显示未接入；不使用账号减 Windows 推算 Mac。

报告只含匿名统计，不含对话正文或登录信息；汇总代表日志所覆盖部分。便携版同时包含 Windows 采集器。Mac 包为独立命令行采集器，附操作说明。账号查询默认开启，首次提示可以关闭，关闭选择会保留。

构建提交：$Commit
发布前已通过前端、Rust、Windows 原生、Mac ARM64/Intel 采集器及打包检查；CI 使用合成日志，没有查询真实账号。
"@ | Set-Content -LiteralPath $notesPath -Encoding utf8
$assets = @(
    "$(Join-Path $artifactRoot $installerName)#Windows installer",
    "$(Join-Path $artifactRoot $portableName)#Windows portable",
    "$(Join-Path $artifactRoot $macArmName)#Mac Apple Silicon log collector",
    "$(Join-Path $artifactRoot $macIntelName)#Mac Intel log collector",
    "$manifestPath#SHA-256 checksums"
)
if (-not $releaseExists) {
    $createArgs = @('release','create',$tag,'--repo',$Repository,'--target',$Commit,'--verify-tag','--draft','--title',"Agent Usage Monitor $tag",'--notes-file',$notesPath)
    if ($version.Contains('-')) { $createArgs += '--prerelease' }
    & gh @createArgs @assets
    if ($LASTEXITCODE -ne 0) { throw 'Could not create the versioned release draft' }
} else {
    & gh release upload $tag @assets --repo $Repository --clobber
    if ($LASTEXITCODE -ne 0) { throw 'Could not complete the matching release draft' }
}
if ((Get-TagCommit) -ne $Commit) { throw 'Release tag does not match the validated build commit' }
$releaseOutput = & gh release view $tag --repo $Repository --json isDraft,assets
if ($LASTEXITCODE -ne 0) { throw 'Could not verify the release draft' }
$draft = $releaseOutput | ConvertFrom-Json
if (-not $draft.isDraft) { throw 'Expected a draft before publication' }
foreach ($name in @($packageNames + 'SHA256SUMS.txt')) {
    $asset = @($draft.assets | Where-Object name -eq $name)
    if ($asset.Count -ne 1 -or $asset[0].size -ne (Get-Item -LiteralPath (Join-Path $artifactRoot $name)).Length) { throw 'Release draft asset verification failed' }
}
& gh release edit $tag --repo $Repository --draft=false
if ($LASTEXITCODE -ne 0) { throw 'Could not publish the verified release draft' }
Write-Output "Published $tag with separate installer, portable and SHA-256 assets."
