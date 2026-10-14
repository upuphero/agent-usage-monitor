$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
$artifactDirectory = Join-Path $projectRoot 'artifacts\windows'
$portableDirectory = Join-Path $projectRoot 'artifacts\portable'
$binary = Join-Path $projectRoot 'target\release\monitor-desktop.exe'
$installerDirectory = Join-Path $projectRoot 'target\release\bundle\nsis'
if (-not (Test-Path -LiteralPath $binary)) { throw 'Desktop executable missing' }
$installers = @(Get-ChildItem -LiteralPath $installerDirectory -Filter '*.exe' -File)
if ($installers.Count -ne 1) { throw 'Expected one NSIS installer' }
$env:AGENT_USAGE_MONITOR_DATA_DIR = Join-Path $projectRoot '.local-data\package-smoke'
$smoke = Start-Process -FilePath $binary -ArgumentList '--verify-synthetic-store' -Wait -PassThru -WindowStyle Hidden
if ($smoke.ExitCode -ne 0) { throw 'Packaged application synthetic storage smoke failed' }
$localSmoke = Start-Process -FilePath $binary -ArgumentList '--verify-local-collector' -Wait -PassThru -WindowStyle Hidden
if ($localSmoke.ExitCode -ne 0) { throw 'Packaged application incremental collector smoke failed' }
New-Item -ItemType Directory -Path $artifactDirectory, $portableDirectory -Force | Out-Null
Copy-Item -LiteralPath $installers[0].FullName -Destination (Join-Path $artifactDirectory "Agent-Usage-Monitor_${version}_x64-setup.exe")
Copy-Item -LiteralPath $binary -Destination $portableDirectory
Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\monitor-collector.exe') -Destination $portableDirectory
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs\device-collection.md') -Destination $portableDirectory
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $portableDirectory
Copy-Item -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md') -Destination $portableDirectory
$portableZip = Join-Path $artifactDirectory "Agent-Usage-Monitor_${version}_x64-portable.zip"
Compress-Archive -Path (Join-Path $portableDirectory '*') -DestinationPath $portableZip -Force
$checksums = Get-ChildItem -LiteralPath $artifactDirectory -File | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object Name | ForEach-Object {
    $hash = Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256
    "$($hash.Hash.ToLowerInvariant())  $($_.Name)"
}
Set-Content -LiteralPath (Join-Path $artifactDirectory 'SHA256SUMS.txt') -Value $checksums -Encoding utf8
Write-Output "Staged Windows installer, portable ZIP and SHA-256 checksums."
