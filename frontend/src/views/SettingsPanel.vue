<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { relaunch } from '@tauri-apps/plugin-process'
import { Loader2, Plus, Save, Trash2, RefreshCw, Download } from 'lucide-vue-next'
import { Button } from '../components/ui/button'
import { Progress } from '../components/ui/progress'
import { Toast } from '../components/ui/toast'
import OptionPicker from '../components/OptionPicker.vue'
import { humanizeError } from '../lib/humanizeError'
import { useMonitor } from '../composables/useMonitor'
import {
  autoUpdateEnabled,
  checkingUpdate,
  downloadingUpdate,
  downloadProgress,
  updateAvailable,
  updateVersion,
  markUpdateSeen,
  checkForUpdate,
  installUpdate,
} from '../composables/useUpdater'

interface AppConfig {
  alert_cpu_pct: number
  alert_mem_pct: number
  autostart: boolean
  auto_update: boolean
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
  /** TASK-028：磁盘分析参数（旧配置缺失为 null，UI 回退默认 100/3） */
  disk_scan?: { min_bytes_mb?: number | null; dir_depth?: number | null } | null
}

const CATEGORY_OPTIONS = [
  { value: 'temp', label: '临时文件' },
  { value: 'browser_cache', label: '浏览器缓存' },
  { value: 'system_cache', label: '系统缓存' },
  { value: 'cache', label: '缓存（旧版）' },
  { value: 'logs', label: '日志' },
  { value: 'app_cache', label: '应用缓存' },
  { value: 'dev_cache', label: '开发缓存' },
]

const LEVEL_OPTIONS = [
  { value: 'safe', label: '安全（默认勾选）' },
  { value: 'confirm', label: '需确认（不勾选）' },
]

// TASK-028：扫描与清理参数（大文件阈值 + 目录占用分解层数）
const MIN_MB_OPTIONS = [
  { value: 100, label: '≥ 100 MB' },
  { value: 500, label: '≥ 500 MB' },
  { value: 1000, label: '≥ 1 GB' },
]
const DEPTH_OPTIONS = [1, 2, 3, 4, 5].map((v) => ({ value: v, label: `${v} 层` }))
const minBytesMb = ref(100)
const dirDepth = ref(3)

const { setAlertThresholds } = useMonitor()

const cpuPct = ref(80)
const memPct = ref(85)
const autostart = ref(false)
const loading = ref(true)
const saving = ref(false)
const savedMsg = ref('')
/** 保存失败原始错误（toast 复制按钮使用） */
const saveError = ref('')
let savedTimer: ReturnType<typeof setTimeout> | null = null

// 软件更新（tauri-plugin-updater，状态集中在 useUpdater 单例）
const appVersion = ref('')
const updateMsg = ref('')
/** 更新失败原始错误（toast 复制按钮使用） */
const updateError = ref('')
let updateTimer: ReturnType<typeof setTimeout> | null = null

function flashUpdateMsg(msg: string) {
  updateMsg.value = msg
  if (updateTimer) clearTimeout(updateTimer)
  updateTimer = setTimeout(() => { updateMsg.value = '' }, 6000)
}

async function handleCheckUpdate() {
  if (checkingUpdate.value || downloadingUpdate.value) return
  flashUpdateMsg('')
  const version = await checkForUpdate()
  if (version) {
    flashUpdateMsg(`发现新版本 ${version}，可点击安装`)
  } else {
    flashUpdateMsg('✓ 已是最新版本')
  }
}

