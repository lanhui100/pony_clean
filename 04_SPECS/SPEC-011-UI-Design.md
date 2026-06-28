# SPEC-011: UI 设计规范

> 基于 `docs/UI_DESIGN.md` 提炼的可执行规范，直接指导 TASK-008/009 实现。
>
> **Design Read:** Windows 桌面 widget 界面设计，深色玻璃质感 + 轻量工具美学，偏向 Fluent Design 透明语言 + 现代极简风格。
> **Dial Values:** `VARIANCE: 4 | DENSITY: 5`

---

## 1. 设计系统 (Design Tokens)

### 1.1 色彩

维持现有 shadcn-vue `:root` CSS 变量：

```css
:root {
  --background: 0 0% 7%;          /* 窗口背景 */
  --foreground: 210 10% 92%;      /* 主文字 */
  --card: 220 10% 12%;            /* 卡片面板 */
  --card-foreground: 210 10% 92%;
  --primary: 214 90% 76%;         /* 交互色 #8AB4F8 */
  --primary-foreground: 0 0% 7%;
  --secondary: 220 8% 25%;
  --secondary-foreground: 210 10% 92%;
  --muted: 220 8% 20%;
  --muted-foreground: 215 8% 60%;
  --accent: 214 90% 76%;
  --accent-foreground: 0 0% 7%;
  --destructive: 0 80% 73%;       /* 红色 */
  --destructive-foreground: 0 0% 7%;
  --border: 220 8% 20%;
  --input: 220 8% 20%;
  --ring: 214 90% 76%;
  --radius: 0.75rem;
}
```

**新增语义色：** 在 `@theme` 中用硬编码 HSL 值声明，不经过 `:root` CSS 变量层（避免与 shadcn-vue 兼容层混淆）。

| Token | 值 | 用途 |
|---|---|---|
| `--color-success` | `hsl(142 70% 55%)` | 正常/绿色状态 |
| `--color-warning` | `hsl(38 90% 60%)` | 警告/琥珀色 |
| `--color-info` | `hsl(199 85% 55%)` | 信息/青色 |

实现方式 — 在 `globals.css` 的 `@theme` 块中硬编码：
```css
@theme {
  --color-success: hsl(142 70% 55%);
  --color-warning: hsl(38 90% 60%);
  --color-info: hsl(199 85% 55%);
  /* ... 现有其他 token */
}
```

**色彩使用规则：**
- Primary (blue) = 所有可交互元素（按钮、链接、焦点环、输入框聚焦）
- Success (green) = CPU/内存正常指标、扫描完成状态
- Warning (amber) = CPU > 50% 或内存 > 65%
- Destructive (red) = CPU > 80% 或内存 > 85%、终止进程、删除操作
- Muted-foreground = 辅助文字、次要信息
- 一律无 emoji，无手写 SVG 路径

### 1.2 字体

```css
/* 标题/正文 — 项目已有的 Segoe UI 栈，不变 */
font-family: 'Segoe UI Variable', 'Segoe UI', system-ui, -apple-system, sans-serif;

/* 新增：数据等宽字体（可选，降级到 Consolas） */
.data-font {
  font-family: 'Cascadia Code', 'JetBrains Mono', 'Consolas', monospace;
}
```

**字号层级：**

| 场景 | Tailwind | 用途 |
|---|---|---|
| 面板标题 | `text-lg` (18px) `font-semibold` | Monitor "进程监控"、Cleaner "C盘安全清理" |
| 摘要大数字 | `text-2xl` (24px) `font-bold tabular-nums` | Cleaner 可清理总量 |
| 正文/表格 | `text-sm` (14px) `font-normal` | 进程行、文件列表 |
| 辅助信息 | `text-xs` (12px) `font-normal` | 图例标签、文件路径 |
| 数据 | `text-sm tabular-nums` | CPU%、内存值 |

### 1.3 圆角系统

| 层级 | 值 | 用途 |
|---|---|---|
| Panel | `rounded-xl` (12px) | 卡片容器 |
| Element | `rounded-lg` (8px) | 输入框、按钮 |
| Inner | `rounded-md` (6px) | 内部容器 |
| Full | `rounded-full` | 状态点、Badge |
| Table | `rounded-none` | 表格单元格（容器有圆角） |

