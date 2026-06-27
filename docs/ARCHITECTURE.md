# PonyClean 架构

## 模块依赖关系

```
main.rs  (薄入口层)
   │
app.rs   (egui::App, UI 状态, update 渲染)
   │
   ├── monitor.rs  (进程监控：sysinfo 轮询 + mpsc 推送)
   │
   ├── cleaner.rs  (C盘清理：jwalk 遍历 + 安全分级 + 删除)
   │
   ├── error.rs    (统一错误类型)
   │
   └── lib.rs      (库入口，re-export 所有业务模块)
```

## 数据流

```
┌──────────────┐    mpsc::channel    ┌──────────────────┐
│  Tokio Task  │ ──────────────────► │  PonyCleanApp    │
│  (monitor)   │   Vec<ProcessInfo>  │  (egui::App)     │
└──────────────┘                     │                  │
                                     │  rx.try_recv()   │
┌──────────────┐    mpsc::channel    │  每帧轮询         │
│  Tokio Task  │ ──────────────────► │  更新 self.state  │
│  (cleaner)   │   ScanProgress     │                  │
└──────────────┘                     │  ui() 渲染        │
                                     └──────────────────┘
```

UI 操作（kill 进程 / 执行清理）通过反向 channel 发送 command 到后台任务。

## 项目地图

| 目录/文件 | 职责 |
|---|---|
| `src/main.rs` | eframe::run_native + tokio runtime 初始化 |
| `src/lib.rs` | 库入口，业务模块统一 re-export |
| `src/app.rs` | egui 状态 + update 循环 |
| `src/monitor.rs` | 进程快照、阈值检测、kill |
| `src/cleaner.rs` | 路径扫描、安全分级、删除执行 |
| `src/error.rs` | PonyError 枚举 + Result 别名 |
| `tests/` | 集成测试 |
| `docs/` | 项目文档 |
| `00_DASHBOARD.md` | 任务系统总控面板 |
| `01_TASK_BOARD.md` | 任务板 |
| `03_TASKS/` | 单任务详情 |
| `99_LOGS/` | 会话日志 |

## 关键技术约束

- **GUI**: egui 即时模式，无浏览器引擎，单二进制分发
- **异步**: tokio runtime 在独立后台线程运行，不阻塞 UI 帧循环
- **进程数据**: sysinfo 每 2s 全量刷新
- **磁盘遍历**: jwalk 并行遍历，广度优先，进度通过 channel 流式推送
- **IPC**: std::sync::mpsc（后台→UI），tokio::sync::mpsc（UI→后台 command）
