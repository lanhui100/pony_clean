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

// 胶囊仅贴顶边：island 始终从上方滑入
const islandInitial = computed(() => ({ x: 0, y: -120, opacity: 0 }))

const islandAnimate = computed(() =>
  visible.value ? { x: 0, y: 0, opacity: 1 } : { x: 0, y: -120, opacity: 0 },
)

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
      class="island-shell"
      :initial="islandInitial"
      :animate="islandAnimate"
      :transition="islandTransition"
      @mouseenter="notifyCapsule('island-pointer-enter')"
      @mouseleave="notifyCapsule('island-pointer-leave')"
      @mousemove="notifyCapsule('island-user-activity')"
      @mousedown="notifyCapsule('island-user-activity')"
    >
      <!-- 光晕装饰：模拟环境光折射（借鉴 blur_win 的 Glow 方案） -->
      <div class="glow glow-1" />
      <div class="glow glow-2" />

      <!-- 内容卡片：玻璃上的深色圆角卡片 -->
      <div class="island-card">
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
      </div>
    </motion.div>
  </div>
</template>

<style scoped>
.island-root {
  background: transparent;
  position: relative;
}

/* ─── 玻璃壳层：直角铺满整个窗口 ───
   层级说明：Acrylic 由 Rust 侧 HWND 级 apply_island_vibrancy 提供（DWM 合成，
   真实模糊窗口背后的桌面）。本层在 Acrylic 之上叠加渐变基底、光晕与内阴影，
   形成"整块玻璃"观感 —— 与窗口直角一致，杜绝圆角/直角分层。 */
.island-shell {
  position: absolute;
  inset: 0;
  z-index: 10;
  pointer-events: auto;
  isolation: isolate;
  will-change: transform, opacity;
  transform-origin: top center;
  /* ① 多层渐变基底：白色透层 + 暖色径向光晕 */
  background:
    linear-gradient(
      135deg,
      rgba(255, 255, 255, 0.07),
      rgba(255, 255, 255, 0.02) 42%,
      rgba(255, 255, 255, 0.05)
    ),
    radial-gradient(circle at 18% 12%, rgba(255, 163, 71, 0.10), transparent 32%),
    radial-gradient(circle at 85% 18%, rgba(255, 163, 71, 0.06), transparent 26%);
  /* ② 边框 + 内阴影（玻璃厚度感） */
  border: 1px solid rgba(255, 255, 255, 0.10);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.12),
    inset 0 0 0 1px rgba(255, 255, 255, 0.05),
    inset 0 -24px 80px rgba(0, 0, 0, 0.18);
  /* ③ 背景模糊（辅助；真实桌面模糊由 Acrylic 提供） */
  backdrop-filter: blur(30px) saturate(1.6);
  -webkit-backdrop-filter: blur(30px) saturate(1.6);
}

/* 光晕装饰 */
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
  background: rgba(255, 163, 71, 0.13);
}
.glow-2 {
  width: 140px;
  height: 140px;
  right: 12px;
  bottom: 24px;
  background: rgba(255, 120, 60, 0.09);
}

/* ─── 内容层：全屏填满窗口，不做圆角/边距 ───
   圆角形状由原生层 Region 裁剪负责（window.rs apply_full_round_region），
   Vue 层直接铺满整个窗口，避免出现"圆角面板 + 直角底部"的第二层。 */
.island-card {
  position: absolute;
  inset: 0;
  z-index: 1;
  background:
    linear-gradient(180deg, hsl(30 12% 9% / 0.62), hsl(30 8% 7% / 0.55));
  overflow: hidden;
}

.island-card > * {
  position: relative;
  z-index: 1;
}
</style>