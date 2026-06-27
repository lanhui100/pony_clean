# 设计决策记录

所有重大技术决策以 ADR（Architecture Decision Record）格式记录。

---

## ADR-001: 选择 egui 而非 Webview

**状态**: 已采纳

**上下文**: 需要一个 Windows 桌面悬浮窗，要求启动快、内存低、单二进制分发。

**方案对比**:
| 方案 | 启动时间 | 二进制大小 | 内存 | 透明窗口 |
|---|---|---|---|---|
| egui + eframe | <1s | ~5MB | ~10MB | 原生支持 |
| Tauri + Webview | 1-3s | ~10MB | ~50MB+ | 需 hack |
| Electron | 3-5s | ~150MB | ~100MB+ | 支持但笨重 |

**决策**: 选 egui。

**理由**:
1. 单二进制分发，不需嵌入浏览器引擎
2. 启动快（<1s），内存低（~10MB）
3. 透明无边框悬浮窗原生支持
4. 即时模式 GUI，状态管理简单

**权衡**: 样式能力有限，不适合复杂布局；无 DOM，自定义控件需用 egui 原生 widget 组合。

---

## ADR-002: lib.rs 作为业务入口

**状态**: 已采纳

**上下文**: 需要让业务逻辑可被单元测试直接 import，且为未来 CLI 模式预留扩展点。

**决策**: `src/lib.rs` 统一 re-export 所有业务模块（`pub mod error; pub mod monitor; pub mod cleaner;`），`src/main.rs` 仅做初始化。

**理由**:
1. 业务逻辑与 GUI 解耦，可直接被测试 import
2. main.rs 保持薄层，只负责初始化
3. 未来扩展 CLI 模式时只需换一个 binary 入口（如 `src/bin/cli.rs`）

**结构**:
```
src/
├── lib.rs      # 库入口
├── main.rs     # 薄 binary，依赖 lib
├── app.rs      # GUI 相关（不在 lib 中 re-export）
└── monitor.rs  # 业务模块（在 lib 中 re-export）
```

---

## ADR-003: std::sync::mpsc 用于后台→UI 通信

**状态**: 待定（TASK-004 验证后最终确定）

**上下文**: 后台 tokio 任务需要将进程数据和扫描进度推送到 GUI 线程。

**选项**:
1. `std::sync::mpsc` — 简单，Receiver 是 `Send + !Sync`，适合单消费者
2. `tokio::sync::mpsc` — 需要 tokio 上下文，GUI 线程无 tokio runtime
3. `tokio::sync::watch` — 适合流式状态广播，但需要 tokio context
4. `Arc<Mutex<T>>` — 共享状态，适合连续更新的计数器

**倾向方案**: `std::sync::mpsc`，因为 GUI 线程是唯一的消费者，不需要阻塞等待。

**最终决定**: 待 TASK-004 时根据实际数据流模式确认。

---

## ADR-004: 安全清理路径分级策略

**状态**: 已采纳

**上下文**: C盘清理需要区分可安全删除、需确认、禁止删除的路径。

**决策**: 三级分级制度。

| 级别 | 标签 | UI 行为 | 示例 |
|---|---|---|---|
| Safe | 🟢 | 默认勾选，一键清理 | `%TEMP%`, Prefetch, 浏览器 Cache |
| Confirm | 🟡 | 展示但不勾选，用户手动确认 | Downloads >90天未访问文件 |
| Forbidden | 🔴 | 不在 UI 显示，跳过 | `System32`, `Installer`, `ProgramData` |

**理由**: 无法预期所有用户的文件使用习惯，分级让用户有选择权同时保护系统安全。

---

## ADR-005: 删除策略 — 永久删除不走回收站

**状态**: 已采纳

**上下文**: 用户主动执行 C盘清理时，已明确意图是释放空间。

**决策**: 使用 `MoveFileExW + MOVEFILE_DELAY_UNTIL_REBOOT` 绕过占用锁，永久删除。回收站清空使用 `SHEmptyRecycleBinW`。

**理由**: 清理工具的目的是释放空间，走回收站违背用户意图。延迟删除机制可以绕过当前被占用的文件。
