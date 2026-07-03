# PonyClean 窗口形态与胶囊 UX — 设计规约 v3.2（最终实现）

> **⚠️ 废弃状态**: 本规约已被 `DynamicIsland-Spec.md v1.0` 替代。请参考新文档。

**文档**: `docs/design/Window-Morph-Spec.md`  
**版本**: 3.2  
**状态**: ✅ 已实现（2026-06-29）  
**关联**: ARCHITECTURE.md, ADR-007, ADR-008 (待创建)

---

## 1. 概述

本文档定义 PonyClean 的两种窗口形态（**全窗口** ↔ **胶囊**）及其自动变换、吸附行为。

| 形态 | 用途 |
|---|---|
| **全窗口 (FullWindow)** | 进程监控面板 / C盘清理面板，交互操作 |
| **胶囊 (Capsule)** | 系统指标悬浮条，显示 CPU + MEM 占用，不可拖拽，单击展开 |

> **核心约束**: 窗口**始终维持初始化尺寸 `315×340`，永不调用 `setSize` / `setMinSize` / `setMaxSize`**。胶囊态通过 CSS 遮罩 + 窗口位置偏移实现，而非物理 resize。

---

## 2. 窗口规格

### 2.1 全窗口（固定，永不更改）

| 属性 | 值 |
|---|---|
| 尺寸 | `315 × 340` — **固定** |
| 装饰 | `decorations: false`，无原生标题栏 |
| 可缩放 | `resizable: false` |
| 透明 | `transparent: true` |
| 置顶 | `alwaysOnTop: true` |
| 任务栏 | `skipTaskbar: true` |

> `tauri.conf.json` 中设置 `resizable: false`。`minWidth/maxWidth` 不需要设置——`resizable: false` 已阻止缩放。声明式约束与运行时 `setMinSize()` API 无关。
>
> **拖拽**: `resizable: false` **不影响** `startDragging()`。通过 TitleBar 的 `@mousedown` 调用 `getCurrentWindow().startDragging()` 实现全窗口拖动。

### 2.2 胶囊形态（伪态）

胶囊**不是独立窗口尺寸**，而是 FullWindow 的"可视裁剪态"：

| 属性 | 值 |
|---|---|
| 实际窗口尺寸 | 始终 `315 × 340` |
| 可视区域 | 窗口底部 `160 × 40` 的胶囊条，其余区域**全透明**且可点击穿透 |
| 实现机制 | `window transparent: true` + 根元素 CSS `background: transparent` + 胶囊 DOM 独自持有 `background`；透明区域 `pointer-events: none`；无需 `setOpacity` |

### 2.3 胶囊条规格

| 属性 | 值 |
|---|---|
| 宽度 | `160` |
| 高度 | `40` |
| 圆角 | `20`（CSS `border-radius: 9999px`） |
| 背景 | `rgba(30, 28, 26, 0.95)`，**无 `backdrop-filter`**（避免动画掉帧 + 全透窗口降级） |
| 边框 | `1px solid hsla(0,0%,100%,0.08)` |
| 阴影 | `0 4px 8px rgba(0,0,0,0.4)` |
| 定位 | CSS 绝对居中于窗口：`top: 50%; left: 50%; transform: translate(-50%, -50%)` |

> 定位从 v3.0 的 `bottom: 0` 改为几何中心，使 shrink/expand 动画以窗口中心为轴心缩放。

---

## 3. 胶囊内部布局

```
┌──────────────────────────────────────────────────────┐
│  ┌──────────────────────┐ ┊ ┌──────────────────────┐ │
│  │ ████████████████░░░░ │ ┊ │ ████████████████████  │ │
│  │          45 CPU      │ ┊ │           78 MEM     │ │
│  └──────────────────────┘ ┊ └──────────────────────┘ │
│         (填充层)            ┊         (填充层)         │
└──────────────────────────────────────────────────────┘
```

### 3.1 分半规则

- 胶囊条等分为**左半区 (CPU)** 和**右半区 (MEM)**，各占 `50%`
- 中间 `1px` 分隔线（`bg-white/10`）

