# Agent Usage Monitor

一个容纳多家 Agent 服务的独立 Windows 用量分析应用。沿用 Agent Usage Dashboard 的深灰、薄荷绿 UI，首先建立 Codex 账号与 Windows 本机双账本；后续服务商通过独立适配器接入。

## B 阶段状态

当前版本 `0.3.2` 的账号查询默认开启，首次启动会显示用途说明并提供立即关闭选项。设置页“账号查询”这一行有真正可点击的开关；开启后启动自动同步最近七天，关闭选择在重启后保留。可以选择最多 31 个日期手动同步；刷新视图只读缓存，没有定时采样。

账号总范围 `codex-account` 与 Desktop/CLI 等客户端范围分开筛选，快照版本不相加。服务端日期标签、覆盖和回填水位保持未验证；本机日志身份不自动绑定到登录账号，真实差分及设备归因仍未开放。额度按实际窗口时长识别，缺失 5 小时窗口不构造数值；过期、关闭查询或请求失败的缓存明确标记并显示采集时间。

后端只允许对 `https://chatgpt.com` 的历史、额度两条固定路由发送 GET，验证 TLS，禁用重定向，限制超时和响应大小。首次说明处理前不读取登录；同步时短暂读取官方 Codex 已有登录，不自行刷新登录、不向前端或数据库传递凭据。关闭后阻止后续请求并拒绝提交未完成同步。账号切换隔离历史和额度；401/403/404/429、断网和不兼容响应均保留已有账本。

`0.3.0` 的 [B 验证记录](docs/b-validation.md) 单独保留；`0.3.2` 的默认开启、首次说明、开关及发布验证见 [release-validation.md](docs/release-validation.md)。

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

GitHub Actions 在 Linux 执行前端与核心验证，在 Windows 编译、测试并打包 NSIS 安装包和便携 ZIP，附 SHA-256 校验值。main 的验证全部通过后，自动按 `package.json` 版本创建 `v版本号` tag 和 GitHub Release，安装版 `.exe`、便携版 `.zip`、`SHA256SUMS.txt` 分别提供下载。PR 不发布；已有版本 tag 不移动，修改代码再次发布必须增加版本号。

## 工程入口

| 目录 | 内容 |
| --- | --- |
| `apps/dashboard` | 复用视觉系统的 React 界面与 UsageClient |
| `apps/desktop/src-tauri` | 独立窗口、SQLite 组装与最小 IPC |
| `crates/monitor-contracts` | Rust DTO，生成前端类型 |
| `crates/monitor-core` | Token 校验、双账本比较、阻断理由 |
| `crates/monitor-adapters` | 增量日志、账号只读适配、事务迁移与追加快照 |
| `tests/fixtures` | 可复现合成数据，无真实账号或日志 |

数据协议见 [api-contracts.md](docs/api-contracts.md)，验证记录见 [p0-validation.md](docs/p0-validation.md)。复用来源与许可见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) 和 [LICENSE](LICENSE)。

Windows `0.3.0` [安装包与便携包](https://github.com/upuphero/agent-usage-monitor/actions/runs/38479825578/artifacts/11857415660) 已产出，哈希见 [B 验证记录](docs/b-validation.md)。A 的历史 [CI 与本机验证](docs/api-contracts.md#a-阶段验证2026-10-12-americaphoenix) 单独保留。开发者可用 `--verify-local-collector` 检查合成日志的增量、半行、重启与归档行为，必须显式设置 `AGENT_USAGE_MONITOR_DATA_DIR`。`--sync-local` 按需读取本机日志并在该独立目录保存 `local-dashboard.json`；只读取账号缓存，不查询账号。该报告留在本机，禁止加入 public 提交。

开发期可把本机白名单报告放在项目 `.local-data/preview-dashboard.json`，用 `http://127.0.0.1:1430/?local-report=1` 检查只读界面。该专用预览只读固定报告文件，不提供任意文件路径读取或同步功能。

原始规格书和历史证据包保留于本地 `docs/`，不纳入 public 仓库；其来源文件与哈希保持原样。
