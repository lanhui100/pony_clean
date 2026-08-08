<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { motion } from 'motion-v'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import CapsuleBar from '@/components/CapsuleBar.vue'
import { useMonitor } from '@/composables/useMonitor'
import { useWindowMorph } from '@/composables/useWindowMorph'
import { WINDOW_MORPH } from '@/lib/windowMorphConfig'

const isScanning = ref(false)
let unlistenScanState: UnlistenFn | null = null
const { cpuPercent, memPercent, setPollInterval } = useMonitor()
const {
  islandState,
  capsuleHovered,
  isDragging,
  onCapsuleEnter,
  onCapsuleLeave,
  onCapsuleDragStart,
  onCapsuleClick,
  onEnterDone,
  onLeaveDone,
} = useWindowMorph(isScanning)

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

// Hover scale boost: only when idle/leaving and not dragging
const capsuleHoverAnimate = computed(() => {
  if (isDragging.value) return {}
  return { scale: 1.04 }
})

const rootStyle = computed(() => ({
  '--morph-capsule-w': `${WINDOW_MORPH.capsuleW}px`,
  '--morph-capsule-h': `${WINDOW_MORPH.capsuleH}px`,
  '--morph-pill-w': `${WINDOW_MORPH.pillW}px`,
  '--morph-pill-h': `${WINDOW_MORPH.pillH}px`,
}))

watch(islandState, (state) => {
  setPollInterval(state === 'visible' ? 2000 : 3000)
})

onMounted(async () => {
  unlistenScanState = await listen<{ scanning: boolean }>('scan-state-changed', (e) => {
    isScanning.value = e.payload?.scanning ?? false
  }).catch(() => null)
})

onUnmounted(() => {
  unlistenScanState?.()
})

function onCapsuleAnimComplete() {
  if (islandState.value === 'entering') onEnterDone()
  else if (islandState.value === 'leaving') onLeaveDone()
}
</script>

<template>
  <div class="capsule-root h-screen w-screen overflow-hidden select-none" :style="rootStyle">
    <motion.div
      class="capsule-layer"
      :animate="capsuleAnimate"
      :transition="capsuleTransition"
      :on-animation-complete="onCapsuleAnimComplete"
      :while-hover="capsuleHoverAnimate"
      @mouseenter="onCapsuleEnter"
      @mouseleave="onCapsuleLeave"
      @mousedown="onCapsuleDragStart"
    >
      <CapsuleBar
        :cpu-percent="cpuPercent"
        :mem-percent="memPercent"
        :is-hovered="capsuleHovered"
        @click="onCapsuleClick"
      />
    </motion.div>
  </div>
</template>

<style scoped>
.capsule-root {
  background: transparent;
  position: relative;
}

.capsule-layer {
  position: absolute;
  top: calc((var(--morph-capsule-h) - var(--morph-pill-h)) / 2);
  left: calc((var(--morph-capsule-w) - var(--morph-pill-w)) / 2);
  width: var(--morph-pill-w);
  height: var(--morph-pill-h);
  z-index: 10;
  pointer-events: auto;
  cursor: grab;
  will-change: transform, opacity;
  transform-origin: center center;
}

.capsule-layer:active {
  cursor: grabbing;
}
</style>
