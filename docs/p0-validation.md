# P0 验证记录

日期：2026-10-12（America/Phoenix）。新工程为 Agent Usage Monitor `0.1.0`，与旧应用及旧 V0.3/V0.4 证据分别验收。

## 当前证据

- 本地：前端生成契约、源码边界、实际 SQLite SQL 迁移与失败回滚检查；14 项前端契约/界面测试；TypeScript 和 ESLint 已通过。构建结果将在本轮收尾后更新。
- 本地 Rust 编译：受 MSVC `link.exe` 与 Windows SDK 缺失阻断，没有把旧项目 CI 结果作为新工程通过证据。
- 用户已指定使用新的 GitHub public repo 和 pipeline 完成原生测试、编译与打包。未安装系统 Build Tools。
- GitHub CI 结果及下载产物将在工作流完成后记录。

## 验收范围

P0 覆盖独立工程、应用标识、双账本与质量/错误契约、事务迁移、追加快照、合成数据界面。源码内没有真实账号接口调用、凭据读取、后台采样或旧项目 sidecar。

完整日志发现与增量、真实账号历史、导出与实际设备校准属于后续阶段。真实 Windows/MacBook 归因误差保持未知，P0 合成测试不代表这些阶段通过。

## CI 入口

`.github/workflows/ci.yml`：push/PR/手动触发；只读仓库权限；前端/核心通过后执行 Windows build、native tests、clippy 和打包。产物为 NSIS、portable ZIP 和 SHA256SUMS，保留 14 天，无自动 Release 发布。
