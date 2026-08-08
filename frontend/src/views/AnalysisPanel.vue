<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { FileSearch, FolderSearch, Loader2, RotateCcw, Trash2, X, Check, AlertCircle } from 'lucide-vue-next'
import { Button } from '../components/ui/button'
import { Progress } from '../components/ui/progress'
import OptionPicker from '../components/OptionPicker.vue'
import { useDisk, type LargeFile } from '../composables/useDisk'

const {
  state,
  scanned,
  current,
  largeFiles,
  dirUsage,
  errorMessage,
  deleteResult,
  startLargeScan,
  startDirScan,
  cancel,
  deleteFiles,
} = useDisk()

const view = ref<'files' | 'dirs'>('files')
const minMb = ref(100)
const selected = ref(new Set<string>())
const confirmPaths = ref(new Set<string>())
const batchConfirm = ref(false)
let confirmTimer: ReturnType<typeof setTimeout> | null = null
const deleting = ref(false)
const deleteMsg = ref('')
let deleteMsgTimer: ReturnType<typeof setTimeout> | null = null

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

function fmtSize(bytes: number): string {
  if (bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), 4)
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

function truncatePath(path: string, maxLen = 52): string {
  if (path.length <= maxLen) return path
  return '...' + path.slice(-(maxLen - 3))
}

const maxDirSize = computed(() => {
  const top = dirUsage.value[0]
  return top ? top.size_bytes : 1
})

function toggleSelect(path: string) {
  const next = new Set(selected.value)
  if (next.has(path)) next.delete(path)
  else next.add(path)
  selected.value = next
}

function toggleAll() {
  selected.value = selected.value.size === largeFiles.value.length
    ? new Set()
    : new Set(largeFiles.value.map((f) => f.path))
}

const allSelected = computed(() =>
  largeFiles.value.length > 0 && selected.value.size === largeFiles.value.length,
)

async function handleDelete(files: LargeFile[]) {
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
    selected.value = new Set()
  } else {
    confirmPaths.value = new Set(paths)
    if (confirmTimer) clearTimeout(confirmTimer)
    confirmTimer = setTimeout(() => { confirmPaths.value = new Set() }, 3000)
  }
}

/** 底部批量删除：首次点击进入确认态（按钮变色），再次点击执行 */
async function handleCleanSelected() {
  const files = largeFiles.value.filter((f) => selected.value.has(f.path))
  if (files.length === 0) return
  if (batchConfirm.value) {
    batchConfirm.value = false
    await handleDelete(files)
  } else {
    batchConfirm.value = true
    if (confirmTimer) clearTimeout(confirmTimer)
    confirmTimer = setTimeout(() => { batchConfirm.value = false }, 3000)
  }
}

onUnmounted(() => {
  if (confirmTimer) clearTimeout(confirmTimer)
  if (deleteMsgTimer) clearTimeout(deleteMsgTimer)
})
</script>

