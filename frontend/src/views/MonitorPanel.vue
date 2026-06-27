<script setup lang="ts">
import { ref, computed } from 'vue'
import { X, Search, ChevronUp, ChevronDown } from 'lucide-vue-next'
import { useMonitor, type ProcessInfo } from '../composables/useMonitor'

const { processes, summary, loading, error, killProcess } = useMonitor()

const search = ref('')
const sortKey = ref<'name' | 'cpu' | 'mem_mb' | 'mem_pct'>('cpu')
const sortDir = ref<'asc' | 'desc'>('desc')
const killTarget = ref<{ pid: number; name: string } | null>(null)
const killMsg = ref('')

const filtered = computed(() => {
  let list = processes.value
  if (!search.value.trim()) {
    list = list.filter(p => p.cpu > 10 || p.mem_mb > 200)
  } else {
    const q = search.value.toLowerCase()
    list = list.filter(p => p.name.toLowerCase().includes(q))
  }
  const key = sortKey.value
  const dir = sortDir.value
  return [...list].sort((a, b) => {
    let va: string | number = a[key]
    let vb: string | number = b[key]
    if (va < vb) return dir === 'asc' ? -1 : 1
    if (va > vb) return dir === 'asc' ? 1 : -1
    return 0
  })
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

function cpuColor(v: number) {
  if (v < 50) return 'text-green-400'
  if (v < 80) return 'text-amber-400'
  return 'text-red-400'
}

function memColor(v: number, total: number) {
  if (total <= 0) return 'text-muted-foreground'
  const pct = v / total
  if (pct < 0.65) return 'text-teal-400'
  if (pct < 0.85) return 'text-amber-400'
  return 'text-red-400'
}

function cpuBarColor(v: number) {
  if (v < 50) return 'bg-green-500'
  if (v < 80) return 'bg-amber-500'
  return 'bg-red-500'
}

function fmMem(mb: number) {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)}GB`
  return `${Math.round(mb)}MB`
}

function fmMemPct(mb: number) {
  if (!summary.value || summary.value.mem_total_mb <= 0) return '—'
  return `${((mb / summary.value.mem_total_mb) * 100).toFixed(1)}%`
}

function summaryCpuColor(v: number) {
  if (v < 50) return 'text-green-400'
  if (v < 80) return 'text-amber-400'
  return 'text-red-400'
}

function summaryMemColor(used: number, total: number) {
  if (total <= 0) return 'text-muted-foreground'
  const pct = used / total
  if (pct < 0.65) return 'text-teal-400'
  if (pct < 0.85) return 'text-amber-400'
  return 'text-red-400'
}

async function confirmKill() {
  if (!killTarget.value) return
  killMsg.value = await killProcess(killTarget.value.pid, killTarget.value.name)
  setTimeout(() => { killMsg.value = ''; killTarget.value = null }, 2500)
}
</script>

<template>
  <div class="space-y-4">
    <!-- Summary bar -->
    <div class="flex items-center gap-3 rounded-lg border border-border bg-card px-4 py-3 text-sm">
      <span>
        CPU:
        <strong :class="summaryCpuColor(summary?.cpu_total ?? 0)">
          {{ summary ? `${summary.cpu_total.toFixed(1)}%` : '—' }}
        </strong>
      </span>
      <span class="text-muted-foreground">|</span>
      <span>
        内存:
        <strong :class="summaryMemColor(summary?.mem_used_mb ?? 0, summary?.mem_total_mb ?? 0)">
          {{ summary ? `${fmMem(summary.mem_used_mb)}/${fmMem(summary.mem_total_mb)}` : '—' }}
        </strong>
      </span>
      <span class="text-muted-foreground">|</span>
      <span>
        进程:
        <strong class="text-muted-foreground">{{ summary?.process_count ?? '—' }}</strong>
      </span>
    </div>

    <!-- Search -->
    <div class="relative">
      <Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
      <input
        v-model="search"
        maxlength="64"
        placeholder="🔍 搜索进程..."
        class="h-9 w-full rounded-md border border-input bg-background pl-9 pr-3 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
      />
    </div>

    <!-- Kill feedback -->
    <div
      v-if="killMsg"
      class="rounded-md border px-3 py-2 text-sm"
      :class="killMsg.startsWith('✓') ? 'border-green-500/30 bg-green-500/10 text-green-400' : 'border-red-500/30 bg-red-500/10 text-red-400'"
    >
      {{ killMsg }}
    </div>

    <!-- Loading skeleton -->
    <div v-if="loading" class="space-y-2">
      <div v-for="i in 6" :key="i" class="flex h-10 animate-pulse items-center gap-4 rounded-md bg-muted px-3">
        <div class="h-3 w-32 rounded bg-muted-foreground/20" />
        <div class="h-3 w-12 rounded bg-muted-foreground/20" />
        <div class="h-3 w-16 rounded bg-muted-foreground/20" />
        <div class="h-3 w-12 rounded bg-muted-foreground/20" />
        <div class="ml-auto h-6 w-6 rounded bg-muted-foreground/20" />
      </div>
    </div>

    <!-- Error state -->
    <div
      v-else-if="error"
      class="rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-400"
    >
      {{ error }}
    </div>

    <!-- Empty state -->
    <div
      v-else-if="filtered.length === 0"
      class="flex flex-col items-center justify-center py-12 text-muted-foreground"
    >
      <p class="text-sm">没有匹配的进程</p>
    </div>

    <!-- Process table -->
    <div v-else class="overflow-hidden rounded-lg border border-border">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b border-border bg-muted/50 text-muted-foreground">
            <th
              class="cursor-pointer px-3 py-2 text-left font-medium hover:text-foreground"
              @click="toggleSort('name')"
            >
              名称 {{ sortIcon('name') }}
            </th>
            <th
              class="cursor-pointer px-3 py-2 text-right font-medium hover:text-foreground"
              @click="toggleSort('cpu')"
            >
              CPU% {{ sortIcon('cpu') }}
            </th>
            <th
              class="cursor-pointer px-3 py-2 text-right font-medium hover:text-foreground"
              @click="toggleSort('mem_mb')"
            >
              内存 {{ sortIcon('mem_mb') }}
            </th>
            <th
              class="cursor-pointer px-3 py-2 text-right font-medium hover:text-foreground"
              @click="toggleSort('mem_pct')"
            >
              Mem% {{ sortIcon('mem_pct') }}
            </th>
            <th class="w-10 px-3 py-2" />
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="p in filtered"
            :key="p.pid"
            class="border-b border-border transition-colors hover:bg-muted/30"
          >
            <td class="max-w-[200px] truncate px-3 py-2 font-medium text-foreground" :title="p.name">
              {{ p.name }}
            </td>
            <td class="px-3 py-2 text-right">
              <span class="inline-flex items-center gap-1.5">
                <span class="h-1.5 w-10 overflow-hidden rounded-full bg-muted-foreground/20">
                  <span
                    class="block h-full rounded-full transition-all"
                    :class="cpuBarColor(p.cpu)"
                    :style="{ width: Math.min(p.cpu, 100) + '%' }"
                  />
                </span>
                <span :class="cpuColor(p.cpu)">{{ p.cpu.toFixed(1) }}%</span>
              </span>
            </td>
            <td class="px-3 py-2 text-right tabular-nums" :class="memColor(p.mem_mb, summary?.mem_total_mb ?? 0)">
              {{ fmMem(p.mem_mb) }}
            </td>
            <td class="px-3 py-2 text-right tabular-nums text-muted-foreground">
              {{ fmMemPct(p.mem_mb) }}
            </td>
            <td class="px-3 py-2 text-right">
              <button
                class="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
                title="终止进程"
                @click="killTarget = { pid: p.pid, name: p.name }"
              >
                <X class="h-3.5 w-3.5" />
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Kill confirmation dialog -->
    <Teleport to="body">
      <div
        v-if="killTarget"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
        @click.self="killTarget = null"
      >
        <div class="w-[90vw] max-w-md rounded-lg border border-border bg-card p-6 shadow-lg">
          <h3 class="text-base font-semibold text-card-foreground">确认终止进程</h3>
          <p class="mt-2 text-sm text-muted-foreground">
            确定要终止进程 <strong class="text-foreground">{{ killTarget.name }}</strong>（PID: {{ killTarget.pid }}）吗？
          </p>
          <div class="mt-4 flex justify-end gap-2">
            <button
              class="inline-flex h-8 items-center rounded-md border border-border bg-background px-3 text-sm text-foreground transition-colors hover:bg-muted"
              @click="killTarget = null"
            >
              取消
            </button>
            <button
              class="inline-flex h-8 items-center rounded-md bg-destructive px-3 text-sm text-destructive-foreground transition-colors hover:bg-destructive/90"
              @click="confirmKill"
            >
              终止
            </button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>
