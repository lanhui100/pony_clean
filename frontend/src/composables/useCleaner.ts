import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ref, onMounted, onUnmounted, shallowRef } from 'vue'

export interface CleanItem {
  path: string
  size_bytes: number
  category: string
}

export interface DeleteResult {
  success: number
  failed: number
  errors: string[]
}

export type ScanState = 'idle' | 'scanning' | 'done' | 'cancelled' | 'error' | 'deleting'

export function useCleaner() {
  const state = ref<ScanState>('idle')
  const scanned = ref(0)
  const currentFile = ref('')
  const items = ref<CleanItem[]>([])
  const totalBytes = ref(0)
  const deleteResult = ref<DeleteResult | null>(null)
  const errorMessage = ref('')

  let unlistenProgress: UnlistenFn | null = null
  let unlistenItems: UnlistenFn | null = null
  let unlistenDone: UnlistenFn | null = null
  let unlistenError: UnlistenFn | null = null
  let unlistenCancelled: UnlistenFn | null = null
  let invokeSeq = 0

  function guardScanning(seq: number, fn: (e: any) => void) {
    return (e: any) => { if (state.value === 'scanning' && invokeSeq === seq) fn(e) }
  }

  onMounted(() => {
    const seq = invokeSeq
    Promise.all([
      listen<{ scanned: number; current: string }>('scan-progress', guardScanning(seq, (e) => {
        scanned.value = e.payload.scanned
        currentFile.value = e.payload.current
      })),
      listen<{ items: CleanItem[] }>('scan-items', guardScanning(seq, (e) => {
        items.value = e.payload.items
      })),
      listen<{ total_items: number; total_bytes: number }>('scan-done', guardScanning(seq, (e) => {
        state.value = 'done'
        totalBytes.value = e.payload.total_bytes
      })),
      listen<{ message: string }>('scan-error', guardScanning(seq, (e) => {
        state.value = 'error'
        errorMessage.value = e.payload.message
      })),
      listen('scan-cancelled', guardScanning(seq, () => {
        state.value = 'cancelled'
      })),
    ]).then((listeners) => {
      ;[unlistenProgress, unlistenItems, unlistenDone, unlistenError, unlistenCancelled] = listeners
    }).catch((e) => {
      console.error('Failed to register cleaner event listeners:', e)
    })
    // TODO: checkResumedScan — 需要后端 get_scan_state 命令实现后再启用
  })

  onUnmounted(() => {
    unlistenProgress?.()
    unlistenItems?.()
    unlistenDone?.()
    unlistenError?.()
    unlistenCancelled?.()
  })

  async function startScan() {
    const seq = ++invokeSeq
    state.value = 'scanning'
    scanned.value = 0
    currentFile.value = ''
    items.value = []
    totalBytes.value = 0
    deleteResult.value = null
    errorMessage.value = ''
    try {
      await invoke('start_scan')
    } catch (e: any) {
      if (seq !== invokeSeq) return
      state.value = 'error'
      errorMessage.value = String(e)
    }
  }

  async function cancelScan() {
    try {
      await invoke('cancel_scan')
    } catch (e) {
      console.warn('cancelScan failed:', e)
    }
  }

  async function executeClean(paths: string[]): Promise<DeleteResult> {
    state.value = 'deleting'
    try {
      const result = await invoke<DeleteResult>('execute_clean', { paths })
      deleteResult.value = result
      state.value = 'idle'
      return result
    } catch (e: any) {
      state.value = 'error'
      errorMessage.value = String(e)
      throw e
    }
  }

  function reset() {
    state.value = 'idle'
    scanned.value = 0
    currentFile.value = ''
    items.value = []
    totalBytes.value = 0
    deleteResult.value = null
    errorMessage.value = ''
  }

  return {
    state,
    scanned,
    currentFile,
    items,
    totalBytes,
    deleteResult,
    errorMessage,
    startScan,
    cancelScan,
    executeClean,
    reset,
  }
}
