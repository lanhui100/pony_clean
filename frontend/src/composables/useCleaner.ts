import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ref, onMounted, onUnmounted, shallowRef } from 'vue'

export interface CleanItem {
  path: string
  size_bytes: number
  level: string
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
  const skippedSmall = ref(0)
  const deleteResult = ref<DeleteResult | null>(null)
  const errorMessage = ref('')
  const deleteProgress = ref({ done: 0, total: 0, current: '' })

  let unlistenProgress: UnlistenFn | null = null
  let unlistenItems: UnlistenFn | null = null
  let unlistenDone: UnlistenFn | null = null
  let unlistenError: UnlistenFn | null = null
  let unlistenCancelled: UnlistenFn | null = null
  let unlistenDeleteProgress: UnlistenFn | null = null
  let invokeSeq = 0

  function guardScanning(fn: (e: any) => void) {
    return (e: any) => { if (state.value === 'scanning') fn(e) }
  }

  let listenersReady = false
  let listenersReadyResolve: (() => void) | null = null
  const listenersReadyPromise = new Promise<void>((resolve) => {
    listenersReadyResolve = resolve
  })

  onMounted(() => {
    Promise.all([
      listen<{ scanned: number; current: string }>('scan-progress', guardScanning((e) => {
        scanned.value = e.payload.scanned
        currentFile.value = e.payload.current
      })),
      listen<{ items: CleanItem[]; total_bytes: number }>('scan-items', guardScanning((e) => {
        items.value = items.value.concat(e.payload.items)
      })),
      listen<{ total_items: number; total_bytes: number; skipped_small: number }>('scan-done', guardScanning((e) => {
        state.value = 'done'
        totalBytes.value = e.payload.total_bytes
        skippedSmall.value = e.payload.skipped_small ?? 0
      })),
      listen<{ message: string }>('scan-error', guardScanning((e) => {
        state.value = 'error'
        errorMessage.value = e.payload.message
      })),
      listen('scan-cancelled', guardScanning(() => {
        state.value = 'cancelled'
      })),
      listen<{ done: number; total: number; current: string }>('delete-progress', (e) => {
        if (state.value === 'deleting') {
          deleteProgress.value = e.payload
        }
      }),
    ]).then((listeners) => {
      ;[unlistenProgress, unlistenItems, unlistenDone, unlistenError, unlistenCancelled, unlistenDeleteProgress] = listeners
      listenersReady = true
      listenersReadyResolve?.()
    }).catch((e) => {
      console.error('Failed to register cleaner event listeners:', e)
      listenersReady = true
      listenersReadyResolve?.()
    })
  })

  onUnmounted(() => {
    unlistenProgress?.()
    unlistenItems?.()
    unlistenDone?.()
    unlistenError?.()
    unlistenCancelled?.()
    unlistenDeleteProgress?.()
  })

  async function startScan() {
    state.value = 'scanning'
    scanned.value = 0
    currentFile.value = ''
    items.value = []
    totalBytes.value = 0
    skippedSmall.value = 0
    deleteProgress.value = { done: 0, total: 0, current: '' }
    deleteResult.value = null
    errorMessage.value = ''
    if (!listenersReady) {
      await listenersReadyPromise
    }
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
    } catch (e) {
      console.warn('cancelScan failed:', e)
    }
  }

  async function executeClean(paths: string[]): Promise<DeleteResult> {
    const seq = ++invokeSeq
    state.value = 'deleting'
    deleteProgress.value = { done: 0, total: paths.length, current: '' }
    try {
      const result = await invoke<DeleteResult>('execute_clean', { paths })
      if (seq !== invokeSeq) return result
      deleteResult.value = result
      state.value = 'idle'
      return result
    } catch (e: any) {
      if (seq !== invokeSeq) throw e
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
    skippedSmall.value = 0
    deleteProgress.value = { done: 0, total: 0, current: '' }
    deleteResult.value = null
    errorMessage.value = ''
  }

  return {
    state,
    scanned,
    currentFile,
    items,
    totalBytes,
    skippedSmall,
    deleteProgress,
    deleteResult,
    errorMessage,
    startScan,
    cancelScan,
    executeClean,
    reset,
  }
}
