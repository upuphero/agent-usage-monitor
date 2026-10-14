# Agent Usage Monitor

一个容纳多家 Agent 服务的独立 Windows 用量分析应用。沿用 Agent Usage Dashboard 的深灰、薄荷绿 UI，首先建立 Codex 账号与 Windows 本机双账本；后续服务商通过独立适配器接入。

## 0.4.0：账号额度和设备日志分开

概览分别显示账号的 5 小时/每周额度、Windows 日志 Token、Mac 日志 Token、设备日志去重汇总。账号历史在使用历史页单独展开，曲线只画本机日志。不通过账号减 Windows 推算 Mac；未导入 Mac 日志时显示“未接入”。日志日期选择只来自设备记录，右侧组成使用所选 UTC 日期。

Windows 桌面端同步本机日志，设置中支持导出、导入和移除设备报告。Mac 的独立命令行采集器读取本机 Codex 日志并生成 JSON，复制到 Windows 后导入；Apple Silicon / Intel 二进制随同版本 Release 发布，不需要 Rust。便携版同时附 Windows 采集器。详细步骤和汇总边界见 [device-collection.md](docs/device-collection.md)。

报告只包含匿名事件标记、UTC 时间、Token 和采集状态，不含对话正文、路径、工具输入、模型或登录信息。相同报告幂等，新报告按设备替换，旧报告拒绝覆盖；相同观察去重，冲突排除。统计代表日志所覆盖部分，复制日志的采集设备不等于执行设备，不能保证全部请求都被识别或完整设备消费。主线程和子代理独立选择。

账号查询保持 0.3.2 的首次说明、默认开启及可关闭选择；额度使用现有账号查询接口，独立于日志 Token。首次说明处理前不读取登录；关闭后停止后续请求并保留缓存。后端固定 GET、TLS 校验、无重定向，凭据只在内存短暂使用。账号历史、额度与设备日志互不相加。

[v0.4.0 Release](https://github.com/upuphero/agent-usage-monitor/releases/tag/v0.4.0) 已公开，Windows 安装版、便携版、Mac Apple Silicon/Intel 采集器与 SHA-256 清单分别下载。[完整验证记录](docs/release-validation.md) 包含 CI、公开文件哈希与本机原生合成检查。

Schema 4 保留两个历史账本、账号快照、增量检查点、安装标记和关闭选择；仅新增设备报告表。CI 在 Linux、Windows 和两种 Mac 架构测试，再发布不可变版本 tag 的 Release。当前开发验证和构建使用合成日志；没有读取你的 Mac 或查询真实账号。

## A 阶段本机账本

A 阶段 `0.2.0` 接入 Windows Codex 本机日志：默认先显示独立 SQLite 缓存，再增量同步；概览、历史、模型与会话、质量、设置五个页面支持真实本机账本。四种 P0 合成场景保留为显式演示，不混入本机数据。

自动发现 `CODEX_HOME` 或用户 `.codex` 下的 `sessions`、`archived_sessions`、`session-cleanup-archive`，支持设置附加 Codex 根目录。新增完整 JSONL 行按事务持久化统计投影与检查点；半行等待后续追加；副本和归档按稳定事件身份去重；重写重新核对；退出后继续处理。来源删除后保留已验证历史并标记缺口。

累计回退、初始继承余额、增量/last 矛盾和跨日不明区间不计入有效增量，保留解释。主线程与子代理分开查询；日志账号使用不可逆标记且保持未核实。显示的是**已观测部分**，文件发现不等于完整设备消费。模型与 effort 是日志配置，speed 缺失保持未知。

Token 使用十进制字符串，缓存只计一次；缺失不补零，账号与本机量不相加。Claude Code 与 Antigravity 保持后续接入状态。

## 本地运行

Node `24.19.0`、pnpm `9.15.0`、Rust `1.91.1`。Windows 原生构建需要 MSVC 与 Windows SDK。

```powershell
pnpm install --frozen-lockfile
pnpm dev
```

浏览器默认预览：`http://127.0.0.1:1430`，使用内存合成数据。真实日志同步使用 Windows 桌面端。

```powershell
# 所有开发数据保存在本项目内
$env:AGENT_USAGE_MONITOR_DATA_DIR = Join-Path (Get-Location).Path '.local-data'
pnpm desktop:dev
```

正常桌面启动使用 `dev.agentusage.monitor` 对应的独立应用数据目录和 `monitor.sqlite3`。不会接管相邻旧应用的数据。语言、主题与显示时区使用独立偏好键。

## 验证与构建

```powershell
pnpm check
cargo fmt --all --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
pnpm desktop:build
```

GitHub Actions 在 Linux 执行前端与核心验证，在 Windows 编译、测试并打包 NSIS 安装包和便携 ZIP，在 Mac ARM64/Intel 原生测试并打包采集器，附 SHA-256 校验值。main 的验证全部通过后，自动按 `package.json` 版本创建 `v版本号` tag 和 GitHub Release，安装版 `.exe`、便携版 `.zip`、两种 Mac 采集器 `.tar.gz`、`SHA256SUMS.txt` 分别提供下载。PR 不发布；已有版本 tag 不移动，修改代码再次发布必须增加版本号。

## 工程入口

| 目录 | 内容 |
| --- | --- |
| `apps/dashboard` | 复用视觉系统的 React 界面与 UsageClient |
| `apps/desktop/src-tauri` | 独立窗口、SQLite 组装与最小 IPC |
| `crates/monitor-contracts` | Rust DTO，生成前端类型 |
| `crates/monitor-core` | Token 校验、双账本比较、阻断理由 |
| `crates/monitor-collector` | Windows/Mac/Linux 独立日志采集器 |
| `crates/monitor-adapters` | 增量日志、账号只读适配、事务迁移与追加快照 |
| `tests/fixtures` | 可复现合成数据，无真实账号或日志 |

数据协议见 [api-contracts.md](docs/api-contracts.md)，验证记录见 [p0-validation.md](docs/p0-validation.md)。复用来源与许可见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 和 [LICENSE](LICENSE)。

Windows `0.3.0` [安装包与便携包](https://github.com/upuphero/agent-usage-monitor/actions/runs/38479825578/artifacts/11857415660) 已产出，哈希见 [B 验证记录](docs/b-validation.md)。A 的历史 [CI 与本机验证](docs/api-contracts.md#a-阶段验证2026-10-12-americaphoenix) 单独保留。开发者可用 `--verify-local-collector` 检查合成日志的增量、半行、重启与归档行为，必须显式设置 `AGENT_USAGE_MONITOR_DATA_DIR`。`--sync-local` 按需读取本机日志并在该独立目录保存 `local-dashboard.json`；只读取账号缓存，不查询账号。该报告留在本机，禁止加入 public 提交。

开发期可把本机白名单报告放在项目 `.local-data/preview-dashboard.json`，用 `http://127.0.0.1:1430/?local-report=1` 检查只读界面。该专用预览只读固定报告文件，不提供任意文件路径读取或同步功能。

原始规格书和历史证据包保留于本地 `docs/`，不纳入 public 仓库；其来源文件与哈希保持原样。
