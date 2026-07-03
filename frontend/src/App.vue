<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { motion } from 'motion-v'
import TitleBar from '@/components/TitleBar.vue'
import CapsuleBar from '@/components/CapsuleBar.vue'
import IslandSummary from '@/components/IslandSummary.vue'
import { useMonitor } from '@/composables/useMonitor'
import { useWindowMorph } from '@/composables/useWindowMorph'

const activeTab = ref('monitor')
const searchQuery = ref('')
const isScanning = ref(false)

const { cpuPercent, memPercent, diskPct, summary, setPollInterval } = useMonitor()
const {
  islandState, capsuleHovered,
  onCapsuleEnter, onCapsuleLeave,
  onCapsuleDragStart, onCapsuleClick,
  onIslandEnter, onIslandLeave, onIslandUserActivity,
  onEnterDone, onLeaveDone,
} = useWindowMorph(isScanning)

/** Capsule exists when idle, and while entering so it can fade out under user action. */
const capsuleMounted = computed(() =>
  islandState.value === 'idle' || islandState.value === 'entering'
)

/** Island exists when not idle */
const islandMounted = computed(() =>
  islandState.value !== 'idle'
)

/** Island translateY: entering slides down, leaving slides up with bounce */
const islandAnimate = computed(() => {
  if (islandState.value === 'entering' || islandState.value === 'visible') {
    return { y: 0, opacity: 1 }
  }
  return { y: -120, opacity: 0 }
})

/** Capsule opacity: idle=visible, entering=fade out. It stays unmounted while island leaves. */
const capsuleAnimate = computed(() => {
  if (islandState.value === 'idle') return { opacity: 1, scale: 1 }
  return { opacity: 0, scale: 0.85 }
})

const islandTransition = computed(() => {
  if (islandState.value === 'leaving') {
    return { type: 'spring' as const, stiffness: 180, damping: 12, mass: 1.0 }
  }
  return { type: 'spring' as const, stiffness: 250, damping: 20, mass: 0.8 }
})

const capsuleTransition = computed(() => ({
  type: 'spring' as const,
  stiffness: 200,
  damping: 15,
  mass: 0.8,
}))

watch(islandState, (state) => {
  setPollInterval(state === 'visible' ? 2000 : 3000)
})

function onIslandAnimComplete() {
  if (islandState.value === 'entering') onEnterDone()
  else if (islandState.value === 'leaving') onLeaveDone()
}
</script>

<template>
  <div class="root h-screen w-screen overflow-visible select-none">
    <!-- ─── Capsule — fades out when island enters, fades in when island leaves ─── -->
    <motion.div
      v-if="capsuleMounted"
      class="capsule-layer"
      :animate="capsuleAnimate"
      :transition="capsuleTransition"
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

    <!-- ─── Dynamic Island — replaces capsule, landscape (short & wide) ─── -->
    <motion.div
      v-if="islandMounted"
      class="island-panel"
      :class="{ 'panel-active': islandState === 'visible' || islandState === 'entering' }"
      :initial="{ y: -120, opacity: 0 }"
      :animate="islandAnimate"
      :transition="islandTransition"
      @animation-complete="onIslandAnimComplete"
      @mouseenter="onIslandEnter"
      @mouseleave="onIslandLeave"
      @mousemove="onIslandUserActivity"
      @mousedown="onIslandUserActivity"
    >
      <div class="flex h-full w-full">
        <!-- Left: icon sidebar -->
        <TitleBar
          v-model:activeTab="activeTab"
          v-model:searchQuery="searchQuery"
        />
        <!-- Right: compact summary -->
        <main class="flex-1 overflow-hidden">
          <IslandSummary
            :cpu-percent="cpuPercent"
            :mem-percent="memPercent"
            :disk-pct="diskPct"
            :process-count="summary?.process_count ?? 0"
            :active-tab="activeTab"
          />
        </main>
      </div>
    </motion.div>
  </div>
</template>

<style scoped>
.root {
  background: transparent;
  position: relative;
}

/* ─── Capsule at top center ─── */
.capsule-layer {
  position: absolute;
  top: 0;
  left: calc(50% - 80px);  /* center of 315px window: (315-160)/2 = 77.5 ≈ 80px offset */
  width: 160px;
  height: 40px;
  z-index: 10;
  pointer-events: auto;
  cursor: grab;
  will-change: transform, opacity;
}

.capsule-layer:active {
  cursor: grabbing;
}

/* ─── Dynamic Island (landscape: wide & short, at top) ─── */
.island-panel {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100px;
  z-index: 10;
  pointer-events: none;
  will-change: transform, opacity;
  transform-origin: top center;
  isolation: isolate;
  background: hsl(30 12% 9% / 0.86);
  border: 0;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  overflow: hidden;
}

.island-panel::before {
  content: "";
  position: absolute;
  inset: -18px;
  z-index: 0;
  pointer-events: none;
  background:
    radial-gradient(circle at 22% 18%, hsl(38 85% 58% / 0.18), transparent 30%),
    radial-gradient(circle at 78% 80%, hsl(140 48% 55% / 0.10), transparent 34%),
    linear-gradient(135deg, hsl(30 10% 18% / 0.34), hsl(30 12% 8% / 0.18));
  filter: blur(18px);
  opacity: 0.72;
}

.island-panel > * {
  position: relative;
  z-index: 1;
}

.island-panel.panel-active {
  pointer-events: auto;
}
</style>
