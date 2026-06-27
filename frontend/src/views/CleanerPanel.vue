<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useCleaner, type CleanItem } from '../composables/useCleaner'
import { Button } from '../components/ui/button'
import { Progress } from '../components/ui/progress'
import { Alert, AlertDescription } from '../components/ui/alert'
import { Checkbox } from '../components/ui/checkbox'
import { ScrollArea } from '../components/ui/scroll-area'
import { Badge } from '../components/ui/badge'
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
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[Math.min(i, 3)]}`
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
    }, 5000)
  }
})
</script>

<template>
  <div class="relative flex h-full flex-col">
    <!-- IDLE -->
    <div v-if="state === 'idle'" class="flex flex-1 flex-col items-center justify-center gap-4">
      <div class="flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10">
        <Scan class="h-8 w-8 text-primary" />
      </div>
      <h2 class="text-xl font-semibold">C盘安全清理</h2>
      <p class="max-w-xs text-center text-sm text-muted-foreground">
        扫描 C 盘临时文件、浏览器缓存、Prefetch 和回收站，安全释放磁盘空间
      </p>
      <Button size="lg" @click="handleStartScan">
        <Scan class="mr-2 h-4 w-4" />
        开始扫描
      </Button>
    </div>

    <!-- SCANNING -->
    <div v-else-if="state === 'scanning'" class="flex flex-1 flex-col items-center justify-center gap-4 px-6">
      <Loader2 class="h-8 w-8 animate-spin text-primary" />
      <Progress class="w-full max-w-sm" />
      <div class="text-center">
        <p class="text-sm text-foreground">已扫描 <span class="font-medium">{{ scanned }}</span> 个文件</p>
        <p v-if="currentFile" class="mt-1 max-w-sm truncate text-xs text-muted-foreground">
          {{ truncatePath(currentFile) }}
        </p>
      </div>
      <Button variant="ghost" size="sm" @click="handleCancel">
        <X class="mr-1 h-4 w-4" />
        取消
      </Button>
    </div>

    <!-- DELETING -->
    <div v-else-if="state === 'deleting'" class="flex flex-1 flex-col items-center justify-center gap-4 px-6">
      <Loader2 class="h-8 w-8 animate-spin text-primary" />
      <Progress class="w-full max-w-sm" />
      <p class="text-sm text-muted-foreground">清理中...</p>
    </div>

    <!-- ERROR -->
    <div v-else-if="state === 'error'" class="flex flex-1 flex-col items-center justify-center gap-4 px-6">
      <Alert variant="destructive" class="max-w-md">
        <div class="flex items-start gap-3">
          <AlertCircle class="mt-0.5 h-5 w-5 shrink-0" />
          <div>
            <p class="font-medium">扫描失败</p>
            <AlertDescription class="mt-1">
              {{ errorMessage || '发生未知错误，请重试' }}
            </AlertDescription>
          </div>
        </div>
      </Alert>
      <Button variant="outline" @click="handleStartScan">
        <RotateCcw class="mr-2 h-4 w-4" />
        重试
      </Button>
    </div>

    <!-- CANCELLED -->
    <div v-else-if="state === 'cancelled'" class="flex flex-1 flex-col items-center justify-center gap-4">
      <p class="text-sm text-muted-foreground">扫描已取消</p>
      <Button variant="outline" @click="handleStartScan">
        <RotateCcw class="mr-2 h-4 w-4" />
        重新扫描
      </Button>
    </div>

    <!-- DONE (empty) -->
    <div v-else-if="state === 'done' && items.length === 0" class="flex flex-1 flex-col items-center justify-center gap-3">
      <div class="flex h-14 w-14 items-center justify-center rounded-full bg-green-500/10">
        <Check class="h-7 w-7 text-green-500" />
      </div>
      <h3 class="text-lg font-medium">没有发现可清理文件</h3>
      <p class="text-sm text-muted-foreground">你的 C 盘状况良好</p>
      <Button variant="outline" size="sm" class="mt-2" @click="handleStartScan">
        <RotateCcw class="mr-2 h-4 w-4" />
        重新扫描
      </Button>
    </div>

    <!-- DONE (with items) -->
    <div v-else-if="state === 'done' && items.length > 0" class="flex flex-1 flex-col overflow-hidden">
      <!-- Header summary -->
      <div class="flex items-center justify-between border-b border-border px-1 pb-3">
        <div>
          <p class="text-sm text-muted-foreground">可清理</p>
          <p class="text-2xl font-bold text-foreground">{{ formatBytes(totalBytes) }}</p>
        </div>
        <Button variant="outline" size="sm" @click="handleStartScan">
          <RotateCcw class="mr-1 h-4 w-4" />
          重新扫描
        </Button>
      </div>

      <!-- Category legend -->
      <div class="flex flex-wrap gap-3 py-3">
        <div v-for="group in groupedItems" :key="group.category" class="flex items-center gap-1.5 text-xs text-muted-foreground">
          <span :class="['h-2.5 w-2.5 rounded-full', group.color]" />
          <span>{{ group.label }}</span>
          <span class="font-medium text-foreground">{{ formatBytes(group.totalBytes) }}</span>
        </div>
      </div>

      <!-- Scrollable category list -->
      <ScrollArea class="flex-1 -mx-1 px-1">
        <div class="space-y-1 pb-4">
          <Collapsible
            v-for="group in groupedItems"
            :key="group.category"
            v-slot="{ open }"
            :open="openCategories.has(group.category)"
            @update:open="(v) => { const n = new Set(openCategories); v ? n.add(group.category) : n.delete(group.category); openCategories = n }"
          >
            <div class="rounded-lg border border-border">
              <!-- Category header -->
              <CollapsibleTrigger class="flex items-center gap-2 px-3 py-2.5 text-sm font-medium hover:bg-accent/50 rounded-lg">
                <ChevronRight
                  class="h-4 w-4 shrink-0 text-muted-foreground transition-transform duration-200"
                  :class="open ? 'rotate-90' : ''"
                />
                <Checkbox
                  :checked="isCategoryFullySelected(group.category)"
                  :class="isCategoryPartiallySelected(group.category) ? 'opacity-60' : ''"
                  class="mr-1"
                  @click.stop="toggleCategory(group.category)"
                />
                <span :class="['h-2.5 w-2.5 rounded-full shrink-0', group.color]" />
                <span class="flex-1">{{ group.label }}</span>
                <Badge variant="secondary" class="text-xs font-normal">
                  {{ formatBytes(group.totalBytes) }}
                </Badge>
              </CollapsibleTrigger>

              <!-- Category items -->
              <CollapsibleContent class="border-t border-border">
                <div class="divide-y divide-border">
                  <label
                    v-for="item in group.items"
                    :key="item.path"
                    class="flex cursor-pointer items-center gap-3 px-3 py-2 text-sm transition-colors hover:bg-accent/30"
                  >
                    <Checkbox
                      :checked="selectedPaths.has(item.path)"
                      @update:checked="toggleItem(item.path)"
                    />
                    <span class="flex-1 truncate text-muted-foreground">
                      {{ truncatePath(item.path, 50) }}
                    </span>
                    <span class="shrink-0 text-xs font-medium tabular-nums text-foreground">
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
      <div class="sticky bottom-0 flex items-center justify-between border-t border-border bg-background px-1 py-3">
        <div class="flex items-center gap-2">
          <span class="text-xs text-muted-foreground">
            <template v-if="selectedCount > 0">
              已选 <span class="font-medium text-foreground">{{ selectedCount }}</span> 项
              <span class="text-muted-foreground">({{ formatBytes(selectedBytes) }})</span>
            </template>
            <template v-else>未选择文件</template>
          </span>
          <button
            class="text-xs text-primary hover:underline"
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
          <Trash2 class="mr-1.5 h-4 w-4" />
          清理选中
        </Button>
      </div>
    </div>

    <!-- Delete result toast -->
    <Transition name="toast">
      <div
        v-if="deleteResult"
        class="absolute bottom-4 left-4 right-4 z-10"
      >
        <Alert
          :variant="deleteResult.failed > 0 ? 'destructive' : 'default'"
          class="shadow-lg"
        >
          <div class="flex items-center gap-3">
            <Check v-if="deleteResult.failed === 0" class="h-5 w-5 shrink-0 text-green-500" />
            <AlertCircle v-else class="h-5 w-5 shrink-0" />
            <AlertDescription>
              清理完成
              <span class="font-medium text-green-500">{{ deleteResult.success }}</span> 项成功
              <template v-if="deleteResult.failed > 0">
                ，<span class="font-medium text-destructive">{{ deleteResult.failed }}</span> 项失败
              </template>
            </AlertDescription>
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
