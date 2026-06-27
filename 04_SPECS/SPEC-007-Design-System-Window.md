# SPEC-007: Vue 设计系统 + 窗口布局

## 1. 目标

实现 Tauri 窗口外壳 + shadcn-vue 主题配置，确保 UI 基础层与现有深色玻璃设计语言一致。

## 2. 窗口配置 (tauri.conf.json)

```json
{
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "PonyClean",
        "width": 420,
        "height": 680,
        "minWidth": 380,
        "minHeight": 500,
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

## 3. 无边框窗口拖拽

使用 Tauri v2 的 `data-tauri-drag-region` 属性:
```html
<div data-tauri-drag-region class="flex items-center h-9 px-3 select-none">
  <span class="text-sm font-semibold text-foreground">PonyClean</span>
  <div class="ml-auto">
    <button @click="closeWindow" class="text-muted-foreground hover:text-foreground px-2">×</button>
  </div>
</div>
```

注意: Tauri v2 自动从拖拽区域中排除 `button`、`input` 等交互式元素，关闭按钮可安全放置在拖拽 div 内。

关闭函数:
```typescript
import { getCurrentWindow } from '@tauri-apps/api/window';
function closeWindow() { getCurrentWindow().close(); }
```

## 4. 透明窗口 + 玻璃效果

⚠️ **Windows WebView2 不支持 `backdrop-filter: blur()` 在透明窗口上生效**。使用以下方案替代：

```css
/* 卡片背景：半透明深色，无模糊（回退方案） */
.card-panel {
  @apply bg-card/90 rounded-xl border border-border;
}

/* 如果后续 WebView2 支持 backdrop-filter 可启用 */
@supports (backdrop-filter: blur(20px)) {
  .card-panel {
    @apply backdrop-blur-xl;
  }
}
```

## 5. 设计 Token (shadcn-vue CSS Variables)

```css
@import "tailwindcss";

/* Tailwind v4 设计 Token */
@theme {
  --color-background: hsl(0 0% 7%);
  --color-foreground: hsl(210 10% 92%);
  --color-card: hsl(220 10% 12%);
  --color-card-foreground: hsl(210 10% 92%);
  --color-primary: hsl(214 90% 76%);
  --color-primary-foreground: hsl(0 0% 7%);
  --color-muted: hsl(220 8% 20%);
  --color-muted-foreground: hsl(215 8% 60%);
  --color-border: hsl(220 8% 20%);
  --color-destructive: hsl(0 80% 73%);
  --radius: 0.75rem;
}

/* shadcn-vue CSS Variables 兼容层 */
@layer base {
  :root {
    --background: 0 0% 7%;
    --foreground: 210 10% 92%;
    --card: 220 10% 12%;
    --card-foreground: 210 10% 92%;
    --primary: 214 90% 76%;
    --primary-foreground: 0 0% 7%;
    --secondary: 220 8% 25%;
    --secondary-foreground: 210 10% 92%;
    --muted: 220 8% 20%;
    --muted-foreground: 215 8% 60%;
    --accent: 214 90% 76%;
    --accent-foreground: 0 0% 7%;
    --destructive: 0 80% 73%;
    --destructive-foreground: 0 0% 7%;
    --border: 220 8% 20%;
    --input: 220 8% 20%;
    --ring: 214 90% 76%;
    --radius: 0.75rem;
  }
}
```

## 6. 字体栈

```css
font-family: 'Segoe UI Variable', 'Segoe UI', system-ui, -apple-system, sans-serif;
```

Windows 优先: `Segoe UI Variable` 是 Win11+ 可变字体，天然支持 CJK。WebView2 通过 DirectWrite 渲染，CJK 质量远优于 egui 的 ab_glyph。

## 7. 窗口布局

### App.vue
```vue
<template>
  <div class="h-screen flex flex-col">
    <TitleBar />
    <Tabs v-model="activeTab" class="flex-1 flex flex-col">
      <TabsList class="px-3 justify-start border-b border-border">
        <TabsTrigger value="monitor">进程监控</TabsTrigger>
        <TabsTrigger value="cleaner">C盘清理</TabsTrigger>
      </TabsList>
      <div class="flex-1 p-3">
        <TabsContent value="monitor" class="h-full">
          <div class="h-full rounded-xl bg-card/90 border border-border p-3">
            <MonitorPanel />
          </div>
        </TabsContent>
        <TabsContent value="cleaner" class="h-full">
          <div class="h-full rounded-xl bg-card/90 border border-border p-3">
            <CleanerPanel />
          </div>
        </TabsContent>
      </div>
    </Tabs>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
// shadcn-vue Tabs 组件
</script>
```

## 8. motion-vue 集成 (main.ts)

```typescript
import { createApp } from 'vue'
import { MotionPlugin } from 'motion-vue'
import App from './App.vue'
import './styles/globals.css'

const app = createApp(App)
app.use(MotionPlugin)
app.mount('#app')
```

使用场景：
- Tab 切换: `<TabsContent>` 的 Vue `<Transition name="fade">` 即可，无需 motion-vue
- Toast 出现/消失: motion-vue `v-motion-slide-in-right`
- 其他面板暂不使用 motion-vue，避免引入复杂性

```vue
<!-- Toast 动画示例 -->
<div v-motion-slide-in-right v-if="toastVisible">
  <Alert>删除完成</Alert>
</div>
```

## 9. 主题强制深色

```html
<!-- index.html -->
<html class="dark">
```
