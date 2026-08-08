<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Loader2, Save } from 'lucide-vue-next'
import { Button } from '../components/ui/button'
import { useMonitor } from '../composables/useMonitor'

interface AppConfig {
  alert_cpu_pct: number
  alert_mem_pct: number
  autostart: boolean
}

const { setAlertThresholds } = useMonitor()

const cpuPct = ref(80)
const memPct = ref(85)
const autostart = ref(false)
const loading = ref(true)
const saving = ref(false)
const savedMsg = ref('')
let savedTimer: ReturnType<typeof setTimeout> | null = null

onMounted(async () => {
  try {
    const cfg = await invoke<AppConfig>('get_config')
    cpuPct.value = cfg.alert_cpu_pct || 80
    memPct.value = cfg.alert_mem_pct || 85
    autostart.value = cfg.autostart ?? false
  } catch {
    // 使用默认值
  }
  loading.value = false
})

onUnmounted(() => {
  if (savedTimer) clearTimeout(savedTimer)
})

async function handleSave() {
  if (saving.value) return
  saving.value = true
  savedMsg.value = ''
  try {
    await invoke('set_config', {
      config: {
        alert_cpu_pct: cpuPct.value,
        alert_mem_pct: memPct.value,
        autostart: autostart.value,
      },
    })
    setAlertThresholds(cpuPct.value, memPct.value)
    savedMsg.value = '✓ 已保存'
    if (savedTimer) clearTimeout(savedTimer)
    savedTimer = setTimeout(() => { savedMsg.value = '' }, 2500)
  } catch (e) {
    savedMsg.value = `✗ ${e}`
  }
  saving.value = false
}
</script>

<template>
  <div class="flex h-full flex-col gap-4 overflow-y-auto px-1 py-1">
    <div v-if="loading" class="flex flex-1 items-center justify-center">
      <Loader2 class="h-4 w-4 animate-spin text-muted-foreground" />
    </div>

    <template v-else>
      <!-- CPU threshold -->
      <div class="rounded-lg border border-white/10 bg-white/5 px-3 py-2.5">
        <div class="flex items-center justify-between">
          <span class="text-xs font-medium text-foreground/90">CPU 告警阈值</span>
          <span class="text-xs tabular-nums text-primary">{{ cpuPct }}%</span>
        </div>
        <input
          v-model.number="cpuPct"
          type="range"
          min="50"
          max="100"
          step="5"
          class="mt-2 w-full accent-primary"
        />
        <p class="mt-1 text-[10px] text-muted-foreground/70">CPU 占用超过该值时发送系统通知</p>
      </div>

      <!-- Memory threshold -->
      <div class="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2.5">
        <div class="flex items-center justify-between">
          <span class="text-xs font-medium text-foreground/90">内存告警阈值</span>
          <span class="text-xs tabular-nums text-primary">{{ memPct }}%</span>
        </div>
        <input
          v-model.number="memPct"
          type="range"
          min="50"
          max="100"
          step="5"
          class="mt-2 w-full accent-primary"
        />
        <p class="mt-1 text-[10px] text-muted-foreground/70">内存占用超过该值时发送系统通知</p>
      </div>

      <!-- Autostart -->
      <label class="flex cursor-pointer items-center justify-between rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2.5">
        <div>
          <span class="text-xs font-medium text-foreground/90">开机自启</span>
          <p class="mt-0.5 text-[10px] text-muted-foreground/70">登录 Windows 后自动启动 PonyClean</p>
        </div>
        <button
          class="relative h-5 w-9 shrink-0 rounded-full transition-colors"
          :class="autostart ? 'bg-primary/80' : 'bg-white/15'"
          role="switch"
          :aria-checked="autostart"
          @click="autostart = !autostart"
        >
          <span
            class="absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-all"
            :class="autostart ? 'left-[18px]' : 'left-0.5'"
          />
        </button>
      </label>

      <!-- Save -->
      <div class="mt-auto flex items-center justify-between pt-1">
        <span v-if="savedMsg" class="text-[11px]" :class="savedMsg.startsWith('✓') ? 'text-success' : 'text-destructive'">
          {{ savedMsg }}
        </span>
        <span v-else class="text-[10px] text-muted-foreground/60">设置保存在本地配置文件中</span>
        <Button size="sm" :disabled="saving" @click="handleSave">
          <Save v-if="!saving" class="h-3.5 w-3.5" />
          <Loader2 v-else class="h-3.5 w-3.5 animate-spin" />
          <span class="ml-1">保存</span>
        </Button>
      </div>
    </template>
  </div>
</template>