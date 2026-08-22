import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { check } from '@tauri-apps/plugin-updater'

/**
 * 自动更新全局状态（单例模式，供 TitleBar 角标 / SettingsPanel 开关共用）。
 *
 * 定时检查策略（VS Code 模式：只提醒，不代用户做主）：
 *  - 应用启动后 3 秒首次检查（只提示角标）
 *  - 之后每 1 小时检查一次，发现新版本置角标提醒；是否安装由用户在设置页
 *    点击安装按钮确认触发——不在使用中静默强杀进程（Tauri Windows 安装器
 *    在 passive 模式下会直接 kill 运行中的应用，可能打断正在执行的清理任务）
 *
 * 角标状态 updateBadgeVisible 暴露给 TitleBar：设置 tab 显示角标提醒；
 * 更新可用状态 updateAvailable 供 SettingsPanel 展示，两者独立互不影响。
 */

const CHECK_INTERVAL_MS = 60 * 60 * 1000 // 1 小时
const FIRST_CHECK_DELAY_MS = 3000 // 启动后 3 秒

export const updateAvailable = ref(false)
export const updateVersion = ref('')
/** 设置 tab 角标是否显示（进入设置页后由 markUpdateSeen 清除，不影响 updateAvailable） */
export const updateBadgeVisible = ref(false)
export const checkingUpdate = ref(false)
/** 正在下载安装更新（与 checkingUpdate 分离，避免双 loading） */
export const downloadingUpdate = ref(false)
/** 下载进度百分比（0-100），null 表示尚未开始接收数据 */
export const downloadProgress = ref<number | null>(null)
export const autoUpdateEnabled = ref(true)
/** 最近一次检查更新的失败原因（空 = 成功）。用于区分「检查失败」与「确认无更新」——
 *  双端点全失败时 check() 会抛错，此前被吞成 null 导致 UI 误报「已是最新版本」 */
export const lastCheckError = ref('')

const CHECK_MAX_ATTEMPTS = 2
const CHECK_RETRY_DELAY_MS = 1500

/** 单次请求超时：插件默认无超时（reqwest 黑洞连接会让 await 永不返回），
 *  必须显式传入，否则重试循环无法介入、UI 永久停在「准备中…」 */
const CHECK_TIMEOUT_MS = 15_000
/** 下载总超时：须覆盖慢网完整安装包下载，不能设 30s 级 */
const DOWNLOAD_TIMEOUT_MS = 600_000

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

/** 清除设置 tab 角标（用户进入设置页后调用），不影响面板内的更新可用状态 */
export function markUpdateSeen() {
  updateBadgeVisible.value = false
}

/**
 * 检查更新（带重试）。发现新版本仅置角标提醒，安装一律由用户在设置页手动触发。
 * 失败时不抛出（定时任务需静默），错误写入 lastCheckError 供 UI 区分提示。
 * @returns 更新版本号（有更新）/ null（无更新或失败，配合 lastCheckError 判断）
 */
export async function checkForUpdate(): Promise<string | null> {
  if (checkingUpdate.value || downloadingUpdate.value) return null
  checkingUpdate.value = true
  lastCheckError.value = ''
  try {
    let lastErr: unknown = null
    for (let attempt = 1; attempt <= CHECK_MAX_ATTEMPTS; attempt++) {
      try {
        const update = await check({ timeout: CHECK_TIMEOUT_MS })
        if (!update) {
          return null
        }
        updateVersion.value = update.version
        updateAvailable.value = true
        updateBadgeVisible.value = true
        return update.version
      } catch (e) {
        lastErr = e
        console.warn(`check update attempt ${attempt}/${CHECK_MAX_ATTEMPTS} failed:`, e)
        if (attempt < CHECK_MAX_ATTEMPTS) {
          await new Promise((resolve) => setTimeout(resolve, CHECK_RETRY_DELAY_MS * attempt))
        }
      }
    }
    lastCheckError.value = String(lastErr)
    return null
  } finally {
    checkingUpdate.value = false
  }
}

