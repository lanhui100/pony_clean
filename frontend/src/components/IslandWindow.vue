<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { motion } from 'motion-v'
import TitleBar from '@/components/TitleBar.vue'
import MonitorPanel from '@/views/MonitorPanel.vue'
import SpacePanel from '@/views/SpacePanel.vue'
import StartupPanel from '@/views/StartupPanel.vue'
import SettingsPanel from '@/views/SettingsPanel.vue'
import { initUpdater, disposeUpdater } from '@/composables/useUpdater'

const activeTab = ref('monitor')
const searchQuery = ref('')
const visible = ref(false)
const expanded = ref(false)
const islandWindow = getCurrentWindow()
let unlistenEnter: UnlistenFn | null = null
let unlistenLeave: UnlistenFn | null = null
let shrinkFallback: ReturnType<typeof setTimeout> | null = null

// 胶囊仅贴顶边：island 始终从上方滑入
const islandInitial = computed(() => ({ x: 0, y: -120, opacity: 0 }))

// 收起：向顶边胶囊收缩（scaleY 折叠 + 淡出），transform-origin 顶边居中，
// 与胶囊顶边对齐，视觉上「面板缩回胶囊」；展开：从上方滑入（spring）
const islandAnimate = computed(() =>
  visible.value
    ? { x: 0, y: 0, opacity: 1, scaleY: 1 }
    : { x: 0, y: 0, opacity: 0, scaleY: 0.12 },
)

const prefersReducedMotion =
  typeof window !== 'undefined' &&
  window.matchMedia('(prefers-reduced-motion: reduce)').matches

const islandTransition = computed(() => {
  if (prefersReducedMotion) return { duration: 0.001 }
  if (visible.value) {
    return { type: 'spring' as const, stiffness: 250, damping: 20, mass: 0.8 }
  }
  // 收起动画比进入更快（exit-faster-than-enter），缩放折叠向顶边
  return { duration: 0.22, ease: 'easeIn' as const }
})

function notifyCapsule(event: string) {
  emitTo('capsule', event).catch(() => {})
}

async function syncWindowSize() {
  try {
    await invoke('set_island_expanded', { expanded: expanded.value })
  } catch (e) {
    console.warn('set_island_expanded failed:', e)
  }
}

/**
 * 收起动画完成后再把窗口切回概要态高度——避免「先切物理高度、后播动画」
 * 造成的硬裁剪残影（SPEC-029）。
 * motion-v complete 在进入极早期可能不派发（P2-1），加一个兜底定时器，
 * 保证 expanded 标记最终被清（幂等；下次展开 set_island_expanded(true) 自愈）。
 */
function onIslandAnimComplete() {
  if (!visible.value && expanded.value) {
    expanded.value = false
    syncWindowSize()
  }
  if (shrinkFallback) {
    clearTimeout(shrinkFallback)
    shrinkFallback = null
  }
}

function scheduleShrinkFallback() {
  if (shrinkFallback) clearTimeout(shrinkFallback)
  shrinkFallback = setTimeout(() => {
    shrinkFallback = null
    onIslandAnimComplete()
  }, 600)
}

function onScanStart() {
  emitTo('capsule', 'scan-state-changed', { scanning: true }).catch(() => {})
}

function onScanEnd() {
  emitTo('capsule', 'scan-state-changed', { scanning: false }).catch(() => {})
}

/** 交互元素（按钮/输入/勾选/滚动等）上不触发拖动 */
const DRAG_IGNORE =
  'button, a, input, select, textarea, [role="button"], [role="checkbox"], [data-drag-ignore]'

/**
 * 面板态水平拖动：按住面板空白处（非交互元素）沿顶边拖动，
 * 胶囊+面板同步移动（由 capsule 窗口执行定位）。mousedown 后在本窗口
 * document 上监听 move/up（WebView2 隐式鼠标捕获保证移出窗口仍持续），
 * 位移经事件转发给 capsule（screenX 为逻辑 CSS px，capsule 端按 DPR 换算）。
 */
/** 面板态拖动中光标状态（空白处按住 → 抓手） */
const islandDragGrabbing = ref(false)

function onIslandDragStart(e: MouseEvent) {
  if (e.button !== 0) return
  const t = e.target as HTMLElement | null
  if (!t || t.closest(DRAG_IGNORE)) return
  islandDragGrabbing.value = true
  emitTo('capsule', 'island-drag-start', { screenX: e.screenX }).catch(() => {})
  const onMove = (ev: MouseEvent) => {
    emitTo('capsule', 'island-drag-move', { screenX: ev.screenX }).catch(() => {})
  }
  const onUp = () => {
    islandDragGrabbing.value = false
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
    emitTo('capsule', 'island-drag-end').catch(() => {})
  }
  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp)
}

onMounted(async () => {
  await islandWindow.setDecorations(false).catch(() => {})
  // 阴影由 CSS box-shadow 画在窗口阴影边距内（SPEC-029），禁用原生阴影
  await islandWindow.setShadow(false).catch(() => {})
  // 注意：不清除 window effects — Rust 侧已对 HWND 应用 SWCA Acrylic 毛玻璃
  // （apply_island_vibrancy，ACCENT_ENABLE_ACRYLICBLURBEHIND）

  unlistenEnter = await listen('island-enter', () => {
    visible.value = true
    expanded.value = true
    syncWindowSize()
  })
  unlistenLeave = await listen('island-leave', () => {
    visible.value = false
    // expanded/物理高度切换延迟到收起动画完成（onIslandAnimComplete / 兜底）
    scheduleShrinkFallback()
  })

  await nextTick()
  await new Promise<void>((resolve) => setTimeout(resolve, 0))

  // 自动更新：启动定时检查（首次 3 秒后提示角标，之后周期检查）
  initUpdater()
})

