<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { Loader2, Plus, Save, Trash2 } from 'lucide-vue-next'
import { Button } from '../components/ui/button'
import { useMonitor } from '../composables/useMonitor'

interface AppConfig {
  alert_cpu_pct: number
  alert_mem_pct: number
  autostart: boolean
}

interface CustomTarget {
  id: string
  path: string
  level: 'safe' | 'confirm' | 'forbidden'
  category: string
  description: string
  enabled: boolean
}

interface CleanConfig {
  version?: number
  disabled_target_ids: string[]
  disabled_targets: string[]
  custom_exclude_paths: string[]
  per_target_config: Record<string, unknown>
  custom_targets: CustomTarget[]
}

const CATEGORY_OPTIONS = [
  { value: 'temp', label: '临时文件' },
  { value: 'cache', label: '缓存' },
  { value: 'logs', label: '日志' },
  { value: 'app_cache', label: '应用缓存' },
  { value: 'dev_cache', label: '开发缓存' },
]

const LEVEL_OPTIONS = [
  { value: 'safe', label: '安全（默认勾选）' },
  { value: 'confirm', label: '需确认（不勾选）' },
]

const { setAlertThresholds } = useMonitor()

const cpuPct = ref(80)
const memPct = ref(85)
const autostart = ref(false)
const loading = ref(true)
const saving = ref(false)
const savedMsg = ref('')
let savedTimer: ReturnType<typeof setTimeout> | null = null

// 自定义清理规则
const customTargets = ref<CustomTarget[]>([])
const showAddForm = ref(false)
const newPath = ref('')
const newCategory = ref('temp')
const newLevel = ref<'safe' | 'confirm'>('safe')
const newDesc = ref('')
const ruleMsg = ref('')
let ruleTimer: ReturnType<typeof setTimeout> | null = null

onMounted(async () => {
  try {
    const cfg = await invoke<AppConfig>('get_config')
    cpuPct.value = cfg.alert_cpu_pct || 80
    memPct.value = cfg.alert_mem_pct || 85
    autostart.value = cfg.autostart ?? false
  } catch {
    // 使用默认值
  }
  try {
    const cleanCfg = await invoke<CleanConfig>('get_clean_config')
    customTargets.value = cleanCfg.custom_targets ?? []
  } catch {
    // 无清理配置
  }
  loading.value = false
})

onUnmounted(() => {
  if (savedTimer) clearTimeout(savedTimer)
  if (ruleTimer) clearTimeout(ruleTimer)
})

function flashRuleMsg(msg: string) {
  ruleMsg.value = msg
  if (ruleTimer) clearTimeout(ruleTimer)
  ruleTimer = setTimeout(() => { ruleMsg.value = '' }, 2500)
}

function addRule() {
  const path = newPath.value.trim()
  if (!path) {
    flashRuleMsg('请输入目录路径')
    return
  }
  customTargets.value.push({
    id: `custom_${Date.now().toString(36)}`,
    path,
    level: newLevel.value,
    category: newCategory.value,
    description: newDesc.value.trim() || path,
    enabled: true,
  })
  newPath.value = ''
  newDesc.value = ''
  showAddForm.value = false
}

function removeRule(index: number) {
  customTargets.value.splice(index, 1)
}

function toggleRule(index: number) {
  customTargets.value[index].enabled = !customTargets.value[index].enabled
}

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

    // 保存清理配置（保留原有字段，仅更新自定义目标）
    const cleanCfg = await invoke<CleanConfig>('get_clean_config')
    cleanCfg.custom_targets = customTargets.value
    await invoke('save_clean_config', { config: cleanCfg })

    savedMsg.value = '✓ 已保存'
    if (savedTimer) clearTimeout(savedTimer)
    savedTimer = setTimeout(() => { savedMsg.value = '' }, 2500)
  } catch (e) {
    savedMsg.value = `✗ ${e}`
  }
  saving.value = false
}

