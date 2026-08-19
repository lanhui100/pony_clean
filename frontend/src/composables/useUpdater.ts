import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { check } from '@tauri-apps/plugin-updater'

/**
 * 自动更新全局状态（单例模式，供 TitleBar 角标 / SettingsPanel 开关共用）。
 *
 * 定时检查策略：
 *  - 应用启动后 3 秒首次检查（只提示角标，不自动装）
 *  - 之后每 1 小时检查一次（与 VS Code / Discord 等主流一致），若用户开启了
 *    「自动更新」则发现新版本自动下载安装；检查仅拉取 latest.json，开销极低
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
 * 检查更新。autoInstall 由调用方决定：
 *  - 定时器自动检查：按 autoUpdateEnabled 决定是否自动安装
 *  - 设置页手动点击：仅提示（installMode 由用户交互驱动）
 * @returns 更新版本号（有更新）/ null（无更新或失败）
 */
export async function checkForUpdate(autoInstall = false): Promise<string | null> {
  if (checkingUpdate.value) return null
  checkingUpdate.value = true
  try {
    const update = await check()
    if (!update) {
      return null
    }
    updateVersion.value = update.version
    if (autoInstall && autoUpdateEnabled.value) {
      // 自动更新：下载并安装（passive 模式，完成后自动重启）
      await update.downloadAndInstall()
      updateAvailable.value = false
      return update.version
    }
    // 仅提醒：设置 tab 显示角标
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
  // 定时自动检查：若开启自动更新则直接安装，否则仅置角标
  await checkForUpdate(true)
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