**规则：** 同一页面内严格使用上述 5 级，不引入新的圆角值。

### 1.4 阴影

```css
/* 卡片阴影 */
shadow-sm: 0 1px 2px rgb(255 255 255 / 0.04), 0 0 0 1px rgb(255 255 255 / 0.04);
/* 对话框阴影 */
shadow-lg: 0 4px 24px rgb(0 0 0 / 0.3), 0 0 0 1px rgb(255 255 255 / 0.06);
```

### 1.5 图标

仅使用项目已有的 `lucide-vue-next`（唯一例外：TitleBar 的 SVG pony icon，见 §2.2）。关键映射：

| 场景 | 图标 | 大小 |
|---|---|---|
| 搜索 | `Search` | `h-4 w-4` |
| 扫描 | `Scan` | `h-4 w-4` (按钮内), `h-8 w-8` (Idle 状态) |
| 清理 | `Trash2` | `h-4 w-4` |
| 重试 | `RotateCcw` | `h-4 w-4` |
| 成功 | `Check` | `h-5 w-5` |
| 错误 | `AlertCircle` | `h-5 w-5` |
| 关闭 | `X` | `h-4 w-4` |
| 展开 | `ChevronRight` | `h-4 w-4` |
| 加载 | `Loader2` | `h-4 w-4` |
| 进程 | `Activity` | `h-4 w-4` |
| 磁盘 | `HardDrive` | `h-4 w-4` |

---

## 2. 窗口框架 (App.vue + TitleBar)

### 2.1 整体布局

```
┌─ TitleBar (h-10, draggable) ──────────────────────┐
│ [🪽 PonyClean]                              [×] │
├─ TabNav (h-9) ─────────────────────────────────────┤
│ [进程监控]  [C盘清理]                               │
├─ Content Area (flex-1, p-4) ───────────────────────┤
│                                                     │
│  (MonitorPanel / CleanerPanel)                      │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### 2.2 TitleBar 组件

**实现要求：**
- 高度 `h-10` (40px)
- 左侧：SVG pony icon（独立组件 `frontend/src/components/PonyIcon.vue`，16×16px，暖色调）+ `PonyClean` 文字（`text-sm font-semibold text-primary`）
  - PonyIcon 是**唯一的**手写 SVG 例外，其他所有图标必须使用 `lucide-vue-next`
  - 未来如需增加窗口控件（最小化等），TitleBar 应拆分为子组件，当前粒度在 h-10 内合理
- 可拖拽区域：使用 `data-tauri-drag-region` 或 `appWindow.startDragging()`
- 右侧关闭按钮：hover 时 `bg-destructive text-destructive-foreground`，`rounded-md h-6 w-6`
- 关闭按钮使用 `X` 图标（`h-3.5 w-3.5`）
- 底部 1px `border-b border-border`

**拖拽实现：** 使用 `@mousedown="appWindow.startDragging()"`（已在 TitleBar.vue 实现），保留。

### 2.3 Tab 导航

**推荐方案**：从当前的下划线式 tabs 改为 **Pill Tabs**（可选——下划线式 tabs 当前实现也可接受，不阻塞 TASK-008/009）：

```html
<div class="inline-flex items-center gap-1 rounded-lg bg-muted/50 p-1">
  <button class="rounded-md px-3 py-1.5 text-sm font-medium transition-all duration-200"
          :class="activeTab === 'monitor' ? 'bg-card text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground/80'">
    进程监控
  </button>
  <button ...>C盘清理</button>