<template>
  <div class="flex h-full flex-col gap-2">
    <!-- View switch -->
    <div class="flex gap-1 rounded-lg bg-white/5 p-0.5">
      <button
        class="flex flex-1 items-center justify-center gap-1 rounded-md py-1 text-[11px] transition-colors"
        :class="view === 'files' ? 'bg-primary/15 text-primary' : 'text-muted-foreground hover:text-foreground'"
        @click="view = 'files'"
      >
        <FileSearch class="h-3 w-3" />
        大文件
      </button>
      <button
        class="flex flex-1 items-center justify-center gap-1 rounded-md py-1 text-[11px] transition-colors"
        :class="view === 'dirs' ? 'bg-primary/15 text-primary' : 'text-muted-foreground hover:text-foreground'"
        @click="view = 'dirs'"
      >
        <FolderSearch class="h-3 w-3" />
        目录占用
      </button>
    </div>

    <!-- ===== 大文件 ===== -->
    <template v-if="view === 'files'">
      <!-- Controls -->
      <div class="flex items-center gap-1.5">
        <OptionPicker
          v-model="minMb"
          :options="MIN_OPTIONS"
          :disabled="state === 'scanning'"
        />
        <Button v-if="state !== 'scanning'" size="sm" @click="startLargeScan(minMb)">
          <FileSearch class="h-3 w-3" />
        </Button>
        <Button v-else size="sm" variant="outline" @click="cancel">
          <X class="h-3 w-3" />
        </Button>
      </div>

      <!-- Scanning -->
      <div v-if="state === 'scanning'" class="flex flex-col items-center gap-2 py-4">
        <Loader2 class="h-5 w-5 animate-spin text-primary" />
        <Progress class="h-1 w-44" />
        <p class="text-[11px] text-muted-foreground">已扫描 {{ scanned }} 个文件</p>
        <p class="w-52 truncate text-center text-[10px] text-muted-foreground/60">{{ current }}</p>
      </div>

      <!-- Error -->
      <div v-else-if="state === 'error'" class="flex flex-col items-center gap-2 py-4">
        <p class="text-[11px] text-destructive">{{ errorMessage }}</p>
        <Button size="sm" variant="outline" @click="startLargeScan(minMb)">
          <RotateCcw class="h-3 w-3" />
        </Button>
      </div>

      <!-- Results -->
      <template v-else-if="state === 'done'">
        <div v-if="largeFiles.length === 0" class="flex flex-1 items-center justify-center py-6">
          <p class="text-[11px] text-muted-foreground">未找到大于 {{ minMb }} MB 的文件</p>
        </div>
        <template v-else>
          <div class="flex items-center justify-between">
            <span class="text-[11px] text-muted-foreground">
              共 {{ largeFiles.length }} 个大文件
              <span v-if="selected.size > 0" class="text-foreground/80">· 已选 {{ selected.size }}</span>
            </span>
            <button class="text-[11px] text-primary hover:underline" @click="toggleAll">
              {{ allSelected ? '取消全选' : '全选' }}
            </button>
          </div>
          <div class="scrollbar-none flex-1 space-y-[1px] overflow-auto py-0.5">
            <div
              v-for="f in largeFiles"
              :key="f.path"
              class="group flex cursor-pointer items-center gap-2 rounded px-2 py-1 transition-colors hover:bg-muted/20"
              @click="toggleSelect(f.path)"
            >
              <span
                class="h-1.5 w-1.5 shrink-0 rounded-full"
                :class="selected.has(f.path) ? 'bg-primary' : KIND_COLORS[f.kind] || 'bg-gray-400'"
                :title="KIND_LABELS[f.kind] || f.kind"
              />
              <div class="min-w-0 flex-1">
                <p class="truncate text-[11px] text-foreground/90" :title="f.path">{{ fileName(f.path) }}</p>
                <p class="truncate text-[10px] text-muted-foreground/60">{{ truncatePath(f.path) }} · {{ fmtDate(f.modified_secs) }}</p>
              </div>
              <span class="shrink-0 text-[11px] tabular-nums text-foreground/70">{{ fmtSize(f.size_bytes) }}</span>
              <button
                class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-muted-foreground/60 opacity-50 transition-all group-hover:opacity-100 hover:bg-destructive/20 hover:text-destructive"
                :class="confirmPaths.has(f.path) ? 'bg-destructive text-destructive-foreground opacity-100' : ''"
                :title="confirmPaths.has(f.path) ? '再次点击确认删除' : '删除'"
                @click.stop="handleDelete([f])"
              >
                <Trash2 v-if="!confirmPaths.has(f.path)" class="h-3 w-3" />
                <Check v-else class="h-3 w-3" />
              </button>
            </div>
          </div>
          <!-- Bottom bar -->
          <div class="flex items-center justify-between pt-0.5">
            <span v-if="deleteMsg" class="text-[11px]" :class="deleteMsg.startsWith('✓') ? 'text-success' : 'text-destructive'">
              {{ deleteMsg }}
            </span>
            <span v-else-if="deleting" class="flex items-center gap-1 text-[11px] text-muted-foreground">
              <Loader2 class="h-3 w-3 animate-spin" /> 删除中...
            </span>
            <span v-else class="text-[10px] text-muted-foreground/60">删除会记录审计日志</span>
            <Button
              size="sm"
              variant="destructive"
              :disabled="selected.size === 0 || deleting"
              :class="batchConfirm ? 'ring-2 ring-destructive/50' : ''"
              @click="handleCleanSelected"
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
      </template>

      <!-- Idle -->
      <div v-else class="flex flex-1 flex-col items-center justify-center gap-2 py-6">
        <div class="flex h-10 w-10 items-center justify-center rounded-full bg-primary/10">
          <FileSearch class="h-5 w-5 text-primary" />
        </div>
        <p class="max-w-48 text-center text-[11px] text-muted-foreground">
          扫描用户目录中的大文件，定位空间黑洞
        </p>
      </div>
    </template>

    <!-- ===== 目录占用 ===== -->
    <template v-else>
      <div class="flex items-center gap-1.5">
        <span class="flex-1 text-[11px] text-muted-foreground">用户目录 · 3 层深度</span>
        <Button v-if="state !== 'scanning'" size="sm" @click="startDirScan(3)">
          <FolderSearch class="h-3 w-3" />
        </Button>
        <Button v-else size="sm" variant="outline" @click="cancel">
          <X class="h-3 w-3" />
        </Button>
      </div>

      <div v-if="state === 'scanning'" class="flex flex-col items-center gap-2 py-4">
        <Loader2 class="h-5 w-5 animate-spin text-primary" />
        <Progress class="h-1 w-44" />
        <p class="text-[11px] text-muted-foreground">已扫描 {{ scanned }} 个文件</p>
        <p class="w-52 truncate text-center text-[10px] text-muted-foreground/60">{{ current }}</p>
      </div>

      <div v-else-if="state === 'error'" class="flex flex-col items-center gap-2 py-4">
        <p class="text-[11px] text-destructive">{{ errorMessage }}</p>
        <Button size="sm" variant="outline" @click="startDirScan(3)">
          <RotateCcw class="h-3 w-3" />
        </Button>
      </div>

      <template v-else-if="state === 'done'">
        <div v-if="dirUsage.length === 0" class="flex flex-1 items-center justify-center py-6">
          <p class="text-[11px] text-muted-foreground">暂无数据</p>
        </div>
        <div v-else class="scrollbar-none flex-1 space-y-1 overflow-auto py-0.5">
          <div v-for="(d, i) in dirUsage.slice(0, 12)" :key="d.path" class="px-1">
            <div class="flex items-center justify-between gap-2 text-[11px]">
              <span class="truncate text-foreground/90" :title="d.path">{{ truncatePath(d.path, 40) }}</span>
              <span class="shrink-0 tabular-nums text-foreground/70">{{ fmtSize(d.size_bytes) }}</span>
            </div>
            <div class="mt-0.5 h-1 overflow-hidden rounded-full bg-white/10">
              <div
                class="h-full rounded-full transition-all"
                :class="i === 0 ? 'bg-destructive' : i < 3 ? 'bg-warning' : 'bg-primary/60'"
                :style="{ width: Math.max((d.size_bytes / maxDirSize) * 100, 2) + '%' }"
              />
            </div>
          </div>
        </div>
      </template>

      <div v-else class="flex flex-1 flex-col items-center justify-center gap-2 py-6">
        <div class="flex h-10 w-10 items-center justify-center rounded-full bg-primary/10">
          <FolderSearch class="h-5 w-5 text-primary" />
        </div>
        <p class="max-w-48 text-center text-[11px] text-muted-foreground">
          分析各目录空间占用，找出最大的空间消耗者
        </p>
      </div>
    </template>
  </div>
</template>