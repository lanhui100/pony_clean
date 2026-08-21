import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { check } from '@tauri-apps/plugin-updater'

/**
 * 自动更新全局状态（单例模式，供 TitleBar 角标 / SettingsPanel 开关共用）。
 *
 * 定时检查策略（VS Code 模式：只提醒，不代用户做主）：
 *  - 应用启动后 3 秒首次检查（只提示角标）
 *  - 之后每 1 小时检查一次，发现新版本置角标提醒；是否安装由用户在设置页
 *    点击「立即安装」确认触发——不在使用中静默强杀进程（Tauri Windows 安装器
 *    在 passive 模式下会直接 kill 运行中的应用，可能打断正在执行的清理任务）
 *
 * 更新可用状态 updateAvailable 暴露给 TitleBar：设置 tab 显示角标提醒。
 */

const CHECK_INTERVAL_MS = 60 * 60 * 1000 // 1 小时
const FIRST_CHECK_DELAY_MS = 3000 // 启动后 3 秒

export const updateAvailable = ref(false)
export const updateVersion = ref('')
export const checkingUpdate = ref(false)
export const autoUpdateEnabled = ref(true)

let timer: ReturnType<typeof setInterval> | null = null
let inited = false

export async function loadAutoUpdateConfig() {
  try {
    const cfg = await invoke<{ auto_update?: boolean }>('get_config')
    autoUpdateEnabled.value = cfg.auto_update ?? true
  } catch {
    // 保持默认
  }
}

/** 清除设置 tab 角标（用户进入设置页后调用） */
export function markUpdateSeen() {
  updateAvailable.value = false
}

/**
 * 检查更新。发现新版本仅置角标提醒，安装一律由用户在设置页手动触发。
 * @returns 更新版本号（有更新）/ null（无更新或失败）
 */
export async function checkForUpdate(): Promise<string | null> {
  if (checkingUpdate.value) return null
  checkingUpdate.value = true
  try {
    const update = await check()
    if (!update) {
      return null
    }
    updateVersion.value = update.version
    updateAvailable.value = true
    return update.version
  } catch (e) {
    console.warn('check update failed:', e)
    return null
  } finally {
    checkingUpdate.value = false
  }
}

async function runScheduledCheck() {
  // 开关只控制「自动检查」；发现更新仅提醒，是否安装由用户决定
  if (!autoUpdateEnabled.value) return
  await checkForUpdate()
}

export function initUpdater() {
  if (inited) return
  inited = true

  loadAutoUpdateConfig()
  // 启动后 3 秒首次检查（不自动安装，只提示角标）
  setTimeout(() => {
    checkForUpdate(false)
  }, FIRST_CHECK_DELAY_MS)

  // 周期检查（自动安装与否由配置决定）
  timer = setInterval(runScheduledCheck, CHECK_INTERVAL_MS)
}

export function disposeUpdater() {
  if (timer) {
    clearInterval(timer)
    timer = null
  }
  inited = false
}
