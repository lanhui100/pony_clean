<script setup lang="ts">
import { ref } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import CleanerPanel from '@/views/CleanerPanel.vue'
import MonitorPanel from '@/views/MonitorPanel.vue'

const activeTab = ref('monitor')
</script>

<template>
  <div class="flex h-screen w-screen flex-col overflow-hidden bg-background text-foreground">
    <TitleBar v-model:activeTab="activeTab" />
    <main class="flex-1 overflow-hidden p-4 pt-2">
      <div class="h-full">
        <KeepAlive>
          <MonitorPanel v-if="activeTab === 'monitor'" key="monitor" />
          <CleanerPanel v-else key="cleaner" />
        </KeepAlive>
      </div>
    </main>
  </div>
</template>
