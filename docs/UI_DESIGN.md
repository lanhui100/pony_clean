# PonyClean UI 设计稿

> **Design Read:** Windows 桌面 widget 界面重设计，目标用户为 Windows 高级用户/开发者，深色玻璃质感 + 轻量工具美学，偏向 Fluent Design 透明语言 + 现代极简工具风格。
>
> **Dial Values:** `VARIANCE: 4 | MOTION: 6 | DENSITY: 5`

---

## 1. 设计哲学

**"飞驰小马" — 轻快、友好、透明**

- **轻快** — 420×680 的悬浮 widget，信息密集但不压抑，数据刷新如呼吸般自然
- **友好** — 系统监控工具往往令人紧张（大红色、警告框），PonyClean 用柔和的蓝绿色系传递"一切尽在掌握"的安全感
- **透明** — 物理上的窗口透明 + 视觉上的层级透明，让 widget "浮"在桌面上，而非挡住桌面

---

## 2. 设计系统

### 2.1 色彩

保留现有 shadcn-vue 深色主题的色板，进行细微调优：

#### 主色板（当前已有的，保持不变）

| Token | HSL | 用途 |
|---|---|---|
| `--background` | `0 0% 7%` | 窗口背景 |
| `--foreground` | `210 10% 92%` | 主文字 |
| `--card` | `220 10% 12%` | 卡片面板 |
| `--primary` | `214 90% 76%` | 交互色 (#8AB4F8) |
| `--muted` | `220 8% 20%` | 弱背景 |
| `--muted-foreground` | `215 8% 60%` | 辅助文字 |
| `--border` | `220 8% 20%` | 边框 |
| `--destructive` | `0 80% 73%` | 危险操作 |

#### 新增状态色

| Token | HSL | 用途 |
|---|---|---|
| `--success` | `142 70% 55%` | 正常/成功状态 |
| `--warning` | `38 90% 60%` | 警告状态 |
| `--info` | `199 85% 55%` | 信息提示 |
| `--accent-warm` | `25 85% 65%` | 小马暖色点缀 |

#### 色彩使用规则

- **Primary (blue)** 用于所有可交互元素：按钮、链接、焦点环、选中态
- **Success (green)** 用于正常指标、完成状态
- **Warning (amber)** 用于 CPU > 50%、内存 > 65%
- **Destructive (red)** 用于 CPU > 80%、内存 > 85%、终止操作
- **Accent-warm (coral)** 仅用于品牌点缀：标题栏小马标记、加载动画、特殊高亮——全页面不超过 3 处
- 所有饱和度 ≤ 80%，保持深色模式下的视觉舒适度

### 2.2 字体

```css
/* 标题/正文 */
font-family: 'Segoe UI Variable', 'Segoe UI', system-ui, -apple-system, sans-serif;

/* 数据/代码 */
font-family: 'Cascadia Code', 'JetBrains Mono', 'Consolas', monospace;
```

#### 字号层级

| 层级 | 大小 | 字重 | 用途 |
|---|---|---|---|
| Display | text-lg (18px) | font-semibold | 面板标题 |
| Body | text-sm (14px) | font-normal | 正文/表格 |
| Small | text-xs (12px) | font-normal | 辅助信息 |
| Data | text-sm (14px) | tabular-nums | 数字/指标 |
| Mono | text-xs (12px) | font-normal | 文件路径/进程名 |

- 整个 widget 只有 680px 高，不需要 h1/h2/h3 层级。用字重+颜色区分层级更有效
- 数字一律 `tabular-nums` 等宽，避免跳变

### 2.3 间距

| 层级 | 值 | 用途 |
|---|---|---|
| Section | `gap-4` (16px) | 面板间间距 |
| Block | `gap-3` (12px) | 组件内块间距 |
| Element | `gap-2` (8px) | 内联元素间距 |
| Compact | `gap-1` (4px) | 表格/列表紧凑间距 |
| Padding | `p-4` (16px) | 面板内边距 |
| Padding-sm | `p-3` (12px) | 紧凑卡片内边距 |

### 2.4 圆角

统一圆角系统，严格执行配对规则：

| 层级 | 值 | 用途 |
|---|---|---|
| Panel | `rounded-xl` (12px) | 卡片容器 |
| Element | `rounded-lg` (8px) | 输入框、按钮 |
| Indicator | `rounded-full` | 状态点、图标容器 |
| Sharp | `rounded-none` | 表格单元格（只有表格容器有圆角） |

### 2.5 阴影

所有阴影使用透明白色而非纯黑，保持深色模式质感：

```css
shadow: 0 1px 2px rgb(255 255 255 / 0.04),
        0 4px 12px rgb(0 0 0 / 0.2),
        0 0 0 1px rgb(255 255 255 / 0.04);
```

### 2.6 玻璃效果

由于 Windows WebView2 在透明窗口上不支持 `backdrop-filter: blur()`，采用分层半透明方案：

```css
/* 窗口背景层 */
.window-bg {
  background: hsl(0 0% 7% / 0.92);
}

/* 卡片面板 - 略浅，模拟玻璃分层 */
.card-panel {
  background: hsl(220 10% 12% / 0.85);
  border: 1px solid hsl(220 8% 20% / 0.8);
}

/* 悬浮态 - 更亮，模拟玻璃反光 */
.card-panel:hover {
  background: hsl(220 10% 14% / 0.9);
}

/* 标题栏 - 半透明背景遮罩 */
.titlebar {
  background: hsl(0 0% 7% / 0.75);
}
```

未来 WebView2 支持 `backdrop-filter` 后，在 1px 边框上增加 `backdrop-blur-xl`。

---

## 3. 窗口框架

### 3.1 整体结构

```
┌──────────────────────────────────┐
│ [🪽 PonyClean]              [×] │  ← TitleBar (h-10)
├──────────────────────────────────┤
│ [进程监控] [C盘清理]              │  ← Tabs (h-9)
├──────────────────────────────────┤
│                                  │
│         [Main Content]           │  ← Content Area (flex-1, p-4)
│                                  │
│                                  │
└──────────────────────────────────┘
```

### 3.2 TitleBar

当前设计：
- 左侧：`text-sm font-semibold text-primary` 显示 "PonyClean"
- 右侧：关闭按钮（自定义 SVG X）

改进设计：
- 左侧：SVG pony icon（独立组件 `components/PonyIcon.vue`，16×16px，暖色调）+ `PonyClean` 文字（`text-sm font-semibold text-primary`）
- PonyIcon 是**唯一的手写 SVG 例外**（其他图标统一用 lucide）
- 右侧：关闭按钮（hover 时红色背景）
- 未来如需增加窗口控件应拆分子组件，当前粒度合理

### 3.3 Tab 导航

当前设计：
- 带 `border-b` 下划线的文字按钮
- 选中态：底部 2px blue primary 指示条

改进设计（Pill Tabs）：
- 圆角 pill 按钮组，背景 `bg-muted/50`
- 选中态：`bg-card` + `text-foreground` + 轻微阴影
- 未选中：`text-muted-foreground`
- 动画：选中态切换时 `transition-all duration-200`

```html
<div class="inline-flex items-center gap-1 rounded-lg bg-muted/50 p-1">
  <button class="rounded-md px-3 py-1.5 text-sm font-medium transition-all duration-200"
          :class="active === 'monitor' ? 'bg-card text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'">
    进程监控
  </button>
  <button ...>C盘清理</button>
</div>
```

### 3.4 内容区域

- 统一 `p-4` 内边距
- 主内容容器：`rounded-xl border border-border bg-card/85`（如果有顶层卡片容器）
- 或直接使用背景层（Monitor 页面需要全宽表格时）

---

## 4. Monitor Panel 设计

### 4.1 布局结构

```
┌──────────────────────────────────┐
│ CPU: 32.1% | 内存: 6.2/16GB | P  │  ← SummaryBar (compact)
├──────────────────────────────────┤
│ 🔍 搜索进程...                   │  ← SearchInput
├──────────────────────────────────┤
│ 名称        CPU%  内存   Mem%  ⓧ │  ← TableHeader
│ svchost     12.3%  48MB  0.3%  ⓧ │
│ chrome      8.7%   1.2GB 7.5%  ⓧ │  ← ProcessTable
│ code        6.2%   620MB 3.9%  ⓧ │
│ ...                              │
└──────────────────────────────────┘
```

### 4.2 SummaryBar

当前设计（简化为 `border border-border bg-card` 一行）：

```
CPU: 32.1%  |  内存: 6.2/16.0GB (38.8%)  |  进程: 142
```

改进设计：
- 背景改为 `bg-muted/30` 而非 `bg-card`，与表格区隔
- 颜色阈值更精细：
  - CPU: `< 50%` 默认色, `50-80%` amber, `> 80%` red
  - 内存: `< 65%` 默认色, `65-85%` amber, `> 85%` red
- 分隔符使用 `text-muted-foreground/40` 弱化视觉噪音
- 数字加粗 `font-semibold tabular-nums`
- 动画：数字更新时轻微 `opacity` 过渡（可选，Motion 实现）

### 4.3 SearchInput

当前设计：带搜索图标的 `<input>`，placeholder 含 emoji 🔍

改进：
- 移除 placeholder 中的 emoji 🔍（per skill policy），使用 `Search` 图标
- 搜索图标在输入框左侧 `absolute left-3`
- 输入框 `h-9`，聚焦时 `ring-1 ring-primary`
- 有输入时右侧显示清除按钮（`X` 图标）

### 4.4 ProcessTable

当前设计：纯 `<table>` + `<thead>` + `<tbody>` 实现

改进设计要点：

**表格头：**
- `bg-muted/30` 背景区分表头
- 表头可点击排序，当前排序列显示箭头 `▲` / `▼`
- 排序箭头使用 Primary 蓝色

**表格行：**
- 交替行背景 `even:bg-muted/10` 优化长列表可读性
- hover 行 `hover:bg-muted/20`
- 行高 `h-9` 紧凑尺寸

**CPU% 列：**
- 微型进度条 + 百分比数值并列
- 进度条宽度 40px，高度 6px，`rounded-full`
- 颜色三档：green (< 50%), amber (50-80%), red (> 80%)

**内存列：**
- 数值 + 百分比两列紧挨，百分比使用 `text-muted-foreground` 弱化
- 颜色三档：teal (< 65%), amber (65-85%), red (> 85%)

**Kill 按钮列：**
- 宽 32px，hover 时红色背景 + 白色 X 图标
- 仅 hover 到该行才显示 kill 按钮（默认隐藏，减少视觉噪音）

**空状态：**
```
     ┌──────────────────────┐
     │   🔍 (Search icon)   │
     │  没有匹配的进程       │
     │  text-muted-foreground│
     └──────────────────────┘
```

**加载骨架屏：**
- 6 行脉冲动画骨架，匹配表格行形状
- `h-8` 行高，`rounded-md bg-muted` 占位

### 4.5 Kill 确认对话框

当前设计：Teleport 到 body 的固定定位模态框

改进设计：
- 使用 shadcn-vue `AlertDialog`（如果已实现）或保持当前 Teleport 方案
- 对话框结构：
  - 标题："终止进程"
  - 内容：`确定要终止 [processName]（PID: [pid]）吗？`
  - 底部按钮：取消（ghost）+ 终止（destructive）
- 动画：弹出时 `scale 0.95 → 1.0` + `opacity 0 → 1`，使用 Motion

---

## 5. Cleaner Panel 设计

### 5.1 状态机

```
idle ──[开始扫描]──→ scanning ──[扫描完成]──→ done (with items)
                      │                       └── done (empty)
                      ├──[取消]──→ cancelled
                      └──[错误]──→ error

done (with items) ──[清理选中]──→ deleting ──[完成]──→ idle
                                              └── idle (显示 toast)
```

每个状态切换应有平滑过渡动画（Motion 实现）。

### 5.2 Idle 状态

```
         ┌──────────────────────┐
         │    [Scan icon]       │  ← rounded-2xl bg-primary/10, 64px
         │                      │
         │   C盘安全清理         │  ← text-lg font-semibold
         │                      │
         │  扫描C盘临时文件、    │  ← text-sm text-muted-foreground, max-w-[250px]
         │  浏览器缓存、Prefetch │
         │  和回收站，安全释放   │
         │  磁盘空间            │
         │                      │
         │  [📋 开始扫描]       │  ← Button lg, primary
         └──────────────────────┘
```

改进要点：
- 居中布局，`flex flex-col items-center justify-center`
- 扫描图标容器：`rounded-2xl w-16 h-16 bg-primary/10 flex items-center justify-center`
- 标题：`text-lg font-semibold`
- 描述：`max-w-[260px] text-center text-sm text-muted-foreground`
- 按钮：`Button size="lg"` 带 Scan 图标
- 轻微动画：进入时图标 `scale 0.9 → 1.0` + 文字 `fade-in`

### 5.3 Scanning 状态

```
     ┌──────────────────────────┐
     │    [Spinning loader]     │  ← Loader2 animate-spin, h-8 w-8
     │                          │
     │  ════════════════════    │  ← Progress indeterminate
     │                          │
     │  已扫描 1,247 个文件     │  ← text-sm
     │  C:\Users\...\cache.tmp  │  ← text-xs truncate, muted
     │                          │
     │  [取消]                  │  ← Button ghost sm
     └──────────────────────────┘
```

改进要点：
- 居中布局
- 进度条：`Progress` 组件 indeterminate 模式（动画条纹）
- 已扫描数：`font-semibold tabular-nums` 实时更新
- 当前文件路径：`text-xs truncate max-w-[280px]`，溢出省略
- 取消按钮：`Button variant="ghost" size="sm"` 带 X 图标
- 扫描中和当前文件路径的更新：使用数字动画过渡

### 5.4 Done (empty) 状态

```
         ┌──────────────────────┐
         │    [✓ 图标]          │  ← rounded-full bg-green-500/10, 56px
         │                      │
         │  没有发现可清理文件   │  ← text-base font-medium
         │                      │
         │  你的C盘状况良好      │  ← text-sm text-muted-foreground
         │                      │
         │  [重新扫描]          │  ← Button outline sm
         └──────────────────────┘
```

- 绿色勾号动画：scale-in + checkmark 路径绘制
- 比当前更紧凑的布局

### 5.5 Done (with items) 状态

```
┌──────────────────────────────────┐
│ 可清理                            │  ← text-sm text-muted-foreground
│ 2.4 GB              [重新扫描]    │  ← text-2xl font-bold + Button outline
├──────────────────────────────────┤
│ ● 临时文件  1.2GB                │  ← Category legend
│ ● 缓存  892MB                    │
│ ● Prefetch  245MB                │
│ ● 回收站  128MB                  │
├──────────────────────────────────┤
│ ▸ 临时文件 [1.2GB] [▼]          │  ← Collapsible category
│  └─ checkbox C:\...\temp.log 48KB│
│  └─ checkbox C:\...\cache   1.1GB│
│ ▸ 浏览器缓存 [892MB] [▼]        │
│  └─ ...                          │
├──────────────────────────────────┤
│ 已选 128/1,024 项 (1.5GB)        │
│ [清理选中]                       │  ← Bottom action bar
└──────────────────────────────────┘
```

改进要点：

**Header / Summary：**
- 左侧："可清理" 标签 + 大号数字 `text-2xl font-bold tabular-nums`
- 右侧：重新扫描按钮 (Button outline sm)

**Category Legend：**
- 改为一排彩色圆点 + 分类名 + 大小的紧凑布局
- 使用 `flex-wrap` 自动换行
- 圆点 `h-2.5 w-2.5 rounded-full`
- 颜色映射同现有方案

**Category List (Collapsible)：**
- 每个分类使用 shadcn-vue `Collapsible` 组件
- 分类标题行：ChevronRight 图标（旋转动画）+ Checkbox + 彩色圆点 + 分类名 + 大小 Badge
- 展开时内容：文件列表，每行 checkbox + 路径（truncate）+ 大小（tabular-nums）
- 选中状态：全选/部分选/未选 三态

**Bottom Action Bar：**
- 固定在底部，`sticky bottom-0`
- 左侧：已选数量 + 大小 + 全选/取消全选链接
- 右侧：`Button variant="destructive"` 清理选中
- 选中为 0 时禁用

**ScrollArea：**
- 可滚动区域，自定义细滚动条
- 滚动条样式：`w-1.5` + `rounded-full bg-muted`

### 5.6 Deleting 状态

```
     ┌──────────────────────────┐
     │    [Spinning loader]     │
     │                          │
     │  ████████████░░░░░░░     │  ← Progress determinate (可选)
     │                          │
     │  清理中...               │  ← text-sm text-muted-foreground
     └──────────────────────────┘
```

- 保持简单，与 scanning 一致的设计语言
- 可选：显示 "已清理 X/N 项" 进度

### 5.7 Delete Result Toast

当前设计：底部绝对定位 `<Transition name="toast">` 包裹 Alert

改进设计：
- 固定在底部，`bottom-3 left-3 right-3`，`z-10`
- 使用 Motion 的 `slide-up + fade-in` 交替
- Alert 内部分两行：
  - 第一行：状态图标 + "清理完成"
  - 第二行：`N 项成功` (green) + 若有失败 "N 项失败" (destructive)
- 5 秒自动消失，点击可提前关闭

---

## 6. Motion & Animation 设计

### 6.1 动画场景

| 场景 | 动画 | 实现方式 |
|---|---|---|
| Tab 切换 | fade + slide (水平 8px) | Vue `<Transition>` |
| Dialog 打开 | scale 0.92→1.0 + opacity 0→1 | Motion `presence` |
| Kill 结果反馈 | slide-up + fade-in | Motion |
| Clean 结果 Toast | slide-up + fade-in | Motion |
| Update 数字变化 | opacity 0.6→1.0 过渡 | CSS transition |
| Hover 行 | bg-color 0.15s | CSS transition |
| Loading 骨架屏 | pulse 1.5s infinite | CSS animation |
| Checkbox 选中 | scale 0.8→1.0 | CSS transition |

### 6.2 禁止的动画

- ❌ `scroll` 事件监听（widget 不滚动）
- ❌ 视差效果（不需要）
- ❌ 无限循环动画（除 loading spinner 外）
- ❌ 复杂的 GSAP orchestrations（不需要）

### 6.3 无障碍

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

所有动画必须被 `prefers-reduced-motion` 禁用。

---

## 7. 排版 & 栅格系统

### 7.1 页面布局

420×680 的固定尺寸窗口，上下结构：

```
TitleBar:     h-10 (40px)    — 固定
TabNav:       h-9 (36px)     — 固定
Content:      flex-1          — 自适应剩余高度 (约 580px)
Bottom margin: p-4            — 内边距
```

Content 区域内部不再有页面级滚动——表格和文件列表内部使用 `overflow-y-auto`。

### 7.2 Monitor Panel 内部

```
SummaryBar:   h-10 (40px)
SearchInput:  h-9 (36px)
TableHeader:  h-8 (32px)     — sticky top-0
TableRows:    flex-1          — overflow-y-auto
```

可用行数 ≈ `(680 - 40 - 36 - 32 - 16 - 16) / 36 ≈ 14` 行进程数据

### 7.3 Cleaner Panel 内部

```
Idle:         flex-1 居中
Scanning:     flex-1 居中
Done:
  Header:     h-12 (48px)
  Legend:     h-6 (24px)
  CategoryList: flex-1 overflow-y-auto
  ActionBar:  h-12 (48px)
```

---

## 8. 组件级设计细节

### 8.1 Button 变体

| Variant | 样式 | 用途 |
|---|---|---|
| `primary` | `bg-primary text-primary-foreground` | 主要操作 |
| `outline` | `border border-border bg-transparent` | 次要操作 |
| `ghost` | `text-muted-foreground hover:bg-muted` | 最弱操作 |
| `destructive` | `bg-destructive text-destructive-foreground` | 危险操作 |
| `link` | `text-primary underline-offset-4` | 文字链接 |

尺寸：`default (h-9)`, `sm (h-8)`, `lg (h-10)`, `icon (h-8 w-8)`

### 8.2 Progress 变体

| Variant | 样式 | 用途 |
|---|---|---|
| `determinate` | 填充百分比 | 已知进度的操作 |
| `indeterminate` | 条纹动画 | 未知时间的扫描 |
| `compact` | `h-1.5 rounded-full` | 表格内 CPU 进度条 |

### 8.3 Badge 变体

| Variant | 样式 | 用途 |
|---|---|---|
| `secondary` | `bg-secondary text-secondary-foreground` | 分类标签 |
| `outline` | `border text-muted-foreground` | 弱化标签 |

### 8.4 滚动条样式

```css
/* WebView2 自定义滚动条 */
::-webkit-scrollbar {
  width: 4px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-thumb {
  background: hsl(220 8% 25%);
  border-radius: 999px;
}
::-webkit-scrollbar-thumb:hover {
  background: hsl(220 8% 35%);
}
```

---

## 9. 图标体系

使用项目已依赖的 `lucide-vue-next`，禁止手写 SVG 图标。

### 主要图标映射

| 用途 | Lucide 图标 |
|---|---|
| 扫描 | `Scan` |
| 清理 | `Trash2` |
| 搜索 | `Search` |
| 关闭 | `X` |
| 重试 | `RotateCcw` |
| 成功 | `Check` |
| 错误 | `AlertCircle` |
| 展开 | `ChevronRight` |
| 加载 | `Loader2` |
| 进程 | `Activity` |
| 磁盘 | `HardDrive` |
| 内存 | `MemoryStick` |
| 排序 | `ArrowUpDown` |

所有图标使用统一尺寸：`h-4 w-4`（按钮内），`h-5 w-5`（状态图标）。

---

## 10. 状态清单

### 10.1 Monitor Panel

| 状态 | 触发条件 | UI 表现 |
|---|---|---|
| Loading | 首次挂载/刷新 | 6 行骨架屏动画 |
| Empty (初始无数据) | `!loading && processes.length === 0 && !search` | 居中 "暂无进程数据" |
| Empty (搜索无结果) | `search !== '' && filtered.length === 0` | 居中 "没有匹配的进程" |
| Process disappeared | killTarget 不在当前进程列表中 | Toast "进程已结束" 2s |
| Error | invoke 返回 Err | Destructive Alert + 重试按钮 |
| Normal | 数据正常 | 进程表格正常显示 |
| Kill Success | kill 返回成功 | 绿色提示条，2.5s 消失 |
| Kill Failed | kill 返回失败 | 红色提示条，2.5s 消失 |

### 10.2 Cleaner Panel

| 状态 | 触发条件 | UI 表现 |
|---|---|---|
| Idle | 初始/重置 | 居中扫描入口 |
| Scanning | startScan 调用 | 进度条 + 文件计数 |
| Scanning - cancelled | 用户取消 | 确认取消 + 返回 idle |
| Done (empty) | 扫描完成 0 文件 | 绿色勾号 + 良好提示 |
| Done (with items) | 扫描完成有文件 | 分类列表 + 操作栏 |
| Deleting | 执行清理 | 进度指示 |
| Deleting - done | 清理完成 | Toast 通知 + 返回 idle |
| Error | 扫描/清理异常 | Destructive Alert + 重试 |

---

## 11. 暗色模式

项目固定为深色模式（Windows 悬浮 widget 的特性），不需要 light/dark 切换。`<html class="dark">` 锁定。

但所有颜色仍通过 CSS 变量定义，方便未来如果需要支持 light 模式。

---

## 12. 性能注意事项

- **表格虚拟化**：如果进程数 > 200，考虑使用简单虚拟滚动（只渲染视口内行）
- **分类列表**：单分类 > 500 项时默认折叠，展开时仅渲染前 50 项
- **invoke 频率**：进程监控 2s 轮询，使用 `shallowRef` 避免 Vue 深度响应式开销
- **ScrollArea**：使用 `overflow-y-auto` 而非完整虚拟滚动库，保持轻量
- **CSS 动画优先**：能用 CSS transition 的不用 Motion，减少 JS 开销
- **竞态条件保护**：所有 event listener 回调加入前置状态检查（如 `if (state.value !== 'scanning') return`），防止迟到事件覆盖新状态
- **Tab 切换保护**：Cleaner composable 的 `onMounted` 中恢复残留扫描状态（通过 invoke 查询后端），`onUnmounted` 正确注销事件监听

---

## 13. 设计检查清单

- [x] One accent color (blue), used consistently
- [x] Dark mode only (widget invariant)
- [x] No em-dashes in any copy
- [x] No Inter font (using Segoe UI Variable)
- [x] No AI-purple / gradient slop
- [x] Icons from lucide-vue-next (existing dependency)
- [x] Color consistency lock
- [x] Shape consistency lock (rounded-xl panels, rounded-lg elements)
- [x] Proper contrast for all interactive states
- [x] Reduced motion support
- [ ] Real images check (N/A — pure UI widget)