/**
 * 下载安装重试参数：更新包直连 GitHub Release 下载地址，瞬时网络故障
 * （reqwest `error sending request`，部分网络环境对 release 资产的间歇性
 * 阻断）高发。最多 3 次尝试、线性退避；每次尝试重新 check() 刷新清单与
 * 签名地址（更新源自 v0.3.4 起收敛为 GitHub 单端点，见 ADR-013）。
 */
const INSTALL_MAX_ATTEMPTS = 3
const INSTALL_RETRY_DELAY_MS = 2000

/**
 * 下载并安装更新（带重试），通过回调实时上报下载进度到 downloadProgress。
 * @returns true（已开始/完成下载安装）/ false（无可用更新）
 * @throws 重试耗尽后仍失败时抛出最后一次的原始错误
 */
export async function installUpdate(): Promise<boolean> {
  if (downloadingUpdate.value) return false
  downloadingUpdate.value = true
  downloadProgress.value = null
  try {
    let lastErr: unknown = null
    for (let attempt = 1; attempt <= INSTALL_MAX_ATTEMPTS; attempt++) {
      try {
        const update = await check({ timeout: CHECK_TIMEOUT_MS })
        if (!update) {
          // 已确认存在待更版本时（按钮仅在 updateAvailable 时出现），check() 为空
          // 只说明本次命中的端点清单过期/异常（典型：GitHub 下载失败后重检回落到
          // 尚未同步新版本的备用端点），绝不能误判为「已是最新版本」并清除状态——
          // 按本次尝试失败处理，继续重试换源；耗尽后抛错走「更新失败」toast。
          // 合成错误带标记名：不覆盖已捕获的真实传输错误（见下方 catch）。
          if (!updateAvailable.value || !updateVersion.value) {
            return false
          }
          const stale = new Error(
            `更新服务暂时不可用，请稍后重试，或到发布页手动下载 v${updateVersion.value}`,
          )
          stale.name = 'StaleUpdateManifest'
          throw stale
        }
        updateVersion.value = update.version
        updateAvailable.value = true
        let downloaded = 0
        let total = 0
        // 每次尝试重置进度（重试时进度条从准备中重新开始）
        downloadProgress.value = null
        await update.downloadAndInstall(
          (event) => {
            if (event.event === 'Started') {
              total = event.data.contentLength ?? 0
              downloadProgress.value = 0
            } else if (event.event === 'Progress') {
              downloaded += event.data.chunkLength
              if (total > 0) {
                downloadProgress.value = Math.min(100, Math.round((downloaded / total) * 100))
              }
            } else if (event.event === 'Finished') {
              downloadProgress.value = 100
            }
          },
          { timeout: DOWNLOAD_TIMEOUT_MS },
        )
        return true
      } catch (e) {
        // 合成错误（清单滞后）不覆盖真实传输错误：耗尽后优先抛最近一次真实错误，
        // 让 humanizeError 的可操作建议（检查网络/到发布页手动下载）得以命中；
        // 合成错误仅在尚无任何已捕获错误时兜底。
        const synthetic = e instanceof Error && e.name === 'StaleUpdateManifest'
        if (!synthetic || lastErr === null) {
          lastErr = e
        }
        // 进入退避前重置进度：避免退避等待期间 UI 冻结在旧百分比造成「卡死」错觉
        downloadProgress.value = null
        console.warn(`install update attempt ${attempt}/${INSTALL_MAX_ATTEMPTS} failed:`, e)
        if (attempt < INSTALL_MAX_ATTEMPTS) {
          await new Promise((resolve) => setTimeout(resolve, INSTALL_RETRY_DELAY_MS * attempt))
        }
      }
    }
    throw lastErr
  } finally {
    downloadingUpdate.value = false
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
    checkForUpdate()
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
