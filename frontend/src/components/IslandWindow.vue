<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { motion } from 'motion-v'
import TitleBar from '@/components/TitleBar.vue'
import IslandSummary from '@/components/IslandSummary.vue'
import MonitorPanel from '@/views/MonitorPanel.vue'
import CleanerPanel from '@/views/CleanerPanel.vue'
import AnalysisPanel from '@/views/AnalysisPanel.vue'
import SettingsPanel from '@/views/SettingsPanel.vue'
import { useMonitor } from '@/composables/useMonitor'
import { WINDOW_MORPH } from '@/lib/windowMorphConfig'

const activeTab = ref('monitor')
const searchQuery = ref('')
const visible = ref(false)
const expanded = ref(false)
const islandWindow = getCurrentWindow()
let unlistenEnter: UnlistenFn | null = null
let unlistenLeave: UnlistenFn | null = null

const { cpuPercent, memPercent, diskPct, summary } = useMonitor()

const islandHeight = computed(() =>
  expanded.value ? WINDOW_MORPH.expandedH : WINDOW_MORPH.fullH,
)

const islandAnimate = computed(() => (
  visible.value ? { y: 0, opacity: 1 } : { y: -120, opacity: 0 }
))

const islandTransition = computed(() => ({
  type: 'spring' as const,
  stiffness: visible.value ? 250 : 180,
  damping: visible.value ? 20 : 14,
  mass: visible.value ? 0.8 : 0.95,
}))

const rootStyle = computed(() => ({
  '--morph-island-h': `${islandHeight.value}px`,
}))

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

function onScanStart() {
  emitTo('capsule', 'scan-state-changed', { scanning: true }).catch(() => {})
}

function onScanEnd() {
  emitTo('capsule', 'scan-state-changed', { scanning: false }).catch(() => {})
}

onMounted(async () => {
  await islandWindow.setDecorations(false).catch(() => {})
  await islandWindow.setShadow(false).catch(() => {})
  // 注意：不清除 window effects — Rust 侧已对 HWND 应用 Acrylic（apply_island_vibrancy）
  // clearEffects 会移除原生毛玻璃，导致只能看到 CSS 渐变底

  unlistenEnter = await listen('island-enter', () => {
    visible.value = true
    expanded.value = true
    syncWindowSize()
  })
  unlistenLeave = await listen('island-leave', () => {
    visible.value = false
    expanded.value = false
    syncWindowSize()
  })

  await nextTick()
  await new Promise<void>((resolve) => setTimeout(resolve, 0))
})

onUnmounted(() => {
  unlistenEnter?.()
  unlistenLeave?.()
})
</script>

<template>
  <div class="island-root h-screen w-screen overflow-hidden select-none" :style="rootStyle">
    <motion.div
      class="island-panel panel-active"
      :initial="{ y: -120, opacity: 0 }"
      :animate="islandAnimate"
      :transition="islandTransition"
      @mouseenter="notifyCapsule('island-pointer-enter')"
      @mouseleave="notifyCapsule('island-pointer-leave')"
      @mousemove="notifyCapsule('island-user-activity')"
      @mousedown="notifyCapsule('island-user-activity')"
    >
      <div class="flex h-full w-full">
        <TitleBar
          v-model:activeTab="activeTab"
          v-model:searchQuery="searchQuery"
        />
        <div class="flex min-w-0 flex-1 flex-col overflow-hidden">
          <div class="px-4 pt-3">
            <IslandSummary
              :cpu-percent="cpuPercent"
              :mem-percent="memPercent"
              :disk-pct="diskPct"
              :process-count="summary?.process_count ?? 0"
              :active-tab="activeTab"
            />
          </div>
          <main class="flex-1 overflow-hidden px-4 pb-4 pt-2">
            <MonitorPanel v-if="activeTab === 'monitor'" :search="searchQuery" />
            <CleanerPanel
              v-else-if="activeTab === 'cleaner'"
              @scan-start="onScanStart"
              @scan-end="onScanEnd"
            />
            <AnalysisPanel v-else-if="activeTab === 'analysis'" />
            <SettingsPanel v-else />
          </main>
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

.island-panel {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: var(--morph-island-h);
  z-index: 10;
  pointer-events: auto;
  will-change: transform, opacity, height;
  transform-origin: top center;
  isolation: isolate;
  transition: height 0.25s ease;
  /* 背景策略（层级说明）：
     1. 原生 Acrylic 由 Rust 侧 HWND 级 apply_island_vibrancy 提供（DWM 合成，
        能真实模糊窗口背后的桌面）——此时本渐变透明度越低，毛玻璃越明显。
     2. 若 Acrylic 失败（部分系统），此处半透明深色渐变兜底，保证可读性。
     3. CSS backdrop-filter 在透明 WebView2 窗口上无法模糊桌面，仅作辅助。 */
  background:
    linear-gradient(180deg, hsl(30 12% 9% / 0.58), hsl(30 8% 7% / 0.52));
  backdrop-filter: blur(24px) saturate(160%);
  -webkit-backdrop-filter: blur(24px) saturate(160%);
  border: 1px solid rgb(255 255 255 / 0.10);
  border-radius: 16px;
  box-shadow:
    inset 0 -1px 0 rgba(0, 0, 0, 0.40),
    inset 0 1px 0 rgb(255 255 255 / 0.10);
  overflow: hidden;
}

.island-panel > * {
  position: relative;
  z-index: 1;
}
</style>
