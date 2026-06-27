import { invoke } from '@tauri-apps/api/core'
import { ref, shallowRef, onMounted, onUnmounted } from 'vue'

export interface Snapshot {
  summary: SystemSummary
  processes: ProcessInfo[]
}
export interface SystemSummary {
  cpu_total: number
  mem_used_mb: number
  mem_total_mb: number
  process_count: number
}
export interface ProcessInfo {
  pid: number
  name: string
  cpu: number
  mem_mb: number
  status: string
}

export function useMonitor() {
  const processes = shallowRef<ProcessInfo[]>([])
  const summary = ref<SystemSummary | null>(null)
  const loading = ref(true)
  const error = ref<string | null>(null)
  let timer: ReturnType<typeof setInterval> | null = null

  async function fetch() {
    try {
      const snap = await invoke<Snapshot>('get_processes')
      summary.value = snap.summary
      processes.value = snap.processes
      loading.value = false
      error.value = null
    } catch (e) {
      error.value = String(e)
      loading.value = false
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

  onMounted(() => { fetch(); timer = setInterval(fetch, 2000) })
  onUnmounted(() => { if (timer) clearInterval(timer) })

  return { processes, summary, loading, error, killProcess }
}
