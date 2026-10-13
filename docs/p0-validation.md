# P0 验证记录

日期：2026-10-12（America/Phoenix）。新工程为 Agent Usage Monitor `0.1.0`，与旧应用及旧 V0.3/V0.4 证据分别验收。

## 当前证据

- 本地：生成契约、3 项源码边界检查、2 项实际 SQLite SQL 迁移检查、14 项前端契约/界面测试、TypeScript、ESLint 与前端构建通过。
- 本地 Rust 编译：受 MSVC `link.exe` 与 Windows SDK 缺失阻断，没有把旧项目 CI 结果作为新工程通过证据。
- 用户已指定使用新的 GitHub public repo 和 pipeline 完成原生测试、编译与打包。未安装系统 Build Tools。
- 新仓库 [CI 38342864947](https://github.com/upuphero/agent-usage-monitor/actions/runs/38342864947) 全部通过，验证代码提交 `68d84c7e11d66fb534c6f40bdf50989cdb823382`：Linux 和 Windows 各 16 项 Rust 测试；核心与原生 host 严格 clippy；独立 Windows NSIS 构建；便携包与原生合成存储 smoke。
- 浏览器人工操作验证：五个页面、四种演示场景、待接入服务商空状态、中英文、主题、时区、800×560 最小窗口及折叠导航可访问名称通过，控制台无 error/warn。浏览器检查不代表原生 WebView 全部交互已验收。
- Windows 产物已下载并重算 SHA-256。本机便携程序 `--verify-synthetic-store` 返回 0，独立数据库 Schema 1、8 个账号快照、7 个本地日桶、1 个额度快照、0 个校准实验，账号查询关闭。NSIS 安装及真实用户数据升级尚未执行，不属于本次 P0 合成数据验收。

## 产物

[Windows CI 产物](https://github.com/upuphero/agent-usage-monitor/actions/runs/38342864947/artifacts/11788610631)，Actions 保留 14 天。

| 文件 | 字节 | SHA-256 |
| --- | ---: | --- |
| `Agent Usage Monitor_0.1.0_x64-setup.exe` | 2225246 | `178425a9250f9704e21ab29b9396ae96fd8927f8ba00f77e9877d85fa8ada453` |
| `Agent-Usage-Monitor_0.1.0_x64-portable.zip` | 2875370 | `cc712ae6c86de8efac258968037ecadac7e43b24931159d5e513962c776f2f7b` |

便携程序验证仅使用项目内 `.local-data/native-smoke`；原始规格与 13 个参考副本的 14 项来源哈希仍一致，没有执行旧真实探针。

## 验收范围

P0 覆盖独立工程、应用标识、双账本与质量/错误契约、事务迁移、追加快照、合成数据界面。源码内没有真实账号接口调用、凭据读取、后台采样或旧项目 sidecar。

完整日志发现与增量、真实账号历史、导出与实际设备校准属于后续阶段。真实 Windows/MacBook 归因误差保持未知，P0 合成测试不代表这些阶段通过。

## CI 入口

`.github/workflows/ci.yml`：push/PR/手动触发；只读仓库权限；前端/核心通过后执行 Windows build、native tests、clippy 和打包。产物为 NSIS、portable ZIP 和 SHA256SUMS，保留 14 天，无自动 Release 发布。