</div>
```

选中态：`bg-card text-foreground shadow-sm`（白色底，轻微阴影）
未选中：`text-muted-foreground hover:text-foreground/80`
过渡：`transition-all duration-200`
整体容器：`rounded-lg bg-muted/50 p-1`

### 2.4 内容区

- `flex-1 overflow-hidden p-4 pt-4`
- 无页面级滚动（内部组件各自处理滚动）
- 背景：使用 `bg-background`（父级已有），面板内部不再叠加额外卡片容器

---

## 3. Monitor Panel 规范

### 3.1 页面结构

```
MonitorPanel.vue
├── SummaryBar        — 紧凑摘要行，bg-muted/30 rounded-lg px-3 py-2
├── SearchInput       — 输入框 + Search 图标 + 清除按钮
└── ProcessTable      — 进程表格（overflow-y-auto）
```

### 3.2 SummaryBar

**布局：** `flex items-center gap-3 rounded-lg border border-border bg-muted/30 px-4 py-2.5 text-sm`

**内容：** 三段式，用 `|` 分隔（可见分隔符使用 `text-muted-foreground/40`）：

| 段 | 格式 | 着色规则 |
|---|---|---|
| CPU | `CPU: <value>%` | `< 50%` 默认色, `50-80%` warning, `> 80%` destructive |
| 内存 | `内存: <used>/<total> (<pct>%)` | `< 65%` 默认色, `65-85%` warning, `> 85%` destructive |
| 进程 | `进程: <count>` | 默认 |

**数字格式：** `font-semibold tabular-nums`

### 3.3 SearchInput

```
┌─────────────────────────────────────┐
│ 🔍  搜索进程...                  [×]│  ← h-9, rounded-lg, ring-primary focus
└─────────────────────────────────────┘
```

**状态：**
- 默认：Search 图标在左侧 `left-3`，placeholder "搜索进程..."
- 输入中：右侧显示清除按钮（`X`，`h-4 w-4 text-muted-foreground hover:text-foreground`），点击清空并恢复焦点
- Focus：`ring-1 ring-primary`
- 清除按钮仅在 `search.length > 0` 时显示，使用 `v-if` 条件渲染

### 3.4 ProcessTable

**组件结构：** 使用原生 `<table>` + `<thead>` + `<tbody>`（当前实现），不依赖 shadcn-vue Table（表格需要自定义进度条和着色，原生 table 控制力更强，且不增加维护成本）。

> ⚠️ **注意**：此决策与 TASK-008 早期 Acceptance #8（"shadcn-vue Table"）冲突。以本文档为准——TASK-008 已更新对齐。

**表头：**
- 背景 `bg-muted/30`
- 可排序列：名称、CPU%、内存、Mem%
- 排序列显示 `▲`/`▼` 箭头，使用 `text-primary`
- 默认按 CPU% 降序

**行：**
- 高度 `h-9` (`py-2`)
- 交替背景：`even:bg-muted/5`
- Hover: `hover:bg-muted/20`
- 过渡：`transition-colors duration-150`

**列规格：**

| 列 | 对齐 | 宽度 | 格式 |
|---|---|---|---|
| 名称 | left | `max-w-[160px] truncate` | `font-medium` |
| CPU% | right | auto | 进度条 + 百分比 |
| 内存 | right | auto | `tabular-nums`, GB/MB 自适应 |
| Mem% | right | auto | `text-muted-foreground tabular-nums` |
| 操作 | right | `w-8` | Kill 按钮 |

**CPU% 进度条：**
```html
<span class="inline-flex items-center gap-1.5">
  <span class="h-1.5 w-10 overflow-hidden rounded-full bg-muted-foreground/20">
    <span class="block h-full rounded-full transition-all" :style="{ width: Math.min(cpu, 100) + '%' }" :class="cpuBarColor(cpu)" />
  </span>
  <span :class="cpuColor(cpu)">{{ cpu.toFixed(1) }}%</span>