onUnmounted(() => {
  unlistenEnter?.()
  unlistenLeave?.()
  if (shrinkFallback) clearTimeout(shrinkFallback)
  disposeUpdater()
})
</script>

<template>
  <div
    class="island-root h-screen w-screen overflow-hidden select-none"
    :class="islandDragGrabbing ? 'cursor-grabbing' : 'cursor-grab'"
    @mousedown="onIslandDragStart"
  >
    <motion.div
      class="island-shell"
      :initial="islandInitial"
      :animate="islandAnimate"
      :transition="islandTransition"
      :on-animation-complete="onIslandAnimComplete"
      @mouseenter="notifyCapsule('island-pointer-enter')"
      @mouseleave="notifyCapsule('island-pointer-leave')"
      @mousemove="notifyCapsule('island-user-activity')"
      @mousedown="notifyCapsule('island-user-activity')"
    >
      <!-- 光晕装饰：模拟环境光折射（借鉴 blur_win 的 Glow 方案） -->
      <div class="glow glow-1" />
      <div class="glow glow-2" />

      <!-- 内容卡片：与壳层同一圆角形状，单一面板观感 -->
      <div class="island-card">
        <div class="flex h-full w-full">
          <TitleBar
            v-model:activeTab="activeTab"
            v-model:searchQuery="searchQuery"
          />
          <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
            <main class="flex-1 overflow-hidden px-4 pb-4 pt-2">
              <MonitorPanel v-if="activeTab === 'monitor'" :search="searchQuery" />
              <SpacePanel
                v-else-if="activeTab === 'cleaner'"
                @scan-start="onScanStart"
                @scan-end="onScanEnd"
              />
              <StartupPanel v-else-if="activeTab === 'startup'" />
              <SettingsPanel v-else />
            </main>
          </div>
        </div>
      </div>
    </motion.div>
  </div>
</template>

<style scoped>
.island-root {
  background: transparent;
  position: relative;
}

/* ─── 玻璃壳层：占满整个窗口（面板即窗口），**方角** ───
   面板即窗口（SPEC-029 终版，用户裁决）：SWCA Acrylic 毛玻璃铺满整个方形
   窗口且不被 Region 裁剪——若 CSS/Region 圆角，四角会露出底层 SWCA 的直角
   毛玻璃（"两层不重叠"）。故面板整体**方角**：SWCA = CSS = Region 三者方角
   彻底一致，无分层。外阴影由原生 DWM（CS_DROPSHADOW）按方角 Region 投影。 */
.island-shell {
  position: absolute;
  inset: 0;
  z-index: 10;
  pointer-events: auto;
  isolation: isolate;
  will-change: transform, opacity;
  transform-origin: top center;
  overflow: hidden;
  /* ① 玻璃渐变基底：白色透层 + 暖色径向光晕（低饱和香槟色，胶囊态） */
  background:
    linear-gradient(
      135deg,
      rgba(255, 255, 255, 0.06),
      rgba(255, 255, 255, 0.02) 42%,
      rgba(255, 255, 255, 0.04)
    ),
    radial-gradient(circle at 18% 12%, rgba(214, 178, 122, 0.08), transparent 32%),
    radial-gradient(circle at 85% 18%, rgba(214, 178, 122, 0.05), transparent 26%);
  /* ② 边框 + 顶部内高光（外阴影由原生 DWM 提供） */
  border: 1px solid rgba(255, 255, 255, 0.10);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.10);
  /* ③ 背景模糊（辅助；真实桌面毛玻璃由 Rust 的 SWCA Acrylic 提供） */
  backdrop-filter: blur(30px) saturate(1.4);
  -webkit-backdrop-filter: blur(30px) saturate(1.4);
}

/* 光晕装饰（被壳层圆角裁剪） */
.glow {
  position: absolute;
  border-radius: 9999px;
  pointer-events: none;
  z-index: 0;
  filter: blur(40px);
}
.glow-1 {
  width: 120px;
  height: 120px;
  left: 24px;
  top: 12px;
  background: rgba(214, 178, 122, 0.10);
}
.glow-2 {
  width: 140px;
  height: 140px;
  right: 12px;
  bottom: 24px;
  background: rgba(196, 152, 102, 0.07);
}

/* ─── 内容层：与壳层同形（方角）───
   半透明深色渐变，让窗口级 Acrylic 毛玻璃透出（alpha 不宜过高，否则遮住模糊）；
   0.62/0.55 为毛玻璃观感平衡值。方角与 SWCA/CSS 壳层一致，四角无分层。 */
.island-card {
  position: absolute;
  inset: 0;
  z-index: 1;
  overflow: hidden;
  background:
    linear-gradient(180deg, hsl(30 12% 9% / 0.62), hsl(30 8% 7% / 0.55));
}

.island-card > * {
  position: relative;
  z-index: 1;
}
</style>
