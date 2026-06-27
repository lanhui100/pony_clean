# TASK-004 UI 集成与数据流打通

## Basic Info
- ID: TASK-004
- 状态: Backlog
- 优先级: P1
- 负责人: @self
- 创建日期: 2026-06-27
- 更新日期: 2026-06-27
- 预估工时: 5h
- 依赖: TASK-002, TASK-003

## Goal
将 TASK-002（进程监控）和 TASK-003（C盘清理）集成到 egui UI 中。实现双 Tab 面板、非阻塞数据流、窗口生命周期管理、状态保持。

## Output
- `src/app.rs` — 完整 UI 实现（~350 行，含状态管理、渲染、事件处理）
- `src/ui.rs` — UI 组件拆分（进程面板、C盘面板、状态栏）

## 验收标准
1. UI 分为两个 Tab：「进程监控」和「C盘清理」，Tab 切换不丢失勾选状态
2. 进程 Tab：顶栏概览（总 CPU/内存）+ 滚动列表（name, pid, cpu%, mem, status, kill 按钮），超高进程红色高亮
3. 扫描 Tab：扫描状态机（Idle → Scanning → Done/Cancelled），进度展示，可勾选列表，清理按钮
4. 数据通过 `std::sync::mpsc` + drain 循环非阻塞消费
5. 窗口无边框半透明，wgpu backend，可拖动（`ViewportCommand::StartDrag`）
6. 新数据到达时触发 `ctx.request_repaint()`，无数据时不空转 60fps
7. 窗口关闭时 cancel scan + shutdown monitor/cleaner
8. Kill 按钮发送后 UI 侧通过 `pending_kill` 机制显示结果反馈
9. ScanEvent::Done 到达后 UI 切换状态、显示总计可释放空间

## UI 布局设计

```
┌──────────────────────┐
│  PonyClean     — □ × │  ← 点击拖动 (ViewportCommand::StartDrag)
├──────────────────────┤
│  [进程]  [C盘清理]     │  ← egui 手动 Tab (mut selected_tab)
├──────────────────────┤
│                      │
│   (Tab 内容区)        │
│                      │
│                      │
└──────────────────────┘
```

### 进程 Tab

```
┌─ 进程监控 ──────────────────────────┐
│  CPU: 45%  内存: 6.2GB / 16GB      │  ← SystemSummary
├────────────────────────────────────┤
│  Name          CPU%    Mem   Status│
│  chrome.exe    120%   420MB  [✕]  │  ← 红色(超高)
│  code.exe      2.5%   280MB  [✕]  │
│  python.exe    85%    180MB  [✕]  │  ← 红色(CPU超)
│  ...                               │
│  (未连接 — 监控已停止)  [重启]      │  ← channel 断开时显示
└────────────────────────────────────┘
```

### C盘清理 Tab

```
┌─ C盘安全清理 ───────────────────────┐
│  可用: 32GB / 256GB   [开始扫描]   │  ← GetDiskFreeSpaceExW
│  ████████░░░░░░░░░░ 12%           │
├────────────────────────────────────┤
│  扫描中... (1245 个文件)   [取消]   │  ← Scanning 状态
├────────────────────────────────────┤
│  ✅ Temporary Files     1.2GB      │  ← 勾选保持 HashSet<PathBuf>
│  ✅ Prefetch             340MB     │
│  ☐ 浏览器缓存            890MB     │
│                                     │
│  总计可释放: 2.4GB    [清理选中]    │  ← Done 后显示
└────────────────────────────────────┘
```

## 状态机定义

### 扫描状态
```rust
enum ScanState {
    Idle,
    Scanning {
        cancel_token: CancellationToken,
        scanned: u64,
        current: String,
    },
    Done {
        items: Vec<CleanItem>,
        checked: HashSet<PathBuf>,
        total_bytes: u64,
    },
    Cancelled,
    Error(String),
}
```

**超时保护**: Scanning 状态下 120s 无事件 → 自动切换 Error("扫描超时，请重试")

### 监控状态
```rust
enum MonitorState {
    Connected,     // channel 正常
    Disconnected,  // channel 断开，后台 task 可能 panic
}
```

## 应用状态结构

```rust
struct PonyCleanApp {
    // tokio runtime
    rt: tokio::runtime::Runtime,

    // monitor
    monitor_rx: Option<std::sync::mpsc::Receiver<monitor::Snapshot>>,
    monitor_cmd_tx: Option<std::sync::mpsc::Sender<monitor::MonitorCommand>>,
    monitor_state: MonitorState,
    latest_snapshot: Option<monitor::Snapshot>,
    /// Kill 结果反馈：每帧 try_recv 检查
    pending_kill: Option<tokio::sync::oneshot::Receiver<Result<(), String>>>,
    kill_feedback: Option<String>,

    // cleaner
    clean_cmd_tx: Option<std::sync::mpsc::Sender<cleaner::CleanCommand>>,
    clean_rx: Option<std::sync::mpsc::Receiver<cleaner::ScanEvent>>,
    scan_state: ScanState,
    scan_start_time: Option<std::time::Instant>,

    // UI
    selected_tab: Tab,
}

enum Tab { Process, Clean }
```

## 数据流

