import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { ref, onMounted } from 'vue'
import { humanizeError } from '../lib/humanizeError'

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

/* ═══════════ 模块级单例状态（TASK-028） ═══════════
 * v-if 重挂载后结果保留；监听器模块级注册一次，扫描中切 tab 不丢关键事件。
 * Vite HMR 会重置本模块状态，dev 下重扫/重启属正常现象。
 */
const state = ref<DiskState>('idle')
const scanned = ref(0)
const current = ref('')
const largeFiles = ref<LargeFile[]>([])
const dirUsage = ref<DirUsage[]>([])
const errorMessage = ref('')

/** 模块级监听器：幂等注册一次（startScan 前 await，消除 onMounted 注册竞态）。
 * 数据事件带 state 守卫（非 scanning 时丢弃，防旧扫描迟到批次混入）；终态事件
 * 无条件 markScanDone（防迟到终态被丢弃导致下一次 startScan 永久等待）。 */
let listenersReadyPromise: Promise<void> | null = null
function ensureListeners(): Promise<void> {
  if (!listenersReadyPromise) {
    listenersReadyPromise = Promise.all([
      listen<{ scanned: number; current: string }>('disk-user-progress', (e) => {
        if (state.value !== 'scanning') return
        scanned.value = e.payload.scanned
        current.value = e.payload.current
      }),
      listen<{ files: LargeFile[] }>('disk-large-files', (e) => {
        if (state.value !== 'scanning') return
        largeFiles.value = largeFiles.value.concat(e.payload.files)
      }),
      listen<{ dirs: DirUsage[] }>('disk-dir-usage', (e) => {
        if (state.value !== 'scanning') return
        dirUsage.value = e.payload.dirs
      }),
      listen('disk-user-done', () => {
        markScanDone()
        if (state.value !== 'scanning') return
        state.value = 'done'
      }),
      listen<{ message: string }>('disk-user-error', (e) => {
        markScanDone()
        if (state.value !== 'scanning') return
        state.value = 'error'
        errorMessage.value = e.payload.message
      }),
    ]).then(() => {
      // 监听器注册完成
    }).catch((e) => {
      console.error('Failed to register disk listeners:', e)
    })
  }
  return listenersReadyPromise
}

/* ── 扫描代际守卫（P1-2）：后端取消后仍会发出 Done/收尾批次。
 * 新扫描必须先等待上一个事件流收尾（done/error），否则旧扫描的迟到批次
 * 或 Done 事件会混入/提前终结新扫描的数据流。 ── */
let activeScanDone: Promise<void> = Promise.resolve()
let resolveActiveScanDone: (() => void) | null = null

function markScanActive() {
  activeScanDone = new Promise((resolve) => {
    resolveActiveScanDone = resolve
  })
}
function markScanDone() {
  resolveActiveScanDone?.()
}

function reset() {
  state.value = 'idle'
  scanned.value = 0
  current.value = ''
  largeFiles.value = []
  dirUsage.value = []
  errorMessage.value = ''
}

/** TASK-028：无参启动——阈值/层数由后端读设置面板配置（clamp 后）决定 */
async function startScan() {
  await ensureListeners()
  // 等上一个扫描事件流收尾，防取消/重扫数据混叠；3s 超时兜底：
  // 后端极端异常不 emit 终态事件时不永久等待，超时后启动新扫描，
  // 后端防重入锁会拒绝并发并给出错误提示（可接受的降级路径）
  await Promise.race([
    activeScanDone,
    new Promise<void>((resolve) => setTimeout(resolve, 3000)),
  ])
  reset()
  state.value = 'scanning'
  markScanActive()
  try {
    await invoke('start_user_scan', {})
  } catch (e) {
    state.value = 'error'
    errorMessage.value = humanizeError(String(e))
    markScanDone()
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
    // 已请求的路径从列表移除（DeleteResult 无逐路径结果，失败项经结果反馈、重扫可重试）
    largeFiles.value = largeFiles.value.filter((f) => !paths.includes(f.path))
    return result
  } catch (e) {
    return { success: 0, failed: paths.length, errors: [humanizeError(String(e))] }
  }
}

/** SpacePanel 专用 composable：返回模块级单例状态，实例仅负责挂载时确保监听器就绪 */
export function useDisk() {
  onMounted(() => {
    ensureListeners()
  })

  return {
    state,
    scanned,
    current,
    largeFiles,
    dirUsage,
    errorMessage,
    startScan,
    cancel,
    deleteFiles,
    reset,
  }
}
