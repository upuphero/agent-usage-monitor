# 0.4.1 历史计数修复与额度诊断

日期：2026-10-14（America/Phoenix）。用户授权调查 Windows 未计入记录及五小时缺窗，修复后测试并发布新版。

构建提交 `f08a99fa93ca116870873a639b7b6a04c29a11aa`；[CI 38686777436](https://github.com/upuphero/agent-usage-monitor/actions/runs/38686777436) 公共验证、Windows 与两种 Mac 原生测试/打包、Release 发布全部通过。[v0.4.1 Release](https://github.com/upuphero/agent-usage-monitor/releases/tag/v0.4.1) 已公开，tag 精确指向该提交。

- 修复线程创建时间/继承标记变化造成的整线程冲突排除，按计数段独立核对；账号、父线程、执行范围等真实身份冲突继续阻断。
- 计数重置只排除边界，后续经 cumulative delta / last 检查成功的增量恢复。跨 UTC 日区间仍不分摊；午夜后的同日已验证观察可作为新的下界。
- 升级首次以事务从已有白名单观察重算投影，失败回滚，保留来源检查点、原统计观察、账号快照及关闭选择；二次打开不重复重算。Unix 原生工作目录不再误用 Windows 的外来路径判定。
- 数据质量页增加互斥的主要排除原因计数，客户端验证合计与总数一致。设备表明确统计全部历史，非当前选中日期。
- 五小时缺窗不会被填零或用每周值替代，界面明确说明服务器该次没有返回该窗口。额度成功、历史缺日期的状态单独解释。
- 当前用户明确授权的一次 HTTPS GET 仅核对额度窗口白名单，未刷新或写入登录，没有保存原始响应。结果确认 weekly-only 形状，不是解析漏项。账号计划、数值、登录信息和调查缓存均不进入公开仓库。
- 公共验证通过：37 项前端、3 项源码边界、5 项实际 SQL 迁移、2 项离线发布校验、TypeScript、ESLint、生产构建；Linux 64 项、Windows 62 项、Mac ARM64/Intel 各 64 项 Rust 及全部严格 clippy。合成回归覆盖分段、回退边界恢复、跨日、身份/响应冲突、重算/回滚及检查点/偏好保留。

真实缓存只通过 SQLite read-only 连接读取，并用备份 API 在本机 `.local-data/investigation-0.4.1/cache-copy` 留一致性副本；新版离线 --inspect-local-cache 会在该副本重算并输出统计。不会改动正在使用的已安装数据库，不向 CI 上传缓存或原始日志。

代码依据：官方 [HistoryPosition / SessionMeta 协议](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs) 说明回退后物理 rollout 可不同、线程 ID 保持稳定；创建时间和继承标记的变化不能直接当成账号或线程身份冲突。

## 0.4.1 公开产物与本机复核

四个公开包已下载并重新计算 SHA-256，与清单及 GitHub asset digest 一致；Mac Mach-O 架构和可执行权限检查通过。公开 Windows desktop 的 --verify-synthetic-store、--verify-local-collector 均返回 0。

| 文件 | 字节 | SHA-256 |
| --- | ---: | --- |
| `Agent-Usage-Monitor_0.4.1_x64-portable.zip` | 4505103 | `0361a7415630e40c008013f244d95a19e29351d04edd3ef7aed156ad1bd5d3a4` |
| `Agent-Usage-Monitor_0.4.1_x64-setup.exe` | 2703427 | `5b122aec913eb75832eeefccb4aec489dadc843b7f7e119c74cb8aac370ed718` |
| `Agent-Usage-Monitor_0.4.1_macos-arm64-collector.tar.gz` | 955813 | `f299961c9ac18440be5367beef9e323234e6d3499c2d7f7642302ab1297cc3eb` |
| `Agent-Usage-Monitor_0.4.1_macos-x64-collector.tar.gz` | 1028498 | `5722d27cae16156adad65fcb6d3b0f57539f8fb03d9b31b025deb148d7ed56d9` |

用公开 Windows 程序 --inspect-local-cache 对先前保存的真实缓存副本重算成功。逐事件比较确认恢复可核实的历史增量，没有丢失原已计入事件，没有新增或删除观察事件；记录总数保持不变。剩余排除项有互斥主要原因。原观察、来源游标/解析状态、来源关联、账号/额度快照、偏好、迁移记录和设备报告的全表指纹不变；旧 app_meta 值不变，只新增投影版本标记。源数据库仅只读，私有统计详情及缓存均留在 .local-data。

CollectorSummary.reasonCodes 和 lastRun 保留采集期的历史提示，可能重叠；当前未计入记录的可加总解释以 excludedBreakdown 为准。此检查没有扫描新的会话正文、没有再次查询真实账号、没有改动已安装应用数据库，也没有手动执行 NSIS 安装升级。公开文件、校验清单和版本 tag 不覆盖。

---

# 0.4.0 设备日志与独立账号额度验证

日期：2026-10-13（America/Phoenix）。用户明确要求参考 CodexBar，将账号额度和本机 Token 分开；Windows/Mac 各自采集日志再汇总，不通过差值猜 Mac，并分别发布安装版、便携版及版本 tag。

构建提交：`43404104513b286d9c3159c38904587cab745824`。当前 [CI 38507237929](https://github.com/upuphero/agent-usage-monitor/actions/runs/38507237929) 的公共验证、Windows、两种 Mac 原生构建和 Release 发布全部通过。[v0.4.0 Release](https://github.com/upuphero/agent-usage-monitor/releases/tag/v0.4.0) 已公开，tag 精确指向该构建提交。

- 账号额度独立面板；本机趋势不再绘制账号线，移除差值卡片与设备归因阻断提示。账号旧历史在使用历史中单独展开，所选日期只来自日志。
- Schema 4 保留旧账本、开关、安装身份和检查点，新增设备报告表。Windows 桌面支持导出/导入/移除；Mac ARM64/Intel 有同协议独立采集器。
- 报告白名单、复制去重、冲突排除、未知分量与大整数、按范围/UTC 日归档、旧/同时间冲突快照拒绝覆盖、重启恢复、隐私字段不导出、CLI 稳定身份和输出不覆盖均有合成测试。首次同步取消后的已提交记录仍可展示和导出。
- 公共验证：35 项前端测试、3 项源码边界、5 项实际 SQLite 迁移、2 项离线发布校验，TypeScript、ESLint、生产构建及 Linux 57 项、Windows 56 项、Mac ARM64/Intel 各 57 项 Rust 测试、全部严格 clippy 通过。
- 本地浏览器用临时合成 UsageClient 检查真实 App 的概览与设置：Mac 未接入、导入文件后设备状态更新、去重汇总文案、移除入口、单一本机曲线；控制台无 error/warn。临时页面已删除，不进入公开提交。截图留在本机 `.local-data/qa-0.4.0`。
- Mac CI 分别测试 native arm64 / Intel。发布只在全部测试和打包成功后创建精确提交的版本 tag；四个包和校验清单单独提供，公开 tag / 文件不可覆盖。

本轮没有读取真实账号、访问用户的 Mac 或执行真实跨设备校准。平台标签表示日志采集地点；日志复制和不同观察形态不能证明所有请求的执行设备或完整消费。交互验证使用合成数据，原生 UI 下载保存/安装升级仍未手动验收。

## 0.4.0 公开产物复核

公开的四个包已下载到 `.local-data/release-0.4.0/published`，重新计算 SHA-256 与清单、GitHub asset digest 一致。Mac 两个压缩包的 Mach-O CPU 架构和可执行权限均检查通过，包含采集器、操作说明和许可证。

| 文件 | 字节 | SHA-256 |
| --- | ---: | --- |
| `Agent-Usage-Monitor_0.4.0_x64-portable.zip` | 4481243 | `43f63b9b636efaec9322a7055a2f46561bf1a3b04bfcecf3d8f4f1ad59abef31` |
| `Agent-Usage-Monitor_0.4.0_x64-setup.exe` | 2692762 | `fa2bff081e3f64a94f19e13e03ea2cc8a3dca0a8c350703f5d2cb83d8eeb2667` |
| `Agent-Usage-Monitor_0.4.0_macos-arm64-collector.tar.gz` | 946062 | `cf8c474fe2486362a30307b978c0f894be2141511c85cb125052d82518adb537` |
| `Agent-Usage-Monitor_0.4.0_macos-x64-collector.tar.gz` | 1018115 | `32d45d0a61233c0e2d05240ce5038361773ebf80f6d4dfc1d45092656dd6e9d4` |

公开 Windows 便携包包含 monitor-desktop.exe、monitor-collector.exe、device-collection.md 和许可证。本机运行公开 desktop 的 --verify-synthetic-store、--verify-local-collector 均返回 0；再运行公开 collector，对明确指定的合成日志根目录输出 2 条去重记录、39 Token、Windows 平台。检查数据限定在 `.local-data/native-release-0.4.0`，没有读取真实日志或账号。NSIS 安装升级及完整原生窗口交互未手动执行。

后续仅提交 Markdown 验证记录，不改变已测试的代码、版本 tag 或公开文件。

---

# 0.3.2 账号默认与 Release 验证

日期：2026-10-13（America/Phoenix）。用户明确要求首次说明、账号查询默认开启、可点击的关闭开关，以及 CI 自动发布版本 tag / Release，安装版与便携版分开。`0.3.1` 的前端、Linux/Windows Rust、clippy 和打包已通过，但发布草稿的空格文件名被 GitHub 改名，触发校验阻断。`0.3.2` 统一无空格文件名后，完整 CI 和自动公开发布均已通过，既有 tag 未移动。

- 首次说明前不读取凭据或查询账号；继续使用后默认同步最近七天；说明里可以直接关闭。
- 设置页“账号查询”这一行使用有名称、有焦点状态的 switch，关闭选择持久化。关闭同步时阻止后续请求和提交，已有数据保留。
- 原有错误文案“账号账本未接入”更新为“当前范围尚无账号数据”。账号数据和设备归因的功能边界保持独立。
- API `0.3.2` 新增运行时首次说明字段，Schema 3 不变，既有账号快照 JSON 不变。
- main 验证、Windows 测试和打包全部通过后，创建对应版本 tag，再上传安装包、便携 ZIP、SHA-256 清单；确认草稿完整后公开发布。PR 不发布，既有 tag 不移动，公开产物不覆盖。
- [CI 38490048436](https://github.com/upuphero/agent-usage-monitor/actions/runs/38490048436) 全部通过，构建提交 `49c6019db7fcc7380afaf6afb6f10ee9b50a8f0b`：TypeScript、lint、29 项前端、3 项边界、4 项迁移、2 项离线 Release 校验及生产构建；Linux 51 项 / Windows 50 项 Rust 测试和严格 clippy；原生合成 smoke、安装包/便携包打包、tag 创建与 Release 自动发布。
- 用合成 UsageClient 实际操作了首次继续、开关关闭、关闭后重启、重新开启及开启后重启；查询次数证明每次启动至多同步一次，关闭状态不查询。首次说明直接关闭后重启，查询次数仍为零。浏览器控制台无 error/warn。

此验证没有读取真实登录或执行账号 API；交互检查使用合成输入，临时检查页面不进入公开提交。真实账号接口兼容性、设备校准及安装升级仍分别验收。

## 公开产物与本机复核

[v0.3.2 Release](https://github.com/upuphero/agent-usage-monitor/releases/tag/v0.3.2) 已公开，tag 精确指向上述构建提交。安装版与便携版分别下载，SHA-256 清单为独立附件。公开附件下载后重新计算哈希，文件名及哈希均与清单和 GitHub asset digest 一致。

| 文件 | 字节 | SHA-256 |
| --- | ---: | --- |
| `Agent-Usage-Monitor_0.3.2_x64-setup.exe` | 2646367 | `622a3b6daf471d251a3fd34ad4b6ae41b5c34e81d94106d5ef8c3a49071a1acf` |
| `Agent-Usage-Monitor_0.3.2_x64-portable.zip` | 3462569 | `e3874bd6c4080f450c193f2efed7ce7fa2003a41cea4cfa02702cdd7d90559c3` |

本机副本留在 `.local-data/release-0.3.2`；公开便携程序的 `--verify-synthetic-store` 与 `--verify-local-collector` 均返回 0，验证数据限定在 `.local-data/native-release-0.3.2`。数据库、日志、截图和本机副本不进入 public 提交。记录后续只修改 Markdown，不改变已验证代码与 tag。
