import { createApp } from 'vue'
import { MotionPlugin } from 'motion-v'
import { invoke } from '@tauri-apps/api/core'
import App from './App.vue'
import './styles/globals.css'

console.log('[PonyClean] main.ts starting...')

// ─── 前端日志转发到 Rust 终端 ───
// WebView 的 console 默认不显示在 `npm run dev:tauri` 的终端里，
// 这里把 error/warn/关键信息转发到 `log_frontend` 命令（Rust 侧 eprintln）。
declare global {
  interface Window {
    __ponyLog: (level: string, ...args: unknown[]) => void
  }
}

function forwardFrontendLog(level: string, args: unknown[]) {
  try {
    const message = args
      .map((a) => {
        if (typeof a === 'string') return a
        if (a instanceof Error) return `${a.name}: ${a.message}\n${a.stack ?? ''}`
        try {
          return JSON.stringify(a)
        } catch {
          return String(a)
        }
      })
      .join(' ')
    invoke('log_frontend', { level, message }).catch(() => {})
  } catch {
    // 纯浏览器环境（无 Tauri）时静默
  }
}

window.__ponyLog = (level, ...args) => forwardFrontendLog(level, args)

// 前端脚本加载标记：确认 JS 是否真的在 WebView 里执行
forwardFrontendLog('info', ['frontend script loaded'])

// 拦截 console.error / console.warn
const origError = console.error
const origWarn = console.warn
console.error = (...args: unknown[]) => {
  forwardFrontendLog('error', args)
  origError(...args)
}
console.warn = (...args: unknown[]) => {
  forwardFrontendLog('warn', args)
  origWarn(...args)
}

// 未捕获异常 / 资源加载错误
window.addEventListener('error', (e) => {
  forwardFrontendLog('error', [`uncaught: ${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`])
})
window.addEventListener('unhandledrejection', (e) => {
  forwardFrontendLog('error', [`unhandledrejection: ${String(e.reason)}`])
})

const app = createApp(App)

app.config.errorHandler = (err, instance, info) => {
  forwardFrontendLog('vue-error', [String(err), info])
  console.error('[PonyClean] Vue error:', err, info)
}

app.config.warnHandler = (msg, instance, trace) => {
  console.warn('[PonyClean] Vue warn:', msg, trace)
}
try {
  app.use(MotionPlugin)
} catch (e) {
  console.error('[PonyClean] MotionPlugin failed:', e)
}
app.mount('#app')
console.log('[PonyClean] Vue app mounted')
forwardFrontendLog('info', ['vue app mounted'])
