# Windows / Mac 日志汇总（0.4.0）

账号额度是登录账号共享的百分比和重置时间；设备 Token 来自各设备的 Codex 日志。账号历史在使用历史页单独展示，不与本机曲线叠加，也不通过差值推算 Mac。

## Windows

安装版或便携版启动后点击“同步本机日志”。设置中的“导出本机报告”下载 JSON。便携版附 monitor-collector.exe，可在 PowerShell 独立运行：

```powershell
.\monitor-collector.exe collect --output windows-usage.json
```

## Mac

从同一版本 Release 下载 Apple Silicon（arm64）或 Intel（x64）采集器，解压后在 Terminal 进入解压目录：

```sh
./monitor-collector collect --output "mac-usage-$(date +%Y%m%d-%H%M%S).json"
```

将生成的 JSON 自行复制到 Windows，在设置中点击“导入设备报告”。无需在 Mac 登录账号查询，不会访问账号接口。采集器不是 Mac 图形界面应用，无需安装 Rust。若 macOS 提示未识别的开发者，可核对 Release 校验值后，在系统“隐私与安全性”中允许该采集器。

默认读取 CODEX_HOME，未设置时读取用户目录下 .codex 的 sessions、archived_sessions、session-cleanup-archive。可指定一个或多个已有日志根目录：

```sh
./monitor-collector collect --home /absolute/path/to/.codex --output mac-usage.json
```

默认增量缓存：Mac/Linux 为 ~/.agent-usage-monitor-collector，Windows 为 %LOCALAPPDATA%\.agent-usage-monitor-collector。也可用 --state 指定缓存目录。每台设备持续使用自己的缓存，不要将缓存数据库在设备间复制；稳定设备身份保存在其中。升级只替换采集器程序，保留缓存。输出文件已存在时拒绝覆盖，请使用新文件名。

## 汇总规则

- 导入同一报告不会重复计数；新报告替换同一设备旧快照，较旧或同时间不同内容的报告拒绝覆盖。
- 统一按 UTC 日显示；账号历史的服务端日期标签单独保存。
- 相同匿名日志观察标记只计一次；标记相同但时间、范围或计数冲突时，从汇总中排除，并显示冲突数。平台数字分别表示各设备报告，所以复制日志后平台数字之和可能大于去重汇总。
- 主线程与子代理分开筛选，不能将可能重叠的执行范围相加。默认可查看所有日志账号，也可选择单一匿名日志账号；不会把日志身份自动绑定到当前登录。
- 汇总是日志所覆盖的已观测部分，不保证完整账号或完整设备消费。日志复制和会话迁移无法仅凭采集位置证明执行地点；同一请求以不同日志观察形态出现时可能无法识别所有重复。
- 缺失的缓存分量保留未知，不补零；计数精确到 i64 上限，溢出或不兼容报告拒绝整批导入。
- 报告格式 agent-usage-monitor/device-log，版本 1。最多 32 MiB、50,000 条记录、32 台设备（含本机）；超过限制明确失败，不截断。包含匿名标记、UTC 时间、Token、采集状态和排除数量，不含路径、模型、对话正文、工具输入或登录信息。报告含用量信息，留在本机，自行传送。

Windows 桌面端和独立 Windows 采集器使用各自的缓存身份。选择一种方式作为该设备来源，避免将同一设备当成两个独立采集地点；相同观察标记仍会在汇总中去重。

Mac 原生 CI 使用合成日志验证采集、增量重启和报告导出；这不代表已经访问你的 Mac 或核实真实日志覆盖。
