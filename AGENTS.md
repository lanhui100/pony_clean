# PonyClean — Agent 上下文

## 项目定位
Windows 极简桌面小组件：进程监控报警 + C盘安全分析清理。Rust + egui + tokio。

## 文档入口
- [文档索引](docs/README.md) — 架构 / 设计决策 / 任务系统
- [架构图](docs/ARCHITECTURE.md) — 模块依赖、数据流、项目地图
- [设计决策](docs/DESIGN.md) — ADR 技术选型记录

## 任务系统
- [总控面板](00_DASHBOARD.md) — 当前阶段、关键任务、里程碑
- [任务板](01_TASK_BOARD.md) — Kanban 状态流转
- [任务卡](03_TASKS/) — 单任务详情与验收标准

## 关键约定
- `src/lib.rs` 是业务入口，`src/main.rs` 仅为薄初始化层
- 所有业务模块含 `#[cfg(test)]` 单元测试
- 公开函数和类型必须有 `///` doc 注释
- 后台任务与 UI 通过 mpsc channel 通信，UI 永不阻塞

## 快速命令
| 命令 | 用途 |
|---|---|
| `cargo build` | 编译 |
| `cargo run` | 运行（debug） |
| `cargo test` | 运行测试 |
| `cargo clippy` | lint 检查 |
| `cargo fmt --check` | 格式检查 |
| `cargo doc --no-deps` | 生成文档 |
| `just ci` | 完整检查链（fmt + clippy + build + test） |
