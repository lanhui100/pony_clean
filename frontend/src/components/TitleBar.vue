<script setup lang="ts">
import { ref, nextTick, watch, onUnmounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { Activity, HardDrive, Search, X } from 'lucide-vue-next'
import PonyIcon from './PonyIcon.vue'

const appWindow = getCurrentWindow()

const props = defineProps<{
  activeTab: string
  morphState: string
}>()

const emit = defineEmits<{
  (e: 'update:activeTab', value: string): void
  (e: 'update:searchQuery', value: string): void
}>()

const navItems = [
  { value: 'monitor', icon: Activity, label: '进程监控' },
  { value: 'cleaner', icon: HardDrive, label: 'C盘清理' },
]

const showSearch = ref(false)
const localSearch = ref('')
const searchInputRef = ref<HTMLInputElement>()
let searchTimer: ReturnType<typeof setTimeout> | null = null

watch(localSearch, (val) => {
  if (searchTimer) clearTimeout(searchTimer)
  if (showSearch.value) {
    searchTimer = setTimeout(() => {
      emit('update:searchQuery', val.trim())
    }, 300)
  }
})

onUnmounted(() => {
  if (searchTimer) clearTimeout(searchTimer)
})

function onHeaderMouseDown(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.closest('button') || target.closest('input')) return
  appWindow.startDragging()
}

async function handleClose() {
  await invoke('quit_app')
}

function toggleSearch() {
  if (showSearch.value) {
    showSearch.value = false
    localSearch.value = ''
    emit('update:searchQuery', '')
  } else {
    showSearch.value = true
    nextTick(() => searchInputRef.value?.focus())
  }
}

function handleEnter() {
  if (searchTimer) clearTimeout(searchTimer)
  emit('update:searchQuery', localSearch.value.trim())
}

function handleEscape() {
  showSearch.value = false
  localSearch.value = ''
  emit('update:searchQuery', '')
}

function handleBlur() {
  if (!localSearch.value) {
    showSearch.value = false
    emit('update:searchQuery', '')
  }
}
</script>

<template>
  <header @mousedown="onHeaderMouseDown" class="flex h-10 cursor-move items-center border-b border-border/50 bg-background/60 px-4 backdrop-blur-xl select-none gap-4">
    <div class="flex h-full items-center gap-2 shrink-0">
      <PonyIcon :size="16" />
      <span class="text-sm font-bold text-white">Pony Clean</span>
    </div>

    <div class="flex h-full flex-1 items-center justify-between gap-1">
      <div v-show="!showSearch" class="flex h-full items-center gap-1">
        <button
          v-for="item in navItems"
          :key="item.value"
          class="flex h-7 w-7 items-center justify-center rounded-md transition-colors"
          :class="activeTab === item.value
            ? 'bg-primary/15 text-primary'
            : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground/80'"
          :title="item.label"
          @click="emit('update:activeTab', item.value)"
        >
          <component :is="item.icon" :size="16" />
        </button>
      </div>

      <div
        class="overflow-hidden transition-all duration-300 ease-in-out"
        :class="showSearch ? 'w-36' : 'w-0'"
      >
        <div v-if="showSearch" class="relative">
          <input
            ref="searchInputRef"
            v-model="localSearch"
            maxlength="64"
            placeholder="搜索进程..."
            class="h-6 w-full rounded bg-muted/40 px-2 pr-6 text-xs text-foreground placeholder:text-muted-foreground/60 outline-none"
            @keydown.enter="handleEnter"
            @keydown.escape="handleEscape"
            @blur="handleBlur"
          />
      <button
            v-if="localSearch"
            class="absolute right-1 top-1/2 -translate-y-1/2 flex h-4 w-4 items-center justify-center rounded-full bg-muted/60 text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
            @click="handleEscape"
          >
            <X :size="10" />
          </button>
        </div>
      </div>

      <div class="flex items-center gap-0.5">
        <button
          class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted/60 hover:text-foreground/80"
          :title="showSearch ? '关闭搜索' : '搜索'"
          @click="toggleSearch"
        >
          <Search :size="16" />
        </button>

        <button
          v-show="!showSearch"
          class="flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
          title="关闭"
          @click="handleClose"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>
    </div>
  </header>
</template>
