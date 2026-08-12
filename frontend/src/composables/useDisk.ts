import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ref, onMounted, onUnmounted } from 'vue'

export interface LargeFile {
  path: string
  size_bytes: number
  modified_secs: number
  kind: 'video' | 'archive' | 'installer' | 'image' | 'document' | 'other'
  /** 删除风险：installer/AppData 等为 confirm（默认不勾选，需二次确认） */
  level: 'safe' | 'confirm'
}

export interface DirUsage {
  path: string
  size_bytes: number
  file_count: number
}

export interface DeleteResult {
  success: number
  failed: number
  errors: string[]
}

export type DiskState = 'idle' | 'scanning' | 'done' | 'error'

/**
 * 磁盘分析（大文件 + 目录占用，TASK-026）。
 *
 * 后端已合并为单次遍历（`start_user_scan`）：进度/done/error 走 `disk-user-*`，
 * 数据事件 `disk-large-files` / `disk-dir-usage` 分别填充两个区块。
 */
export function useDisk() {
  // ── 合并扫描状态 ──
  const state = ref<DiskState>('idle')
  const scanned = ref(0)
  const current = ref('')
  const largeFiles = ref<LargeFile[]>([])
  const dirUsage = ref<DirUsage[]>([])
  const errorMessage = ref('')
  const deleteResult = ref<DeleteResult | null>(null)

  let unlistenFns: (UnlistenFn | null)[] = []

  onMounted(() => {
    Promise.all([
      listen<{ scanned: number; current: string }>('disk-user-progress', (e) => {
        scanned.value = e.payload.scanned
        current.value = e.payload.current
      }),
      listen<{ files: LargeFile[] }>('disk-large-files', (e) => {
        largeFiles.value = largeFiles.value.concat(e.payload.files)
      }),
      listen<{ dirs: DirUsage[] }>('disk-dir-usage', (e) => {
        dirUsage.value = e.payload.dirs
      }),
      listen('disk-user-done', () => {
        state.value = 'done'
      }),
      listen<{ message: string }>('disk-user-error', (e) => {
        state.value = 'error'
        errorMessage.value = e.payload.message
      }),
    ])
      .then((listeners) => {
        unlistenFns = listeners
      })
      .catch((e) => {
        console.error('Failed to register disk listeners:', e)
      })
  })

  onUnmounted(() => {
    unlistenFns.forEach((fn) => fn?.())
  })

  function reset() {
    state.value = 'idle'
    scanned.value = 0
    current.value = ''
    largeFiles.value = []
    dirUsage.value = []
    errorMessage.value = ''
  }

  async function startScan(minMb: number, maxDepth = 3) {
    reset()
    state.value = 'scanning'
    try {
      await invoke('start_user_scan', { minBytesMb: minMb, maxDepth })
    } catch (e) {
      state.value = 'error'
      errorMessage.value = String(e)
    }
  }

  async function cancel() {
    try {
      await invoke('cancel_disk_scan')
    } catch {
      // 无扫描进行中
    }
  }

  async function deleteFiles(paths: string[]): Promise<DeleteResult> {
    try {
      const result = await invoke<DeleteResult>('delete_large_files', { paths })
      deleteResult.value = result
      // 已处理的路径从列表移除（失败项通过结果 toast 展示，可重新扫描）
      largeFiles.value = largeFiles.value.filter((f) => !paths.includes(f.path))
      return result
    } catch (e) {
      deleteResult.value = { success: 0, failed: paths.length, errors: [String(e)] }
      return deleteResult.value
    }
  }

  return {
    state,
    scanned,
    current,
    largeFiles,
    dirUsage,
    errorMessage,
    deleteResult,
    startScan,
    cancel,
    deleteFiles,
    reset,
  }
}