function categoryLabel(value: string) {
  return CATEGORY_OPTIONS.find(o => o.value === value)?.label ?? value
}
</script>

<template>
  <div class="flex h-full flex-col gap-3 overflow-y-auto px-1 py-1">
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

      <!-- Custom clean rules -->
      <div class="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-2.5">
        <div class="flex items-center justify-between">
          <span class="text-xs font-medium text-foreground/90">自定义清理规则</span>
          <button
            class="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-primary/15 hover:text-primary"
            title="添加规则"
            @click="showAddForm = !showAddForm"
          >
            <Plus class="h-3.5 w-3.5" />
          </button>
        </div>
        <p class="mt-0.5 text-[10px] text-muted-foreground/70">
          添加你自己的缓存/临时目录，扫描时一并纳入（受保护路径自动跳过）
        </p>

        <!-- Add form -->
        <div v-if="showAddForm" class="mt-2 space-y-1.5 rounded bg-white/5 p-2">
          <input
            v-model="newPath"
            type="text"
            placeholder="目录路径，支持 %TEMP% 等环境变量"
            class="w-full rounded border border-white/10 bg-transparent px-2 py-1 text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:border-primary/50 focus:outline-none"
          />
          <div class="flex gap-1.5">
            <select
              v-model="newCategory"
              class="flex-1 rounded border border-white/10 bg-transparent px-1.5 py-1 text-[11px] text-foreground focus:border-primary/50 focus:outline-none"
            >
              <option v-for="o in CATEGORY_OPTIONS" :key="o.value" :value="o.value">{{ o.label }}</option>
            </select>
            <select
              v-model="newLevel"
              class="flex-1 rounded border border-white/10 bg-transparent px-1.5 py-1 text-[11px] text-foreground focus:border-primary/50 focus:outline-none"
            >
              <option v-for="o in LEVEL_OPTIONS" :key="o.value" :value="o.value">{{ o.label }}</option>
            </select>
          </div>
          <input
            v-model="newDesc"
            type="text"
            placeholder="描述（可选）"
            class="w-full rounded border border-white/10 bg-transparent px-2 py-1 text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:border-primary/50 focus:outline-none"
          />
          <div class="flex items-center justify-end gap-1.5">
            <span v-if="ruleMsg" class="text-[10px] text-warning">{{ ruleMsg }}</span>
            <Button size="sm" variant="outline" @click="showAddForm = false">取消</Button>
            <Button size="sm" @click="addRule">添加</Button>
          </div>
        </div>

        <!-- Rule list -->
        <div v-if="customTargets.length > 0" class="mt-2 space-y-1">
          <div
            v-for="(rule, i) in customTargets"
            :key="rule.id"
            class="flex items-center gap-2 rounded bg-white/5 px-2 py-1.5"
          >
            <button
              class="relative h-4 w-7 shrink-0 rounded-full transition-colors"
              :class="rule.enabled ? 'bg-primary/70' : 'bg-white/15'"
              role="switch"
              :aria-checked="rule.enabled"
              :title="rule.enabled ? '已启用' : '已停用'"
              @click="toggleRule(i)"
            >
              <span
                class="absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-all"
                :class="rule.enabled ? 'left-[14px]' : 'left-0.5'"
              />
            </button>
            <div class="min-w-0 flex-1">
              <p class="truncate text-[11px] text-foreground/90" :title="rule.path">{{ rule.path }}</p>
              <p class="text-[10px] text-muted-foreground/70">
                {{ categoryLabel(rule.category) }} · {{ rule.level === 'safe' ? '安全' : '需确认' }}
              </p>
            </div>
            <button
              class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-muted-foreground/60 transition-colors hover:bg-destructive/20 hover:text-destructive"
              title="删除规则"
              @click="removeRule(i)"
            >
              <Trash2 class="h-3 w-3" />
            </button>
          </div>
        </div>
        <p v-else-if="!showAddForm" class="mt-1.5 text-[10px] text-muted-foreground/50">暂无自定义规则</p>
      </div>

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