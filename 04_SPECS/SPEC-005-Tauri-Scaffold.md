# SPEC-005: Tauri v2 + Vue 3 + shadcn-vue 脚手架

## 1. 目标

在当前 `pony_clean` 仓库中搭建 Tauri v2 项目骨架，新旧结构并存不影响构建。

## ⚠️ 关键技术决策

### Tailwind v4
Tailwind v4 使用 `@tailwindcss/vite` 插件，无配置文件（no `tailwind.config.ts`、no PostCSS），配置通过 CSS `@theme` 指令完成。

```bash
npm install tailwindcss @tailwindcss/vite
```

**shadcn-vue 兼容性**: shadcn-vue 需要 v4 兼容版本。安装时使用 `npx shadcn-vue@next init`（canary 通道）或确认当前 `latest` 已支持 v4。如果 shadcn-vue init 生成 v3 风格配置，手动删除 `tailwind.config.ts` 和 `postcss.config.js`，改用 CSS `@theme` 配置。

### pony_core 必须依赖 serde
用于跨 FFI 序列化（SPEC-006 要求），以下依赖段已包含。

## 2. Cargo Workspace 重构

### 2.1 当前 src/ 目录结构
```
src/
├── lib.rs       → pub mod error; pub mod monitor; pub mod cleaner;
├── main.rs      → eframe 入口（已删除）
├── app.rs       → egui App ~1086 行 GUI 代码（已删除）
├── monitor.rs   → 进程监控（275 行）
├── cleaner.rs   → C盘清理（783 行）
├── error.rs     → 错误类型（15 行）
└── theme.rs     → 设计系统（62 行）
```

### 2.2 目标结构
```
Cargo.toml              → workspace（纯 workpace，无 [package]）
crates/pony_core/
├── Cargo.toml
└── src/
    ├── lib.rs          → pub mod error; pub mod monitor; pub mod cleaner;
    ├── monitor.rs      ← 从 src/ 移入，不变
    ├── cleaner.rs      ← 从 src/ 移入，不变
    └── error.rs        ← 从 src/ 移入，不变
src/                    ← 保留仅作参考，不参与 workspace 构建
src-tauri/
├── Cargo.toml          → 依赖 pony_core + tauri
├── tauri.conf.json
├── capabilities/default.json
├── build.rs
├── src/main.rs         → Tauri 入口
└── icons/
```

### 2.3 Cargo.toml (workspace root)
```toml
[workspace]
resolver = "2"
members = [
    "crates/pony_core",
    "src-tauri",
]
```

### 2.4 pony_core/Cargo.toml
```toml
[package]
name = "pony_core"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { version = "1", features = ["rt", "macros", "sync", "time"] }
sysinfo = "0.30"
jwalk = "0.8"
tracing = "0.1"
tokio-util = "0.7"
thiserror = "2"
serde = { version = "1", features = ["derive"] }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.54", features = [
    "Win32_Foundation",
    "Win32_Storage_FileSystem",
    "Win32_UI_Shell",
    "Win32_System_Threading",
    "Win32_System_Com",
] }
```

### 2.5 src-tauri/Cargo.toml
```toml
[package]
name = "pony_clean"
version = "0.1.0"
edition = "2024"

[dependencies]
pony_core = { path = "../crates/pony_core" }
tauri = { version = "2.2", features = ["tray-icon"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["sync", "time"] }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

## 3. Tauri v2 项目初始化

### 3.1 创建方式
手动创建 `src-tauri/` 目录，不通过 CLI。

### 3.2 tauri.conf.json 关键配置
```json
{
  "productName": "PonyClean",
  "version": "0.1.0",
  "identifier": "com.pony.ponyclean",
  "build": {
    "devUrl": "http://localhost:5173",
    "frontendDist": "../frontend/dist"
  },
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
    ],
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'"
    }
  }
}
```

### 3.3 capabilities/default.json
```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": ["core:default", "shell:allow-open"]
}
```

## 4. 前端工程搭建

### 4.1 创建方式
使用 Vite 8.1 创建项目（手动配置，避免 CLI 版本不匹配）：
手动创建 `package.json`，详见下方依赖。

### 4.2 依赖安装
```bash
cd frontend
npm init -y
npm install vue@3
npm install -D vite@8.1 @vitejs/plugin-vue typescript vue-tsc
npm install tailwindcss @tailwindcss/vite
npm install shadcn-vue@latest
npm install motion-vue
npm install @tauri-apps/api@latest
npm install @tauri-apps/plugin-shell@latest