### 3.2 每个半区的结构

每个半区是一个**独立进度条组件**：

- **背景层**: 半透明底色，颜色跟随语义等级（`@12% opacity`）
- **填充层**: 从左侧起始，按 `min(pct, 100)%` 宽度填充，颜色跟随语义等级（`@75% opacity`），`border-radius: 9999px` 覆盖所在半区的外角
- **文本层**: 数字**居中**叠加在填充条之上，**纯白对比色**（`text-white font-bold`），**不显示 `%` 符号**，格式为 `"45"` + 半透明标签 `"CPU"`（`text-white/70 text-[10px]`）

### 3.3 语义颜色（仅用于填充条）

| 占用率 | 填充色 | CSS | WCAG AA 对比度（白字） |
|---|---|---|---|
| 0–49% | 绿 | `hsl(142, 65%, 42%)` | ≥4.5:1 |
| 50–79% | 黄 | `hsl(42, 75%, 38%)` | ≥4.5:1 |
| 80–100% | 红 | `hsl(0, 75%, 48%)` | ≥4.5:1 |

> 色值有意调暗以保证白色文本对比度达标。文本始终 `text-white`。

---

## 4. 状态机与变换流程

### 4.1 状态定义

| 状态 | 窗口尺寸 | 窗口位置 | 根背景 | 可见内容 |
|---|---|---|---|---|
| **FullWindow** | 315×340 | 用户定位/居中 | 渐变深色 `linear-gradient(...)` | 全面板 (TitleBar + Panel) |
| **Shrinking** | 315×340 | 当前位置 | CSS 动画过渡中 | 面板 scale/opacity + 胶囊渐入 |
| **Capsule** | 315×340 | 当前位置（未吸附） | `transparent` | 仅胶囊条 |
| **Docking** | 315×340 | 当前位置 → 吸附边缘 | `transparent` | 仅胶囊条 |
| **Docked** | 315×340 | 吸附于屏幕边缘 | `transparent` | 仅胶囊条 |

### 4.2 核心设计决策

- **永不调用 `setSize` / `setMinSize` / `setMaxSize`**（`tauri.conf.json` 中的 `resizable: false` 是声明式，非运行时 API）
- **永不调用 `setOpacity`**（因全窗口 `transparent: true`，CSS 控制背景透明度即可）
- 形态切换通过 CSS class 切换 + `setPosition` 实现
- 删除 `animate_window` 和 `cancel_animation` Rust commands（死代码）
- 胶囊区域 `pointer-events: auto`，透明区域 `pointer-events: none`（点击穿透）

### 4.3 状态转换图

```
[FullWindow] ──(15s system-idle)──► [Shrinking] ──(CSS anim 500ms)──► [Capsule] ──(pause 800ms)──► [Docking] ──(setPosition 0ms)──► [Docked]
     ▲                                    │                                                         │
     │                                    │ (mouseenter → 反向动画)                                  │ (mouseenter)
     │                                    ▼                                                         ▼
     └──────────────────────────── [FullWindow] ◄───────────────────────────────────────────────────┘
                                                                          │
                                                                          │ (click)
                                                                          ▼
                                                                      [FullWindow]

[FullWindow] ──(扫描/清理任务进行中)──► 暂停空闲计时器
```

### 4.4 空闲检测

同 v2.0：系统级 `GetLastInputInfo` + WebView 事件辅助。扫描任务中暂停。

### 4.5 收缩动画（Shrinking）

**CSS 动画，无 Tauri window resize**：

```
总时长: 500ms
```

| 阶段 | 时间 | CSS 操作 |
|---|---|---|
| 面板缩小 | 0–400ms | `.panel-layer { transform: scale(1→0.3); opacity: 1→0; }` |
| 胶囊放大 | 200–500ms | `.capsule-layer { animation: capsule-enter 300ms cubic-bezier(...) 200ms both; }`，从窗口中心 `scale(0.3)` → `scale(1)` |
| DWM 阴影 | 400ms (transitionend) | `win.setShadow(false)` + `setEffects([])` 清除 |

