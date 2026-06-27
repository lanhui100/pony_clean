# SPEC-006: Rust 后端 Tauri 命令封装

## 1. 目标

将 `pony_core` 的业务能力暴露为 Tauri commands + events，前端通过 `@tauri-apps/api` 调用。

## 2. 核心设计原则

- **最小修改 pony_core** — 仅在必要处做适配性修改（新增 `start_shared()` 接口，保留旧 `start()`）
- **命令层只做适配** — 类型转换、错误映射、并发管理
- **流式数据用 Tauri Event** — cleaner 扫描进度通过 Event 推送

## 3. 需要修改的 pony_core API

### 3.1 monitor: 新增 `start_shared()` 替代 mpsc 模式

```rust
// 新增（保留旧的 start() 不变）
pub fn start_shared(
    snapshot: Arc<RwLock<Option<Snapshot>>>,
) -> (Sender<MonitorCommand>, JoinHandle<()>)
```

- 后台线程每 2s 刷新 sysinfo，更新 `Arc<RwLock<Option<Snapshot>>>`
- 与旧 `start(tx)` 区别：不通过 mpsc Sender 推送，而是写入共享状态
- kill 命令通过命令通道发送，与旧逻辑一致

### 3.2 cleaner: start_scan 改为回调模式

```rust
// 新增回调类型
pub type ScanCallback = Box<dyn Fn(ScanEvent) + Send + Sync + 'static>;

pub fn start_scan_with_callback(
    on_event: ScanCallback,
) -> Result<(CleanCmdSender, CancellationToken), String>
```

- 扫描进度通过 `on_event(ScanEvent::Progress {..})` 回调通知
- Tauri 命令层在回调中 emit event
- 保留旧的 `start_scan()` 不变（使用 mpsc）

## 4. 状态管理 (Tauri State)

### 4.1 MonitorState
```rust
struct MonitorState {
    snapshot: Arc<RwLock<Option<Snapshot>>>,
    cmd_tx: Mutex<Option<Sender<MonitorCommand>>>,
    _thread: Mutex<Option<JoinHandle<()>>>,
}
```
- `snapshot`: 后台线程实时更新，`get_processes` 读取
- `cmd_tx`: 用于发送 kill/shutdown 命令
- `_thread`: drop 时 join

### 4.2 CleanerState
```rust
struct CleanerState {
    cancel_token: Arc<CancellationToken>,
    is_scanning: Arc<AtomicBool>,
}
```

## 5. 命令定义

### 5.1 get_processes
```rust
#[tauri::command]
async fn get_processes(state: State<'_, MonitorState>) -> Result<Snapshot, String>
```
- 从 `state.snapshot.read()` 读取最新快照并 clone
- 频率: 前端每 2s 轮询
- 错误: RwLock poisoned → 返回错误消息

### 5.2 kill_process
```rust
#[tauri::command]
async fn kill_process(pid: u32, name: String, state: State<'_, MonitorState>) -> Result<(), String>
```
- 通过 `state.cmd_tx` 发送 `MonitorCommand::Kill`
- 等待 oneshot 响应（与现有逻辑一致）
- **不直接访问 sysinfo::System** — 通过命令通道委托给后台线程处理

### 5.3 start_scan
```rust
#[tauri::command]
async fn start_scan(app: AppHandle, state: State<'_, CleanerState>) -> Result<(), String>
```
- 调用 `pony_core::cleaner::start_scan_with_callback()`
- 回调中使用 `app.emit("scan-progress", payload)` 推送进度
- 扫描完成时 emit "scan-done"

### 5.4 execute_clean
```rust
#[tauri::command]
async fn execute_clean(paths: Vec<String>) -> Result<DeleteResult, String>
```
- `paths: Vec<String>` → `Vec<PathBuf>` 转换
- 在 `tokio::task::spawn_blocking` 中调用 `pony_core::cleaner::delete_files()`
- 返回 `DeleteResult { success, failed, errors }`

### 5.5 empty_recycle_bin
```rust
#[tauri::command]
async fn empty_recycle_bin() -> Result<(), String>
```
- 在 `spawn_blocking` 中调用 `pony_core::cleaner::empty_recycle_bin()`
- COM 初始化在 spawn_blocking 线程内完成（cleaner 内部已处理）

## 6. 事件定义

```rust
#[derive(Serialize, Clone)]
struct ScanProgressPayload {
    scanned: u64,
    current: String,
}

#[derive(Serialize, Clone)]
struct ScanDonePayload {
    total_items: u64,
    total_bytes: u64,
}
```

## 7. 类型序列化

在 `pony_core` 中添加 serde 依赖，为以下类型添加 `#[derive(Serialize, Deserialize)]`：

- `monitor::ProcessInfo` — Serialize
- `monitor::SystemSummary` — Serialize  
- `monitor::Snapshot` — Serialize
- `cleaner::CleanItem` — Serialize
- `cleaner::DeleteResult` — Serialize
- `cleaner::SafetyLevel` — Serialize

**TypeScript 接口对应**（注意：TS 中所有数值类型都是 `number`，无 `f32`/`f64`）:
```typescript
interface Snapshot {
  summary: SystemSummary;
  processes: ProcessInfo[];
}
interface SystemSummary {
  cpu_total: number;
  mem_used_mb: number;
  mem_total_mb: number;
  process_count: number;
}
interface ProcessInfo {
  pid: number;
  name: string;
  cpu: number;
  mem_mb: number;
  status: string;
}
interface DeleteResult {
  success: number;
  failed: number;
  errors: string[];
}
```

## 8. 测试策略

- 单元测试覆盖 command 函数（mock Tauri State）
- 集成测试通过 `tauri::test::mock_builder()` 验证 IPC
- 手动：`cargo tauri dev` + 前端 invoke 验证
