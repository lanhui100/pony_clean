<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import {
  ScanSearch, RefreshCw, X, Check, AlertCircle, ChevronRight, Loader2, Trash2, TriangleAlert, Zap,
} from 'lucide-vue-next'
import { Button } from '../components/ui/button'
import { Alert, AlertDescription } from '../components/ui/alert'
import { Checkbox } from '../components/ui/checkbox'
import { ScrollArea } from '../components/ui/scroll-area'
import {
  Collapsible, CollapsibleTrigger, CollapsibleContent,
} from '../components/ui/collapsible'
import { useCleaner, type CleanItem } from '../composables/useCleaner'
import { useDisk, type LargeFile } from '../composables/useDisk'
import { useMonitor } from '../composables/useMonitor'

const emit = defineEmits<{
  (e: 'scan-start'): void
  (e: 'scan-end'): void
}>()

/* ═══════════ 磁盘概况 ═══════════ */

const { summary } = useMonitor()

function diskThreshold(pct: number) {
  if (pct < 65) return 'low'
  if (pct < 85) return 'mid'
  return 'high'
}
function diskColor(v: number) {
  const t = diskThreshold(v)
  if (t === 'low') return 'bg-success'
  if (t === 'mid') return 'bg-warning'
  return 'bg-destructive'
}
function diskTextColor(v: number) {
  const t = diskThreshold(v)
  if (t === 'low') return 'text-success'
  if (t === 'mid') return 'text-warning'
  return 'text-destructive'
}
const diskPct = computed(() => {
  const s = summary.value
  if (!s || s.disk_total_gb <= 0) return 0
  return (s.disk_used_gb / s.disk_total_gb) * 100
})

/* ═══════════ 垃圾清理（useCleaner） ═══════════ */

const {
  state: garbageState,
  scanned,
  currentFile,
  progressRevision,
  items,
  totalBytes,
  skippedSmall,
  deleteProgress,
  deleteResult,
  errorMessage,
  startScan,
  cancelScan,
  executeClean,
} = useCleaner()

const categoryColors: Record<string, string> = {
  temp: 'bg-blue-400',
  cache: 'bg-purple-400',
  logs: 'bg-amber-400',
  prefetch: 'bg-green-400',
  recycle_bin: 'bg-amber-500',
  old_install: 'bg-red-400',
  app_cache: 'bg-pink-400',
  dev_cache: 'bg-indigo-400',
}

const categoryLabels: Record<string, string> = {
  temp: '临时文件',
  cache: '浏览器缓存',
  logs: '日志与报告',
  prefetch: 'Prefetch',
  recycle_bin: '回收站',
  old_install: '旧系统安装',
  app_cache: '应用缓存',
  dev_cache: '开发工具缓存',
}

interface CategoryGroup {
  category: string
  label: string
  color: string
  items: CleanItem[]
  totalBytes: number
}

const groupedItems = computed<CategoryGroup[]>(() => {
  const groups: Record<string, CleanItem[]> = {}
  for (const item of items.value) {
    if (!groups[item.category]) groups[item.category] = []
    groups[item.category].push(item)
  }
  return Object.entries(groups).map(([cat, catItems]) => ({
    category: cat,
    label: categoryLabels[cat] || cat,
    color: categoryColors[cat] || 'bg-gray-400',
    items: catItems,
    totalBytes: catItems.reduce((sum, i) => sum + i.size_bytes, 0),
  }))
})

const selectedPaths = ref(new Set<string>())
const openCategories = ref(new Set<string>())