**动画曲线**: `cubic-bezier(0.32, 0.72, 0, 1)`

**关键**: `animation-delay: 200ms` 确保面板缩小至 `scale(0.3)` 后胶囊才开始从中心放大进入。

### 4.6 展开动画（Expanding）

| 阶段 | 时间 | CSS 操作 |
|---|---|---|
| 胶囊缩小 | 0–200ms | `.capsule-layer { animation: capsule-exit 200ms cubic-bezier(...) both; }`，从中心 `scale(1)` → `scale(0.3)` |
| 面板放大 | 0–400ms | `.panel-layer` 从 `panel-hidden` → `panel-ready` 触发 `scale(0.3→1); opacity: 0→1` |
| 阴影/特效 | 400ms (transitionend) | `win.setShadow(true)` + `setEffects({ acrylic })` 恢复 |

两个动画同时进行，形成交叉溶解效果。`@transitionend` 触发后卸载胶囊 DOM。

### 4.7 停留 → Docking

停留 `800ms`，期间 `mouseenter` 可中止（反向动画恢复 FullWindow）。

Docking 无动画：直接 `win.setPosition(targetX, targetY)`，因窗口已全透（`background: transparent`），跳跃不可见。

**目标位置计算**（窗口左上角）：

```
EDGE_PADDING = 8
FULL_W = 315, FULL_H = 340
CAPSULE_W = 160, CAPSULE_H = 40

上边缘吸附:
  targetY = EDGE_PADDING - (FULL_H - CAPSULE_H)  // = 8 - 300 = -292
  targetX = clamp(8, 胶囊中心.x - CAPSULE_W/2, monitorWidth - FULL_W - 8)

左边缘吸附:
  targetX = EDGE_PADDING - (FULL_W - CAPSULE_W)  // = 8 - 155 = -147
  targetY = clamp(8, 当前 Y, monitorHeight - FULL_H - 8)

右边缘吸附:
  targetX = monitorWidth - CAPSULE_W - EDGE_PADDING
  targetY = clamp(8, 当前 Y, monitorHeight - FULL_H - 8)
```

> 使用 `getCurrentWindow().currentMonitor()` 获取当前显示器尺寸。

### 4.7 胶囊 → 全窗口（展开）

**展开顺序（防止闪跳）**:

1. **先** `win.setPosition(targetX, targetY)` — 不可见状态完成位置恢复
2. 根元素 `background` 从 `transparent` → `linear-gradient(...)`
3. CSS 动画 `400ms`：胶囊 `scale(1→0.3)` + `opacity(1→0)`，面板 `scale(0.3→1)` + `opacity(0→1)`
4. unmount 胶囊视图，mount 全窗口面板
5. 重置空闲计时器

**位置恢复**（同 v2.0 展开位置逻辑——全窗口完全可见）：

- 若左区域→左对齐 `x: 8`；若右区域→右对齐 `x: monitorWidth - 315 - 8`
- 若顶部→水平居中并 clamp 于 `[8, monitorWidth - 315 - 8]`
- `targetY = max(8, 当前 Y + CAPSULE_H/2 - FULL_H/2)`

### 4.8 鼠标逃逸

Shrinking 阶段 `mouseleave` → 反向动画恢复 FullWindow（`direction: reverse`）。

---

## 5. 退出路径

Docked 状态下胶囊是唯一可见元素，无关闭按钮。提供以下退出途径：

1. **系统托盘图标**: 最小化至系统托盘，右键菜单含「退出」
2. **胶囊右键菜单**: `@contextmenu.prevent` 弹出「退出 PonyClean」

> 系统托盘为必选实现，胶囊右键为增强体验。二者至少提供其一。

---

## 6. 保留的 v2.0 功能

- 首次 Docked 呼吸动画（`CapsuleBar.vue` 的 `animate-breath` class，`opacity: 0.7↔1`，3 次循环）
- 吸附方位偏好设置（TitleBar ⚙菜单 → `localStorage.setItem('pony_dock_pref')`）
- 系统空闲检测（`GetLastInputInfo` + WebView 事件）
- `useMonitor` 的 `cpuPercent`/`memPercent` computed + `setPollInterval`
- 扫描任务中暂停空闲计时器