```
monitor::start() → mpsc::Sender<Snapshot>
   │
   └── UI 线程每帧 drain loop:
         while let Ok(snapshot) = self.monitor_rx.try_recv() {
             self.latest_snapshot = Some(snapshot);
             // 只在数据更新时 request_repaint
             ctx.request_repaint();
         }

cleaner::start_scan() → (mpsc::Sender<CleanCommand>, CancellationToken)
   │
   └── UI 线程每帧 drain loop:
         while let Ok(event) = self.clean_rx.try_recv() {
             match event {
                 ScanEvent::ItemsFound { items, batch_complete } => ...
                 ScanEvent::Done { total_items, total_bytes } => ...
                 ScanEvent::Cancelled => ...
                 ScanEvent::Warning(msg) => ...
             }
             ctx.request_repaint();
         }
```

## 窗口生命周期

```rust
impl Drop for PonyCleanApp {
    fn drop(&mut self) {
        // 1. 取消正在进行的扫描
        if let ScanState::Scanning { cancel_token, .. } = &self.scan_state {
            cancel_token.cancel();
        }
        // 2. 通知后台任务关闭
        if let Some(tx) = &self.monitor_cmd_tx {
            let _ = tx.send(MonitorCommand::Shutdown);
        }
        if let Some(tx) = &self.clean_cmd_tx {
            let _ = tx.send(CleanCommand::Shutdown);
        }
        // 3. tokio runtime 非阻塞关闭
        self.rt.shutdown_background();
    }
}
```

## 实现要点

### Kill 反馈机制
```rust
// 按钮点击时
if ui.button("✕").clicked() {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    if let Some(tx) = &self.monitor_cmd_tx {
        let _ = tx.send(MonitorCommand::Kill {
            pid: process.pid,
            name: process.name.clone(),
            resp: resp_tx,
        });
    }
    self.pending_kill = Some(resp_rx);
    self.kill_feedback = None;
}

// 每帧检查结果
if let Some(rx) = &mut self.pending_kill {
    if let Ok(result) = rx.try_recv() {
        self.kill_feedback = Some(match result {
            Ok(()) => "✓ 进程已终止".into(),
            Err(e) => format!("✗ {e}"),
        });
        self.pending_kill = None;
    }
}
```

### 跨 Tab 勾选保持
```rust
// checked 存储在 ScanState::Done 中，Tab 切换不销毁
match &mut self.scan_state {
    ScanState::Done { items, checked, total_bytes } => {
        for item in items {
            ui.checkbox(checked.contains(&item.path), ...);
            // 点击时更新 checked
            if ui.checkbox(checked.contains(&item.path), ...).clicked() {
                if checked.contains(&item.path) {
                    checked.remove(&item.path);
                } else {
                    checked.insert(item.path.clone());
                }
            }
        }
    }
    // 新扫描开始时清空
    _ => { /* 新建 checked HashSet */ }
}
```

### 省帧策略
```rust
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    // 只在数据到达或交互时触发重绘
    let new_data = self.drain_channels(); // 返回 bool
    if new_data {
        ctx.request_repaint();
    }
    // 无新数据时不调用 request_repaint（鼠标事件由 egui 自动处理）
    self.render_ui(ctx);
}
```

### 透明窗口 + Panel
```rust
// CentralPanel 必须 Frame::none()
egui::CentralPanel::default()
    .frame(egui::Frame::none())
    .show(ctx, |ui| {
        ui.style_mut().visuals.window_fill = egui::Color32::TRANSPARENT;
        ui.style_mut().visuals.panel_fill = egui::Color32::TRANSPARENT;
        // ... 渲染内容
    });
```

### Card 布局（压缩极简）
使用带圆角和阴影的卡片包裹每个 Tab 的内容容器，而非裸 Panel，以在保持极简的同时提升视觉呼吸感：

```rust
egui::Frame::none()
    .fill(egui::Color32::from_black_alpha(0))  // 完全透明
    .rounding(12.0.into())
    .stroke(egui::Stroke::NONE)
}
```

### 冷色盘 Widget
替代黑白扁平方案，采用静谧深色盘 + 低饱和冷色调，适合长时间驻留的桌面工具：

```rust
// 深色底（微偏冷）
egui::Color32::from_rgb(18, 20, 24)     // #121418
// 卡片/面板底（半透明冷灰）
egui::Color32::from_rgba_premultiplied(28, 32, 38, 200)  // #1C2026 @ 78%
// 主文字（冷白）
egui::Color32::from_rgb(220, 222, 228)   // #DCDEE4
// 辅助文字（低饱和蓝灰）
egui::Color32::from_rgb(148, 155, 164)   // #949BA4
// 强调/报警（保持红但降低饱和度）
egui::Color32::from_rgb(207, 102, 102)   // #CF6666
```

## 测试策略
- **状态机测试**: 构造 mock Snapshot 和 ScanEvent，验证 UI 状态机转换正确
- **Kill 反馈测试**: 发送 Kill + 模拟 oneshot 响应，验证 UI 显示结果
- **不测**: egui 像素级渲染、窗口行为

## Current Progress
- 尚未开始

## Next Action
等待 TASK-002 和 TASK-003 完成后，在 `src/app.rs` 和 `src/ui.rs` 中实现。

## Resume Hint
打开 `src/app.rs`。先定义状态结构和枚举（Tab, ScanState, MonitorState），然后实现 `drain_channels()` 和 `render_ui()` 两大方法。渲染按 Tab 分两路。Drop 中 cancel scan + shutdown 所有后台任务。完成后创建 `src/ui.rs` 拆分进程面板和 C盘面板渲染逻辑。