async function handleInstallUpdate() {
  if (downloadingUpdate.value) return
  flashUpdateMsg('')
  try {
    const found = await installUpdate()
    if (!found) {
      flashUpdateMsg('✓ 已是最新版本')
      return
    }
    // Windows：passive 安装器会接管进程（/R 自动重启），走不到这里；
    // macOS/Linux 需显式重启完成替换
    await relaunch()
  } catch (e) {
    updateMsg.value = ''
    updateError.value = String(e)
  }
}

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
  // 进入设置页：清除设置 tab 的更新角标（用户已注意到）
  markUpdateSeen()
  try {
    appVersion.value = await getVersion()
  } catch {
    // 版本获取失败不阻塞设置页
  }
  try {
    const cfg = await invoke<AppConfig>('get_config')
    cpuPct.value = cfg.alert_cpu_pct || 80
    memPct.value = cfg.alert_mem_pct || 85
    autostart.value = cfg.autostart ?? false
    autoUpdateEnabled.value = cfg.auto_update ?? true
  } catch {
    // 使用默认值
  }
  try {
    const cleanCfg = await invoke<CleanConfig>('get_clean_config')
    customTargets.value = cleanCfg.custom_targets ?? []
    // TASK-028：旧配置 disk_scan 缺失/null → 回退默认
    minBytesMb.value = cleanCfg.disk_scan?.min_bytes_mb ?? 100
    dirDepth.value = cleanCfg.disk_scan?.dir_depth ?? 3
  } catch {
    // 无清理配置
  }
  loading.value = false
})

onUnmounted(() => {
  if (savedTimer) clearTimeout(savedTimer)
  if (ruleTimer) clearTimeout(ruleTimer)
  if (updateTimer) clearTimeout(updateTimer)
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
        auto_update: autoUpdateEnabled.value,
      },
    })
    setAlertThresholds(cpuPct.value, memPct.value)

    // 保存清理配置（保留原有字段，更新自定义目标 + TASK-028 扫描参数）
    const cleanCfg = await invoke<CleanConfig>('get_clean_config')
    cleanCfg.custom_targets = customTargets.value
    cleanCfg.disk_scan = { min_bytes_mb: minBytesMb.value, dir_depth: dirDepth.value }
    await invoke('save_clean_config', { config: cleanCfg })

    savedMsg.value = '✓ 已保存'
    if (savedTimer) clearTimeout(savedTimer)
    savedTimer = setTimeout(() => { savedMsg.value = '' }, 2500)
  } catch (e) {
    savedMsg.value = ''
    saveError.value = String(e)
  }
  saving.value = false
}

function categoryLabel(value: string) {
  return CATEGORY_OPTIONS.find(o => o.value === value)?.label ?? value
}
</script>

