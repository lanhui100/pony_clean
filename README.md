# PonyClean

Windows 极简桌面小组件。两个核心功能：

- **进程监控** — 实时检测 CPU/内存异常超高的进程，报警并支持一键 kill
- **C盘安全清理** — 分级扫描可清理文件，安全删除临时文件、缓存、回收站等

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + shadcn-vue |
| 样式 | TailwindCSS 4 |
| 后端 | Rust (pony_core) |
| 异步 | tokio |
| 进程 | sysinfo |
| 磁盘 | jwalk + windows-rs |

## 开发

```bash
npm run dev:tauri      # Tauri dev（前端 HMR + Rust 热重载）
cargo test -p pony_core # 运行单元测试
```

## 项目结构

```
pony_clean/
├── crates/pony_core/  # 业务核心库（零框架依赖）
├── src-tauri/         # Tauri 壳层
├── frontend/          # Vue 3 前端
├── docs/              # 项目文档
├── 00_DASHBOARD.md    # 任务总控面板
├── 01_TASK_BOARD.md   # 任务板
└── 03_TASKS/          # 任务卡
```
