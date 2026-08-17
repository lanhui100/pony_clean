<script setup lang="ts">
import { ref, watch, nextTick, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emitTo } from '@tauri-apps/api/event'
import { Activity, ChevronUp, HardDrive, Rocket, Search, Settings, X } from 'lucide-vue-next'

const props = defineProps<{
  activeTab: string
  searchQuery: string
}>()

const emit = defineEmits<{
  (e: 'update:activeTab', value: string): void
  (e: 'update:searchQuery', value: string): void
}>()

const navItems = [
  { value: 'monitor', icon: Activity, label: '监控' },
  { value: 'cleaner', icon: HardDrive, label: '清理' },
  { value: 'startup', icon: Rocket, label: '启动项' },
  { value: 'settings', icon: Settings, label: '设置' },
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

async function handleClose() {
  await invoke('quit_app')
}

/** 收起到胶囊态：通知胶囊窗口执行 hideIsland（扫描等后台任务不中断） */
function collapseToCapsule() {
  emitTo('capsule', 'island-collapse-request').catch(() => {})
}
</script>

<template>
  <!-- Left icon sidebar for compact island -->
  <div class="sidebar flex flex-col items-center py-2 gap-1 border-r border-white/10 w-[42px] shrink-0">
    <!-- Nav icons -->
    <button
      v-for="item in navItems"
      :key="item.value"
      class="flex h-7 w-7 items-center justify-center rounded-lg transition-all duration-200"
      :class="activeTab === item.value
        ? 'bg-primary/15 text-primary'
        : 'text-muted-foreground/60 hover:text-foreground/80 hover:bg-white/5'"
      :title="item.label"
      :aria-label="item.label"
      :aria-current="activeTab === item.value ? 'page' : undefined"
      @click="emit('update:activeTab', item.value)"
    >
      <component :is="item.icon" :size="16" />
    </button>

    <div class="flex-1" />

    <!-- 收起到胶囊（扫描不中断，长时扫描不遮蔽窗口） -->
    <button
      class="mb-1 flex h-6 w-6 items-center justify-center rounded-lg text-muted-foreground/60 transition-all duration-200 hover:bg-white/5 hover:text-foreground/80"
      title="收起到胶囊（扫描不中断）"
      aria-label="收起到胶囊"
      @click="collapseToCapsule"
    >
      <ChevronUp :size="14" />
    </button>

    <!-- Close -->
    <button
      class="flex h-6 w-6 items-center justify-center rounded-lg text-muted-foreground/60 transition-all duration-200 hover:bg-destructive/20 hover:text-destructive"
      title="退出"
      aria-label="退出"
      @click="handleClose"
    >
      <X :size="14" />
    </button>
  </div>
</template>

<style scoped>
.sidebar {
  background: transparent;
}
</style>