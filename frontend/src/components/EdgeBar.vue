<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  cpuPercent: number
  memPercent: number
}>()

const cpuFill = computed(() => ({ width: `${Math.min(props.cpuPercent, 100)}%` }))
const memFill = computed(() => ({ width: `${Math.min(props.memPercent, 100)}%` }))

const cpuTint = computed(() => {
  const p = props.cpuPercent
  if (p >= 80) return 'bg-red-500/75'
  if (p >= 50) return 'bg-amber-600/75'
  return 'bg-green-600/75'
})

/** 轨道未填充底色（与胶囊 CapsuleBar 一致） */
const cpuBg = computed(() => {
  const p = props.cpuPercent
  if (p >= 80) return 'bg-red-500/12'
  if (p >= 50) return 'bg-amber-600/12'
  return 'bg-green-600/12'
})

const memTint = computed(() => {
  const p = props.memPercent
  if (p >= 80) return 'bg-red-500/75'
  if (p >= 50) return 'bg-amber-600/75'
  return 'bg-green-600/75'
})

const memBg = computed(() => {
  const p = props.memPercent
  if (p >= 80) return 'bg-red-500/12'
  if (p >= 50) return 'bg-amber-600/12'
  return 'bg-green-600/12'
})
</script>

<template>
  <div class="edge-bar">
    <!-- CPU track（左半，向右填充；轨道底色与胶囊一致） -->
    <div class="track" :class="cpuBg">
      <div class="fill fill-cpu" :class="cpuTint" :style="cpuFill" />
      <div class="track-label">
        <span class="num">{{ cpuPercent }}</span>
        <span class="lbl">CPU</span>
      </div>
    </div>
    <div class="sep" />
    <!-- MEM track（右半，向左填充） -->
    <div class="track" :class="memBg">
      <div class="fill fill-mem" :class="memTint" :style="memFill" />
      <div class="track-label">
        <span class="num">{{ memPercent }}</span>
        <span class="lbl">MEM</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.edge-bar {
  display: flex;
  flex-direction: row;
  width: 100%;
  height: 100%;
  overflow: hidden;
  border-radius: 9999px;
  /* 底色与胶囊（CapsuleBar）保持一致，避免缩放时底色跳变 */
  background:
    linear-gradient(180deg, rgba(42, 39, 35, 0.98), rgba(20, 19, 18, 0.96));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
}

.track {
  position: relative;
  overflow: hidden;
  flex: 1;
  height: 100%;
}

.fill {
  position: absolute;
  top: 0;
  height: 100%;
  /* 填充样式与胶囊（CapsuleBar.progress-fill）一致 */
  opacity: 0.72;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.10);
}
.fill-cpu {
  left: 0;
}
.fill-mem {
  right: 0;
}

.sep {
  width: 1px;
  flex-shrink: 0;
  background: rgba(255, 255, 255, 0.12);
}

/* ─── 各自轨道内居中的数值（适配 10px 细条） ─── */
.track-label {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  pointer-events: none;
  white-space: nowrap;
}

.num {
  font-size: 8px;
  line-height: 10px;
  font-weight: 700;
  color: rgba(255, 255, 255, 0.95);
  font-variant-numeric: tabular-nums;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.55);
}

.lbl {
  font-size: 6px;
  line-height: 10px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.55);
  letter-spacing: 0.06em;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.55);
}
</style>
