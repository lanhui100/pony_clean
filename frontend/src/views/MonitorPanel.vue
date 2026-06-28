<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { X, Cpu, MemoryStick } from 'lucide-vue-next'
import { useMonitor, type ProcessInfo } from '../composables/useMonitor'

const { processes, summary, loading, error, killProcess } = useMonitor()

defineProps<{
  search: string
}>()

const sortKey = ref<'name' | 'cpu' | 'mem_mb' | 'mem_pct'>('cpu')
const sortDir = ref<'asc' | 'desc'>('desc')
const killMsg = ref('')
let killTimer: ReturnType<typeof setTimeout> | null = null

interface ProcessRow extends ProcessInfo {
  mem_pct: number
}

const filtered = computed(() => {
  const total = summary.value?.mem_total_mb ?? 0
  let list: ProcessRow[] = processes.value.map(p => ({
    ...p,
    mem_pct: total > 0 ? (p.mem_mb / total) * 100 : 0,
  }))
  if (!search.trim()) {
    list = list.filter(p => p.cpu > 10 || p.mem_mb > 200)
  } else {
    const q = search.toLowerCase()
    list = list.filter(p => p.name.toLowerCase().includes(q))
  }
  const key = sortKey.value
  const dir = sortDir.value
  return list.toSorted((a, b) => {
    let va: string | number = a[key]
    let vb: string | number = b[key]
    if (va < vb) return dir === 'asc' ? -1 : 1
    if (va > vb) return dir === 'asc' ? 1 : -1
    return 0
  })
})

const memPct = computed(() => {
  const s = summary.value
  if (!s || s.mem_total_mb <= 0) return 0
  return (s.mem_used_mb / s.mem_total_mb) * 100
})

function toggleSort(key: typeof sortKey.value) {
  if (sortKey.value === key) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortKey.value = key
    sortDir.value = 'desc'
  }
}

function sortIcon(key: typeof sortKey.value) {
  if (sortKey.value !== key) return ''
  return sortDir.value === 'asc' ? '▲' : '▼'
}

function cpuThreshold(v: number) {
  if (v < 50) return 'low'
  if (v < 80) return 'mid'
  return 'high'
}

function memThreshold(pct: number) {
  if (pct < 65) return 'low'
  if (pct < 85) return 'mid'
  return 'high'
}

function cpuColor(v: number) {
  const t = cpuThreshold(v)
  if (t === 'low') return 'bg-success'
  if (t === 'mid') return 'bg-warning'
  return 'bg-destructive'
}

function cpuTextColor(v: number) {
  const t = cpuThreshold(v)
  if (t === 'low') return ''
  if (t === 'mid') return 'text-warning'
  return 'text-destructive'
}

function memColor(pct: number) {
  const t = memThreshold(pct)
  if (t === 'low') return 'bg-success'
  if (t === 'mid') return 'bg-warning'
  return 'bg-destructive'
}

function fmMemGb(mb: number) {
  if (!Number.isFinite(mb)) return '—'
  return (mb / 1024).toFixed(1)
}

function memTextColor(pct: number) {
  const t = memThreshold(pct)
  if (t === 'low') return ''
  if (t === 'mid') return 'text-warning'
  return 'text-destructive'
}