</span>
```

**Kill 按钮：** 默认隐藏 `opacity-0 group-hover:opacity-100`，hover 行时显示。hover 时 `bg-destructive/20 text-destructive`。

**排序逻辑（保持当前实现）：**
```typescript
const filtered = computed(() => {
  if (!search.value.trim()) return processes.filter(p => p.cpu > 10 || p.mem_mb > 200)
  const q = search.value.toLowerCase()
  return processes.filter(p => p.name.toLowerCase().includes(q))
})
```

**空状态（三场景区分）：**
| 场景 | 区分条件 | 文案 |
|---|---|---|
| 初始无数据 | `loading === false && processes.length === 0 && search === ''` | "暂无进程数据" |
| 搜索无结果 | `search !== '' && filtered.length === 0` | "没有匹配的进程" |
| 列表非空但过滤后空 | `search !== '' && processes.length > 0 && filtered.length === 0` | "没有匹配的进程" |

视觉：居中显示 Search 图标 + 对应文案（`text-muted-foreground py-12`）

**加载态：** 6 行骨架屏（`animate-pulse`），匹配表格行形状

### 3.5 Kill 确认对话框

使用 Teleport 到 body 的模态框（保持当前实现，不引入 shadcn-vue AlertDialog 以避免额外依赖）。

**进程已存在检查：** 对话框打开时，检查 `killTarget` 引用的进程是否仍在当前轮询列表中。如果进程已不存在，关闭对话框并显示"进程已结束"提示（`text-sm text-muted-foreground`，2s 自动消失）。

```
┌─ Kill Dialog ──────────────────────────┐
│  终止进程                               │
│                                         │
│  确定要终止 <name>（PID: <pid>）吗？    │
│                                         │
│            [取消]  [终止]               │
└─────────────────────────────────────────┘
```

- 背景：`fixed inset-0 bg-black/60 z-50 flex items-center justify-center`
- 容器：`rounded-lg border border-border bg-card p-6 shadow-lg max-w-md w-[90vw]`
- 标题：`text-base font-semibold`
- 内容：`text-sm text-muted-foreground`
- 按钮：取消 `variant="outline"` + 终止 `variant="destructive"`

---

## 4. Cleaner Panel 规范

### 4.1 状态机

```
idle → scanning → done (with items) → deleting → idle (with toast)
                 → done (empty)
                 → cancelled (from scanning)
                 → error (from scanning or deleting)
