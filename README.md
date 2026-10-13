# Agent Usage Monitor

一个容纳多家 Agent 服务的独立 Windows 用量分析应用。沿用 Agent Usage Dashboard 的深灰、薄荷绿 UI，首先建立 Codex 账号与 Windows 本机双账本；后续服务商通过独立适配器接入。

## P0 状态

当前版本 `0.1.0` 是**合成数据预览**：概览、历史、模型与会话、质量、设置五个页面；正常、缺失分量、负差值、空数据四种场景；中文/英文和深浅主题。

独立 Tauri 2 + React/TypeScript + Rust + SQLite 工程已初始化。真实日志发现、增量采集、账号 API 与真实设备校准尚未接入。Claude Code 与 Antigravity 显示为后续服务商，没有伪造用量。

账号服务端量可能已包含本机量，二者不相加。Token 使用十进制字符串；未知值为 null；诊断差值保留负数，不命名为 MacBook 用量。P0 没有网络客户端、凭据读取、后台采样、自启动或旧应用 sidecar。

## 本地运行

Node `24.19.0`、pnpm `9.15.0`、Rust `1.91.1`。Windows 原生构建需要 MSVC 与 Windows SDK。

```powershell
pnpm install --frozen-lockfile
pnpm dev
```

浏览器预览：`http://127.0.0.1:1430`，使用内存合成数据。

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

GitHub Actions 在 Linux 执行前端与核心验证，在 Windows 编译、测试并打包 NSIS 安装包和便携 ZIP，附 SHA-256 校验值。产物位于对应工作流的 Artifacts，当前版本不自动发布 Release。

## 工程入口

| 目录 | 内容 |
| --- | --- |
| `apps/dashboard` | 复用视觉系统的 React 界面与 UsageClient |
| `apps/desktop/src-tauri` | 独立窗口、SQLite 组装与最小 IPC |
| `crates/monitor-contracts` | Rust DTO，生成前端类型 |
| `crates/monitor-core` | Token 校验、双账本比较、阻断理由 |
| `crates/monitor-adapters` | 事务迁移、合成缓存与追加快照 |
| `tests/fixtures` | 可复现合成数据，无真实账号或日志 |

数据协议见 [api-contracts.md](docs/api-contracts.md)，验证记录见 [p0-validation.md](docs/p0-validation.md)。复用来源与许可见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 和 [LICENSE](LICENSE)。

原始规格书和历史证据包保留于本地 `docs/`，不纳入 public 仓库；其来源文件与哈希保持原样。
