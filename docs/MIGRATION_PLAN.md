# PonyClean UI 迁移方案：egui → Tauri v2 + Vue 3 + shadcn-vue

## 决策背景

当前 egui 架构在 UI 表现力上已触达上限：
- 无设计系统/组件库，所有 UI 手写
- 即时模式布局难以精确对齐
- 无 CSS 级动画/过渡
- 字体渲染依赖 ab_glyph 软件渲染，CJK 显示质量差
- 无虚拟滚动，大列表性能不佳

迁移到 Tauri v2 后，前端获得完整 Web 生态能力，后端业务逻辑（monitor/cleaner）完全不动。

## 目标技术栈

| 层 | 技术 | 版本 |
|---|---|---|
| 桌面框架 | Tauri | v2 |
| 前端框架 | Vue | 3 |
| 构建工具 | Vite | 8.1 |
| 组件库 | shadcn-vue (基于 Radix Vue) | latest |
| 样式 | Tailwind CSS | v4 |
|--|--|--|
| 组件库 | shadcn-vue (基于 Radix Vue) | latest (v4 兼容版本) |
| 动画 | motion-vue | latest |
| 动画 | motion-vue (Framer Motion 的 Vue 移植) | latest |
| 后端 | Rust | 1.85 |
| 系统监控 | sysinfo | 0.30 (不变) |
| 磁盘扫描 | jwalk | 0.8 (不变) |
| Windows API | windows-rs | 0.54 (不变) |

## 架构对比

```
当前 (egui):
┌─────────────────────────────────────────┐
│  main.rs (eframe init)                  │
│  app.rs (egui::App, 全部 UI 逻辑)       │
│    ├── render_monitor_panel()           │
│    ├── render_cleaner_panel()           │
│    └── theme.rs (硬编码主题)             │
│  monitor.rs / cleaner.rs / error.rs     │
│  (业务逻辑, 通过 mpsc 与 UI 通信)        │
└─────────────────────────────────────────┘

迁移后 (Tauri):
┌─────────────────────────────────────────┐
│  frontend/ (Vue 3 + shadcn-vue)         │
│    ├── App.vue (窗口布局 + Tab 路由)      │
│    ├── views/MonitorPanel.vue            │
│    ├── views/CleanerPanel.vue            │
│    ├── components/ui/ (shadcn-vue 组件)   │
│    ├── composables/useMonitor.ts         │
│    └── composables/useCleaner.ts         │
├─────────────────────────────────────────┤
│  src-tauri/ (Rust 后端)                  │
│    ├── src/main.rs (Tauri 入口)          │
│    ├── src/commands/ (Tauri 命令)         │
│    │   ├── monitor.rs                    │
│    │   └── cleaner.rs                    │
│    └── Cargo.toml (依赖 tauri + pony_core)│
├─────────────────────────────────────────┤
│  crates/pony_core/ (业务核心, 从 src/ 迁入)│
│    ├── src/monitor.rs                    │
│    ├── src/cleaner.rs                    │
│    └── src/error.rs                      │
└─────────────────────────────────────────┘
```

## 数据流变化

```
当前 mpsc 模式:
  monitor Task --[mpsc::Sender]--> app.rs rx.try_recv()

迁移后 Tauri Events 模式:
  Tauri Command (轮询) → 返回 Snapshot (同步)
  或
  Tauri Event (后台 push) → frontend listen()
```

推荐方案：**混合模式**
- **进程列表**：Tauri Command `get_processes()` 前端定时轮询（~2s），避免 WebSocket/Event 复杂度
- **扫描进度**：Tauri Event `scan-progress` 后台推送到前端（流式，中间态多）
- **删除结果**：Tauri Command `execute_clean()` 异步执行，返回 `DeleteResult`

## 迁移步骤（6 个任务）

| ID | 任务 | 工时 | 依赖 |
|---|---|---|---|
| TASK-005 | Tauri v2 项目脚手架 + shadcn-vue 设置 | 4h | 无 |
| TASK-006 | Rust 后端 Tauri 命令封装 | 6h | TASK-005 |
| TASK-007 | Vue 设计系统 + 窗口布局 | 4h | TASK-005 |
| TASK-008 | Vue 监控面板 | 6h | TASK-006, TASK-007 |
| TASK-009 | Vue 清理面板 | 6h | TASK-006, TASK-007 |
| TASK-010 | 集成测试 + 旧代码清理 + ADR 更新 | 3h | TASK-008, TASK-009 |

## 保留/删除清单

