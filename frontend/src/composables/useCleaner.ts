import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { computed, ref, onMounted } from 'vue'
import { humanizeError } from '../lib/humanizeError'

/**
 * 后端 `SafetyLevel` 序列化为小写（`#[serde(rename_all = "lowercase")]`）。
 * level 类型化为联合类型后，大写 'Confirm' 比较会触发 TS2367 编译错误，
 * 这是 TASK-028 P0（大小写判断恒真导致一键清理误删 Confirm 级）的编译期回归门禁。
 */
export type SafetyLevel = 'safe' | 'confirm' | 'forbidden'

export interface CleanItem {
  path: string
  size_bytes: number
  level: SafetyLevel
  category: string
  /** 所属扫描目标的中文描述（如「旧驱动备份」），高级区按目标语义分组展示用 */
  label?: string
}

export interface DeleteResult {
  success: number
  failed: number
  errors: string[]
}

export interface CategorySummary {
  files: number
  bytes: number
}

export interface CleanLogEntry {
  timestamp: string
  total_files: number
  total_bytes: number
  success: number
  failed: number
  errors: string[]
  by_category: Record<string, CategorySummary>
}

export interface CleanLogSummary {
  entries: CleanLogEntry[]
}

export interface ScanWarningPayload {
  type: string
  target_id?: string
  items?: number
  path?: string
  pattern?: string
  service?: string
  reason?: string
  count?: number
  first_error?: string
}

export type ScanState = 'idle' | 'scanning' | 'done' | 'cancelled' | 'error' | 'deleting'

interface ScanProgress {
  scanned: number
  current: string
}

const EMPTY_SCAN_PROGRESS: ScanProgress = { scanned: 0, current: '' }

/* ═══════════ 模块级单例状态（TASK-028） ═══════════
 * v-if 重挂载后结果保留；监听器在模块级注册一次（窗口生命周期存活），
 * 扫描中切 tab 不会丢 scan-done 等关键事件导致状态机卡死。
 * 注意：Vite HMR 重新执行本模块会重置状态并重复注册监听器，dev 下重扫/重启属正常现象。
 */
const state = ref<ScanState>('idle')
const items = ref<CleanItem[]>([])
const totalBytes = ref(0)
const skippedSmall = ref(0)
const deleteResult = ref<DeleteResult | null>(null)
const errorMessage = ref('')
const deleteProgress = ref({ done: 0, total: 0, current: '' })
const cleanLogs = ref<CleanLogEntry[]>([])
/** 本次扫描中枚举受限的目标数（enum_errors 告警去重按 target_id），驱动 UI 提示"结果可能偏少" */
const enumWarningTargets = ref<Set<string>>(new Set())

const progress = ref<ScanProgress>(EMPTY_SCAN_PROGRESS)
const scanned = computed(() => progress.value.scanned)
const currentFile = computed(() => progress.value.current)
let lastProgressPush = 0

/** 最近一次清理的待删字节（toast "释放约 X"），模块级：跨实例重挂载不丢 */
const lastCleanBytes = ref(0)
/** 最近一次清理是否已执行（done 空态文案区分"无垃圾"与"已清理完毕"），模块级 */
const justCleaned = ref(false)
let dismissTimer: ReturnType<typeof setTimeout> | null = null

let invokeSeq = 0

/* ── 扫描代际守卫（P1-2）：后端取消后仍会发出 Cancelled/收尾批次。
 * 新扫描必须先等待上一个事件流收尾（done/error/cancelled），否则旧扫描的
 * 迟到批次或终态事件会混入/提前终结新扫描。markScanDone 无条件执行，
 * 防止迟到终态事件被状态守卫丢弃导致下一次 startScan 永久等待。 ── */
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

/** 模块级监听器：幂等注册一次，返回共享 Promise（startScan 前 await，消除注册竞态） */
let listenersReadyPromise: Promise<void> | null = null
function ensureListeners(): Promise<void> {
  if (!listenersReadyPromise) {
    listenersReadyPromise = Promise.all([
      listen<{ scanned: number; current: string }>('scan-progress', (e) => {
        if (state.value !== 'scanning') return
        // 模块级节流（替代 per-instance 缓冲流，避免组件作用域依赖）
        const now = Date.now()
        if (now - lastProgressPush >= 150 || e.payload.scanned === 0) {
          lastProgressPush = now
          progress.value = e.payload
        }
      }),
      listen<{ items: CleanItem[]; total_bytes: number }>('scan-items', (e) => {
        if (state.value !== 'scanning') return
        items.value = items.value.concat(e.payload.items)
      }),
      listen<{ total_items: number; total_bytes: number; skipped_small: number }>('scan-done', (e) => {
        markScanDone()
        if (state.value !== 'scanning') return
        state.value = 'done'
        progress.value = EMPTY_SCAN_PROGRESS
        totalBytes.value = e.payload.total_bytes
        skippedSmall.value = e.payload.skipped_small ?? 0
      }),
      listen<{ message: string }>('scan-error', (e) => {
        markScanDone()
        if (state.value !== 'scanning') return
        state.value = 'error'
        progress.value = EMPTY_SCAN_PROGRESS
        errorMessage.value = e.payload.message
      }),
      listen('scan-cancelled', () => {
        markScanDone()
        if (state.value !== 'scanning') return
        state.value = 'cancelled'
        progress.value = EMPTY_SCAN_PROGRESS
      }),
      listen<{ done: number; total: number; current: string }>('delete-progress', (e) => {
        if (state.value === 'deleting') {
          deleteProgress.value = e.payload
        }
      }),
      listen<ScanWarningPayload>('scan-warning', (e) => {
        console.warn('Scan warning:', e.payload.type, e.payload)
        if (e.payload.type === 'enum_errors' && e.payload.target_id) {
          const next = new Set(enumWarningTargets.value)
          next.add(e.payload.target_id)
          enumWarningTargets.value = next
        }
      }),
    ]).then(() => {
      // 监听器注册完成
    }).catch((e) => {
      console.error('Failed to register cleaner event listeners:', e)
    })
  }
  return listenersReadyPromise
}

