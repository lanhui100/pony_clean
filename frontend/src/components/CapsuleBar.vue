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
      <div class="absolute left-0 top-0 h-full rounded-l-full transition-all duration-500 ease-out" :class="[cpuTint, cpuFill]" />
      <span class="relative z-10 text-white font-bold text-[13px] leading-none drop-shadow-sm">
        {{ cpuPercent }}<span class="text-white/60 text-[10px] ml-0.5">CPU</span>
      </span>
    </div>

    <!-- Divider -->
    <div class="h-5 w-px bg-white/10 shrink-0" />

    <!-- MEM half -->
    <div class="relative flex flex-1 items-center justify-center h-full overflow-hidden rounded-r-full" :class="memBg">
      <div class="absolute left-0 top-0 h-full rounded-r-full transition-all duration-500 ease-out" :class="[memTint, memFill]" />
      <span class="relative z-10 text-white font-bold text-[13px] leading-none drop-shadow-sm">
        {{ memPercent }}<span class="text-white/60 text-[10px] ml-0.5">MEM</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.capsule-bar {
  background: rgba(30, 28, 26, 0.95);
  border: 0;
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.4);
}

.capsule-hovered {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5), 0 0 20px hsla(38, 85%, 58%, 0.08);
}
</style>