---

## 7. 配置文件变更

`src-tauri/tauri.conf.json` 窗口配置：

```json
{
  "label": "main",
  "title": "PonyClean",
  "width": 315,
  "height": 340,
  "resizable": false,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "center": true
}
```

> `resizable: false` 生效时「minWidth/maxWidth」为冗余，不设置。

---

## 8. 改动清单

| 文件 | 操作 | 说明 |
|---|---|---|
| `src-tauri/tauri.conf.json` | 改 | 加入 `resizable: false`，移除 `minWidth/minHeight`（v2.0 已移除，确认不再恢复） |
| `src-tauri/src/commands/window.rs` | 改 | **删除** `animate_window` / `cancel_animation` 函数，仅保留 `quit_app` 和 `get_system_idle_ms` |
| `src-tauri/src/main.rs` | 改 | 注销 `animate_window` / `cancel_animation` |
| `src-tauri/capabilities/default.json` | 改 | 还原窗口权限（set-size / set-position 等不再需要） |
| `frontend/src/composables/useWindowMorph.ts` | 重写 | 移除 `invoke('animate_window')` / `invoke('cancel_animation')`；状态切换改为 CSS class + `setOpacity` 替代为 CSS `background` 切换 + `setPosition` |
| `frontend/src/components/CapsuleBar.vue` | 重写 | 尺寸 `120×32` → `160×40`；去除 `%` 符号；文本 `text-white`；填充条样式重做 |
| `frontend/src/App.vue` | 改 | 新增 CSS 动画 class 绑定（`.is-shrinking` / `.is-docked`）；根元素 `background` 按形态切换 |
| `frontend/src/styles/globals.css` | 改 | 新增 `@keyframes shrink-panel` / `grow-capsule` |

---

## 9. 验收标准

| # | 验收条件 | 关联审查意见 |
|---|---|---|
| 1 | 全窗口形态下 15s 无输入 → 触发 Shrinking CSS 动画 | — |
| 2 | 扫描/清理任务运行时暂停空闲计时器 | v2.0 |
| 3 | 收缩动画为 CSS scale/opacity，`animation-delay: 200ms` 协调面板缩小与胶囊放大 | 完整-#5 |
| 4 | 根元素 `background` 从 `linear-gradient(...)` 切换为 `transparent`，窗口无 `setOpacity` 调用 | 架构-#1 |
| 5 | 透明区域 `pointer-events: none`，点击穿透到底层应用 | 架构-#2 |
| 6 | 停留 `800ms` 后窗口移动到屏幕边缘，`setPosition` 无跳跃 | 架构-#3 |
| 7 | 展开时**先 setPosition 恢复位置 → 再 setOpacity/CSS 动画**，无闪跳 | UX-#2 |
| 8 | 胶囊条尺寸 `160×40`，左右对半 | — |
| 9 | 每个半区：填充条按 `min(pct,100)%` 填充 + 白色数字居中 + 无 `%` 符号 | — |
| 10 | 语义颜色满足白字 WCAG AA 对比度（≥4.5:1） | UX-#5 |
| 11 | 从不吸附到底部 | — |
| 12 | Alt+Tab / Win+Tab / Win+Z 不显示 Docked 态窗口 | UX-#3 |
| 13 | 胶囊右键菜单或系统托盘提供退出路径 | UX-#6 |
| 14 | `resizable: false` 下 TitleBar 拖拽仍正常工作 | 完整-#1 |
| 15 | 首次 Docked 触发呼吸动画 | v2.0 |
| 16 | 吸附方位偏好设置（⚙菜单）生效 | v2.0 |

---

## 10. 不在此范围内

- 胶囊拖拽手动调整位置
- `backdrop-filter` 毛玻璃效果（移除以保性能）
- 多显示器适配（首版仅当前窗口所在显示器）
- 覆盖全屏独占游戏的顶层展示
