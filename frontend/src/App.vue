<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import TitleBar from '@/components/TitleBar.vue'
import CleanerPanel from '@/views/CleanerPanel.vue'
import MonitorPanel from '@/views/MonitorPanel.vue'

const activeTab = ref('monitor')
const searchQuery = ref('')

onMounted(async () => {
  try {
    await getCurrentWindow().setEffects({
      effects: [{ effect: 'acrylic' }],
      color: { r: 30, g: 28, b: 26, a: 200 },
    })
  } catch (e) {
    console.warn('[PonyClean] window effects not supported:', e)
  }
  console.log('[PonyClean] App.vue mounted, activeTab:', activeTab.value)
})
</script>

<template>
  <div class="flex h-screen w-screen flex-col overflow-hidden text-foreground"
    style="background: linear-gradient(145deg, hsl(30,12%,9%) 0%, hsl(30,12%,14%) 40%, hsl(28,10%,18%) 70%, hsl(25,8%,22%) 100%)">
    <TitleBar v-model:activeTab="activeTab" v-model:searchQuery="searchQuery" />
    <main class="flex-1 overflow-hidden p-4 pt-2">
      <div class="h-full">
        <KeepAlive>
          <MonitorPanel v-if="activeTab === 'monitor'" key="monitor" :search="searchQuery" />
          <CleanerPanel v-else key="cleaner" />
        </KeepAlive>
      </div>
    </main>
  </div>
</template>