### 完全保留（零改动）
- `crates/pony_core/src/monitor.rs` — 监控逻辑
- `crates/pony_core/src/cleaner.rs` — 清理逻辑
- `crates/pony_core/src/error.rs` — 错误类型
- 所有 `#[cfg(test)]` 单元测试
- `tests/` 集成测试
- `docs/` 文档

### 新增
- `frontend/` — Vue 前端
- `src-tauri/` — Tauri Rust 后端
- `crates/pony_core/` — 从 `src/` 迁出的业务核心 (lib crate)

### 删除
- `src/app.rs` — egui UI 代码（功能被 Vue 替换）
- `src/main.rs` — eframe 入口（被 Tauri 入口替换）
- `src/theme.rs` — 硬编码主题（被 Tailwind + shadcn-vue theme 替换）
- 依赖: egui, eframe, wgpu（不再需要）

## 设计语言对应

| egui 设计 | shadcn-vue / Tailwind 方案 |
|---|---|
| `CARD_BG: rgba(31,36,43,235)` | `bg-card` (shadcn-vue CSS variable) |
| `CARD_ROUNDING: 12px` | `rounded-xl` (Tailwind) |
| `TEXT_PRIMARY: #E8EAED` | `text-foreground` |
| `TEXT_SECONDARY: #9AA0A6` | `text-muted-foreground` |
| `ACCENT_BLUE: #8AB4F8` | `--primary` CSS variable |
| 玻璃效果 (透明度 + 无模糊) | `bg-card/80 backdrop-blur-xl` |
| `Separator` | `<Separator />` shadcn-vue 组件 |
| `ProgressBar` | `<Progress />` shadcn-vue 组件 |
| `CollapsingState` | `<Collapsible />` shadcn-vue 组件 |
| `Grid` 表格 | `<Table />` shadcn-vue 组件 |

## 窗口配置 (tauri.conf.json)

```json
{
  "app": {
    "windows": [
      {
        "title": "PonyClean",
        "width": 420,
        "height": 680,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "center": true
      }
    ]
  }
}
```

## 风险与缓解

| 风险 | 概率 | 缓解 |
|---|---|---|
| WebView2 未安装 (Win10 以下) | 低 | Tauri 内置检测 + 下载引导 |
| shadcn-vue 对 Tailwind v4 的兼容性 | 中 | 使用 `shadcn-vue@next` canary 通道 |
| mpsc→Event 迁移数据丢失 | 低 | 先实现 Command 轮询模式，Event 作为优化 |
| sysinfo 在 Tauri 上下文行为差异 | 低 | sysinfo 是纯同步库，不依赖框架 |
| 透明窗口拖拽兼容性 | 中 | Tauri v2 有 `data-tauri-drag-region` 属性 |

## 目录结构变更

```
pony_clean/
├── Cargo.toml              → 工作区 (workspace)
├── crates/
│   └── pony_core/          ← NEW (从 src/ 迁出)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── monitor.rs  ← 现有, 不变
│           ├── cleaner.rs  ← 现有, 不变
│           └── error.rs    ← 现有, 不变
├── src/                    ← KEEP (旧 egui 入口, 过渡期参考)
│   ├── main.rs
│   ├── app.rs
│   ├── lib.rs
│   ├── monitor.rs
│   ├── cleaner.rs
│   ├── error.rs
│   └── theme.rs
├── src-tauri/              ← NEW
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   ├── src/
│   │   ├── main.rs
│   │   └── commands/
│   │       ├── mod.rs
│   │       ├── monitor.rs
│   │       └── cleaner.rs
│   └── icons/
├── frontend/               ← NEW
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── index.html
│   ├── src/
│   │   ├── main.ts
│   │   ├── App.vue
│   │   ├── styles/
│   │   │   └── globals.css
│   │   ├── lib/
│   │   │   └── utils.ts
│   │   ├── components/
│   │   │   └── ui/ (shadcn-vue)
│   │   ├── composables/
│   │   │   ├── useMonitor.ts
│   │   │   └── useCleaner.ts
│   │   └── views/
│   │       ├── MonitorPanel.vue
│   │       └── CleanerPanel.vue
│   └── components.json (shadcn-vue config)
├── tests/                  ← KEEP (集成测试, 指向 pony_core)
├── docs/                   ← KEEP
├── 00_DASHBOARD.md         ← UPDATE
├── 01_TASK_BOARD.md        ← UPDATE
└── 03_TASKS/               ← ADD task cards
```
