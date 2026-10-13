# B 阶段验证记录

日期：2026-10-13（America/Phoenix）。版本 `0.3.0`，API `0.3.0`，Schema 3。B 的合成接口、持久化、前端与原生工程验收通过；真实账号联网兼容性和设备校准未执行。

## 已实现

- 默认关闭的账号开关和手动同步；固定 HTTPS GET 路由、TLS 校验、无重定向、超时/大小/日期范围/持久化节流限制。
- 官方现有登录的内存白名单读取、匿名账号绑定、账号切换隔离；前端和数据库没有凭据字段。
- 每日绝对文本分量、账号总范围与各客户端范围、多个追加快照、缺日期/缺分量/未知日期边界与水位状态。
- 根据实际时长识别 5 小时/周额度，未提供窗口保持未知；错误和过期缓存保留来源时间。
- 历史与额度分别记录成功/错误；离线降级、事务回滚、关闭查询时拒绝同步提交；B2/C/D 保持未验收。

## 已执行的验证

- `pnpm check`：契约生成漂移检查、3 项源码边界、4 项真实 SQLite 迁移、TypeScript、ESLint、27 项前端契约/界面测试、生产构建。
- `cargo fmt --all --check` 和 `cargo metadata --locked --no-deps`；清单与锁文件一致。
- V0.3 已保存脱敏证据：原有 `node --test test/v03-online-validation.test.mjs` 的 8 项检查通过，复核三日文本分量恒等式、客户端类型、实际返回上界、缺水位与模型 Token、既有文件哈希。没有执行探针或新请求。这些是历史证据复核，不是新 Rust 解析器的运行证明。
- 浏览器预览：概览、账号追加快照历史、B 设置说明及浏览器只读状态。observed 的账号绑定、过期额度、离线状态、缺日期和差分禁用通过合成 DTO 渲染测试；尚未进行原生 WebView 交互验收。
- [CI 38479825578](https://github.com/upuphero/agent-usage-monitor/actions/runs/38479825578) 全部通过，验证代码提交 `8f2819b99e8166ca35f808ad9fbf70b5fc5dc525`：27 项前端测试、3 项源码边界、4 项迁移；Linux 48 项与 Windows 47 项 Rust 测试（Unix symlink 用例按平台执行）；两端严格 clippy；Windows NSIS、便携 ZIP 以及打包时的合成存储/增量采集 smoke。
- 产物下载到项目内后逐项重新计算 SHA-256，与 CI 清单完全一致。本机便携程序 `--verify-synthetic-store` 和 `--verify-local-collector` 均返回 0，数据目录显式限定在 `.local-data/b-native-verification/8f2819b`，仅使用合成输入。

## 修复与本机编译限制

本机 `cargo check -p monitor-adapters` 曾因 `linker link.exe not found` 无法执行；没有安装系统构建工具，原生编译与严格验证通过上述 CI 完成。12 项新增 Rust 用例覆盖范围、分量、单位、额度窗口、HTTP 错误、限流、账号切换、事务失败、断网和磁盘重启，均已在 Linux/Windows 通过。

首轮 Rust 测试通过，严格 clippy 要求简化额度窗口布尔表达式，已修复。最后将日期选择器从“UTC 统计日期”改为“统计日期标签”，避免暗示账号日界线已验证。最终代码通过完整 CI；旧 A/P0 CI 未被当作 B 的验收证据。

## Windows 产物

[安装包与便携包](https://github.com/upuphero/agent-usage-monitor/actions/runs/38479825578/artifacts/11857415660)，Actions 保留 14 天。

| 文件 | 字节 | SHA-256 |
| --- | ---: | --- |
| `Agent Usage Monitor_0.3.0_x64-setup.exe` | 2644331 | `f418b8633fe2d7cab3e7cb66861bbe209b18ec9d07dd1ad89f1e5d96c18a1a8c` |
| `Agent-Usage-Monitor_0.3.0_x64-portable.zip` | 3453884 | `036d213e835af5665140b46e9a4b130583b8366452b0bae5e25ef44bd73babc4` |

本机副本留在 `.local-data/b-packages/8f2819b`，不进入公开提交。验证记录后续仅修改 Markdown，不改变上述已验证代码。

## 未执行

没有读取真实 `auth.json`，没有调用真实账号历史或额度接口，没有刷新登录、后台采样、设备实验或发布 Release。真实接口兼容性仍需明确的独立联网授权；无证据的日期对齐、产品覆盖和设备误差保持 UNKNOWN/null。

NSIS 实际安装/升级和原生 WebView 全交互尚未执行；便携 CLI 与 CI smoke 不能代替这些验收。
