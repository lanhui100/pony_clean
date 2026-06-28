<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import { useCleaner, type CleanItem } from '../composables/useCleaner'
import { Button } from '../components/ui/button'
import { Progress } from '../components/ui/progress'
import { Alert, AlertDescription } from '../components/ui/alert'
import { Checkbox } from '../components/ui/checkbox'
import { ScrollArea } from '../components/ui/scroll-area'
import {
  Collapsible,
  CollapsibleTrigger,
  CollapsibleContent,
} from '../components/ui/collapsible'
import { Scan, Trash2, RotateCcw, X, Check, AlertCircle, ChevronRight, Loader2 } from 'lucide-vue-next'

const {
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
} = useCleaner()

const categoryColors: Record<string, string> = {
  temp: 'bg-blue-400',
  cache: 'bg-purple-400',
  prefetch: 'bg-green-400',
  recycle_bin: 'bg-amber-400',
}

const categoryLabels: Record<string, string> = {
  temp: '临时文件',
  cache: '浏览器缓存',
  prefetch: 'Prefetch',
  recycle_bin: '回收站',
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
  if (selectedPaths.value.size === items.value.length) {
    selectedPaths.value = new Set()
  } else {
    selectedPaths.value = new Set(items.value.map(i => i.path))
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
const allSelected = computed(() => items.value.length > 0 && selectedPaths.value.size === items.value.length)

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), 3)
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function truncatePath(path: string, maxLen = 60): string {
  if (path.length <= maxLen) return path
  return '...' + path.slice(-(maxLen - 3))
}

async function handleStartScan() {
  selectedPaths.value = new Set()
  openCategories.value = new Set()
  await startScan()
}

async function handleClean() {
  const paths = Array.from(selectedPaths.value)
  if (paths.length === 0) return
  await executeClean(paths)
  selectedPaths.value = new Set()
}

async function handleCancel() {
  await cancelScan()
}

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
})
</script>

