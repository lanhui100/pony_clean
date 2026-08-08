import { invoke } from '@tauri-apps/api/core'
import { ref, shallowRef, onMounted, onUnmounted, computed } from 'vue'
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'

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

export interface TrimResult {
  attempted: number
  success: number
  failed: number
  skipped: number
  freed_mb: number
}

const sharedProcesses = shallowRef<ProcessInfo[]>([])
const sharedSummary = ref<SystemSummary | null>(null)
const sharedLoading = ref(true)
const sharedError = ref<string | null>(null)
let sharedTimer: ReturnType<typeof setInterval> | null = null
let sharedRefCount = 0
let hasData = false
let currentInterval = 2000

// 告警阈值（从后端配置加载，默认 80/85）
let alertCpuPct = 80
let alertMemPct = 85
let alertActive = false
let alertNotified = false

async function loadConfig() {
  try {
    const cfg = await invoke<{ alert_cpu_pct: number; alert_mem_pct: number }>('get_config')
    alertCpuPct = cfg.alert_cpu_pct || 80
    alertMemPct = cfg.alert_mem_pct || 85
  } catch {
    // 使用默认阈值
  }
}

async function maybeSendAlert(summary: SystemSummary) {
  const cpuHigh = summary.cpu_total >= alertCpuPct
  const memHigh = summary.mem_total_mb > 0
    && (summary.mem_used_mb / summary.mem_total_mb) * 100 >= alertMemPct
  alertActive = cpuHigh || memHigh
  if (!alertActive) {
    alertNotified = false
    return
  }
  if (alertNotified) return
  alertNotified = true

  try {
    let granted = await isPermissionGranted()
    if (!granted) granted = (await requestPermission()) === 'granted'
    if (!granted) return
    const parts: string[] = []
    if (cpuHigh) parts.push(`CPU ${summary.cpu_total.toFixed(0)}%`)
    if (memHigh) parts.push(`内存 ${((summary.mem_used_mb / summary.mem_total_mb) * 100).toFixed(0)}%`)
    sendNotification({
      title: 'PonyClean 占用提醒',
      body: `${parts.join(' / ')} 占用过高，建议打开面板查看或清理`,
    })
  } catch {
    // 通知失败静默（dev 环境 toast 可能不可用）
  }
}

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
      maybeSendAlert(snap.summary)
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

  async function trimMemory(): Promise<TrimResult> {
    return invoke<TrimResult>('trim_memory')
  }

  /** 更新告警阈值（设置面板保存后调用） */
  function setAlertThresholds(cpuPct: number, memPct: number) {
    alertCpuPct = cpuPct
    alertMemPct = memPct
  }

  function start() {
    if (sharedTimer) return
    loadConfig().finally(() => {
      fetch()
      sharedTimer = setInterval(fetch, currentInterval)
    })
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
    trimMemory,
    setAlertThresholds,
    fetch,
    setPollInterval,
    start,
    stop,
  }
}