<template>
  <div class="scrollbar-none flex h-full flex-col gap-6 overflow-y-auto px-1 py-1">
    <div v-if="loading" class="flex flex-1 items-center justify-center">
      <Loader2 class="h-4 w-4 animate-spin text-muted-foreground" />
    </div>

    <template v-else>
      <!-- ═══ 告警阈值：边距分区，无背景块 ═══ -->
      <section class="space-y-4">
        <h3 class="text-[11px] font-semibold text-foreground/85">告警阈值</h3>

        <div class="space-y-4">
          <!-- CPU threshold -->
          <div>
            <div class="flex items-baseline justify-between">
              <span class="text-xs text-foreground/90">CPU 告警阈值</span>
              <span class="text-xs tabular-nums text-primary">{{ cpuPct }}%</span>
            </div>
            <input
              v-model.number="cpuPct"
              type="range"
              min="50"
              max="100"
              step="5"
              class="slider-capsule mt-2.5 w-full"
            />
            <p class="mt-1 text-[10px] text-muted-foreground/70">CPU 占用超过该值时发送系统通知</p>
          </div>

          <!-- Memory threshold -->
          <div>
            <div class="flex items-baseline justify-between">
              <span class="text-xs text-foreground/90">内存告警阈值</span>
              <span class="text-xs tabular-nums text-primary">{{ memPct }}%</span>
            </div>
            <input
              v-model.number="memPct"
              type="range"
              min="50"
              max="100"
              step="5"
              class="slider-capsule mt-2.5 w-full"
            />
            <p class="mt-1 text-[10px] text-muted-foreground/70">内存占用超过该值时发送系统通知</p>
          </div>
        </div>
      </section>

      <!-- ═══ 启动 ═══ -->
      <section class="space-y-2.5">
        <h3 class="text-[11px] font-semibold text-foreground/85">启动</h3>
        <div class="flex items-center justify-between">
          <div>
            <span class="text-xs text-foreground/90">开机自启</span>
            <p class="mt-0.5 text-[10px] text-muted-foreground/70">登录 Windows 后自动启动 PonyClean</p>
          </div>
          <button
            class="relative h-4 w-7 shrink-0 rounded-full transition-colors"
            :class="autostart ? 'bg-success/80' : 'bg-white/15'"
            role="switch"
            :aria-checked="autostart"
            @click="autostart = !autostart"
          >
            <span
              class="absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-all"
              :class="autostart ? 'left-[14px]' : 'left-0.5'"
            />
          </button>
        </div>
      </section>

      <!-- ═══ 自定义清理规则 ═══ -->
      <section class="space-y-2.5">
        <div class="flex items-center justify-between">
          <h3 class="text-[11px] font-semibold text-foreground/85">自定义清理规则</h3>
          <button
            class="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-primary/15 hover:text-primary"
            title="添加规则"
            @click="showAddForm = !showAddForm"
          >
            <Plus class="h-3.5 w-3.5" />
          </button>
        </div>
        <p class="text-[10px] text-muted-foreground/70">
          添加你自己的缓存/临时目录，扫描时一并纳入（受保护路径自动跳过）
        </p>

        <!-- Add form -->
        <div v-if="showAddForm" class="space-y-2 pt-0.5">
          <input
            v-model="newPath"
            type="text"
            placeholder="目录路径，支持 %TEMP% 等环境变量"
            class="w-full border-0 border-b border-white/10 bg-transparent px-0.5 pb-1 pt-0.5 text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:border-primary/60 focus:outline-none"
          />
          <div class="flex gap-2">
            <OptionPicker
              v-model="newCategory"
              :options="CATEGORY_OPTIONS"
            />
            <OptionPicker
              v-model="newLevel"
              :options="LEVEL_OPTIONS"
            />
          </div>
          <input
            v-model="newDesc"
            type="text"
            placeholder="描述（可选）"
            class="w-full border-0 border-b border-white/10 bg-transparent px-0.5 pb-1 pt-0.5 text-[11px] text-foreground placeholder:text-muted-foreground/50 focus:border-primary/60 focus:outline-none"
          />
          <div class="flex items-center justify-end gap-1.5">
            <span v-if="ruleMsg" class="text-[10px] text-warning">{{ ruleMsg }}</span>
            <Button size="sm" variant="ghost" @click="showAddForm = false">取消</Button>
            <Button size="sm" @click="addRule">添加</Button>
          </div>
        </div>

        <!-- Rule list -->
        <div v-if="customTargets.length > 0" class="space-y-0.5 pt-0.5">
          <div
            v-for="(rule, i) in customTargets"
            :key="rule.id"
            class="group flex items-center gap-2 rounded px-1 py-1 transition-colors hover:bg-muted/20"
          >
            <button
              class="relative h-4 w-7 shrink-0 rounded-full transition-colors"
              :class="rule.enabled ? 'bg-success/70' : 'bg-white/15'"
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
              class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-muted-foreground/60 opacity-60 transition-all group-hover:opacity-100 hover:bg-destructive/20 hover:text-destructive"
              title="删除规则"
              @click="removeRule(i)"
            >
              <Trash2 class="h-3 w-3" />
            </button>
          </div>
        </div>
        <p v-else-if="!showAddForm" class="text-[10px] text-muted-foreground/50">暂无自定义规则</p>
      </section>

      <!-- ═══ 扫描与清理参数（TASK-028） ═══ -->
      <section class="space-y-2.5">
        <h3 class="text-[11px] font-semibold text-foreground/85">扫描与清理参数</h3>
        <p class="text-[10px] text-muted-foreground/70">
          调整空间分析的范围，保存后下次扫描生效
        </p>
        <div class="flex gap-2">
          <OptionPicker v-model="minBytesMb" :options="MIN_MB_OPTIONS" />
          <OptionPicker v-model="dirDepth" :options="DEPTH_OPTIONS" />
        </div>
        <p class="text-[10px] text-muted-foreground/60">
          大文件最小体积 · 目录占用分解层数（层数只影响目录分解粒度，不影响扫描范围）
        </p>
      </section>

      <!-- ═══ 软件更新 ═══ -->
      <section class="space-y-2.5">
        <div class="flex items-baseline justify-between">
          <h3 class="text-[11px] font-semibold text-foreground/85">软件更新</h3>
          <span v-if="appVersion" class="text-[10px] tabular-nums text-muted-foreground/60">当前版本 v{{ appVersion }}</span>
        </div>
        <p class="text-[10px] text-muted-foreground/70">
          自动检查新版本并提醒；安装由你确认，安装后自动重启应用
        </p>

        <!-- 自动更新开关 -->
        <div class="flex items-center justify-between">
          <div>
            <span class="text-xs text-foreground/90">自动检查更新</span>
            <p class="mt-0.5 text-[10px] text-muted-foreground/70">每 1 小时自动检查，发现新版本提醒，不自动安装</p>
          </div>
          <button
            class="relative h-4 w-7 shrink-0 rounded-full transition-colors"
            :class="autoUpdateEnabled ? 'bg-success/80' : 'bg-white/15'"
            role="switch"
            :aria-checked="autoUpdateEnabled"
            @click="autoUpdateEnabled = !autoUpdateEnabled"
          >
            <span
              class="absolute top-0.5 h-3 w-3 rounded-full bg-white shadow transition-all"
              :class="autoUpdateEnabled ? 'left-[14px]' : 'left-0.5'"
            />
          </button>
        </div>

        <!-- 更新状态 + 操作 -->
        <div class="flex items-center justify-between gap-2">
          <div class="min-w-0 flex-1">
            <!-- 下载中：进度条 + 单个 loading，隐藏安装按钮 -->
            <div v-if="downloadingUpdate" class="flex items-center gap-2">
              <Loader2 class="h-3 w-3 shrink-0 animate-spin text-primary" />
              <Progress :model-value="downloadProgress" class="h-1.5 min-w-0 flex-1" />
              <span class="shrink-0 text-[10px] tabular-nums text-muted-foreground">
                {{ downloadProgress === null ? '准备中…' : `${downloadProgress}%` }}
              </span>
            </div>
            <span
              v-else-if="updateAvailable && updateVersion"
              class="text-[11px] text-warning"
            >
              有可用更新 v{{ updateVersion }}
            </span>
            <span v-else-if="updateMsg" class="text-[11px]" :class="updateMsg.startsWith('✓') ? 'text-success' : 'text-destructive'">
              {{ updateMsg }}
            </span>
          </div>

          <div class="flex shrink-0 items-center gap-1.5">
            <Button
              v-if="updateAvailable"
              size="icon-sm"
              variant="ghost"
              :disabled="checkingUpdate || downloadingUpdate"
              title="下载并安装更新"
              aria-label="下载并安装更新"
              @click="handleInstallUpdate"
            >
              <Loader2 v-if="downloadingUpdate" class="h-3.5 w-3.5 animate-spin" />
              <Download v-else class="h-3.5 w-3.5" />
            </Button>
            <Button
              size="icon-sm"
              variant="ghost"
              :disabled="checkingUpdate || downloadingUpdate"
              title="检查更新"
              aria-label="检查更新"
              @click="handleCheckUpdate"
            >
              <RefreshCw v-if="!checkingUpdate" class="h-3.5 w-3.5" />
              <Loader2 v-else class="h-3.5 w-3.5 animate-spin" />
            </Button>
          </div>
        </div>
      </section>

      <!-- Save -->
      <div class="mt-auto flex items-center justify-between pt-2">
        <span v-if="savedMsg" class="text-[11px]" :class="savedMsg.startsWith('✓') ? 'text-success' : 'text-destructive'">
          {{ savedMsg }}
        </span>
        <span v-else class="text-[10px] text-muted-foreground/60">设置保存在本地配置文件中</span>
        <Button
          size="icon-sm"
          variant="ghost"
          :disabled="saving"
          title="保存设置"
          aria-label="保存设置"
          @click="handleSave"
        >
          <Save v-if="!saving" class="h-3.5 w-3.5" />
          <Loader2 v-else class="h-3.5 w-3.5 animate-spin" />
        </Button>
      </div>
    </template>

    <!-- ═══ 保存失败 toast ═══ -->
    <Toast
      :show="!!saveError"
      :message="`保存失败：${saveError}`"
      :raw-error="saveError"
      variant="error"
      @close="saveError = ''"
    />

    <!-- ═══ 更新失败 toast ═══ -->
    <Toast
      :show="!!updateError"
      :message="`更新失败：${humanizeError(updateError)}`"
      :raw-error="updateError"
      variant="error"
      @close="updateError = ''"
    />
  </div>
</template>