```

**状态说明：**
| 状态 | 含义 | 进入条件 | 退出条件 |
|---|---|---|---|
| `idle` | 初始态/完成态 | 页面加载、清理完成、重置 | 用户点击"开始扫描" |
| `scanning` | 扫描中 | 用户点击"开始扫描" | 扫描完成/取消/出错 |
| `done` | 扫描完成 | 后端 `scan-done` 事件 | 用户执行清理/重新扫描 |
| `cancelled` | 用户取消扫描 | 用户点击"取消" + 后端确认 | 用户点击"重新扫描" |
| `deleting` | 清理中 | 用户点击"清理选中" | 清理完成/出错 |
| `error` | 异常状态 | invoke/event 返回错误 | 用户点击"重试" |

**竞态条件保护：** 所有 event listener 回调中，检查 `state.value` 与事件预期状态一致再执行。例如 `scan-cancelled` 回调中：
```typescript
if (state.value !== 'scanning') return  // 忽略迟到事件
```

**Tab 切换保护：** Cleaner composable 的 `onMounted` 中增加状态恢复逻辑——检查是否有残留的后端扫描任务（通过 Tauri invoke 查询），如果有则恢复 scanning 状态并重新注册事件监听。

### 4.2 Idle 状态

**居中布局：**
- 图标容器：`rounded-2xl w-16 h-16 bg-primary/10 flex items-center justify-center` + `<Scan class="h-8 w-8 text-primary" />`
- 标题：`text-xl font-semibold` — "C盘安全清理"
- 描述：`max-w-[260px] text-center text-sm text-muted-foreground`
- 按钮：`Button size="lg"` + `<Scan class="mr-2 h-4 w-4" />` + "开始扫描"

### 4.3 Scanning 状态

**居中布局：**
- 图标：`Loader2 class="h-8 w-8 animate-spin text-primary"`
- 进度条：`<Progress />` (indeterminate 模式)
- 文件计数：`text-sm` "已扫描 <count> 个文件"
- 当前路径：`text-xs text-muted-foreground truncate max-w-[280px]`

### 4.4 Done (empty) 状态

- 图标：`rounded-full w-14 h-14 bg-green-500/10 flex items-center justify-center` + `<Check class="h-7 w-7 text-green-500" />`
- 标题：`text-lg font-medium` — "没有发现可清理文件"
- 副标题：`text-sm text-muted-foreground` — "你的 C 盘状况良好"
- 按钮：`Button variant="outline"` + 重新扫描

### 4.5 Done (with items) 状态

**布局：**
```
┌─────────────────────────────────────────┐
│ 可清理                    [重新扫描]     │  ← Header (border-b)
│ 2.4 GB                                   │
├─────────────────────────────────────────┤
│ 🟦 临时文件  1.2GB                      │  ← Legend (flex-wrap)
│ 🟪 缓存  892MB                          │
│ 🟩 Prefetch  245MB                      │
│ 🟨 回收站  128MB                        │
├─ ScrollArea ────────────────────────────┤
│ ▸ 临时文件                    [1.2GB]  │  ← Collapsible per category
│  └─ ☑ ...\temp.log       1.2 MB        │
│  └─ ☑ ...\cache          1.1 GB        │
│ ▸ 浏览器缓存                 [892MB]    │
│  └─ ...                                │
├─ Sticky Bottom ─────────────────────────┤
│ 已选 128/1,024 项 (1.5GB)  [清理选中]   │  ← Action bar (border-t)
└─────────────────────────────────────────┘
```

**Header：**
- `flex items-center justify-between border-b border-border pb-3`
- 左侧："可清理" (`text-sm text-muted-foreground`) + 总量 (`text-2xl font-bold tabular-nums`)
- 右侧：重新扫描 `Button variant="outline" size="sm"` + `<RotateCcw class="mr-1 h-4 w-4" />`

**Legend：**
- `flex flex-wrap gap-3 py-3`
- 每项：`rounded-full h-2.5 w-2.5` + 分类名 (`text-xs text-muted-foreground`) + 大小 (`text-xs font-medium`)

**Category 折叠（使用 shadcn-vue Collapsible）：**
- 背景：`rounded-lg border border-border`
- Trigger：`flex items-center gap-2 px-3 py-2.5 text-sm font-medium rounded-lg hover:bg-accent/50`
- Trigger 内：ChevronRight (旋转 90° 展开) + Checkbox + 彩色圆点 + 分类名 + 大小 Badge
- Content：`border-t border-border` + `divide-y divide-border`

**文件行：**
- `flex items-center gap-3 px-3 py-2 text-sm hover:bg-accent/30`
- Checkbox + 路径 (`flex-1 truncate text-muted-foreground`) + 大小 (`text-xs font-medium tabular-nums`)

**Bottom Action Bar：**
- `sticky bottom-0 flex items-center justify-between border-t border-border bg-background px-1 py-3`
- 左侧：选中计数 + 全选/取消全选
- 右侧：`Button variant="destructive" size="sm"` + `<Trash2 class="mr-1.5 h-4 w-4" />` + "清理选中"
- 选中 0 项时 disabled

### 4.6 Deleting 状态

与 Scanning 保持视觉一致性：
- Loader2 + Progress + "清理中..." (`text-sm text-muted-foreground`)

### 4.7 Delete Result Toast

- 位置：`absolute bottom-3 left-3 right-3 z-10`
- 容器：shadcn-vue `Alert` + `shadow-lg`
- 内容：Check/AlertCircle + "清理完成 N 项成功" + 若有失败 "N 项失败"
- 动画：`<Transition name="toast">` slide-up + fade（保持当前实现）
- 5 秒自动消失（保持当前实现）
- **部分删除失败展开：** 当 `deleteResult.failed > 0` 时，Toast 下方展开失败详情列表（显示最多 5 条错误路径 + "等 N 项"），点击可手动关闭。用户可据此决定是否重新扫描后单独清理失败项。

---

## 5. 动画规范

| 场景 | 实现 | 触发 |
|---|---|---|
| Tab 切换 | CSS transition `all 0.2s` | click |
| Dialog 打开/关闭 | CSS `scale + opacity` 过渡 | open/close |
| 行 hover | CSS `transition-colors duration-150` | hover |
| Toast 出现/消失 | Vue `<Transition>` slide-up | show/hide |
| 骨架屏 pulse | CSS `animate-pulse 1.5s infinite` | loading |
| Checkbox 选中 | CSS `transition-all` 自带 | check |
| Progress 条纹 | CSS `animate-progress` | scanning |
| 进度条宽度 | CSS `transition-all` | data update |

### 5.2 禁用场景

- 无 scroll 事件监听
- 无视差
- 无 GSAP（此 widget 无 scroll 场景）
- 无无限循环动画（loading spinner 除外）

### 5.3 无障碍

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## 6. 响应式与滚动

由于窗口固定 420×680，响应式只需处理窗口最小化到 380×500 时的内容适配：

| 窗口宽度 | 行为 |
|---|---|
| ≥ 420px | 正常布局 |
| 380-419px | 内边距从 `p-4` 降至 `p-3` |
| < 380px | minWidth 限制，不会出现 |

滚动仅发生在以下内部容器：
- Monitor：进程表格 `overflow-y-auto`（表格头 `sticky top-0`）
- Cleaner：分类列表 `overflow-y-auto`（使用 shadcn-vue ScrollArea 或原生 `overflow-y-auto`）

### 滚动条样式

```css
::-webkit-scrollbar { width: 4px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: hsl(220 8% 25%); border-radius: 999px; }
::-webkit-scrollbar-thumb:hover { background: hsl(220 8% 35%); }
```

---

## 7. 状态清单

### Monitor Panel

| 状态 | UI | 区分条件 |
|---|---|---|
| Loading | 6 行骨架屏 | `loading === true` |
| Empty (initial) | 居中 "暂无进程数据" | `!loading && processes.length === 0 && !search` |
| Empty (search) | 居中 "没有匹配的进程" | `search !== '' && filtered.length === 0` |
| Process disappeared (kill) | Toast "进程已结束" 2s | killTarget 不在当前进程列表中 |
| Error | Destructive Alert + 重试 | `error !== null` |
| Normal | 进程表格 | 默认 |
| Kill Success | 绿色 toast 2.5s | kill invoke 成功 |
| Kill Failed | 红色 toast 2.5s | kill invoke 失败 |

### Cleaner Panel

| 状态 | UI |
|---|---|
| Idle | 居中扫描入口 |
| Scanning | 进度条 + 文件计数 + 当前路径 |
| Cancelled | "扫描已取消" + 重新扫描按钮 |
| Done (empty) | 绿色勾号 + "没有发现可清理文件" |
| Done (with items) | 总量 + 分类列表 + 操作栏 |
| Deleting | 进度 + "清理中..." |
| Error (scan) | Destructive Alert + "扫描失败" + 重试 |
| Error (delete) | Destructive Alert + 失败详情 + 重试 |
| Delete Result Toast | 成功计数 + 失败列表展开 |

---

## 8. 实现约束

1. **依赖锁定：** 不新增 npm 依赖。图标只用 `lucide-vue-next`（已有），TitleBar pony icon 是唯一的 SVG 例外。motion-v 已安装，仅用于 Dialog 和 Toast 动画——如果项目方确认无 motion-v 使用场景，应从 `package.json` 移除。
2. **兼容性：** 后端 `pony_core` crate 零改动。所有 Tauri invoke 签名与 TASK-006 保持一致。
3. **i18n：** 全部中文 UI，不引入 i18n 框架。
4. **性能：** 2s 轮询使用 `shallowRef`；长列表虚拟化不做初版，单分类 > 500 项时默认折叠。
5. **Theme：** 固定深色，`<html class="dark">`，无 light/dark 切换。
6. **Tab 切换保护：** composable 的 `onMounted`/`onUnmounted` 必须正确处理事件监听生命周期。Cleaner composable 在 `onMounted` 中恢复可能的残留扫描状态（通过 Tauri invoke 查询后端状态）。
7. **竞态条件保护：** 所有 event listener 回调加入前置状态检查，防止迟到事件覆盖新状态。`onUnmounted` 中正确注销所有事件监听。

---

## 9. 与现有实现的对比（迁移检查清单）

| 特性 | 当前实现 | SPEC-011 目标 | 变更 |
|---|---|---|---|
| TitleBar | `h-10` + drag + close | 同上，+ SVG pony icon | 小改 |
| Tab | 下划线式 border-b | Pill Tabs `bg-muted/50` | 重写 |
| SummaryBar | `bg-card border` | `bg-muted/30 border` | 微调 |
| Search | emoji placeholder | 无 emoji，纯图标 | 微调 |
| ProcessTable | `<table>` HTML | 同上，+ 交替行背景 + 分组 hover 按钮 | 小改 |
| Kill Dialog | Teleport + `<div>` | 同上（保持） | 不变 |
| Cleaner Idle | 居中 Scan | 同上（保持） | 不变 |
| Cleaner Scanning | Progress + 计数 | 同上（保持） | 不变 |
| Cleaner Done | Collapsible 列表 | 同上（保持） | 不变 |
| Toast | Transition slide-up | 同上（保持） | 不变 |
| Pony 品牌 | 仅 "PonyClean" 文字 | + SVG 小马图标（TitleBar） | 新增 |
