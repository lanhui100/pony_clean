import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ref, onMounted, onUnmounted } from 'vue'

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

  onMounted(async () => {
    unlistenProgress = await listen<{ scanned: number; current: string }>('scan-progress', (e) => {
      scanned.value = e.payload.scanned
      currentFile.value = e.payload.current
    })
    unlistenItems = await listen<{ items: CleanItem[] }>('scan-items', (e) => {
      items.value = e.payload.items
    })
    unlistenDone = await listen<{ total_items: number; total_bytes: number }>('scan-done', (e) => {
      state.value = 'done'
      totalBytes.value = e.payload.total_bytes
    })
    unlistenError = await listen<{ message: string }>('scan-error', (e) => {
      state.value = 'error'
      errorMessage.value = e.payload.message
    })
    unlistenCancelled = await listen('scan-cancelled', () => {
      state.value = 'cancelled'
    })
  })

  onUnmounted(() => {
    unlistenProgress?.()
    unlistenItems?.()
    unlistenDone?.()
    unlistenError?.()
    unlistenCancelled?.()
  })

  async function startScan() {
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
      state.value = 'error'
      errorMessage.value = String(e)
    }
  }

  async function cancelScan() {
    try {
      await invoke('cancel_scan')
    } catch {
      // fallback: ignore if command doesn't exist yet
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