function fmMem(mb: number) {
  if (!Number.isFinite(mb)) return '—'
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)}GB`
  return `${Math.round(mb)}MB`
}

function fmPct(v: number) {
  if (v <= 0) return '—'
  return `${v.toFixed(1)}%`
}

async function handleKill(p: ProcessRow) {
  killMsg.value = ''
  const exists = processes.value.some(proc => proc.pid === p.pid)
  if (!exists) {
    killMsg.value = '✓ 进程已结束'
    scheduleKillClear()
    return
  }
  killMsg.value = await killProcess(p.pid, p.name)
  scheduleKillClear()
}

function scheduleKillClear() {
  if (killTimer) clearTimeout(killTimer)
  killTimer = setTimeout(() => { killMsg.value = '' }, 2500)
}

onUnmounted(() => {
  if (killTimer) clearTimeout(killTimer)
})
</script>

<template>
  <div class="flex h-full flex-col gap-3">
    <!-- Summary bar -->
    <div class="flex items-center justify-center gap-6">
      <!-- CPU -->
      <div class="flex flex-1 min-w-0 items-center justify-center gap-4 rounded-xl px-5 py-4">
        <Cpu class="h-6 w-6 shrink-0 text-muted-foreground" />
        <div class="flex flex-col items-center">
          <div class="flex items-baseline gap-0.5">
            <span class="text-2xl font-bold tabular-nums leading-none" :class="cpuTextColor(summary?.cpu_total ?? 0)">
              {{ summary ? summary.cpu_total.toFixed(1) : '—' }}
            </span>
            <span class="text-xs tabular-nums" :class="cpuTextColor(summary?.cpu_total ?? 0)">%</span>
          </div>
          <div class="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted/50">
            <span
              class="block h-full rounded-full transition-all"
              :class="cpuColor(summary?.cpu_total ?? 0)"
              :style="{ width: Math.min(summary?.cpu_total ?? 0, 100) + '%' }"
            />
          </div>
        </div>
      </div>

      <!-- Memory -->
      <div class="flex flex-1 min-w-0 items-center justify-center gap-4 rounded-xl px-5 py-4">
        <MemoryStick class="h-6 w-6 shrink-0 text-muted-foreground" />
        <div class="flex flex-col items-center">
          <div class="flex items-baseline gap-0.5">
            <span class="text-2xl font-bold tabular-nums leading-none" :class="memTextColor(memPct)">
              {{ memPct ? memPct.toFixed(1) : '—' }}
            </span>
            <span class="text-xs tabular-nums" :class="memTextColor(memPct)">%</span>
          </div>
          <div class="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-muted/50">
            <span
              class="block h-full rounded-full transition-all"
              :class="memColor(memPct)"
              :style="{ width: memPct + '%' }"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- Kill feedback toast -->
    <div
      v-if="killMsg"
      class="rounded px-2.5 py-1.5 text-xs font-medium"
      :class="killMsg.startsWith('✓') ? 'bg-success/10 text-success' : 'bg-destructive/10 text-destructive'"
    >
      {{ killMsg }}
    </div>

    <!-- Loading skeleton -->
    <div v-if="loading" class="flex flex-col gap-0.5">
      <div v-for="i in 8" :key="i" class="flex h-6 items-center gap-3 rounded px-2">
        <div class="h-2.5 w-32 rounded-sm bg-muted/50 animate-pulse" />
        <div class="h-2.5 w-12 rounded-sm bg-muted/50 animate-pulse" />
        <div class="h-2.5 w-14 rounded-sm bg-muted/50 animate-pulse" />
        <div class="ml-auto h-2.5 w-10 rounded-sm bg-muted/50 animate-pulse" />
      </div>
    </div>

    <!-- Error state -->
    <div
      v-else-if="error"
      class="rounded bg-destructive/10 px-3 py-2 text-xs text-destructive"
    >
      {{ error }}
    </div>

    <!-- Empty states -->
    <div
      v-else-if="!loading && !error && processes.length === 0 && !search"
      class="flex flex-1 items-center justify-center text-xs text-muted-foreground"
    >
      暂无进程数据
    </div>
    <div
      v-else-if="!loading && !error && !search && filtered.length === 0"
      class="flex flex-1 items-center justify-center text-xs text-muted-foreground"
    >
      所有进程运行正常
    </div>
    <div
      v-else-if="!loading && !error && filtered.length === 0"
      class="flex flex-1 items-center justify-center text-xs text-muted-foreground"
    >
      {{ processes.length === 0 ? '暂无进程数据' : '没有匹配的进程' }}
    </div>

    <!-- Process list -->
    <div v-else class="scrollbar-thin flex-1 -mx-1 overflow-auto px-1">
      <div class="flex h-full flex-col">
        <!-- Header -->
        <div class="flex items-center gap-2 rounded bg-muted/30 px-2 py-1 text-[11px] font-medium text-muted-foreground shrink-0">
          <button class="flex-1 text-left hover:text-foreground transition-colors" @click="toggleSort('name')">
            名称 {{ sortIcon('name') }}
          </button>
          <button class="w-14 text-right hover:text-foreground transition-colors" @click="toggleSort('cpu')">
            CPU {{ sortIcon('cpu') }}
          </button>
          <button class="w-16 text-right hover:text-foreground transition-colors" @click="toggleSort('mem_mb')">
            内存 {{ sortIcon('mem_mb') }}
          </button>
          <button class="w-12 text-right hover:text-foreground transition-colors" @click="toggleSort('mem_pct')">
            % {{ sortIcon('mem_pct') }}
          </button>
          <div class="w-5" />
        </div>

        <!-- Rows -->
        <div class="flex-1 space-y-[1px] py-[1px]">
          <div
            v-for="p in filtered"
            :key="p.pid"
            class="group flex items-center gap-2 rounded px-2 py-1 text-xs transition-colors hover:bg-muted/20"
          >
            <span class="flex-1 truncate text-foreground/90" :title="p.name">{{ p.name }}</span>
            <span class="flex w-14 items-center justify-end gap-1.5 tabular-nums">
              <span class="h-1 w-8 overflow-hidden rounded-full bg-muted/50">
                <span
                  class="block h-full rounded-full transition-all"
                  :class="cpuColor(p.cpu)"
                  :style="{ width: Math.min(p.cpu, 100) + '%' }"
                />
              </span>
              <span :class="['text-[11px]', cpuTextColor(p.cpu)]">{{ Math.min(p.cpu, 999).toFixed(1) }}%</span>
            </span>
            <span :class="['w-16 text-right tabular-nums', memTextColor(p.mem_pct)]">
              {{ fmMem(p.mem_mb) }}
            </span>
            <span class="w-12 text-right text-[11px] tabular-nums text-muted-foreground">
              {{ fmPct(p.mem_pct) }}
            </span>
            <div class="flex w-5 items-center justify-center">
              <button
                class="flex h-5 w-5 items-center justify-center rounded-full text-muted-foreground opacity-0 transition-all group-hover:opacity-100 hover:bg-destructive/20 hover:text-destructive focus-visible:opacity-100 max-md:opacity-100"
                :title="`终止 ${p.name}`"
                @click="handleKill(p)"
              >
                <X class="h-3 w-3" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>