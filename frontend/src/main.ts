import { createApp } from 'vue'
import { MotionPlugin } from 'motion-v'
import App from './App.vue'
import './styles/globals.css'

console.log('[PonyClean] main.ts starting...')
const app = createApp(App)

app.config.errorHandler = (err, instance, info) => {
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
