<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { X, Search, ChevronUp, ChevronDown } from 'lucide-vue-next'
import { useMonitor, type ProcessInfo } from '../composables/useMonitor'

const { processes, summary, loading, error, killProcess } = useMonitor()

const search = ref('')
const searchInput = ref<HTMLInputElement>()
const sortKey = ref<'name' | 'cpu' | 'mem_mb' | 'mem_pct'>('cpu')
const sortDir = ref<'asc' | 'desc'>('desc')
const killTarget = ref<{ pid: number; name: string } | null>(null)
const killTargetSnapshot = ref<{ pid: number; name: string; exists: boolean } | null>(null)
const killMsg = ref('')
let killTimer: ReturnType<typeof setTimeout> | null = null

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
  return list.toSorted((a, b) => {
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

function cpuThreshold(v: number) {
  if (v < 50) return 'low'
  if (v < 80) return 'mid'
  return 'high'
}

function pcThreshold(pct: number) {
  if (pct < 0.65) return 'low'
  if (pct < 0.85) return 'mid'
  return 'high'
}

function cpuTextColor(v: number) {
  const t = cpuThreshold(v)
  if (t === 'low') return 'text-foreground'
  if (t === 'mid') return 'text-warning'
  return 'text-destructive'
}

function cpuBarColor(v: number) {
  const t = cpuThreshold(v)
  if (t === 'low') return 'bg-success'
  if (t === 'mid') return 'bg-warning'
  return 'bg-destructive'
}

function memTextColor(used: number, total: number) {
  if (total <= 0) return 'text-muted-foreground'
  const t = pcThreshold(used / total)
  if (t === 'low') return 'text-foreground'
  if (t === 'mid') return 'text-warning'
  return 'text-destructive'
}

function fmMem(mb: number) {
  if (!Number.isFinite(mb)) return '—'
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)}GB`
  return `${Math.round(mb)}MB`
}

function fmMemPct(mb: number) {
  if (!summary.value || summary.value.mem_total_mb <= 0) return '—'
  return `${((mb / summary.value.mem_total_mb) * 100).toFixed(1)}%`
}

function openKillDialog(p: { pid: number; name: string }) {
  killTarget.value = { pid: p.pid, name: p.name }
  killTargetSnapshot.value = {
    pid: p.pid,
    name: p.name,
    exists: processes.value.some(proc => proc.pid === p.pid),
  }
}

function scheduleKillClear() {
  if (killTimer) clearTimeout(killTimer)
  killTimer = setTimeout(clearKillState, 2500)
}

async function confirmKill() {
  const snap = killTargetSnapshot.value
  if (!snap) return
  if (!snap.exists) {
    killMsg.value = '进程已结束'
    scheduleKillClear()
    return
  }
  killMsg.value = await killProcess(snap.pid, snap.name)
  scheduleKillClear()
}

function clearKillState() {
  killMsg.value = ''
  killTarget.value = null
  killTargetSnapshot.value = null
}

onUnmounted(() => {
  if (killTimer) clearTimeout(killTimer)
})
</script>

<template>
  <div class="space-y-4">
    <!-- Summary bar -->
    <div class="flex items-center gap-3 rounded-lg border border-border bg-muted/30 px-4 py-2.5 text-sm">
      <span>
        CPU:
        <strong :class="cpuTextColor(summary?.cpu_total ?? 0)" class="tabular-nums">
          {{ summary ? `${summary.cpu_total.toFixed(1)}%` : '—' }}
        </strong>
      </span>
      <span class="text-muted-foreground/40">|</span>
      <span>
        内存:
        <strong :class="memTextColor(summary?.mem_used_mb ?? 0, summary?.mem_total_mb ?? 0)" class="tabular-nums">
          {{ summary && summary.mem_total_mb > 0 ? `${fmMem(summary.mem_used_mb)}/${fmMem(summary.mem_total_mb)} (${((summary.mem_used_mb / summary.mem_total_mb) * 100).toFixed(1)}%)` : summary ? `${fmMem(summary.mem_used_mb)}/${fmMem(summary.mem_total_mb)}` : '—' }}
        </strong>
      </span>
      <span class="text-muted-foreground/40">|</span>
      <span>
        进程:
        <strong class="text-muted-foreground tabular-nums">{{ summary?.process_count ?? '—' }}</strong>
      </span>
    </div>

    <!-- Search -->
    <div class="relative">
      <Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground pointer-events-none" />
      <input
        ref="searchInput"
        v-model="search"
        maxlength="64"
        placeholder="搜索进程..."
        class="h-9 w-full rounded-md border border-input bg-background pl-9 pr-8 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
      />
      <button
        v-if="search.length > 0"
        class="absolute right-2 top-1/2 -translate-y-1/2 flex h-5 w-5 items-center justify-center rounded text-muted-foreground hover:text-foreground transition-colors"
        @click="search = ''; searchInput?.focus()"
      >
        <X class="h-3.5 w-3.5" />
      </button>
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

    <!-- Empty states -->
    <div
      v-else-if="!loading && !error && processes.length === 0 && !search"
      class="flex flex-col items-center justify-center py-12 text-muted-foreground"
    >
      <p class="text-sm">暂无进程数据</p>
    </div>
    <div
      v-else-if="!loading && !error && !search && filtered.length === 0"
      class="flex flex-col items-center justify-center py-12 text-muted-foreground"
    >
      <p class="text-sm">所有进程运行正常</p>
    </div>
    <div
      v-else-if="!loading && !error && filtered.length === 0"
      class="flex flex-col items-center justify-center py-12 text-muted-foreground"
    >
      <p class="text-sm">{{ processes.length === 0 ? '暂无进程数据' : '没有匹配的进程' }}</p>
    </div>

    <!-- Process table -->
    <div v-else class="scrollbar-thin overflow-hidden rounded-lg border border-border">
      <table class="w-full text-sm">
        <thead class="sticky top-0 z-10">
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
            <th class="w-8 px-3 py-2" />
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="p in filtered"
            :key="p.pid"
            class="group border-b border-border transition-colors duration-150 hover:bg-muted/20 even:bg-muted/5"
          >
            <td class="max-w-[160px] truncate px-3 py-1.5 font-medium text-foreground" :title="p.name">
              {{ p.name }}
            </td>
            <td class="px-3 py-1.5 text-right">
              <span class="inline-flex items-center gap-1.5">
                <span class="h-1.5 w-10 overflow-hidden rounded-full bg-muted-foreground/20">
                  <span
                    class="block h-full rounded-full transition-all"
                    :class="cpuBarColor(p.cpu)"
                    :style="{ width: Math.min(p.cpu, 100) + '%' }"
                  />
                </span>
                <span :class="cpuTextColor(p.cpu)">{{ Math.min(p.cpu, 999).toFixed(1) }}%</span>
              </span>
            </td>
            <td class="px-3 py-1.5 text-right tabular-nums" :class="memTextColor(p.mem_mb, summary?.mem_total_mb ?? 0)">
              {{ fmMem(p.mem_mb) }}
            </td>
            <td class="px-3 py-1.5 text-right tabular-nums text-muted-foreground">
              {{ fmMemPct(p.mem_mb) }}
            </td>
            <td class="px-3 py-1.5 text-right">
              <button
                class="inline-flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground opacity-0 transition-all group-hover:opacity-100 focus-visible:opacity-100 hover:bg-destructive/20 hover:text-destructive max-md:opacity-100"
                title="终止进程"
                @click="openKillDialog(p)"
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
        v-if="killTargetSnapshot"
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
        @click.self="killTarget.value = null; killTargetSnapshot = null"
      >
        <div class="w-[90vw] max-w-md rounded-lg border border-border bg-card p-6 shadow-lg">
          <template v-if="killTargetSnapshot.exists">
            <h3 class="text-base font-semibold text-card-foreground">确认终止进程</h3>
            <p class="mt-2 text-sm text-muted-foreground">
              确定要终止进程 <strong class="text-foreground">{{ killTargetSnapshot.name }}</strong>（PID: {{ killTargetSnapshot.pid }}）吗？
            </p>
            <div class="mt-4 flex justify-end gap-2">
              <button
                class="inline-flex h-8 items-center rounded-md border border-border bg-background px-3 text-sm text-foreground transition-colors hover:bg-muted"
                @click="killTarget.value = null; killTargetSnapshot = null"
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
          </template>
          <template v-else>
            <h3 class="text-base font-semibold text-card-foreground">进程已结束</h3>
            <p class="mt-2 text-sm text-muted-foreground">
              该进程已不在当前进程列表中，可能已自然退出或被其他工具终止。
            </p>
            <div class="mt-4 flex justify-end">
              <button
                class="inline-flex h-8 items-center rounded-md border border-border bg-background px-3 text-sm text-foreground transition-colors hover:bg-muted"
                @click="killTarget.value = null; killTargetSnapshot = null"
              >
                关闭
              </button>
            </div>
          </template>
        </div>
      </div>
    </Teleport>
  </div>
</template>
