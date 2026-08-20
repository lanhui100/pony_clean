<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import {
  ScanSearch, RefreshCw, X, Check, AlertCircle, ChevronRight, Loader2, Trash2, TriangleAlert, Zap,
} from 'lucide-vue-next'
import { Button } from '../components/ui/button'
import { Checkbox } from '../components/ui/checkbox'
import { ScrollArea } from '../components/ui/scroll-area'
import { Toast } from '../components/ui/toast'
import {
  Collapsible, CollapsibleTrigger, CollapsibleContent,
} from '../components/ui/collapsible'
import { useCleaner, type CleanItem } from '../composables/useCleaner'
import { useDisk, type LargeFile } from '../composables/useDisk'
import { useMonitor } from '../composables/useMonitor'
import { humanizeErrors } from '../lib/humanizeError'

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
  items,
  skippedSmall,
  deleteProgress,
  deleteResult,
  errorMessage,
  lastCleanBytes,
  justCleaned,
  startScan,
  cancelScan,
  executeClean,
  emptyRecycleBin,
} = useCleaner()

const categoryColors: Record<string, string> = {
  temp: 'bg-cat-temp',
  cache: 'bg-cat-cache',
  logs: 'bg-cat-logs',
  prefetch: 'bg-cat-prefetch',
  recycle_bin: 'bg-cat-recycle',
  old_install: 'bg-cat-old',
  app_cache: 'bg-cat-app',
  dev_cache: 'bg-cat-dev',
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

function groupItems(list: CleanItem[]): CategoryGroup[] {
  const groups: Record<string, CleanItem[]> = {}
  for (const item of list) {
    if (!groups[item.category]) groups[item.category] = []
    groups[item.category].push(item)
  }
  return Object.entries(groups).map(([cat, catItems]) => ({
    category: cat,
    label: categoryLabels[cat] || cat,
    color: categoryColors[cat] || 'bg-cat-gray',
    items: catItems,
    totalBytes: catItems.reduce((sum, i) => sum + i.size_bytes, 0),
  }))
}

/* ── Safe 级：分类行 + 一键清理 ── */

const safeGroups = computed(() =>
  groupItems(items.value.filter((i) => i.level !== 'confirm')),
)

/** 勾选中的 Safe 分类（dev_cache 默认不勾选，consultant 裁决："Safe"≠"删了零成本"） */
const selectedCategories = ref<Set<string>>(new Set())
/** 默认勾选是否已初始化（避免用 size===0 判初始化与「取消全选」空集冲突） */
let categoriesInitialized = false

// 扫描完成后初始化分类默认勾选（除 dev_cache）。
// immediate: true —— 切 tab 重挂载后 items 是模块级保留的，若状态已为 done，
// 本实例的 selectedCategories 为空集，需立即按已有结果重建默认勾选（P1-2 修复）。
watch(garbageState, (s) => {
  if (s === 'done' && !categoriesInitialized && safeGroups.value.length > 0) {
    selectedCategories.value = new Set(
      safeGroups.value.map((g) => g.category).filter((c) => c !== 'dev_cache'),
    )
    categoriesInitialized = true
  }
}, { immediate: true })

function toggleCategory(category: string) {
  const next = new Set(selectedCategories.value)
  if (next.has(category)) next.delete(category)
  else next.add(category)
  selectedCategories.value = next
}

function toggleAllCategories() {
  const all = safeGroups.value.map((g) => g.category)
  const allChecked = all.length > 0 && all.every((c) => selectedCategories.value.has(c))
  selectedCategories.value = allChecked ? new Set() : new Set(all)
}

/** 一键清理的待删路径 = 勾选分类中的全部 Safe 项 */
const oneClickPaths = computed(() =>
  items.value
    .filter((i) => i.level !== 'confirm' && selectedCategories.value.has(i.category))
    .map((i) => i.path),
)

const safeTotalBytes = computed(() =>
  items.value
    .filter((i) => i.level !== 'confirm' && selectedCategories.value.has(i.category))
    .reduce((sum, i) => sum + i.size_bytes, 0),
)

/* ── Confirm 级：折叠「高级」区 ── */

const confirmGroups = computed(() =>
  groupItems(items.value.filter((i) => i.level === 'confirm')),
)

const selectedConfirmPaths = ref(new Set<string>())
const advancedOpen = ref(false)

function toggleConfirmItem(path: string) {
  const next = new Set(selectedConfirmPaths.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  selectedConfirmPaths.value = next
}

function toggleConfirmCategory(category: string) {
  const catItems = confirmGroups.value.find((g) => g.category === category)?.items ?? []
  const allSelected = catItems.length > 0 && catItems.every((i) => selectedConfirmPaths.value.has(i.path))
  const next = new Set(selectedConfirmPaths.value)
  for (const item of catItems) {
    if (allSelected) next.delete(item.path)
    else next.add(item.path)
  }
  selectedConfirmPaths.value = next
}

const selectedConfirmCount = computed(() => selectedConfirmPaths.value.size)

/* ── 清理确认弹窗（Safe 一键 / Confirm 高级共用） ── */

const showConfirmDialog = ref(false)
const pendingClean = ref<Set<string> | null>(null)

const dialogCount = computed(() => pendingClean.value?.size ?? 0)
const dialogBytes = computed(() => {
  let total = 0
  for (const item of items.value) {
    if (pendingClean.value?.has(item.path)) total += item.size_bytes
  }
  return total
})
const dialogBreakdown = computed(() => {
  const list = items.value.filter((i) => pendingClean.value?.has(i.path))
  return groupItems(list).map((g) => ({ ...g, files: g.items.length }))
})
const dialogHasConfirm = computed(() =>
  items.value.some((i) => pendingClean.value?.has(i.path) && i.level === 'confirm'),
)

function handleOneClickClean() {
  if (oneClickPaths.value.length === 0) return
  pendingClean.value = new Set(oneClickPaths.value)
  showConfirmDialog.value = true
}

function handleAdvancedClean() {
  if (selectedConfirmPaths.value.size === 0) return
  pendingClean.value = new Set(selectedConfirmPaths.value)
  showConfirmDialog.value = true
}

function cancelConfirm() {
  showConfirmDialog.value = false
  pendingClean.value = null
}

async function confirmClean() {
  showConfirmDialog.value = false
  const paths = pendingClean.value ? Array.from(pendingClean.value) : []
  pendingClean.value = null
  if (paths.length === 0) return
  // items 过滤已在 useCleaner.executeClean 内完成（置 done 前），此处只清选中态
  await executeClean(paths)
  selectedConfirmPaths.value = new Set()
  if (items.value.length === 0) {
    selectedCategories.value = new Set()
    categoriesInitialized = true
  }
}

/* ── 清空回收站 ── */

const showRecycleDialog = ref(false)
const recycleMsg = ref('')
/** 清空回收站原始错误信息（toast 复制按钮使用） */
const rawRecycleError = ref('')

async function confirmRecycleBin() {
  showRecycleDialog.value = false
  const err = await emptyRecycleBin()
  recycleMsg.value = err === null ? '✓ 回收站已清空' : `✗ ${err.message}`
  rawRecycleError.value = err === null ? '' : err.raw
}

/* ═══════════ 空间分析（useDisk：大文件 + 目录占用） ═══════════ */

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

const analysisOpen = ref(false)

const KIND_COLORS: Record<string, string> = {
  video: 'bg-cat-old',
  archive: 'bg-cat-recycle',
  installer: 'bg-cat-temp',
  image: 'bg-cat-prefetch',
  document: 'bg-cat-cache',
  other: 'bg-cat-gray',
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
const deletingLarge = ref(false)
const deleteMsg = ref('')
/** 删除大文件原始错误信息（toast 复制按钮使用） */
const rawDeleteError = ref('')

function toggleLargeSelect(path: string) {
  const next = new Set(largeSelected.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  largeSelected.value = next
}

function toggleLargeAll() {
  const safe = largeFiles.value.filter((f) => f.level !== 'confirm')
  const allSelected = safe.length > 0 && safe.every((f) => largeSelected.value.has(f.path))
  largeSelected.value = allSelected ? new Set() : new Set(safe.map((f) => f.path))
}

const largeAllSelected = computed(() => {
  const safe = largeFiles.value.filter((f) => f.level !== 'confirm')
  return safe.length > 0 && safe.every((f) => largeSelected.value.has(f.path))
})

/* ── 大文件删除：统一一次确认弹窗（废除 3 秒双点） ── */

const pendingLargeFiles = ref<LargeFile[] | null>(null)

const dialogLargeCount = computed(() => pendingLargeFiles.value?.length ?? 0)
const dialogLargeBytes = computed(() =>
  (pendingLargeFiles.value ?? []).reduce((sum, f) => sum + f.size_bytes, 0),
)
const dialogLargeConfirmCount = computed(() =>
  (pendingLargeFiles.value ?? []).filter((f) => f.level === 'confirm').length,
)

function requestDeleteLarge(files: LargeFile[]) {
  if (files.length === 0) return
  pendingLargeFiles.value = files
}

async function confirmDeleteLarge() {
  const files = pendingLargeFiles.value ?? []
  pendingLargeFiles.value = null
  if (files.length === 0) return
  deletingLarge.value = true
  const result = await deleteFiles(files.map((f) => f.path))
  deletingLarge.value = false
  if (result.failed > 0) {
    const reasons = humanizeErrors(result.errors, 1)
    deleteMsg.value = `✗ 成功 ${result.success} / 失败 ${result.failed}${reasons.length > 0 ? ` · ${reasons[0]}` : ''}`
    rawDeleteError.value = result.errors.join('\n')
  } else {
    deleteMsg.value = `✓ 已删除 ${result.success} 个文件`
    rawDeleteError.value = ''
  }
  largeSelected.value = new Set()
}

function handleDeleteLargeSelected() {
  requestDeleteLarge(largeFiles.value.filter((f) => largeSelected.value.has(f.path)))
}

const maxDirSize = computed(() => {
  const top = dirUsage.value[0]
  return top ? top.size_bytes : 1
})

/* ═══════════ 统一扫描编排 ═══════════ */

function startAllScan() {
  // 重置区块选择
  selectedCategories.value = new Set()
  categoriesInitialized = false
  selectedConfirmPaths.value = new Set()
  largeSelected.value = new Set()
  pendingClean.value = null
  pendingLargeFiles.value = null
  // 并行启动：垃圾扫描（cleaner）+ 用户目录合并扫描（大文件 + 目录占用，单遍历）
  startScan()
  startUserScan()
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

/** 清理结果 toast 摘要文案 */
const cleanToastMsg = computed(() => {
  const r = deleteResult.value
  if (!r) return ''
  let s = r.failed > 0 ? '✗ 清理完成' : '✓ 清理完成'
  s += ` ${r.success} 项成功`
  if (lastCleanBytes.value > 0) s += `，释放约 ${formatBytes(lastCleanBytes.value)}`
  if (r.failed > 0) s += `，${r.failed} 项失败`
  return s
})

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

onUnmounted(() => {})
</script>

<template>
  <div class="relative flex h-full flex-col overflow-hidden">
    <!-- ═══ 顶部：磁盘概况 + 统一扫描控制（无背景框） ═══ -->
    <div class="shrink-0 px-1 pt-3">
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
              空间分析
            </span>
            <span v-if="anyScanning" class="ml-auto inline-flex items-center gap-1 text-primary">
              <Loader2 class="h-3 w-3 animate-spin" />
              扫描中
            </span>
          </div>
        </div>
        <!-- 统一扫描/刷新/取消按钮（极简：仅图标，hover 才显示背景，SPEC-029） -->
        <Button
          v-if="anyScanning"
          size="icon-sm"
          variant="ghost"
          title="取消扫描"
          aria-label="取消扫描"
          @click="cancelAll"
        >
          <X class="h-3.5 w-3.5" />
        </Button>
        <Button
          v-else-if="garbageState === 'deleting'"
          size="icon-sm"
          variant="ghost"
          disabled
          title="清理中"
          aria-label="清理中"
        >
          <Loader2 class="h-3.5 w-3.5 animate-spin" />
        </Button>
        <Button
          v-else
          size="icon-sm"
          variant="ghost"
          :title="hasAnyResult ? '重新扫描' : '开始扫描'"
          :aria-label="hasAnyResult ? '重新扫描' : '开始扫描'"
          @click="startAllScan"
        >
          <RefreshCw v-if="hasAnyResult" class="h-3.5 w-3.5" />
          <ScanSearch v-else class="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>

    <!-- ═══ 区块（无背景框，超出隐藏滚动条） ═══ -->
    <ScrollArea class="scrollbar-none mt-5 flex-1">
      <div class="space-y-8 px-1 pb-2">
        <!-- ─── 区块 1：一键清理（边距分区，无分隔线） ─── -->
        <div>
          <h3 class="text-[11px] font-semibold text-foreground/85">一键清理</h3>

          <!-- 扫描中 -->
          <div v-if="garbageState === 'scanning'" class="flex items-center gap-1.5 py-1.5">
            <Loader2 class="h-3 w-3 shrink-0 animate-spin text-primary" />
            <span class="text-[10px] text-muted-foreground">正在分析垃圾文件（<span class="tabular-nums">{{ scanned }}</span>）</span>
            <span class="min-w-0 flex-1 truncate text-right text-[10px] text-muted-foreground/50">
              {{ currentFile }}
            </span>
          </div>

          <!-- 清理中 -->
          <div v-else-if="garbageState === 'deleting'" class="flex items-center gap-1.5 py-1.5">
            <Loader2 class="h-3 w-3 shrink-0 animate-spin text-primary" />
            <span class="text-[10px] text-muted-foreground">
              清理中 {{ deleteProgress.done }}/{{ deleteProgress.total || '…' }}
            </span>
          </div>

          <!-- 错误（详情在 toast 展示） -->
          <p v-else-if="garbageState === 'error'" class="py-1.5 text-[10px] text-muted-foreground">
            扫描失败，请重新扫描
          </p>

          <!-- 已取消 -->
          <p v-else-if="garbageState === 'cancelled'" class="py-1.5 text-[10px] text-muted-foreground">
            扫描已取消
          </p>

          <!-- 完成（空） -->
          <div v-else-if="garbageState === 'done' && items.length === 0" class="flex items-center gap-1.5 py-1.5 text-[10px] text-muted-foreground">
            <Check class="h-3 w-3 text-success" />
            {{ justCleaned ? '已清理完毕，点击右上角重新扫描查看剩余空间' : '没有发现可清理垃圾' }}
          </div>

          <!-- 完成（有结果）：Safe 分类行 → 「高级」折叠区 → 一键清理主按钮
               （SPEC-029：高级菜单在一键清理按钮上方） -->
          <template v-else-if="garbageState === 'done' && items.length > 0">
            <!-- Safe 分类行 -->
            <template v-if="safeGroups.length > 0">
              <div class="mt-1.5 space-y-0.5">
                <div
                  v-for="group in safeGroups"
                  :key="group.category"
                  class="flex cursor-pointer items-center gap-1.5 rounded px-1 py-1 text-[10px] transition-colors hover:bg-muted/20"
                  @click="toggleCategory(group.category)"
                >
                  <Checkbox
                    :checked="selectedCategories.has(group.category)"
                    @click.stop="toggleCategory(group.category)"
                  />
                  <span :class="['h-1.5 w-1.5 rounded-full shrink-0', group.color]" />
                  <span class="flex-1" :title="group.category === 'dev_cache' ? '清理后需重新下载依赖，默认不勾选' : undefined">{{ group.label }}</span>
                  <span class="tabular-nums text-muted-foreground">{{ formatBytes(group.totalBytes) }}</span>
                </div>
              </div>
            </template>

            <!-- ── 「高级」折叠区：Confirm 级项目（位于一键清理按钮上方） ──
                 轻背景无边框表面，与功能区划（纯边距）区分开，保持入口可辨识 -->
            <Collapsible
              v-if="confirmGroups.length > 0"
              v-slot="{ open }"
              :open="advancedOpen"
              @update:open="(v) => { advancedOpen = v }"
              class="mt-1.5"
            >
              <div class="overflow-hidden rounded-md bg-white/[0.03]">
                <CollapsibleTrigger class="flex w-full items-center gap-1.5 px-1.5 py-1 text-[10px] font-medium text-muted-foreground hover:bg-muted/20 transition-colors">
                  <ChevronRight
                    class="h-3 w-3 shrink-0 transition-transform duration-200"
                    :class="open ? 'rotate-90' : ''"
                  />
                  <TriangleAlert class="h-3 w-3 shrink-0 text-warning" />
                  <span class="flex-1">高级（需确认的项目）</span>
                  <span class="tabular-nums">{{ confirmGroups.reduce((n, g) => n + g.items.length, 0) }} 项</span>
                </CollapsibleTrigger>
                <CollapsibleContent>
                  <div class="space-y-0.5 px-1.5 pb-1">
                    <p class="text-[9px] text-muted-foreground/60">
                      旧下载文件等需谨慎确认的项目，默认不清理
                    </p>
                    <div v-for="group in confirmGroups" :key="group.category" class="py-px">
                      <div
                        class="flex cursor-pointer items-center gap-1.5 rounded px-1 py-1 text-[10px] hover:bg-muted/20"
                        @click="toggleConfirmCategory(group.category)"
                      >
                        <Checkbox
                          :checked="group.items.every(i => selectedConfirmPaths.has(i.path))"
                          :class="group.items.some(i => selectedConfirmPaths.has(i.path)) && !group.items.every(i => selectedConfirmPaths.has(i.path)) ? 'opacity-60' : ''"
                          @click.stop="toggleConfirmCategory(group.category)"
                        />
                        <span :class="['h-1.5 w-1.5 rounded-full shrink-0', group.color]" />
                        <span class="flex-1">{{ group.label }}</span>
                        <span class="tabular-nums text-muted-foreground">{{ formatBytes(group.totalBytes) }}</span>
                      </div>
                      <div
                        v-for="item in group.items"
                        :key="item.path"
                        class="flex cursor-pointer items-center gap-1.5 px-6 py-1 text-[10px] transition-colors hover:bg-muted/15"
                        @click="toggleConfirmItem(item.path)"
                      >
                        <Checkbox
                          :checked="selectedConfirmPaths.has(item.path)"
                          @click.stop="toggleConfirmItem(item.path)"
                        />
                        <span class="flex-1 truncate text-muted-foreground">{{ truncatePath(item.path, 30) }}</span>
                        <span class="shrink-0 tabular-nums text-foreground/70">{{ formatBytes(item.size_bytes) }}</span>
                      </div>
                    </div>
                    <div class="flex justify-end pt-1.5">
                      <Button
                        size="sm"
                        variant="destructive-ghost"
                        class="h-6 px-2"
                        :disabled="selectedConfirmCount === 0"
                        @click="handleAdvancedClean"
                      >
                        <Trash2 class="h-3 w-3" />
                        <span class="ml-1">清理所选（{{ selectedConfirmCount }}）</span>
                      </Button>
                    </div>
                  </div>
                </CollapsibleContent>
              </div>
            </Collapsible>

            <!-- Safe 汇总 -->
            <template v-if="safeGroups.length > 0">
              <div class="mt-2 flex items-center justify-between pt-1">
                <span class="text-[10px] text-muted-foreground">
                  可释放 <span class="font-bold tabular-nums text-foreground/90">{{ formatBytes(safeTotalBytes) }}</span>
                </span>
                <span v-if="skippedSmall > 0" class="text-[9px] text-muted-foreground/50">
                  已忽略 {{ skippedSmall }} 个小文件
                </span>
                <button class="text-[10px] text-primary hover:underline" @click="toggleAllCategories">
                  {{ safeGroups.every(g => selectedCategories.has(g.category)) ? '取消全选' : '全选' }}
                </button>
              </div>
            </template>

            <!-- 清理后无可清理 Safe 项 -->
            <div v-else class="flex items-center gap-1.5 py-1.5 text-[10px] text-muted-foreground">
              <Check class="h-3 w-3 text-success" />
              安全项已清理完毕，点击右上角重新扫描查看剩余空间
            </div>
          </template>

          <!-- 空闲占位 -->
          <p v-else class="py-1.5 text-[10px] text-muted-foreground/70">
            点击右上角扫描，开始分析垃圾文件与空间占用
          </p>

          <!-- 底部操作行：一键清理 + 清空回收站，平分一行 -->
          <div class="mt-2 flex items-stretch gap-2">
            <Button
              class="flex-1"
              size="sm"
              variant="soft"
              :disabled="oneClickPaths.length === 0"
              @click="handleOneClickClean"
            >
              <Zap class="h-3.5 w-3.5" />
              <span class="ml-1">一键清理</span>
            </Button>
            <Button
              class="flex-1"
              size="sm"
              variant="outline"
              :disabled="garbageState === 'scanning' || garbageState === 'deleting'"
              title="清空回收站（需确认）"
              @click="showRecycleDialog = true"
            >
              <Trash2 class="h-3.5 w-3.5" />
              <span class="ml-1">清空回收站</span>
            </Button>
          </div>
        </div>

        <!-- ─── 区块 2：空间分析（默认折叠） ─── -->
        <div>
          <Collapsible v-slot="{ open }" :open="analysisOpen" @update:open="(v) => { analysisOpen = v }">
            <div>
              <CollapsibleTrigger class="flex w-full items-center gap-1.5">
                <ChevronRight
                  class="h-3 w-3 shrink-0 text-muted-foreground transition-transform duration-200"
                  :class="open ? 'rotate-90' : ''"
                />
                <h3 class="text-[11px] font-semibold text-foreground/85">空间分析</h3>
                <span v-if="diskState === 'scanning'" class="flex items-center gap-1 text-[10px] text-muted-foreground">
                  <Loader2 class="h-3 w-3 animate-spin text-primary" />
                  分析中
                </span>
                <span v-else class="text-[9px] text-muted-foreground/50">
                  大文件 · 目录占用
                </span>
              </CollapsibleTrigger>

              <CollapsibleContent>
                <!-- 扫描中 -->
                <div v-if="diskState === 'scanning'" class="flex items-center gap-1.5 py-1">
                  <span class="text-[10px] text-muted-foreground">已扫描 <span class="tabular-nums">{{ diskScanned }}</span> 个文件</span>
                  <span class="min-w-0 flex-1 truncate text-right text-[10px] text-muted-foreground/50">{{ diskCurrent }}</span>
                </div>

                <!-- 错误（详情在 toast 展示） -->
                <p v-else-if="diskState === 'error'" class="py-1 text-[10px] text-muted-foreground">
                  分析失败，请重新扫描
                </p>

                <!-- 大文件 -->
                <template v-if="diskState === 'done' && largeFiles.length > 0">
                  <div class="flex items-center justify-between pt-0.5">
                    <span class="text-[10px] text-muted-foreground">
                      共 {{ largeFiles.length }} 个
                      <span v-if="largeSelected.size > 0" class="text-foreground/80">· 已选 {{ largeSelected.size }}</span>
                    </span>
                    <button class="text-[10px] text-primary hover:underline" @click="toggleLargeAll">
                      {{ largeAllSelected ? '取消全选' : '全选' }}
                    </button>
                  </div>

                  <div class="mt-1 space-y-0.5">
                    <div
                      v-for="f in largeFiles.slice(0, 20)"
                      :key="f.path"
                      class="group flex cursor-pointer items-center gap-1.5 rounded px-1 py-1 transition-colors hover:bg-muted/20"
                      @click="toggleLargeSelect(f.path)"
                    >
                      <span
                        class="h-1.5 w-1.5 shrink-0 rounded-full"
                        :class="largeSelected.has(f.path) ? 'bg-primary' : KIND_COLORS[f.kind] || 'bg-cat-gray'"
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
                        title="删除"
                        @click.stop="requestDeleteLarge([f])"
                      >
                        <Trash2 class="h-3 w-3" />
                      </button>
                    </div>
                    <p v-if="largeFiles.length > 20" class="px-1 text-[9px] text-muted-foreground/50">
                      仅展示最大的 20 个
                    </p>
                  </div>

                  <!-- 底部栏 -->
                  <div class="mt-1.5 flex items-center justify-between pt-1">
                    <span v-if="deletingLarge" class="flex items-center gap-1 text-[10px] text-muted-foreground">
                      <Loader2 class="h-3 w-3 animate-spin" /> 删除中...
                    </span>
                    <span v-else class="text-[9px] text-muted-foreground/50">勾选后统一删除，安装包类需确认</span>
                    <Button
                      size="sm"
                      variant="destructive-ghost"
                      class="h-6 px-2"
                      :disabled="largeSelected.size === 0 || deletingLarge"
                      @click="handleDeleteLargeSelected"
                    >
                      <Trash2 class="h-3 w-3" />
                      <span class="ml-1">删除所选</span>
                    </Button>
                  </div>
                </template>
                <p v-else-if="diskState === 'done' && largeFiles.length === 0" class="py-1 text-[10px] text-muted-foreground">
                  未发现大文件
                </p>

                <!-- 目录占用 -->
                <div v-if="diskState === 'done' && dirUsage.length > 0" class="mt-2 space-y-1 pt-1">
                  <p class="text-[9px] text-muted-foreground/60">占用最多的目录</p>
                  <div v-for="(d, i) in dirUsage.slice(0, 10)" :key="d.path">
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

                <!-- 空闲 -->
                <p v-if="diskState === 'idle'" class="py-1 text-[10px] text-muted-foreground/70">
                  随扫描一并分析用户目录中的大文件与目录占用
                </p>
              </CollapsibleContent>
            </div>
          </Collapsible>
        </div>
      </div>
    </ScrollArea>

    <!-- ═══ 清理确认弹窗（一键 / 高级共用） ═══ -->
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
              <span class="tabular-nums text-foreground/80">{{ formatBytes(group.totalBytes) }}</span>
            </div>
          </div>

          <div
            v-if="dialogHasConfirm"
            class="mt-3 flex items-center gap-1.5 rounded bg-warning/10 px-2 py-1.5 text-[11px] text-warning"
          >
            <AlertCircle class="h-3.5 w-3.5 shrink-0" />
            <span>包含需谨慎确认的项目（如旧下载文件）</span>
          </div>

          <p class="mt-3 text-[11px] text-muted-foreground">
            即将永久删除
            <span class="font-medium text-foreground/80">{{ dialogCount }}</span> 项
            (<span class="font-medium text-foreground/80">{{ formatBytes(dialogBytes) }}</span>)，
            此操作不可撤销。
          </p>

          <div class="mt-4 flex justify-end gap-2">
            <Button variant="outline" size="sm" @click="cancelConfirm">取消</Button>
            <Button variant="destructive" size="sm" @click="confirmClean">确认删除</Button>
          </div>
        </div>
      </div>
    </Transition>

    <!-- ═══ 大文件删除确认弹窗 ═══ -->
    <Transition name="overlay">
      <div
        v-if="pendingLargeFiles"
        class="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm"
      >
        <div class="mx-4 w-full max-w-xs rounded-lg border bg-card p-4 shadow-lg">
          <h4 class="text-sm font-medium">确认删除大文件</h4>

          <div class="mt-3 max-h-36 space-y-0.5 overflow-y-auto">
            <div
              v-for="f in pendingLargeFiles.slice(0, 5)"
              :key="f.path"
              class="flex items-center gap-2 text-[11px]"
            >
              <span :class="['h-2 w-2 rounded-full shrink-0', KIND_COLORS[f.kind] || 'bg-cat-gray']" />
              <span class="truncate text-foreground/90" :title="f.path">{{ fileName(f.path) }}</span>
              <span class="ml-auto shrink-0 tabular-nums text-muted-foreground">{{ formatBytes(f.size_bytes) }}</span>
            </div>
            <p v-if="pendingLargeFiles.length > 5" class="text-[10px] text-muted-foreground">
              等 {{ pendingLargeFiles.length - 5 }} 项
            </p>
          </div>

          <div
            v-if="dialogLargeConfirmCount > 0"
            class="mt-3 flex items-center gap-1.5 rounded bg-warning/10 px-2 py-1.5 text-[11px] text-warning"
          >
            <AlertCircle class="h-3.5 w-3.5 shrink-0" />
            <span>{{ dialogLargeConfirmCount }} 项为安装包/程序或应用数据，可能是正在使用的程序</span>
          </div>

          <p class="mt-3 text-[11px] text-muted-foreground">
            即将永久删除
            <span class="font-medium text-foreground/80">{{ dialogLargeCount }}</span> 个文件
            (<span class="font-medium text-foreground/80">{{ formatBytes(dialogLargeBytes) }}</span>)，
            此操作不可撤销。
          </p>

          <div class="mt-4 flex justify-end gap-2">
            <Button variant="outline" size="sm" @click="pendingLargeFiles = null">取消</Button>
            <Button variant="destructive" size="sm" @click="confirmDeleteLarge">确认删除</Button>
          </div>
        </div>
      </div>
    </Transition>

    <!-- ═══ 清空回收站确认弹窗 ═══ -->
    <Transition name="overlay">
      <div
        v-if="showRecycleDialog"
        class="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm"
      >
        <div class="mx-4 w-full max-w-xs rounded-lg border bg-card p-4 shadow-lg">
          <h4 class="text-sm font-medium">清空回收站</h4>
          <p class="mt-3 text-[11px] text-muted-foreground">
            回收站中所有文件将被永久删除，此操作不可撤销。
          </p>
          <div class="mt-4 flex justify-end gap-2">
            <Button variant="outline" size="sm" @click="showRecycleDialog = false">取消</Button>
            <Button variant="destructive" size="sm" @click="confirmRecycleBin">确认清空</Button>
          </div>
        </div>
      </div>
    </Transition>

    <!-- ═══ 清空回收站结果 toast ═══ -->
    <Toast
      :show="!!recycleMsg"
      :message="recycleMsg"
      :raw-error="rawRecycleError"
      :variant="recycleMsg.startsWith('✓') ? 'success' : 'error'"
      @close="recycleMsg = ''"
    />

    <!-- ═══ 删除大文件结果 toast ═══ -->
    <Toast
      :show="!!deleteMsg"
      :message="deleteMsg"
      :raw-error="rawDeleteError"
      :variant="deleteMsg.startsWith('✓') ? 'success' : 'error'"
      @close="deleteMsg = ''"
    />

    <!-- ═══ 扫描失败 toast ═══ -->
    <Toast
      :show="garbageState === 'error' && !!errorMessage"
      :message="errorMessage || '扫描失败'"
      :raw-error="errorMessage"
      variant="error"
      @close="errorMessage = ''"
    />

    <!-- ═══ 空间分析失败 toast ═══ -->
    <Toast
      :show="diskState === 'error' && !!diskError"
      :message="diskError || '分析失败'"
      :raw-error="diskError"
      variant="error"
      @close="diskError = ''"
    />

    <!-- ═══ 垃圾清理结果 toast ═══ -->
    <Toast
      :show="!!deleteResult"
      :message="cleanToastMsg"
      :raw-error="deleteResult && deleteResult.failed > 0 ? deleteResult.errors.join('\n') : ''"
      :variant="deleteResult && deleteResult.failed > 0 ? 'error' : 'success'"
      @close="deleteResult = null"
    >
      <template v-if="deleteResult && deleteResult.failed > 0 && deleteResult.errors.length > 0">
        <div class="space-y-0.5">
          <p
            v-for="(err, ei) in humanizeErrors(deleteResult.errors)"
            :key="ei"
            class="truncate text-[11px] text-white/80"
            :title="err"
          >
            {{ err }}
          </p>
        </div>
      </template>
    </Toast>
  </div>
</template>

<style scoped>
.overlay-enter-active,
.overlay-leave-active {
  transition: opacity 0.2s ease;
}
.overlay-enter-from,
.overlay-leave-to {
  opacity: 0;
}
</style>
