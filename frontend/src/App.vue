<script setup lang="ts">
import { ref } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import CleanerPanel from '@/views/CleanerPanel.vue'
import MonitorPanel from './views/MonitorPanel.vue'

const activeTab = ref('monitor')
const tabs = [
  { value: 'monitor', label: '进程监控' },
  { value: 'cleaner', label: 'C盘清理' },
]
</script>

<template>
  <div class="flex h-screen w-screen flex-col overflow-hidden bg-background text-foreground">
    <TitleBar />
    <main class="flex-1 overflow-hidden p-4">
      <div class="flex items-center gap-2 border-b border-border px-1">
        <button
          v-for="tab in tabs"
          :key="tab.value"
          class="relative px-4 py-2 text-sm font-medium transition-colors"
          :class="activeTab === tab.value ? 'text-foreground' : 'text-muted-foreground hover:text-foreground/80'"
          @click="activeTab = tab.value"
        >
          {{ tab.label }}
          <span
            v-if="activeTab === tab.value"
            class="absolute bottom-0 left-0 right-0 h-0.5 bg-primary"
          />
        </button>
      </div>
      <div class="mt-4">
        <MonitorPanel v-if="activeTab === 'monitor'" />
        <CleanerPanel v-else-if="activeTab === 'cleaner'" />
      </div>
    </main>
  </div>
</template>
