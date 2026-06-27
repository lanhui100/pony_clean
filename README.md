# PonyClean

Windows 极简桌面小组件。两个核心功能：

- **进程监控** — 实时检测 CPU/内存异常超高的进程，报警并支持一键 kill
- **C盘安全清理** — 分级扫描可清理文件，安全删除临时文件、缓存、回收站等

## 技术栈

| 层 | 选型 |
|---|---|
| GUI | egui + eframe |
| 异步 | tokio |
| 进程 | sysinfo |
| 磁盘 | jwalk + windows-rs |
| 通知 | egui-toast / tray-icon |

## 开发

```bash
cargo build
cargo run
```

## 项目结构

```
pony_clean/
├── src/
│   ├── main.rs        # eframe 入口 + tokio runtime
│   ├── app.rs         # egui App 状态 + 渲染
│   ├── monitor.rs     # 进程监控模块
│   └── cleaner.rs     # C盘扫描与清理模块
├── 00_DASHBOARD.md    # 总控面板
├── 01_TASK_BOARD.md   # 任务板
├── 03_TASKS/          # 任务卡
├── 02_REVIEWS/        # 审核记录
└── 99_LOGS/           # 会话日志
```
