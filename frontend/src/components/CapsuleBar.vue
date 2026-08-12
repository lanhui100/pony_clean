<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  cpuPercent: number
  memPercent: number
  isHovered?: boolean
}>()

const emit = defineEmits<{
  (e: 'click'): void
}>()

const cpuFill = computed(() => ({ width: `${Math.min(props.cpuPercent, 100)}%` }))
const memFill = computed(() => ({ width: `${Math.min(props.memPercent, 100)}%` }))

const cpuTint = computed(() => {
  const p = props.cpuPercent
  if (p >= 80) return 'bg-red-500/75'
  if (p >= 50) return 'bg-amber-600/75'
  return 'bg-green-600/75'
})

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
  <div
    class="capsule-bar flex cursor-pointer items-center overflow-hidden rounded-full select-none h-full w-full transition-shadow duration-300"
    :class="{ 'capsule-hovered': isHovered }"
    @click="emit('click')"
  >
    <!-- CPU half -->
    <div class="relative flex flex-1 items-center justify-center h-full overflow-hidden rounded-l-full" :class="cpuBg">
      <div
        class="progress-fill absolute left-0 top-0 h-full rounded-l-full"
        :class="cpuTint"
        :style="cpuFill"
      />
      <span class="relative z-10 text-white font-bold text-[13px] leading-none drop-shadow-sm">
        {{ cpuPercent }}<span class="text-white/60 text-[10px] ml-0.5">CPU</span>
      </span>
    </div>

    <!-- Divider -->
    <div class="h-5 w-px bg-white/10 shrink-0" />

    <!-- MEM half -->
    <div class="relative flex flex-1 items-center justify-center h-full overflow-hidden rounded-r-full" :class="memBg">
      <div
        class="progress-fill absolute right-0 top-0 h-full rounded-r-full"
        :class="memTint"
        :style="memFill"
      />
      <span class="relative z-10 text-white font-bold text-[13px] leading-none drop-shadow-sm">
        {{ memPercent }}<span class="text-white/60 text-[10px] ml-0.5">MEM</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.capsule-bar {
  background:
    linear-gradient(180deg, rgba(42, 39, 35, 0.98), rgba(20, 19, 18, 0.96));
  border: 0;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
  transition:
    box-shadow 0.25s ease,
    border-color 0.25s ease;
}

.progress-fill {
  opacity: 0.72;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.10);
}

.capsule-hovered {
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.12),
    0 0 14px rgba(255, 255, 255, 0.07),
    0 0 28px rgba(255, 255, 255, 0.04);
}
</style>
