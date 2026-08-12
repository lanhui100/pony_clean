# PonyClean 架构

## 技术栈

| 层 | 技术 | 说明 |
|---|---|---|
| 桌面壳 | Tauri 2 | WebView + Rust 后端 |
| 前端 | Vue 3 + TypeScript | 组件化 UI，shadcn-vue 设计系统 |
| 前端构建 | Vite 6 + TailwindCSS 4 | HMR 开发，无打包生产 |
| 后端 | Rust (pony_core 库) | 进程监控、磁盘扫描、文件清理 |
| IPC | Tauri invoke + events | 命令调用 + 事件推送替代旧的 mpsc 直连 |

## 模块依赖关系

```
frontend/                  (Vue 3 SPA，运行在 WebView 中)
    │
    │  invoke("get_processes") / invoke("kill_process")
    │  invoke("start_scan") / invoke("execute_clean")
    │  listen("scan-progress") / listen("scan-items") / listen("scan-done")
    ▼
src-tauri/src/main.rs      (Tauri 入口，注册命令处理器)
    │
    ├── commands/monitor.rs  (Tauri 命令层：get_processes, kill_process)
    │     │                   持有 MonitorState (共享快照 + 命令通道)
    │     ▼
    └── commands/cleaner.rs  (Tauri 命令层：start_scan, execute_clean, empty_recycle_bin)
          │                   持有 CleanerState (扫描锁定)，事件推送
          ▼
crates/pony_core/          (业务核心库，纯 Rust，无 Tauri 依赖)
    │
    ├── lib.rs             (库入口，re-export monitor / cleaner / error)
    ├── monitor.rs         (进程监控：sysinfo 轮询 + 快照共享 + kill)
    ├── cleaner.rs         (C盘清理：jwalk 遍历 + 安全分级 + 删除执行)
    └── error.rs           (统一错误类型)
```

## 数据流

```
┌─────────────────────────────────────────────────────────────────┐
│  frontend (Vue 3 WebView)                                       │
│                                                                 │
│  useMonitor() composable                                        │
│    ┌─────────┐   setInterval(2s)   ┌──────────────┐            │
│    │ invoke  │ ──────────────────►  │ Tauri Command │            │
│    │("get_   │ ◄──────────────────  │ get_processes │            │
│    │processes│     Snapshot JSON    └──────┬───────┘            │
│    └─────────┘                             │                    │
│                                            ▼                    │
│  useCleaner() composable         ┌──────────────────────┐       │
│    ┌──────────┐   invoke/event   │  monitor::start_shared│       │
│    │ invoke   │ ───────────────► │  独立线程 sysinfo 轮询│       │
│    │("start   │                  │  共享 Arc<RwLock<>>   │       │
│    │_scan")   │                  └──────────────────────┘       │
│    │          │                                                 │
│    │ listen   │ ◄── Tauri Event ── scan-progress                │
│    │("scan-   │                  scan-items                     │
│    │progress")│                  scan-done                      │
│    └──────────┘                  scan-error                     │
└─────────────────────────────────────────────────────────────────┘

  Kill 进程流：
  invoke("kill_process", { pid, name })
    → command/monitor::kill_process
    → mpsc::send(MonitorCommand::Kill)
    → std::thread (monitor::start_shared) → sysinfo kill
    → oneshot::response 返回

  扫描流：
  invoke("start_scan")
    → command/cleaner::start_scan
    → spawn_blocking → pony_core::cleaner::start_scan
    → ScanEvent 通过 mpsc channel 到达 commands 层
    → commands 层通过 app.emit("scan-*") 推送 Tauri Event
    → frontend listen("scan-*") 更新反应式状态

  清理流：
  invoke("execute_clean", { paths })
    → command/cleaner::execute_clean
    → spawn_blocking → pony_core::cleaner::delete_files
    → security validation (is_path_protected + is_path_allowed)
    → DeleteResult 返回
```

## 项目地图

| 目录/文件 | 职责 |
|---|---|
| `frontend/src/App.vue` | Vue 根组件，Tab 切换 |
| `frontend/src/views/MonitorPanel.vue` | 进程列表、搜索、排序、Kill 弹窗 |
| `frontend/src/views/SpacePanel.vue` | 清理页（方案 B）：一次扫描同时产出垃圾/大文件/目录占用，区块内直接勾选清理/删除 |
| `frontend/src/composables/useMonitor.ts` | Tauri invoke/listen 封装（进程） |
| `frontend/src/composables/useCleaner.ts` | Tauri invoke/listen 封装（垃圾清理，scan-* 事件） |
| `frontend/src/composables/useDisk.ts` | Tauri invoke/listen 封装（大文件/目录合并扫描，disk-user-* 进度 + disk-large-files / disk-dir-usage 数据） |
| `frontend/src/components/` | shadcn-vue UI 组件 |
| `src-tauri/src/main.rs` | Tauri 入口，setup + 命令注册 |
| `src-tauri/src/commands/monitor.rs` | Tauri 命令：进程 get/kill |
| `src-tauri/src/commands/cleaner.rs` | Tauri 命令：扫描/清理/回收站 |
| `src-tauri/src/commands/disk.rs` | Tauri 命令：用户目录合并扫描（start_user_scan，大文件+目录单遍历）+ 大文件删除 |
| `crates/pony_core/src/lib.rs` | 核心库入口 |
| `crates/pony_core/src/monitor.rs` | 进程监控（sysinfo） |
| `crates/pony_core/src/cleaner.rs` | C盘扫描清理（jwalk） |
| `crates/pony_core/src/error.rs` | PonyError 枚举 + Result 别名 |
| `crates/pony_core/` | 纯业务库，零框架依赖 |

| `tests/` | 集成测试 |
| `docs/` | 项目文档 |
| `00_DASHBOARD.md` | 任务系统总控面板 |
| `01_TASK_BOARD.md` | 任务板 |
| `03_TASKS/` | 单任务详情 |

## 关键技术约束

- **桌面壳**: Tauri 2，WebView + Rust IPC，无浏览器引擎
- **前端**: Vue 3 Composition API + TypeScript，shadcn-vue 组件
- **IPC**: `tauri::command`（请求/响应）+ `AppHandle::emit`（事件推送）
- **后端异步**: tokio runtime 在 Tauri 内置 async 中运行，CPU 密集型操作使用 `tokio::task::spawn_blocking`
- **进程数据**: std::thread + sysinfo 每 2s 全量刷新，结果共享到 `Arc<RwLock<Option<Snapshot>>>`
- **磁盘遍历**: jwalk 并行遍历，广度优先，进度通过 std::sync::mpsc + Tauri events 推送
- **安全性**: 后端强制执行路径验证（`is_path_protected` + `is_path_allowed`），拒绝越权操作