<template>
  <div class="relative flex h-full flex-col">
    <!-- IDLE -->
    <div v-if="state === 'idle'" class="flex flex-1 flex-col items-center justify-center gap-3">
      <div class="flex h-12 w-12 items-center justify-center rounded-xl bg-primary/10">
        <Scan class="h-6 w-6 text-primary" />
      </div>
      <h2 class="text-sm font-semibold">C盘安全清理</h2>
      <p class="max-w-56 text-center text-[11px] text-muted-foreground">
        扫描 C 盘临时文件、浏览器缓存、Prefetch 和回收站，安全释放磁盘空间
      </p>
      <Button size="sm" @click="handleStartScan">
        <Scan class="mr-1.5 h-3.5 w-3.5" />
        开始扫描
      </Button>
    </div>

    <!-- SCANNING -->
    <div v-else-if="state === 'scanning'" class="flex flex-1 flex-col items-center justify-center gap-3 px-6">
      <Loader2 class="h-6 w-6 animate-spin text-primary" />
      <Progress class="h-1 w-48" />
      <div class="text-center">
        <p class="text-xs text-foreground">已扫描 <span class="font-medium">{{ scanned }}</span> 个文件</p>
        <p v-if="currentFile" class="mt-1 max-w-56 truncate text-[11px] text-muted-foreground">
          {{ truncatePath(currentFile) }}
        </p>
      </div>
      <Button variant="ghost" size="sm" @click="handleCancel">
        <X class="mr-1 h-3.5 w-3.5" />
        取消
      </Button>
    </div>

    <!-- DELETING -->
    <div v-else-if="state === 'deleting'" class="flex flex-1 flex-col items-center justify-center gap-3 px-6">
      <Loader2 class="h-6 w-6 animate-spin text-primary" />
      <Progress class="h-1 w-48" />
      <p class="text-xs text-muted-foreground">清理中...</p>
    </div>

    <!-- ERROR -->
    <div v-else-if="state === 'error'" class="flex flex-1 flex-col items-center justify-center gap-3 px-6">
      <Alert variant="destructive" class="max-w-sm">
        <div class="flex items-start gap-2">
          <AlertCircle class="mt-0.5 h-4 w-4 shrink-0" />
          <div>
            <p class="text-xs font-medium">扫描失败</p>
            <AlertDescription class="mt-0.5 text-[11px]">
              {{ errorMessage || '发生未知错误，请重试' }}
            </AlertDescription>
          </div>
        </div>
      </Alert>
      <Button variant="outline" size="sm" @click="handleStartScan">
        <RotateCcw class="mr-1.5 h-3.5 w-3.5" />
        重试
      </Button>
    </div>

    <!-- CANCELLED -->
    <div v-else-if="state === 'cancelled'" class="flex flex-1 flex-col items-center justify-center gap-3">
      <p class="text-xs text-muted-foreground">扫描已取消</p>
      <Button variant="outline" size="sm" @click="handleStartScan">
        <RotateCcw class="mr-1.5 h-3.5 w-3.5" />
        重新扫描
      </Button>
    </div>

    <!-- DONE (empty) -->
    <div v-else-if="state === 'done' && items.length === 0" class="flex flex-1 flex-col items-center justify-center gap-2">
      <div class="flex h-10 w-10 items-center justify-center rounded-full bg-success/10">
        <Check class="h-5 w-5 text-success" />
      </div>
      <h3 class="text-sm font-medium">没有发现可清理文件</h3>
      <p class="text-[11px] text-muted-foreground">你的 C 盘状况良好</p>
      <Button variant="outline" size="sm" class="mt-1" @click="handleStartScan">
        <RotateCcw class="mr-1.5 h-3.5 w-3.5" />
        重新扫描
      </Button>
    </div>

    <!-- DONE (with items) -->
    <div v-else-if="state === 'done' && items.length > 0" class="flex flex-1 flex-col overflow-hidden gap-2">
      <!-- Header summary -->
      <div class="flex items-center justify-between">
        <div>
          <p class="text-[11px] text-muted-foreground">可清理</p>
          <p class="text-lg font-bold tabular-nums">{{ formatBytes(totalBytes) }}</p>
        </div>
        <Button variant="outline" size="sm" @click="handleStartScan">
          <RotateCcw class="mr-1 h-3.5 w-3.5" />
          重新扫描
        </Button>
      </div>

      <!-- Category legend (vertical) -->
      <div class="flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-muted-foreground">
        <div v-for="group in groupedItems" :key="group.category" class="inline-flex items-center gap-1.5">
          <span :class="['h-2 w-2 rounded-full shrink-0', group.color]" />
          <span>{{ group.label }}</span>
          <span class="font-medium text-foreground/80">{{ formatBytes(group.totalBytes) }}</span>
        </div>
      </div>

      <!-- Scrollable category list -->
      <ScrollArea class="scrollbar-thin flex-1 -mx-1 px-1">
        <div class="space-y-0.5 pb-2">
          <Collapsible
            v-for="group in groupedItems"
            :key="group.category"
            v-slot="{ open }"
            :open="openCategories.has(group.category)"
            @update:open="(v) => { const n = new Set(openCategories); v ? n.add(group.category) : n.delete(group.category); openCategories = n }"
          >
            <div class="rounded bg-muted/10">
              <!-- Category header -->
              <CollapsibleTrigger class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-xs font-medium hover:bg-muted/20 transition-colors">
                <ChevronRight
                  class="h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform duration-200"
                  :class="open ? 'rotate-90' : ''"
                />
                <Checkbox
                  :checked="isCategoryFullySelected(group.category)"
                  :class="isCategoryPartiallySelected(group.category) ? 'opacity-60' : ''"
                  @click.stop="toggleCategory(group.category)"
                />
                <span :class="['h-2 w-2 rounded-full shrink-0', group.color]" />
                <span class="flex-1">{{ group.label }}</span>
                <span class="text-[11px] tabular-nums text-muted-foreground">{{ formatBytes(group.totalBytes) }}</span>
              </CollapsibleTrigger>

              <!-- Category items -->
              <CollapsibleContent>
                <div class="py-0.5">
                  <label
                    v-for="item in group.items"
                    :key="item.path"
                    class="flex cursor-pointer items-center gap-2 px-7 py-1 text-xs transition-colors hover:bg-muted/15"
                  >
                    <Checkbox
                      :checked="selectedPaths.has(item.path)"
                      @update:checked="toggleItem(item.path)"
                    />
                    <span class="flex-1 truncate text-muted-foreground">
                      {{ truncatePath(item.path, 45) }}
                    </span>
                    <span class="shrink-0 text-[11px] tabular-nums text-foreground/70">
                      {{ formatBytes(item.size_bytes) }}
                    </span>
                  </label>
                </div>
              </CollapsibleContent>
            </div>
          </Collapsible>
        </div>
      </ScrollArea>

      <!-- Bottom action bar -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <span class="text-[11px] text-muted-foreground">
            <template v-if="selectedCount > 0">
              已选 <span class="font-medium text-foreground/80">{{ selectedCount }}</span> 项
              <span class="text-muted-foreground">({{ formatBytes(selectedBytes) }})</span>
            </template>
            <template v-else>未选择文件</template>
          </span>
          <button
            class="text-[11px] text-primary hover:underline"
            @click="toggleAll"
          >
            {{ allSelected ? '取消全选' : '全选' }}
          </button>
        </div>
        <Button
          variant="destructive"
          size="sm"
          :disabled="selectedCount === 0"
          @click="handleClean"
        >
          <Trash2 class="mr-1.5 h-3.5 w-3.5" />
          清理选中
        </Button>
      </div>
    </div>

    <!-- Delete result toast -->
    <Transition name="toast">
      <div
        v-if="deleteResult"
        class="absolute bottom-0 left-0 right-0 z-10"
      >
        <Alert
          :variant="deleteResult.failed > 0 ? 'destructive' : 'default'"
          class="shadow-lg border-0"
        >
          <div class="flex items-start gap-2">
            <Check v-if="deleteResult.failed === 0" class="mt-0.5 h-4 w-4 shrink-0 text-green-500" />
            <AlertCircle v-else class="mt-0.5 h-4 w-4 shrink-0" />
            <div class="flex-1 min-w-0">
              <p class="text-xs">
                清理完成
                <span class="font-medium text-success">{{ deleteResult.success }}</span> 项成功
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
              class="shrink-0 flex h-4 w-4 items-center justify-center rounded text-muted-foreground hover:text-foreground transition-colors"
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
</style>
