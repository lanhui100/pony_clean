import { invoke } from '@tauri-apps/api/core'
import { ref, shallowRef, onMounted, onUnmounted, computed } from 'vue'

export interface Snapshot {
  summary: SystemSummary
  processes: ProcessInfo[]
}
export interface SystemSummary {
  cpu_total: number
  mem_used_mb: number
  mem_total_mb: number
  process_count: number
  disk_used_gb: number
  disk_total_gb: number
}
export interface ProcessInfo {
  pid: number
  name: string
  cpu: number
  mem_mb: number
  status: string
}

const sharedProcesses = shallowRef<ProcessInfo[]>([])
const sharedSummary = ref<SystemSummary | null>(null)
const sharedLoading = ref(true)
const sharedError = ref<string | null>(null)
let sharedTimer: ReturnType<typeof setInterval> | null = null
let sharedRefCount = 0
let hasData = false
let currentInterval = 2000

export function useMonitor() {
  sharedRefCount++

  const processes = sharedProcesses
  const summary = sharedSummary
  const loading = sharedLoading
  const error = sharedError

  const cpuPercent = computed(() => {
    if (!summary.value) return 0
    return Math.round(summary.value.cpu_total)
  })

  const memPercent = computed(() => {
    if (!summary.value || summary.value.mem_total_mb === 0) return 0
    return Math.round((summary.value.mem_used_mb / summary.value.mem_total_mb) * 100)
  })

  const diskPct = computed(() => {
    if (!summary.value || summary.value.disk_total_gb <= 0) return 0
    return (summary.value.disk_used_gb / summary.value.disk_total_gb) * 100
  })

  const diskUsedGb = computed(() => summary.value?.disk_used_gb ?? 0)
  const diskTotalGb = computed(() => summary.value?.disk_total_gb ?? 0)

  async function fetch() {
    try {
      const snap = await invoke<Snapshot>('get_processes')
      summary.value = snap.summary
      processes.value = snap.processes
      loading.value = false
      error.value = null
      hasData = true
    } catch (e) {
      if (hasData) {
        error.value = String(e)
        loading.value = false
      }
    }
  }

  function setPollInterval(ms: number) {
    if (ms === currentInterval) return
    currentInterval = ms
    if (sharedTimer) {
      clearInterval(sharedTimer)
      fetch()
      sharedTimer = setInterval(fetch, ms)
    }
  }

  async function killProcess(pid: number, name: string): Promise<string> {
    try {
      await invoke('kill_process', { pid, name })
      return '✓ 进程已终止'
    } catch (e) {
      return `✗ ${e}`
    }
  }

  function start() {
    if (sharedTimer) return
    fetch()
    sharedTimer = setInterval(fetch, currentInterval)
  }

  function stop() {
    if (sharedRefCount > 1) return
    if (sharedTimer) clearInterval(sharedTimer)
    sharedTimer = null
  }

  onMounted(start)
  onUnmounted(() => {
    sharedRefCount--
    if (sharedRefCount <= 0) {
      if (sharedTimer) clearInterval(sharedTimer)
      sharedTimer = null
    }
  })

  return {
    processes,
    summary,
    loading,
    error,
    cpuPercent,
    memPercent,
    diskPct,
    diskUsedGb,
    diskTotalGb,
    killProcess,
    fetch,
    setPollInterval,
    start,
    stop,
  }
}