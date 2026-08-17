<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  cpuPercent: number
  memPercent: number
  processCount: number
}>()

function barColor(pct: number) {
  if (pct >= 80) return 'bg-destructive'
  if (pct >= 50) return 'bg-warning'
  return 'bg-success'
}

function textColor(pct: number) {
  if (pct >= 80) return 'text-destructive'
  if (pct >= 50) return 'text-warning'
  return 'text-success'
}

const cpuText = computed(() => `${props.cpuPercent.toFixed(1)}%`)
const memText = computed(() => `${props.memPercent.toFixed(1)}%`)
</script>

<template>
  <div class="flex h-full flex-col justify-center px-4 py-2 gap-1.5">
    <!-- CPU row -->
    <div class="flex items-center gap-3 text-xs">
      <span class="w-8 shrink-0 font-semibold" :class="textColor(cpuPercent)">CPU</span>
      <div class="flex-1 h-2 rounded-full bg-white/8 overflow-hidden">
        <div
          class="h-full rounded-full transition-all duration-700 ease-out"
          :class="barColor(cpuPercent)"
          :style="{ width: Math.min(cpuPercent, 100) + '%' }"
        />
      </div>
      <span class="tabular-nums w-14 text-right font-mono text-foreground/80">{{ cpuText }}</span>
    </div>

    <!-- MEM row -->
    <div class="flex items-center gap-3 text-xs">
      <span class="w-8 shrink-0 font-semibold" :class="textColor(memPercent)">MEM</span>
      <div class="flex-1 h-2 rounded-full bg-white/8 overflow-hidden">
        <div
          class="h-full rounded-full transition-all duration-700 ease-out"
          :class="barColor(memPercent)"
          :style="{ width: Math.min(memPercent, 100) + '%' }"
        />
      </div>
      <span class="tabular-nums w-14 text-right font-mono text-foreground/80">{{ memText }}</span>
    </div>

    <!-- Process count stat for monitor -->
    <div class="flex items-center gap-3 text-xs">
      <span class="w-8 shrink-0 font-semibold text-muted-foreground">PIDs</span>
      <span class="tabular-nums text-foreground/60">{{ processCount }} 个进程</span>
    </div>
  </div>
</template>