<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { X, Cpu, MemoryStick } from 'lucide-vue-next'
import { useMonitor, type ProcessInfo } from '../composables/useMonitor'

const { processes, summary, loading, error, killProcess } = useMonitor()

const props = defineProps<{
  search: string
}>()

const sortKey = ref<'name' | 'cpu' | 'mem_mb' | 'mem_pct'>('cpu')
const sortDir = ref<'asc' | 'desc'>('desc')
const killMsg = ref('')
let killTimer: ReturnType<typeof setTimeout> | null = null
const confirmPid = ref<number | null>(null)
let confirmTimer: ReturnType<typeof setTimeout> | null = null
const killingPids = ref<Record<number, boolean>>({})

interface ProcessRow extends ProcessInfo {
  mem_pct: number
}

const filtered = computed(() => {
  const total = summary.value?.mem_total_mb ?? 0
  let list: ProcessRow[] = processes.value.map(p => ({
    ...p,
    mem_pct: total > 0 ? (p.mem_mb / total) * 100 : 0,
  }))
  if (!props.search.trim()) {
    list = list.filter(p => p.cpu > 10 || p.mem_mb > 200)
  } else {
    const q = props.search.toLowerCase()
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
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)}G`
  return `${Math.round(mb)}M`
}

function fmPct(v: number) {
  if (v <= 0) return '—'
  return `${v.toFixed(1)}%`
}

async function handleKill(p: ProcessRow) {
  if (confirmPid.value === p.pid) {
    if (confirmTimer) clearTimeout(confirmTimer)
    confirmTimer = null
    killMsg.value = ''
    killingPids.value = { ...killingPids.value, [p.pid]: true }
    confirmPid.value = null
    const exists = processes.value.some(proc => proc.pid === p.pid)
    if (!exists) {
      killMsg.value = '✓ 进程已结束'
      scheduleKillClear()
      return
    }
    killMsg.value = await killProcess(p.pid, p.name)
    scheduleKillClear()
  } else {
    confirmPid.value = p.pid
    confirmTimer = setTimeout(() => { confirmPid.value = null }, 3000)
  }
}

function scheduleKillClear() {
  if (killTimer) clearTimeout(killTimer)
  killTimer = setTimeout(() => { killMsg.value = '' }, 2500)
}

onUnmounted(() => {
  if (killTimer) clearTimeout(killTimer)
  if (confirmTimer) clearTimeout(confirmTimer)
})
</script>

<template>
  <div class="relative flex h-full flex-col gap-3">
    <!-- Summary bar -->
    <div class="flex items-center justify-center gap-6">
            <!-- CPU -->
      <div class="flex flex-1 min-w-0 items-center justify-center gap-4 rounded-xl px-5 py-2.5">
        <Cpu class="h-5 w-5 shrink-0 text-muted-foreground" />
        <div class="flex flex-col items-center">
          <div class="flex items-baseline gap-0.5">
            <span class="text-xl font-bold tabular-nums leading-none" :class="cpuTextColor(summary?.cpu_total ?? 0)">
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
      <div class="flex flex-1 min-w-0 items-center justify-center gap-4 rounded-xl px-5 py-2.5">
        <MemoryStick class="h-5 w-5 shrink-0 text-muted-foreground" />
        <div class="flex flex-col items-center">
          <div class="flex items-baseline gap-0.5">
            <span class="text-xl font-bold tabular-nums leading-none" :class="memTextColor(memPct)">
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

    <!-- Kill feedback toast — bottom floating -->
    <Transition name="kill-fade">
      <div
        v-if="killMsg"
        class="absolute bottom-0 left-1/2 z-10 -translate-x-1/2 rounded px-2.5 py-1.5 text-xs font-medium"
        :class="killMsg.startsWith('✓') ? 'bg-success/30 text-success' : 'bg-destructive/30 text-destructive'"
      >
        {{ killMsg }}
      </div>
    </Transition>

    <!-- Loading skeleton -->
    <div v-if="loading" class="flex flex-col gap-0.5">
      <div v-for="i in 8" :key="i" class="flex h-6 items-center gap-3 rounded px-2">
        <div class="h-2.5 w-32 rounded-sm bg-muted/50 animate-pulse" />
        <div class="h-2.5 w-12 rounded-sm bg-muted/50 animate-pulse" />
        <div class="h-2.5 w-20 rounded-sm bg-muted/50 animate-pulse" />
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
    <div v-else class="flex flex-1 flex-col -mx-1 px-1">
      <!-- Header -->
      <div class="flex shrink-0 items-center gap-2 rounded px-2 py-1 text-[11px] font-medium text-muted-foreground">
        <button class="flex-[6] text-left hover:text-foreground transition-colors" @click="toggleSort('name')">
          进程 TOP {{ filtered.length }} {{ sortIcon('name') }}
        </button>
        <button class="flex-[4] text-left hover:text-foreground transition-colors" @click="toggleSort('cpu')">
          CPU {{ sortIcon('cpu') }}
        </button>
        <button class="flex-[5] text-center hover:text-foreground transition-colors" @click="toggleSort('mem_mb')">
          内存 {{ summary ? (summary.mem_total_mb / 1024).toFixed(1) : '—' }}G {{ sortIcon('mem_mb') }}
        </button>
        <div class="w-5" />
      </div>

      <!-- Rows -->
      <div class="scrollbar-none flex-1 overflow-auto py-[1px] mt-0.5">
        <div class="space-y-[1px]">
          <div
            v-for="p in filtered"
            :key="p.pid"
            class="group flex items-center gap-2 rounded px-2 py-1 text-xs transition-colors hover:bg-muted/20"
          >
            <span class="flex-[6] truncate text-foreground/90" :title="p.name">{{ p.name }}</span>
            <span class="flex flex-[4] items-center justify-start gap-1.5 tabular-nums">
              <span class="h-1 w-8 overflow-hidden rounded-full bg-muted/50">
                <span
                  class="block h-full rounded-full transition-all"
                  :class="cpuColor(p.cpu)"
                  :style="{ width: Math.min(p.cpu, 100) + '%' }"
                />
              </span>
              <span :class="['text-[11px]', cpuTextColor(p.cpu)]">{{ Math.min(p.cpu, 999).toFixed(1) }}%</span>
            </span>
            <span :class="['flex-[5] text-center tabular-nums whitespace-nowrap', memTextColor(p.mem_pct)]">
              {{ fmMem(p.mem_mb) }} <span class="text-[11px] text-muted-foreground">{{ fmPct(p.mem_pct) }}</span>
            </span>
            <div v-if="!killingPids[p.pid]" class="flex w-5 shrink-0 items-center justify-center">
              <Transition name="btn-swap" mode="out-in">
                <button
                  v-if="confirmPid !== p.pid"
                  key="kill"
                  class="flex h-4 w-4 cursor-pointer items-center justify-center rounded-full text-muted-foreground opacity-0 transition-all group-hover:opacity-100 hover:bg-destructive/20 hover:text-destructive focus-visible:opacity-100"
                  :title="`终止 ${p.name}`"
                  @click="handleKill(p)"
                >
                  <X class="h-2.5 w-2.5" />
                </button>
                <button
                  v-else
                  key="confirm"
                   class="flex h-4 w-4 cursor-pointer items-center justify-center rounded-full bg-destructive text-[11px] font-bold leading-none text-destructive-foreground opacity-0 transition-all group-hover:opacity-100 hover:bg-destructive/90 focus-visible:opacity-100"
                  title="再次点击确认终止"
                  @click="handleKill(p)"
                >
                  ?
                </button>
              </Transition>
            </div>
            <div v-else class="w-5 shrink-0" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.kill-fade-enter-active,
.kill-fade-leave-active {
  transition: opacity 0.25s ease;
}
.kill-fade-enter-from,
.kill-fade-leave-to {
  opacity: 0;
}

.btn-swap-enter-active,
.btn-swap-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.btn-swap-enter-from,
.btn-swap-leave-to {
  opacity: 0;
  transform: scale(0.85);
}
</style>