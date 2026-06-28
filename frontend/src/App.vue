<script setup lang="ts">
import { ref } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import CleanerPanel from '@/views/CleanerPanel.vue'
import MonitorPanel from '@/views/MonitorPanel.vue'

const activeTab = ref('monitor')
const tabs = [
  { value: 'monitor', label: '进程监控' },
  { value: 'cleaner', label: 'C盘清理' },
]
</script>

<template>
  <div class="flex h-screen w-screen flex-col overflow-hidden bg-background text-foreground">
    <TitleBar />
    <main class="flex-1 overflow-hidden p-4 pt-2">
      <div class="inline-flex items-center gap-1 rounded-lg bg-muted/50 p-1">
        <button
          v-for="tab in tabs"
          :key="tab.value"
          class="rounded-md px-3 py-1.5 text-sm font-medium transition-colors duration-200"
          :class="activeTab === tab.value ? 'bg-card text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground/80'"
          @click="activeTab = tab.value"
        >
          {{ tab.label }}
        </button>
      </div>
      <div class="mt-4 h-[calc(100%-2.5rem)]">
        <KeepAlive>
          <MonitorPanel v-if="activeTab === 'monitor'" key="monitor" />
          <CleanerPanel v-else key="cleaner" />
        </KeepAlive>
      </div>
    </main>
  </div>
</template>
