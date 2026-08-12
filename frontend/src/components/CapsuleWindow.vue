<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { motion } from 'motion-v'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import CapsuleBar from '@/components/CapsuleBar.vue'
import EdgeBar from '@/components/EdgeBar.vue'
import { useMonitor } from '@/composables/useMonitor'
import { useWindowMorph } from '@/composables/useWindowMorph'
import { contentRectFor } from '@/lib/windowMorphConfig'

const isScanning = ref(false)
let unlistenScanState: UnlistenFn | null = null
let unlistenResetPos: UnlistenFn | null = null
const pillLayerRef = ref<HTMLElement | null>(null)
const { cpuPercent, memPercent, setPollInterval } = useMonitor()
const {
  islandState,
  capsuleHovered,
  isDragging,
  form,
  onCapsuleEnter,
  onCapsuleLeave,
  onBarEnter,
  onBarLeave,
  onCapsuleDragStart,
  onCapsuleClick,
  onEnterDone,
  onLeaveDone,
  notifyActivity,
  resetToDefault,
} = useWindowMorph(isScanning)

// 胶囊层 / 进度条层的内容矩形（窗口内 CSS 像素，横排）
const pillRect = computed(() => contentRectFor('pill'))
const barRect = computed(() => contentRectFor('bar'))

const pillLayerStyle = computed(() => ({
  left: `${pillRect.value.x}px`,
  top: `${pillRect.value.y}px`,
  width: `${pillRect.value.w}px`,
  height: `${pillRect.value.h}px`,
  pointerEvents: form.value === 'pill' ? 'auto' : 'none',
  zIndex: form.value === 'pill' ? 20 : 10,
}))

const barLayerStyle = computed(() => ({
  left: `${barRect.value.x}px`,
  top: `${barRect.value.y}px`,
  width: `${barRect.value.w}px`,
  height: `${barRect.value.h}px`,
  pointerEvents: form.value === 'bar' ? 'auto' : 'none',
  zIndex: form.value === 'bar' ? 20 : 10,
}))

// 形态变换：transform-origin 为 0 0，用 x/y + scale 精确映射 pillRect ⇄ barRect
const pillAnim = computed(() => {
  if (form.value === 'pill') return { x: 0, y: 0, scaleX: 1, scaleY: 1, opacity: 1 }
  const p = pillRect.value
  const b = barRect.value
  return { x: b.x - p.x, y: b.y - p.y, scaleX: b.w / p.w, scaleY: b.h / p.h, opacity: 0 }
})

const barAnim = computed(() => {
  if (form.value === 'bar') return { x: 0, y: 0, scaleX: 1, scaleY: 1, opacity: 1 }
  const p = pillRect.value
  const b = barRect.value
  return { x: p.x - b.x, y: p.y - b.y, scaleX: p.w / b.w, scaleY: p.h / b.h, opacity: 0 }
})

const morphTransition = { type: 'spring' as const, stiffness: 240, damping: 22, mass: 0.9 }

// island 展开/收起时的整体淡出缩放
const capsuleAnimate = computed(() => {
  if (islandState.value === 'idle' || islandState.value === 'leaving') return { opacity: 1, scale: 1 }
  return { opacity: 0, scale: 0.9 }
})

const capsuleTransition = computed(() => ({
  type: 'spring' as const,
  stiffness: 220,
  damping: 18,
  mass: 0.75,
}))

watch(islandState, () => {
  setPollInterval(islandState.value === 'visible' ? 2000 : 3000)
})

// 扫描状态变化时刷新收起计时（扫描中不收起为进度条）
watch(isScanning, () => {
  notifyActivity()
})

onMounted(async () => {
  unlistenScanState = await listen<{ scanning: boolean }>('scan-state-changed', (e) => {
    isScanning.value = e.payload?.scanning ?? false
  }).catch(() => null)

  // 托盘菜单“重置胶囊位置”：重置到顶边居中
  unlistenResetPos = await listen('reset-capsule-position', () => {
    resetToDefault()
  }).catch(() => null)

  // 渲染自检：挂载后检查胶囊层是否真的可见，结果转发到 Rust 终端
  setTimeout(() => {
    window.__ponyLog?.('info', `render-check: form=${form.value}`)
    const el = pillLayerRef.value
    if (!el) {
      window.__ponyLog?.('error', 'render-check: pill layer ref is null (template render failed?)')
      return
    }
    const cs = window.getComputedStyle(el)
    const pill = pillRect.value
    window.__ponyLog?.(
      'info',
      `render-check: pillLayer opacity=${cs.opacity} display=${cs.display} ` +
        `pos=${cs.left},${cs.top} size=${cs.width}x${cs.height} z=${cs.zIndex} ` +
        `expected=${pill.x},${pill.y} ${pill.w}x${pill.h}`,
    )
  }, 1200)
})

onUnmounted(() => {
  unlistenScanState?.()
  unlistenResetPos?.()
})

function onCapsuleAnimComplete() {
  if (islandState.value === 'entering') onEnterDone()
  else if (islandState.value === 'leaving') onLeaveDone()
}
</script>

<template>
  <div class="capsule-root h-screen w-screen overflow-hidden select-none" @mousedown="onCapsuleDragStart">
    <motion.div
      class="island-fade-layer"
      :animate="capsuleAnimate"
      :transition="capsuleTransition"
      :on-animation-complete="onCapsuleAnimComplete"
    >
      <!-- 胶囊层 -->
      <motion.div
        ref="pillLayerRef"
        class="content-layer"
        :style="pillLayerStyle"
        :animate="pillAnim"
        :transition="morphTransition"
        @mouseenter="onCapsuleEnter"
        @mouseleave="onCapsuleLeave"
        @click="onCapsuleClick"
      >
        <CapsuleBar
          :cpu-percent="cpuPercent"
          :mem-percent="memPercent"
          :is-hovered="capsuleHovered"
        />
      </motion.div>

      <!-- 贴边进度条层 -->
      <motion.div
        class="content-layer"
        :style="barLayerStyle"
        :animate="barAnim"
        :transition="morphTransition"
        @mouseenter="onBarEnter"
        @mouseleave="onBarLeave"
        @click="onCapsuleClick"
      >
        <EdgeBar
          :cpu-percent="cpuPercent"
          :mem-percent="memPercent"
        />
      </motion.div>
    </motion.div>
  </div>
</template>

<style scoped>
.capsule-root {
  background: transparent;
  position: relative;
}

.island-fade-layer {
  position: absolute;
  inset: 0;
  z-index: 10;
  pointer-events: auto;
  will-change: transform, opacity;
}

.content-layer {
  position: absolute;
  cursor: grab;
  will-change: transform, opacity;
  transform-origin: 0 0;
}

.content-layer:active {
  cursor: grabbing;
}
</style>
