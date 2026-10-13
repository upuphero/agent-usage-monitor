# 数据契约与存储

API `0.2.0`。Rust `monitor-contracts` 是 DTO 唯一来源，`pnpm contracts:generate` 生成 TypeScript；`pnpm contracts:check` 拒绝漂移。序列化使用 camelCase，拒绝未知字段。前端只通过 UsageClient 查询，Tauri 调用局限在 transport。

## 范围与单位

每笔账本包含 `providerId`、`productId`、匿名 `accountRef`、身份依据、日期依据、来源、采集时刻、数据模式和质量。服务商 ID 为可扩展字符串，服务商/产品/账号不匹配时禁止差分。

Token 使用规范十进制字符串，范围 `0..9223372036854775807`，持久化为 SQLite INTEGER；诊断差值为相同绝对范围内的有符号字符串。客户端格式化与比例计算使用 BigInt，不把 Token 转成浮点数做算术。

`inclusiveInput = uncachedInput + cachedInput`；`textTotal = inclusiveInput + output`；reasoning 是 output 子集。已知 inclusive input 与 output 时可保留真实总量；cached 缺失则 cached/uncached 均为 null，不能推造分解。无独立已知 input/output 时总量为 null。零是字符串 `"0"`，未知必须附原因。额度百分比放在独立 `QuotaWindow.usedPercent`，不参与 Token 算术。

## 两套账本与诊断

账号量与本机量可能重叠，不提供来源合计。差分要求相同服务商、产品、账号、经过验证的 UTC 日界线及完整分量与覆盖。日期保留 UTC 开始、排他结束，显示时区不会改变日期桶。

同日账号快照按采集时间追加，趋势只选择当前范围的最新快照，保留旧版本供回填比较。负分量差值保持负数并标记 inconsistent；没有证据则 blocked/null。P0 契约没有已校准的设备估算输出，所有服务商的增量/真实采集/校准能力均为 false。

## 独立持久化

Schema 1 在事务中创建账号、来源检查点、最小事件、本地日汇总、服务端日快照、额度快照、校准、归因、偏好和安全审计表。版本通过 `PRAGMA user_version` 与 `schema_migrations` 保存。迁移失败回滚；遇到更高版本拒绝打开，不重置账本。

入库先校验白名单 DTO，再在事务内保存整数列及该 DTO 的 JSON 投影；不是原始日志或服务端响应。账号与服务商使用复合外键。服务端/额度快照拒绝原地 UPDATE，重复 ID 拒绝整批导入。P0 只预置 synthetic 数据，查询也限定 synthetic，真实数据不会混入预览。

最小 IPC 为 `get_dashboard({ query: { scenario } })`。每项诊断保留服务商/产品/账号范围和两条来源 row ID，前端按选中记录绑定，避免仅按日期串用其他范围的结果。桌面正常场景读取 SQLite 缓存；异常场景使用确定性合成 fixture。存储失败显示明确错误，不退回浏览器演示数据。错误只传安全代码和固定文案，不包含 SQL、路径或异常正文。

## A 阶段增量账本

Schema 2 事务升级保留 P0 合成数据与安装 ID，新增文件游标、统计原始快照、来源关联与有效事件投影。normal/local 查询只读 observed 日账本，P0 合成数据显式分开。

读取边界限定在三类会话目录下的普通 `.jsonl`；不跟随 symlink/reparse 跳出来源。持久化只有时间、统计、模型配置与不可逆身份标记，原始正文、工具输入、cwd 和完整响应不保存。配置的附加目录仅保存在本机偏好，不进入账本 DTO。

检查点同时保存已处理字节位置、完整前缀 SHA-256、头尾小摘要、文件大小/mtime 与解析状态。未变化文件不重解析；变化文件先验证旧前缀，再仅解析新增完整行。完整性检查可能读取旧字节，`bytesVerified` 与 `bytesParsed` 分别报告。重写会废弃该来源旧版本关联并重建有效投影；删除的来源保留已验证缓存，明确显示缺口。游标、快照关联、事件和日账本在同一事务提交，失败或退出后不会留下已前移但未入账的游标。

事件身份采用线程标记及统计观察指纹；文件副本/归档不重复计数，共享 session ID 不用于合并不同线程。累计和 per-response last 必须对齐；初始余额仅为基线、reset epoch 及矛盾区间仅诊断。明确的 UTC 下界与计数时刻同日才归日，跨午夜不平均分摊。父子计数范围未核实，`codex-local` 与 `codex-subagent` 独立展示；账号未核实的记录不进入校准集合。

新增 IPC 为 `sync_local`、`cancel_local_sync`、`set_local_roots({ request: { roots } })`；`get_dashboard` 的 `local` 场景只返回真实本机缓存、CollectorSummary、无账号/额度/残差。同步可取消且按需执行，没有定时轮询。窗口退出设置取消标记，SQLite 保证已提交状态可恢复。

本阶段依旧不声称完整 Windows 消耗或 MacBook 归因，模型字段只代表 turn_context 配置。公开仓库与 CI 使用合成输入，真实本机报告和数据库保留在 `.local-data` 或独立应用数据目录。

## 来源与边界

Tauri 命令和配置用法参照 [官方 IPC 文档](https://v2.tauri.app/develop/calling-rust/) 与 [配置参考](https://v2.tauri.app/reference/config/)。A 阶段有本地日志读取，账号网络适配器与凭据读取仍不存在。真实账号账本及设备校准属于后续独立验收。

## A 阶段验证（2026-10-12 America/Phoenix）

版本 `0.2.0` 的代码提交 `db7e7c868fe230f9e7b70ea4f3ac18f553f2a93a` 在 [CI 38350590895](https://github.com/upuphero/agent-usage-monitor/actions/runs/38350590895) 全部通过：21 项前端契约/界面测试、3 项源码边界、3 项 SQL 迁移测试；Linux 36 项 Rust 测试与 Windows 35 项 Rust 测试（Unix symlink 用例按平台执行）；核心和原生 host 严格 clippy；Windows NSIS 与 portable ZIP 构建；原生合成输入验证了完整行、半行追加、检查点重启恢复和归档副本去重。

本机下载产物重新校验 SHA-256 后，用新程序只读同步真实会话目录。重复同步实际只解析变化来源，未变化来源未重解析；检查点与事件唯一键校验通过。旧 P0 SQLite 副本升级至 Schema 2，8 个合成账号快照保留，真实本机视图未混入这些快照。账号查询开关始终为 false，账号 DTO 空、quota null、无跨设备结果。

浏览器只读绑定实际 CLI 白名单报告，检查了真实概览、历史、模型配置/会话、质量原因、账号/执行范围筛选；控制台无 error/warn。数据库、实际报告与截图仅留本机，未上传 public 仓库。原生 WebView 的完整交互、安装升级、睡眠和独立控窗校准仍不由这些检查代表。

[Windows 安装包与便携包](https://github.com/upuphero/agent-usage-monitor/actions/runs/38350590895/artifacts/11790754336)，保留 14 天。

| 文件 | SHA-256 |
| --- | --- |
| `Agent Usage Monitor_0.2.0_x64-setup.exe` | `df44c4c88d6bbac0bcbabc1d04c423570daaf54789abc5dc38c8f444cff270b3` |
| `Agent-Usage-Monitor_0.2.0_x64-portable.zip` | `399eec1b0cd6da8623dd5eae244b94343b349d7d24eb49adee904f3a830058bf` |

真实日志仍存在继承、累计回退、计数间隙与跨日歧义，所以当前显示已观测部分，完整 Windows 真值和设备归因误差仍未知。
