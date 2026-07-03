<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  cpuPercent: number
  memPercent: number
  diskPct: number
  processCount: number
  activeTab: string
}>()

function barColor(pct: number) {
  if (pct >= 80) return 'bg-red-500'
  if (pct >= 50) return 'bg-amber-500'
  return 'bg-green-500'
}

function textColor(pct: number) {
  if (pct >= 80) return 'text-red-400'
  if (pct >= 50) return 'text-amber-400'
  return 'text-green-400'
}

const cpuText = computed(() => `${props.cpuPercent.toFixed(1)}%`)
const memText = computed(() => `${props.memPercent.toFixed(1)}%`)
const diskText = computed(() => `${props.diskPct.toFixed(0)}%`)
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

    <!-- Disk row (shown when cleaner tab active) -->
    <div v-show="activeTab === 'cleaner'" class="flex items-center gap-3 text-xs">
      <span class="w-8 shrink-0 font-semibold" :class="textColor(diskPct)">C盘</span>
      <div class="flex-1 h-2 rounded-full bg-white/8 overflow-hidden">
        <div
          class="h-full rounded-full transition-all duration-700 ease-out"
          :class="barColor(diskPct)"
          :style="{ width: Math.min(diskPct, 100) + '%' }"
        />
      </div>
      <span class="tabular-nums w-14 text-right font-mono text-foreground/80">{{ diskText }}</span>
    </div>

    <!-- Process count stat for monitor -->
    <div v-show="activeTab === 'monitor'" class="flex items-center gap-3 text-xs">
      <span class="w-8 shrink-0 font-semibold text-muted-foreground">PIDs</span>
      <span class="tabular-nums text-foreground/60">{{ processCount }} 个进程</span>
    </div>
  </div>
</template>