function toggleItem(path: string) {
  const next = new Set(selectedPaths.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  selectedPaths.value = next
}

function toggleCategory(category: string) {
  const catItems = items.value.filter(i => i.category === category)
  const allSelected = catItems.every(i => selectedPaths.value.has(i.path))
  const next = new Set(selectedPaths.value)
  for (const item of catItems) {
    if (allSelected) next.delete(item.path)
    else next.add(item.path)
  }
  selectedPaths.value = next
}

function toggleAll() {
  const safeItems = items.value.filter(i => i.level !== 'Confirm')
  if (selectedPaths.value.size === safeItems.length) {
    selectedPaths.value = new Set()
  } else {
    selectedPaths.value = new Set(safeItems.map(i => i.path))
  }
}

function isCategoryFullySelected(category: string) {
  const catItems = items.value.filter(i => i.category === category)
  return catItems.length > 0 && catItems.every(i => selectedPaths.value.has(i.path))
}

function isCategoryPartiallySelected(category: string) {
  const catItems = items.value.filter(i => i.category === category)
  const count = catItems.filter(i => selectedPaths.value.has(i.path)).length
  return count > 0 && count < catItems.length
}

const selectedCount = computed(() => selectedPaths.value.size)
const selectedBytes = computed(() => {
  let total = 0
  for (const item of items.value) {
    if (selectedPaths.value.has(item.path)) total += item.size_bytes
  }
  return total
})
const allSelected = computed(() => {
  const safeItems = items.value.filter(i => i.level !== 'Confirm')
  return safeItems.length > 0 && selectedPaths.value.size === safeItems.length
})

const selectedCategoryBreakdown = computed(() => {
  const groups: Record<string, { files: number; bytes: number }> = {}
  for (const item of items.value) {
    if (selectedPaths.value.has(item.path)) {
      if (!groups[item.category]) groups[item.category] = { files: 0, bytes: 0 }
      groups[item.category].files++
      groups[item.category].bytes += item.size_bytes
    }
  }
  return Object.entries(groups).map(([cat, data]) => ({
    category: cat,
    label: categoryLabels[cat] || cat,
    color: categoryColors[cat] || 'bg-gray-400',
    ...data,
  }))
})

const hasDelayedDelete = computed(() => {
  return items.value.some(i => selectedPaths.value.has(i.path) && i.level === 'Confirm')
})

const showConfirmDialog = ref(false)

/** 确认弹窗的待删集合（普通清理 = 勾选态快照；一键清理 = Safe 级，TASK-024） */
const pendingClean = ref<Set<string> | null>(null)

/** 弹窗数据源：待删集合（不依赖全局勾选态） */
const dialogCount = computed(() => pendingClean.value?.size ?? 0)
const dialogBytes = computed(() => {
  let total = 0
  for (const item of items.value) {
    if (pendingClean.value?.has(item.path)) total += item.size_bytes
  }
  return total
})
const dialogBreakdown = computed(() => {
  const groups: Record<string, { files: number; bytes: number }> = {}
  for (const item of items.value) {
    if (pendingClean.value?.has(item.path)) {
      if (!groups[item.category]) groups[item.category] = { files: 0, bytes: 0 }
      groups[item.category].files++
      groups[item.category].bytes += item.size_bytes
    }
  }
  return Object.entries(groups).map(([cat, data]) => ({
    category: cat,
    label: categoryLabels[cat] || cat,
    color: categoryColors[cat] || 'bg-gray-400',
    ...data,
  }))
})
const dialogHasDelayed = computed(() =>
  items.value.some(i => pendingClean.value?.has(i.path) && i.level === 'Confirm'),
)

/** Safe 级可清理项（一键清理仅覆盖这些） */
const safeItems = computed(() => items.value.filter(i => i.level !== 'Confirm'))
/** 最近一次清理的待删字节（toast 展示"释放约 X"） */
const lastCleanBytes = ref(0)

function handleClean() {
  if (selectedPaths.value.size === 0) return
  pendingClean.value = new Set(selectedPaths.value)
  showConfirmDialog.value = true
}

/** 一键清理：仅 Safe 级，不修改用户勾选态 */
function handleOneClickClean() {
  const safe = safeItems.value.map(i => i.path)
  if (safe.length === 0) return
  pendingClean.value = new Set(safe)
  showConfirmDialog.value = true
}

async function confirmClean() {
  showConfirmDialog.value = false
  const paths = pendingClean.value ? Array.from(pendingClean.value) : []
  const bytesSnapshot = dialogBytes.value
  pendingClean.value = null
  if (paths.length === 0) return
  await executeClean(paths)
  lastCleanBytes.value = bytesSnapshot
  selectedPaths.value = new Set()
}

/* ═══════════ 大文件 + 目录（useDisk） ═══════════ */

const {
  state: diskState,
  scanned: diskScanned,
  current: diskCurrent,
  largeFiles,
  dirUsage,
  errorMessage: diskError,
  startScan: startUserScan,
  cancel: cancelDisk,
  deleteFiles,
} = useDisk()

const minMb = ref(100)
const MIN_OPTIONS = [
  { value: 100, label: '≥ 100 MB' },
  { value: 500, label: '≥ 500 MB' },
  { value: 1000, label: '≥ 1 GB' },
]

const KIND_COLORS: Record<string, string> = {
  video: 'bg-red-400',
  archive: 'bg-amber-400',
  installer: 'bg-blue-400',
  image: 'bg-green-400',
  document: 'bg-purple-400',
  other: 'bg-gray-400',
}

const KIND_LABELS: Record<string, string> = {
  video: '视频',
  archive: '压缩包',
  installer: '安装包',
  image: '图片',
  document: '文档',
  other: '其他',
}

const largeSelected = ref(new Set<string>())
const confirmPaths = ref(new Set<string>())
const batchConfirm = ref(false)
const deleting = ref(false)
const deleteMsg = ref('')
let confirmTimer: ReturnType<typeof setTimeout> | null = null
let deleteMsgTimer: ReturnType<typeof setTimeout> | null = null

function toggleLargeSelect(path: string) {
  const next = new Set(largeSelected.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  largeSelected.value = next
}

function toggleLargeAll() {
  // 全选仅覆盖安全级（installer / AppData 等高风险默认不勾选）
  const safe = largeFiles.value.filter((f) => f.level !== 'confirm')
  largeSelected.value = largeSelected.value.size === safe.length
    ? new Set()
    : new Set(safe.map((f) => f.path))
}

const largeAllSelected = computed(() => {
  const safe = largeFiles.value.filter((f) => f.level !== 'confirm')
  return safe.length > 0 && largeSelected.value.size === safe.length
})

/** 选中项中高风险（installer / AppData）数量 */
const largeConfirmSelected = computed(() =>
  largeFiles.value.filter((f) => largeSelected.value.has(f.path) && f.level === 'confirm').length,
)

async function handleDeleteLarge(files: LargeFile[]) {
  const paths = files.map((f) => f.path)
  if (confirmPaths.value.has(paths[0])) {
    // 二次确认
    confirmPaths.value = new Set()
    deleting.value = true
    const result = await deleteFiles(paths)
    deleting.value = false
    deleteMsg.value = result.failed > 0
      ? `✗ 成功 ${result.success} / 失败 ${result.failed}`
      : `✓ 已删除 ${result.success} 个文件`
    if (deleteMsgTimer) clearTimeout(deleteMsgTimer)
    deleteMsgTimer = setTimeout(() => { deleteMsg.value = '' }, 3000)
    largeSelected.value = new Set()
  } else {
    confirmPaths.value = new Set(paths)
    if (confirmTimer) clearTimeout(confirmTimer)
    confirmTimer = setTimeout(() => { confirmPaths.value = new Set() }, 3000)
  }
}

/** 大文件底部批量删除：首次点击进入确认态（按钮变色），再次点击执行 */
async function handleCleanLargeSelected() {
  const files = largeFiles.value.filter((f) => largeSelected.value.has(f.path))
  if (files.length === 0) return
  if (batchConfirm.value) {
    batchConfirm.value = false
    await handleDeleteLarge(files)
  } else {
    batchConfirm.value = true
    if (confirmTimer) clearTimeout(confirmTimer)
    confirmTimer = setTimeout(() => { batchConfirm.value = false }, 3000)
  }
}

const maxDirSize = computed(() => {
  const top = dirUsage.value[0]
  return top ? top.size_bytes : 1
})

/* ═══════════ 统一扫描编排 ═══════════ */

function startAllScan() {
  // 重置所有区块选择
  selectedPaths.value = new Set()
  openCategories.value = new Set()
  largeSelected.value = new Set()
  confirmPaths.value = new Set()
  batchConfirm.value = false
  // 并行启动：垃圾扫描（cleaner）+ 用户目录合并扫描（大文件 + 目录占用，单遍历）
  startScan()
  startUserScan(minMb.value, 3)
}

function cancelAll() {
  cancelScan()
  cancelDisk()
}

/** 任一扫描/清理进行中（驱动胶囊状态） */
const isBusy = computed(() =>
  garbageState.value === 'scanning'
  || garbageState.value === 'deleting'
  || diskState.value === 'scanning',
)

/** 任一扫描进行中（顶部按钮 → 取消） */
const anyScanning = computed(() =>
  garbageState.value === 'scanning'
  || diskState.value === 'scanning',
)

/** 任一区块已有结果（顶部按钮 → 刷新/重试） */
const hasAnyResult = computed(() =>
  garbageState.value === 'done' || garbageState.value === 'error' || garbageState.value === 'cancelled'
  || diskState.value === 'done' || diskState.value === 'error',
)

watch(isBusy, (val, prev) => {
  if (val && !prev) emit('scan-start')
  if (!val && prev) emit('scan-end')
})

function statusDot(state: string) {
  if (state === 'done' || state === 'deleting') return 'bg-success'
  if (state === 'scanning') return 'bg-primary animate-pulse'
  if (state === 'error') return 'bg-destructive'
  return 'bg-white/20'
}

/* ═══════════ 工具函数 ═══════════ */

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), 3)
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function fmtDate(secs: number): string {
  if (!secs) return '—'
  const d = new Date(secs * 1000)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function fileName(path: string): string {
  const parts = path.split(/[\\/]/)
  return parts[parts.length - 1] || path
}

function truncatePath(path: string, maxLen = 38): string {
  if (path.length <= maxLen) return path
  return '...' + path.slice(-(maxLen - 3))
}

/* ═══════════ 定时器清理 ═══════════ */

let dismissTimer: ReturnType<typeof setTimeout> | null = null
watch(deleteResult, (val) => {
  if (dismissTimer) clearTimeout(dismissTimer)
  if (val) {
    dismissTimer = setTimeout(() => {
      deleteResult.value = null
      dismissTimer = null
    }, 5000)
  }
})

onUnmounted(() => {
  if (dismissTimer) clearTimeout(dismissTimer)
  if (confirmTimer) clearTimeout(confirmTimer)
  if (deleteMsgTimer) clearTimeout(deleteMsgTimer)
})
</script>

<template>
  <div class="relative flex h-full flex-col overflow-hidden">
    <!-- ═══ 顶部：磁盘概况 + 统一扫描控制（无背景框） ═══ -->
    <div class="shrink-0 px-1 pt-1">
      <div class="flex items-center gap-2">
        <div class="min-w-0 flex-1">
          <div class="flex items-baseline gap-1.5">
            <span class="text-xl font-bold tabular-nums leading-none" :class="diskTextColor(diskPct)">
              {{ diskPct.toFixed(0) }}
            </span>
            <span class="text-[10px] text-muted-foreground">% 已用</span>
            <span class="ml-auto text-[10px] tabular-nums text-muted-foreground">
              {{ summary?.disk_used_gb.toFixed(0) }}G / {{ summary?.disk_total_gb.toFixed(0) }}G
            </span>
          </div>
          <div class="mt-1 h-1 overflow-hidden rounded-full bg-white/15">
            <span
              class="block h-full rounded-full transition-all"
              :class="diskColor(diskPct)"
              :style="{ width: Math.min(diskPct, 100) + '%' }"
            />
          </div>
          <div class="mt-1 flex items-center gap-2.5 text-[10px] text-muted-foreground">
            <span class="inline-flex items-center gap-1">
              <span class="h-1.5 w-1.5 rounded-full" :class="statusDot(garbageState)" />
              垃圾
            </span>
            <span class="inline-flex items-center gap-1">
              <span class="h-1.5 w-1.5 rounded-full" :class="statusDot(diskState)" />
              大文件
            </span>
            <span class="inline-flex items-center gap-1">
              <span class="h-1.5 w-1.5 rounded-full" :class="statusDot(diskState)" />
              目录
            </span>
            <span v-if="anyScanning" class="ml-auto inline-flex items-center gap-1 text-primary">
              <Loader2 class="h-3 w-3 animate-spin" />
              扫描中
            </span>
          </div>
        </div>
        <!-- 统一扫描/刷新/取消按钮 -->
        <Button v-if="anyScanning" size="sm" variant="outline" title="取消扫描" @click="cancelAll">
          <X class="h-3.5 w-3.5" />
        </Button>
        <Button v-else-if="garbageState === 'deleting'" size="sm" disabled title="清理中">
          <Loader2 class="h-3.5 w-3.5 animate-spin" />
        </Button>
        <Button v-else size="sm" :title="hasAnyResult ? '重新扫描' : '开始扫描'" @click="startAllScan">
          <RefreshCw v-if="hasAnyResult" class="h-3.5 w-3.5" />
          <ScanSearch v-else class="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>

    <!-- ═══ 三区块（排版区隔，无背景框，超出隐藏滚动条） ═══ -->
    <ScrollArea class="scrollbar-none mt-1.5 flex-1">
      <div class="space-y-2.5 px-1 pb-2">
        <!-- ─── 区块 1：可清理垃圾 ─── -->
        <div>
          <div class="flex items-center gap-1.5 border-t border-white/10 pt-1.5">
            <h3 class="text-[11px] font-semibold text-foreground/85">可清理垃圾</h3>
            <span class="text-[10px] text-muted-foreground/60">8 类</span>
          </div>

          <!-- 扫描中 -->
          <div v-if="garbageState === 'scanning'" class="flex items-center gap-1.5 py-1">
            <Loader2 class="h-3 w-3 shrink-0 animate-spin text-primary" />
            <span class="text-[10px] text-muted-foreground">已扫描 <span class="tabular-nums">{{ scanned }}</span> 个文件</span>
            <span class="min-w-0 flex-1 truncate text-right text-[10px] text-muted-foreground/50">
              {{ currentFile }}
            </span>
          </div>

          <!-- 清理中 -->
          <div v-else-if="garbageState === 'deleting'" class="flex items-center gap-1.5 py-1">
            <Loader2 class="h-3 w-3 shrink-0 animate-spin text-primary" />
            <span class="text-[10px] text-muted-foreground">
              清理中 {{ deleteProgress.done }}/{{ deleteProgress.total || '…' }}
            </span>
          </div>

          <!-- 错误 -->
          <p v-else-if="garbageState === 'error'" class="py-1 text-[10px] text-destructive">
            扫描失败：{{ errorMessage || '未知错误' }}
          </p>

          <!-- 已取消 -->
          <p v-else-if="garbageState === 'cancelled'" class="py-1 text-[10px] text-muted-foreground">
            扫描已取消
          </p>

          <!-- 完成（空） -->
          <div v-else-if="garbageState === 'done' && items.length === 0" class="flex items-center gap-1.5 py-1 text-[10px] text-muted-foreground">
            <Check class="h-3 w-3 text-success" />
            没有发现可清理垃圾
          </div>

          <!-- 完成（有结果） -->
          <template v-else-if="garbageState === 'done' && items.length > 0">
            <div class="flex items-center justify-between pt-0.5">
              <p class="text-[10px] text-muted-foreground">
                可清理 <span class="font-bold tabular-nums text-foreground/90">{{ formatBytes(totalBytes) }}</span>
              </p>
              <span v-if="skippedSmall > 0" class="text-[9px] text-muted-foreground/50">
                跳过 {{ skippedSmall }} 个微效
              </span>
            </div>

            <!-- 分类图例 -->
            <div class="mt-0.5 flex flex-wrap gap-x-2.5 gap-y-0.5 text-[9px] text-muted-foreground">
              <span v-for="group in groupedItems" :key="group.category" class="inline-flex items-center gap-1">
                <span :class="['h-1.5 w-1.5 rounded-full shrink-0', group.color]" />
                {{ group.label }}
                <span class="tabular-nums text-foreground/70">{{ formatBytes(group.totalBytes) }}</span>
              </span>
            </div>

            <!-- 分类折叠列表 -->
            <div class="mt-0.5 space-y-px">
              <Collapsible
                v-for="group in groupedItems"
                :key="group.category"
                v-slot="{ open }"
                :open="openCategories.has(group.category)"
                @update:open="(v) => { const n = new Set(openCategories); v ? n.add(group.category) : n.delete(group.category); openCategories = n }"
              >
                <div class="overflow-hidden rounded">
                  <CollapsibleTrigger class="flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-[10px] font-medium hover:bg-muted/20 transition-colors">
                    <ChevronRight
                      class="h-3 w-3 shrink-0 text-muted-foreground transition-transform duration-200"
                      :class="open ? 'rotate-90' : ''"
                    />
                    <Checkbox
                      :checked="isCategoryFullySelected(group.category)"
                      :class="isCategoryPartiallySelected(group.category) ? 'opacity-60' : ''"
                      @click.stop="toggleCategory(group.category)"
                    />
                    <span :class="['h-1.5 w-1.5 rounded-full shrink-0', group.color]" />
                    <span class="flex-1">{{ group.label }}</span>
                    <span class="tabular-nums text-muted-foreground">{{ formatBytes(group.totalBytes) }}</span>
                  </CollapsibleTrigger>
                  <CollapsibleContent>
                    <div class="py-px">
                      <label
                        v-for="item in group.items"
                        :key="item.path"
                        class="flex cursor-pointer items-center gap-1.5 px-5 py-0.5 text-[10px] transition-colors hover:bg-muted/15"
                      >
                        <Checkbox
                          :checked="selectedPaths.has(item.path)"
                          @update:checked="toggleItem(item.path)"
                        />
                        <span class="flex-1 truncate text-muted-foreground">{{ truncatePath(item.path, 32) }}</span>
                        <span class="shrink-0 tabular-nums text-foreground/70">{{ formatBytes(item.size_bytes) }}</span>
                      </label>
                    </div>
                  </CollapsibleContent>
                </div>
              </Collapsible>
            </div>

            <!-- 底部操作栏 -->
            <div class="mt-0.5 flex items-center justify-between border-t border-white/5 pt-1">
              <span class="text-[10px] text-muted-foreground">
                <template v-if="selectedCount > 0">
                  已选 <span class="font-medium text-foreground/80">{{ selectedCount }}</span> 项
                  <span class="text-muted-foreground">({{ formatBytes(selectedBytes) }})</span>
                </template>
                <template v-else>未选择文件</template>
              </span>
              <button class="text-[10px] text-primary hover:underline" @click="toggleAll">
                {{ allSelected ? '取消全选' : '全选' }}
              </button>
              <Button
                variant="outline"
                size="sm"
                class="h-6 px-2"
                :disabled="safeItems.length === 0"
                title="一键清理安全项（跳过勾选）"
                @click="handleOneClickClean"
              >
                <Zap class="h-3 w-3" />
              </Button>
              <Button
                variant="destructive"
                size="sm"
                class="h-6 px-2"
                :disabled="selectedCount === 0"
                title="清理选中"
                @click="handleClean"
              >
                <Trash2 class="h-3 w-3" />
              </Button>
            </div>
          </template>

          <!-- 空闲占位 -->
          <p v-else class="py-1 text-[10px] text-muted-foreground/70">
            点击右上角扫描，检测 8 类系统垃圾
          </p>
        </div>

        <!-- ─── 区块 2：大文件 ─── -->
        <div>
          <div class="flex items-center gap-1.5 border-t border-white/10 pt-1.5">
            <h3 class="text-[11px] font-semibold text-foreground/85">大文件</h3>
            <div class="ml-auto flex gap-0.5">
              <button
                v-for="opt in MIN_OPTIONS"
                :key="opt.value"
                type="button"
                :disabled="diskState === 'scanning'"
                class="rounded border px-1.5 py-0.5 text-[10px] transition-colors disabled:opacity-50"
                :class="minMb === opt.value
                  ? 'border-primary/50 bg-primary/15 text-primary'
                  : 'border-white/10 text-muted-foreground hover:border-white/25'"
                @click="minMb = opt.value"
              >
                {{ opt.label }}
              </button>
            </div>
          </div>

          <!-- 扫描中 -->
          <div v-if="diskState === 'scanning'" class="flex items-center gap-1.5 py-1">
            <Loader2 class="h-3 w-3 shrink-0 animate-spin text-primary" />
            <span class="text-[10px] text-muted-foreground">已扫描 <span class="tabular-nums">{{ diskScanned }}</span> 个文件</span>
            <span class="min-w-0 flex-1 truncate text-right text-[10px] text-muted-foreground/50">{{ diskCurrent }}</span>
          </div>

          <!-- 错误 -->
          <p v-else-if="diskState === 'error'" class="py-1 text-[10px] text-destructive">
            扫描失败：{{ diskError }}
          </p>

          <!-- 完成（空） -->
          <p v-else-if="diskState === 'done' && largeFiles.length === 0" class="py-1 text-[10px] text-muted-foreground">
            未找到大于 {{ minMb }} MB 的文件
          </p>

          <!-- 完成（有结果） -->
          <template v-else-if="diskState === 'done' && largeFiles.length > 0">
            <div class="flex items-center justify-between pt-0.5">
              <span class="text-[10px] text-muted-foreground">
                共 {{ largeFiles.length }} 个
                <span v-if="largeSelected.size > 0" class="text-foreground/80">· 已选 {{ largeSelected.size }}</span>
              </span>
              <button class="text-[10px] text-primary hover:underline" @click="toggleLargeAll">
                {{ largeAllSelected ? '取消全选' : '全选' }}
              </button>
            </div>

            <div class="mt-0.5 space-y-px">
              <div
                v-for="f in largeFiles"
                :key="f.path"
                class="group flex cursor-pointer items-center gap-1.5 rounded px-1 py-0.5 transition-colors hover:bg-muted/20"
                @click="toggleLargeSelect(f.path)"
              >
                <span
                  class="h-1.5 w-1.5 shrink-0 rounded-full"
                  :class="largeSelected.has(f.path) ? 'bg-primary' : KIND_COLORS[f.kind] || 'bg-gray-400'"
                  :title="KIND_LABELS[f.kind] || f.kind"
                />
                <div class="min-w-0 flex-1">
                  <p class="truncate text-[10px] text-foreground/90" :title="f.path">{{ fileName(f.path) }}</p>
                  <p class="truncate text-[9px] text-muted-foreground/60">{{ truncatePath(f.path, 28) }} · {{ fmtDate(f.modified_secs) }}</p>
                </div>
                <span class="shrink-0 tabular-nums text-[10px] text-foreground/70">{{ formatBytes(f.size_bytes) }}</span>
                <TriangleAlert
                  v-if="f.level === 'confirm'"
                  class="h-3 w-3 shrink-0 text-warning"
                  title="安装包/程序文件或应用数据，删除前请确认"
                />
                <button
                  class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-muted-foreground/60 opacity-50 transition-all group-hover:opacity-100 hover:bg-destructive/20 hover:text-destructive"
                  :class="confirmPaths.has(f.path) ? 'bg-destructive text-destructive-foreground opacity-100' : ''"
                  :title="confirmPaths.has(f.path) ? '再次点击确认删除' : '删除'"
                  @click.stop="handleDeleteLarge([f])"
                >
                  <Trash2 v-if="!confirmPaths.has(f.path)" class="h-3 w-3" />
                  <Check v-else class="h-3 w-3" />
                </button>
              </div>
            </div>

            <!-- 底部栏 -->
            <div class="mt-0.5 flex items-center justify-between border-t border-white/5 pt-1">
              <span v-if="deleteMsg" class="text-[10px]" :class="deleteMsg.startsWith('✓') ? 'text-success' : 'text-destructive'">
                {{ deleteMsg }}
              </span>
              <span v-else-if="deleting" class="flex items-center gap-1 text-[10px] text-muted-foreground">
                <Loader2 class="h-3 w-3 animate-spin" /> 删除中...
              </span>
              <span v-else-if="largeConfirmSelected > 0" class="text-[9px] text-warning">
                已选含 {{ largeConfirmSelected }} 项高风险
              </span>
              <span v-else class="text-[9px] text-muted-foreground/50">删除会记录审计日志</span>
              <Button
                size="sm"
                variant="destructive"
                class="h-6 px-2"
                :disabled="largeSelected.size === 0 || deleting"
                :class="batchConfirm ? 'ring-2 ring-destructive/50' : ''"
                title="删除选中"
                @click="handleCleanLargeSelected"
              >
                <template v-if="batchConfirm">
                  <Check class="h-3 w-3" />
                  <span class="ml-1">确认删除？</span>
                </template>
                <template v-else>
                  <Trash2 class="h-3 w-3" />
                </template>
              </Button>
            </div>
          </template>

          <!-- 空闲占位 -->
          <p v-else class="py-1 text-[10px] text-muted-foreground/70">
            点击扫描，定位用户目录中的大文件
          </p>
        </div>

        <!-- ─── 区块 3：目录占用 ─── -->
        <div>
          <div class="flex items-center gap-1.5 border-t border-white/10 pt-1.5">
            <h3 class="text-[11px] font-semibold text-foreground/85">目录占用</h3>
            <span class="ml-auto text-[9px] text-muted-foreground/50">用户目录 · 3 层</span>
          </div>

          <!-- 扫描中 -->
          <div v-if="diskState === 'scanning'" class="flex items-center gap-1.5 py-1">
            <Loader2 class="h-3 w-3 shrink-0 animate-spin text-primary" />
            <span class="text-[10px] text-muted-foreground">已扫描 <span class="tabular-nums">{{ diskScanned }}</span> 个文件</span>
            <span class="min-w-0 flex-1 truncate text-right text-[10px] text-muted-foreground/50">{{ diskCurrent }}</span>
          </div>

          <!-- 错误 -->
          <p v-else-if="diskState === 'error'" class="py-1 text-[10px] text-destructive">
            扫描失败：{{ diskError }}
          </p>

          <!-- 完成（空） -->
          <p v-else-if="diskState === 'done' && dirUsage.length === 0" class="py-1 text-[10px] text-muted-foreground">
            暂无数据
          </p>

          <!-- 完成（有结果） -->
          <div v-else-if="diskState === 'done'" class="space-y-1 pt-0.5">
            <div v-for="(d, i) in dirUsage.slice(0, 12)" :key="d.path">
              <div class="flex items-center justify-between gap-2 text-[10px]">
                <span class="truncate text-foreground/90" :title="d.path">{{ truncatePath(d.path, 28) }}</span>
                <span class="shrink-0 tabular-nums text-foreground/70">{{ formatBytes(d.size_bytes) }}</span>
              </div>
              <div class="mt-0.5 h-0.5 overflow-hidden rounded-full bg-white/10">
                <div
                  class="h-full rounded-full transition-all"
                  :class="i === 0 ? 'bg-destructive' : i < 3 ? 'bg-warning' : 'bg-primary/60'"
                  :style="{ width: Math.max((d.size_bytes / maxDirSize) * 100, 2) + '%' }"
                />
              </div>
            </div>
          </div>

          <!-- 空闲占位 -->
          <p v-else class="py-1 text-[10px] text-muted-foreground/70">
            点击扫描，分析各目录空间占用
          </p>
        </div>
      </div>
    </ScrollArea>

    <!-- ═══ 垃圾清理确认弹窗 ═══ -->
    <Transition name="overlay">
      <div
        v-if="showConfirmDialog"
        class="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm"
      >
        <div class="mx-4 w-full max-w-xs rounded-lg border bg-card p-4 shadow-lg">
          <h4 class="text-sm font-medium">确认清理</h4>

          <div v-if="dialogBreakdown.length > 0" class="mt-3 space-y-1">
            <div
              v-for="group in dialogBreakdown"
              :key="group.category"
              class="flex items-center gap-2 text-[11px]"
            >
              <span :class="['h-2 w-2 rounded-full shrink-0', group.color]" />
              <span class="text-muted-foreground">{{ group.label }}</span>
              <span class="ml-auto text-foreground/80">{{ group.files }} 项</span>
              <span class="tabular-nums text-foreground/80">{{ formatBytes(group.bytes) }}</span>
            </div>
          </div>

          <div
            v-if="dialogHasDelayed"
            class="mt-3 flex items-center gap-1.5 rounded bg-warning/10 px-2 py-1.5 text-[11px] text-warning"
          >
            <AlertCircle class="h-3.5 w-3.5 shrink-0" />
            <span>部分文件需重启系统后删除</span>
          </div>

          <p class="mt-3 text-[11px] text-muted-foreground">
            即将永久删除
            <span class="font-medium text-foreground/80">{{ dialogCount }}</span> 项
            (<span class="font-medium text-foreground/80">{{ formatBytes(dialogBytes) }}</span>)，
            此操作不可撤销。
          </p>

          <div class="mt-4 flex justify-end gap-2">
            <Button variant="outline" size="sm" @click="showConfirmDialog = false">取消</Button>
            <Button variant="destructive" size="sm" @click="confirmClean">确认删除</Button>
          </div>
        </div>
      </div>
    </Transition>

    <!-- ═══ 垃圾清理结果 toast ═══ -->
    <Transition name="toast">
      <div v-if="deleteResult" class="absolute bottom-0 left-0 right-0 z-10">
        <Alert
          :variant="deleteResult.failed > 0 ? 'destructive' : 'default'"
          class="border-0 shadow-lg"
        >
          <div class="flex items-start gap-2">
            <Check v-if="deleteResult.failed === 0" class="mt-0.5 h-4 w-4 shrink-0 text-green-500" />
            <AlertCircle v-else class="mt-0.5 h-4 w-4 shrink-0" />
            <div class="min-w-0 flex-1">
              <p class="text-xs">
                清理完成
                <span class="font-medium text-success">{{ deleteResult.success }}</span> 项成功
                <template v-if="lastCleanBytes > 0">
                  ，释放约 <span class="font-medium text-success">{{ formatBytes(lastCleanBytes) }}</span>
                </template>
                <template v-if="deleteResult.failed > 0">
                  ，<span class="font-medium text-destructive">{{ deleteResult.failed }}</span> 项失败
                </template>
              </p>
              <template v-if="deleteResult.failed > 0 && deleteResult.errors.length > 0">
                <div class="mt-1 space-y-0.5">
                  <p
                    v-for="(err, ei) in deleteResult.errors.slice(0, 5)"
                    :key="ei"
                    class="truncate text-[11px] text-destructive/80"
                    :title="err"
                  >
                    {{ err }}
                  </p>
                  <p v-if="deleteResult.errors.length > 5" class="text-[11px] text-muted-foreground">
                    等 {{ deleteResult.errors.length - 5 }} 项
                  </p>
                </div>
              </template>
            </div>
            <button
              class="flex h-4 w-4 shrink-0 items-center justify-center rounded text-muted-foreground hover:text-foreground transition-colors"
              @click="deleteResult = null"
            >
              <X class="h-3 w-3" />
            </button>
          </div>
        </Alert>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.toast-enter-active {
  transition: all 0.3s ease-out;
}
.toast-leave-active {
  transition: all 0.3s ease-in;
}
.toast-enter-from {
  transform: translateY(20px);
  opacity: 0;
}
.toast-leave-to {
  transform: translateY(10px);
  opacity: 0;
}
.overlay-enter-active,
.overlay-leave-active {
  transition: opacity 0.2s ease;
}
.overlay-enter-from,
.overlay-leave-to {
  opacity: 0;
}
</style>
