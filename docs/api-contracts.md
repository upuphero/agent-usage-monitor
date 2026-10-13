# P0 数据契约与存储

API `0.1.0`。Rust `monitor-contracts` 是 DTO 唯一来源，`pnpm contracts:generate` 生成 TypeScript；`pnpm contracts:check` 拒绝漂移。序列化使用 camelCase，拒绝未知字段。前端只通过 UsageClient 查询，Tauri 调用局限在 transport。

## 范围与单位

每笔账本包含 `providerId`、`productId`、匿名 `accountRef`、身份依据、日期依据、来源、采集时刻、数据模式和质量。服务商 ID 为可扩展字符串，服务商/产品/账号不匹配时禁止差分。

Token 使用规范十进制字符串，范围 `0..9223372036854775807`，持久化为 SQLite INTEGER；诊断差值为相同绝对范围内的有符号字符串。客户端格式化与比例计算使用 BigInt，不把 Token 转成浮点数做算术。

`inclusiveInput = uncachedInput + cachedInput`；`textTotal = inclusiveInput + output`；reasoning 是 output 子集。缺分量时派生总量也为 null，质量必须为 unknown 并附原因；零是字符串 `"0"`。额度百分比放在独立 `QuotaWindow.usedPercent`，不参与 Token 算术。

## 两套账本与诊断

账号量与本机量可能重叠，不提供来源合计。差分要求相同服务商、产品、账号、经过验证的 UTC 日界线及完整分量与覆盖。日期保留 UTC 开始、排他结束，显示时区不会改变日期桶。

同日账号快照按采集时间追加，趋势只选择当前范围的最新快照，保留旧版本供回填比较。负分量差值保持负数并标记 inconsistent；没有证据则 blocked/null。P0 契约没有已校准的设备估算输出，所有服务商的增量/真实采集/校准能力均为 false。

## 独立持久化

Schema 1 在事务中创建账号、来源检查点、最小事件、本地日汇总、服务端日快照、额度快照、校准、归因、偏好和安全审计表。版本通过 `PRAGMA user_version` 与 `schema_migrations` 保存。迁移失败回滚；遇到更高版本拒绝打开，不重置账本。

入库先校验白名单 DTO，再在事务内保存整数列及该 DTO 的 JSON 投影；不是原始日志或服务端响应。账号与服务商使用复合外键。服务端/额度快照拒绝原地 UPDATE，重复 ID 拒绝整批导入。P0 只预置 synthetic 数据，查询也限定 synthetic，真实数据不会混入预览。

最小 IPC 为 `get_dashboard({ query: { scenario } })`。每项诊断保留服务商/产品/账号范围和两条来源 row ID，前端按选中记录绑定，避免仅按日期串用其他范围的结果。桌面正常场景读取 SQLite 缓存；异常场景使用确定性合成 fixture。存储失败显示明确错误，不退回浏览器演示数据。错误只传安全代码和固定文案，不包含 SQL、路径或异常正文。

## 来源与边界

Tauri 命令和配置用法参照 [官方 IPC 文档](https://v2.tauri.app/develop/calling-rust/) 与 [配置参考](https://v2.tauri.app/reference/config/)。当前 P0 没有账号网络适配器、凭据文件读取或本地日志扫描入口。后续 A/B 阶段需要分别实现并独立验收。
