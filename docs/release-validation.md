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