async function loadCleanLogs() {
  try {
    const result = await invoke<CleanLogSummary>('get_clean_logs', { limit: 50 })
    cleanLogs.value = result.entries
  } catch (e) {
    console.warn('Failed to load clean logs:', e)
  }
}

async function startScan() {
  await ensureListeners()
  // 等上一个扫描事件流收尾，防取消/重扫数据混叠；3s 超时兜底：
  // 后端极端异常（事件丢失/线程崩溃）不 emit 终态事件时不永久等待，
  // 超时后启动新扫描，后端防重入锁会拒绝并发并给出错误提示（可接受的降级路径）
  await Promise.race([
    activeScanDone,
    new Promise<void>((resolve) => setTimeout(resolve, 3000)),
  ])
  state.value = 'scanning'
  items.value = []
  totalBytes.value = 0
  skippedSmall.value = 0
  deleteProgress.value = { done: 0, total: 0, current: '' }
  deleteResult.value = null
  errorMessage.value = ''
  progress.value = EMPTY_SCAN_PROGRESS
  enumWarningTargets.value = new Set()
  justCleaned.value = false
  markScanActive()
  try {
    await invoke('start_scan')
  } catch (e: any) {
    state.value = 'error'
    errorMessage.value = humanizeError(String(e))
    markScanDone()
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
    // 释放量快照（按待删路径求和），随后立即从列表移除已删路径（在置 done 之前，
    // 避免"state=done 但 items 仍含已删项"的中间渲染帧）
    lastCleanBytes.value = items.value
      .filter((i) => paths.includes(i.path))
      .reduce((sum, i) => sum + i.size_bytes, 0)
    if (result.success > 0) {
      items.value = items.value.filter((i) => !paths.includes(i.path))
      justCleaned.value = true
    }
    // TASK-028 P1-3：清理后回 done（而非 idle），保证 Confirm 区与「重新扫描」CTA 继续渲染；
    // toast 自动关闭定时器模块级，跨实例重挂载不悬挂
    state.value = 'done'
    if (dismissTimer) clearTimeout(dismissTimer)
    dismissTimer = setTimeout(() => {
      deleteResult.value = null
      dismissTimer = null
    }, 5000)
    await loadCleanLogs()
    return result
  } catch (e: any) {
    if (seq !== invokeSeq) throw e
    // 清理 invoke 级失败：不回 'error'（那是扫描失败文案），回 done 保留列表，
    // 以合成 DeleteResult 走 toast 呈现失败，不抛 unhandled rejection
    const fail: DeleteResult = {
      success: 0,
      failed: paths.length,
      errors: [humanizeError(String(e))],
    }
    deleteResult.value = fail
    lastCleanBytes.value = 0
    state.value = 'done'
    if (dismissTimer) clearTimeout(dismissTimer)
    dismissTimer = setTimeout(() => {
      deleteResult.value = null
      dismissTimer = null
    }, 5000)
    return fail
  }
}

/** 清空回收站（TASK-028）：成功返回 null，失败返回中文化错误信息 + 原始错误（供 toast 复制） */
async function emptyRecycleBin(): Promise<{ message: string; raw: string } | null> {
  try {
    await invoke('empty_recycle_bin')
    return null
  } catch (e: any) {
    return { message: humanizeError(String(e)), raw: String(e) }
  }
}

function reset() {
  state.value = 'idle'
  progress.value = EMPTY_SCAN_PROGRESS
  items.value = []
  totalBytes.value = 0
  skippedSmall.value = 0
  deleteProgress.value = { done: 0, total: 0, current: '' }
  deleteResult.value = null
  errorMessage.value = ''
  enumWarningTargets.value = new Set()
}

/** SpacePanel 专用 composable：返回模块级单例状态，实例仅负责挂载时确保监听器就绪 */
export function useCleaner() {
  onMounted(() => {
    ensureListeners()
    loadCleanLogs()
  })

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
    lastCleanBytes,
    justCleaned,
    enumWarningTargets,
    startScan,
    cancelScan,
    executeClean,
    emptyRecycleBin,
    reset,
    cleanLogs,
    loadCleanLogs,
  }
}
