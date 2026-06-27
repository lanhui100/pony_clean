# PonyClean — Agent 上下文

## 项目定位
Windows 极简桌面小组件：进程监控报警 + C盘安全分析清理。Rust + Tauri 2 + Vue 3 + TailwindCSS。

## 文档入口
- [文档索引](docs/README.md) — 架构 / 设计决策 / 任务系统
- [架构图](docs/ARCHITECTURE.md) — 模块依赖、数据流、项目地图
- [设计决策](docs/DESIGN.md) — ADR 技术选型记录

## 任务系统
- [总控面板](00_DASHBOARD.md) — 当前阶段、关键任务、里程碑
- [任务板](01_TASK_BOARD.md) — Kanban 状态流转
- [任务卡](03_TASKS/) — 单任务详情与验收标准

## 关键约定
- `crates/pony_core/src/lib.rs` 是业务入口，`src-tauri/src/main.rs` 仅为 Tauri 壳层
- `src/` 目录（egui 版）已废弃，保留但不再维护
- `crates/pony_core` 零 Tauri 依赖，纯业务逻辑，所有模块含 `#[cfg(test)]` 单元测试
- 公开函数和类型必须有 `///` doc 注释
- 后台任务与 UI 通过 Tauri invoke + events 通信，UI 永不阻塞
- CPU 密集型操作使用 `tokio::task::spawn_blocking`

## 快速命令

所有命令通过 `cargo-make` 运行，底层是 TOML 配置（`Makefile.toml`）。

| 命令 | 用途 |
|---|---|
| `npm run dev:tauri` | 启动 Tauri dev（前端 HMR + Rust 热重载） |
| `cd frontend && npm run dev` | 仅启动前端 Vite dev server |
| `cargo build -p pony_core` | 编译 `pony_core` |
| `cargo build -p pony_clean` | 编译 Tauri 二进制 |
| `cargo tauri build` | 打包 Tauri release（.msi） |
| `cargo check -p pony_core -p pony_clean` | 类型检查（两 crate） |
| `cargo clippy -p pony_core -p pony_clean` | clippy 检查两 crate |
| `cargo test -p pony_core` | 运行全部测试 |
| `cargo fmt --check && cargo clippy -p pony_core -p pony_clean && cargo build -p pony_core && cargo test -p pony_core` | CI 完整链 |
| `cargo clean` | 轻度清理（仅 `cargo clean`） |
| `rm -rf frontend/node_modules frontend/dist frontend/.vite src-tauri/target` | 深度清理 |
| `rm -rf src-tauri/target` | 仅清理 Tauri 构建缓存 |
| `du -sh target frontend/node_modules frontend/dist src-tauri/target 2>/dev/null` | 查看各目录体积 |

## 构建优化

| 措施 | 说明 |
|---|---|
| `opt-level = 1` (dev) | 开发构建默认优化，比 `opt-level = 0` 快 30-50% |
| `debug = 1` (dev) | 保留行号信息，去掉完整变量名，显著减小 `.pdb` |
| `incremental = true` | 增量编译，第二次起大幅提速 |
| `codegen-units = 256` | 并行代码生成单元数，加快链接 |
| `lto = "thin"` (release) | 薄 LTO，平衡体积/速度 |
| `strip = "symbols"` (release) | 去掉调试符号，减 ~30% 体积 |
| 共享 workspace target | `pony_core` + `pony_clean` 共用 `target/`，避免重复编译 |

### 清理策略

- **日常**: `cargo make clean`（`cargo clean` 约 1-3 秒）
- **版本发布前**: `cargo make clean-deep`（清空 target + node_modules，然后 npm install）
- **仅清 Tauri 缓存**: `cargo make clean-tauri`（不影响 Rust 编译缓存，下次 `cargo check` 秒回）
