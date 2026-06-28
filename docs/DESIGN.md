# 设计决策记录

所有重大技术决策以 ADR（Architecture Decision Record）格式记录。

---

## ADR-001: 选择 egui 而非 Webview

**状态**: 已废弃（被 ADR-007 替代）

**上下文**: 需要一个 Windows 桌面悬浮窗，要求启动快、内存低、单二进制分发。

**方案对比**:
| 方案 | 启动时间 | 二进制大小 | 内存 | 透明窗口 |
|---|---|---|---|---|
| egui + eframe | <1s | ~5MB | ~10MB | 原生支持 |
| Tauri + Webview | 1-3s | ~10MB | ~50MB+ | 需 hack |
| Electron | 3-5s | ~150MB | ~100MB+ | 支持但笨重 |

**决策**: 最初选 egui。后因 UI 表现力瓶颈迁移至 Tauri v2（参见 ADR-007）。

---

## ADR-002: lib.rs 作为业务入口

**状态**: 已采纳

**上下文**: 需要让业务逻辑可被单元测试直接 import，且为未来 CLI 模式预留扩展点。

**决策**: `crates/pony_core/src/lib.rs` 统一 re-export 所有业务模块（`pub mod error; pub mod monitor; pub mod cleaner;`），`src-tauri/src/main.rs` 仅做 Tauri 初始化。

**理由**:
1. 业务逻辑与 GUI 解耦，可直接被测试 import
2. Tauri 入口保持薄层，只负责命令注册
3. 未来扩展 CLI 模式时只需新增一个 binary crate

---

## ADR-003: std::sync::mpsc 用于后台→UI 通信

**状态**: 已采纳（egui 内部）→ 被 Tauri IPC 替代

**上下文**: 后台 tokio 任务需要将进程数据和扫描进度推送到 GUI 线程（egui 时代）。

**决策**: 采用 `std::sync::mpsc` + `spawn_blocking`。Tauri 迁移后将前端通信层替换为 `tauri::command` + `AppHandle::emit`，mpsc 仅保留在 `pony_core` 内部用于后台线程间通信。

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

---

## ADR-007: egui → Tauri v2 + Vue 3 + shadcn-vue 迁移

**状态**: 已完成

**上下文**: egui UI 表现力无法满足产品级需求（无组件库、字体渲染差、无动画）。

**方案对比**:
| 维度 | egui + eframe | Tauri v2 + shadcn-vue |
|---|---|---|
| 组件库 | 无 | shadcn-vue (30+ 组件) |
| 字体渲染 | ab_glyph 软件渲染 | DirectWrite 原生 ClearType |
| 动画 | 无 | CSS + motion-vue |
| 开发效率 | 改 UI → 改 Rust → 编译 | HMR 热更新 |
| 运行时内存 | ~35MB | ~42MB (含 WebView2) |
| 二进制体积 | ~5MB (单二进制) | ~4.5MB (不含 WebView2 runtime) |

**迁移策略**: `crates/pony_core` 零改动，前后端通过 Tauri IPC 通信。