# 初始化 Tailwind
npx tailwindcss init -p

# shadcn-vue 初始化 (使用 --defaults 跳过交互式提示)
npx shadcn-vue@latest init --defaults

# 添加需要的组件
npx shadcn-vue@latest add button card separator table progress collapsible alert-dialog alert input skeleton
```

### 4.3 vite.config.ts 关键配置
```typescript
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  server: {
    port: 5173,
    strictPort: true,
  },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: 'esnext',
  },
})
```

### 4.5 目录结构
```
frontend/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── tsconfig.app.json
├── tailwind.config.ts
├── postcss.config.js
├── index.html
├── components.json (shadcn-vue)
├── src/
│   ├── main.ts          ← app.use(MotionPlugin) 注册 motion-vue
│   ├── App.vue
│   ├── styles/
│   │   └── globals.css  ← @tailwind base/components/utilities + shadcn-vue css vars
│   ├── lib/
│   │   └── utils.ts     ← cn() helper
│   ├── components/
│   │   └── ui/          ← shadcn-vue 组件
│   └── vite-env.d.ts
```

## 5. Tailwind v4 样式入口 (globals.css)

```css
@import "tailwindcss";

@theme {
  --color-background: hsl(0 0% 7%);
  --color-foreground: hsl(210 10% 92%);
  --color-card: hsl(220 10% 12%);
  --color-card-foreground: hsl(210 10% 92%);
  --color-popover: hsl(220 10% 12%);
  --color-popover-foreground: hsl(210 10% 92%);
  --color-primary: hsl(214 90% 76%);
  --color-primary-foreground: hsl(0 0% 7%);
  --color-secondary: hsl(220 8% 25%);
  --color-secondary-foreground: hsl(210 10% 92%);
  --color-muted: hsl(220 8% 20%);
  --color-muted-foreground: hsl(215 8% 60%);
  --color-accent: hsl(214 90% 76%);
  --color-accent-foreground: hsl(0 0% 7%);
  --color-destructive: hsl(0 80% 73%);
  --color-destructive-foreground: hsl(0 0% 7%);
  --color-border: hsl(220 8% 20%);
  --color-input: hsl(220 8% 20%);
  --color-ring: hsl(214 90% 76%);
  --radius: 0.75rem;
}
```

Tailwind v4 配置完全通过 `@theme` 指令完成，无需 `tailwind.config.ts` 和 `postcss.config.js`。shadcn-vue CSS Variables 可通过 `@layer base { :root { ... } }` 在 `globals.css` 中补充定义以保持兼容。

## 6. motion-vue 注册 (main.ts)

```typescript
import { createApp } from 'vue'
import { MotionPlugin } from 'motion-vue'
import App from './App.vue'
import './styles/globals.css'

const app = createApp(App)
app.use(MotionPlugin)
app.mount('#app')
```

## 7. 构建验证

```bash
# 1. Rust workspace 构建
cargo build -p pony_core           # 业务核心 OK
cargo test -p pony_core            # 单元测试 OK

# 2. 前端构建
cd frontend && npm run build       # Vite build OK

# 3. Tauri 开发模式
cd .. && cargo tauri dev           # 弹出窗口 OK
```

## 8. 风险

1. Tailwind v3.4 + shadcn-vue 兼容性通过锁定版本来保证
2. shadcn-vue `--defaults` 标志可能不存在于所有版本，备选方案为手动选择
3. tauri v2 的 `tauri.conf.json` 字段与 v1 有 breaking change，需参考 [v2 文档](https://v2.tauri.app)
4. 旧 `src/` 目录不参与 workspace 构建，不会产生编译冲突
