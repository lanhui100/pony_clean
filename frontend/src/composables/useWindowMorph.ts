import { ref, onMounted, onUnmounted, nextTick, type Ref } from 'vue'
import { getCurrentWindow, LogicalSize, PhysicalPosition, Window } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { WINDOW_MORPH, type CapsuleForm } from '@/lib/windowMorphConfig'

export type IslandState = 'idle' | 'entering' | 'visible' | 'leaving'

const win = getCurrentWindow()
const DOCK_KEY = 'ponyclean.dock'

interface MonitorBounds {
  left: number
  top: number
  right: number
  bottom: number
  width: number
}

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(v, max))
}

export function useWindowMorph(scanning: Ref<boolean>) {
  // ─── 状态 ───
  const islandState = ref<IslandState>('idle')
  const capsuleHovered = ref(false)
  const isInsideIsland = ref(false)
  const form = ref<CapsuleForm>('pill')

  // 胶囊沿顶边的水平位置（物理像素，距工作区 left 的 X 偏移）
  const dockX = ref(0)

  // ─── 拖动状态（仅沿顶边水平拖动） ───
  let currentWinX = 0
  let currentWinY = 0
  let dragStartX = 0
  let dragStartWinX = 0
  const isDragging = ref(false)
  let dragMoved = false
  let dragRafId: number | null = null
  let pendingDragX: number | null = null
  let onMoveRef: ((e: MouseEvent) => void) | null = null
  let onUpRef: (() => void) | null = null
  let suppressNextCapsuleClick = false

  // ─── 定时器 ───
  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let idlePollTimer: ReturnType<typeof setTimeout> | null = null
  let barTimer: ReturnType<typeof setTimeout> | null = null
  let barHoverTimer: ReturnType<typeof setTimeout> | null = null
  let leaveWatchdog: ReturnType<typeof setTimeout> | null = null
  let lastActivityMs = Date.now()
  let unlistenIslandEnter: UnlistenFn | null = null
  let unlistenIslandLeave: UnlistenFn | null = null
  let unlistenIslandActivity: UnlistenFn | null = null

  // island 收起动画期间再次请求展开时，等待动画完成后重试
  let pendingShowAfterLeave = false

  /** ─── 显示器工作区（排除任务栏等系统区域，物理像素） ─── */
  async function getMonitorBounds(): Promise<MonitorBounds> {
    try {
      const wa = await invoke<{ left: number; top: number; right: number; bottom: number }>(
        'get_monitor_work_area',
      )
      if (wa) {
        return {
          left: wa.left,
          top: wa.top,
          right: wa.right,
          bottom: wa.bottom,
          width: wa.right - wa.left,
        }
      }
    } catch {
      // 回退到 currentMonitor
    }
    try {
      const m = await win.currentMonitor()
      if (m) {
        return {
          left: m.position.x,
          top: m.position.y,
          right: m.position.x + m.size.width,
          bottom: m.position.y + m.size.height,
          width: m.size.width,
        }
      }
    } catch {
      // 回退到浏览器屏幕
    }
    const dpr = window.devicePixelRatio || 1
    const width = Math.round(window.screen.width * dpr)
    return {
      left: 0,
      top: 0,
      right: width,
      bottom: Math.round(window.screen.height * dpr),
      width,
    }
  }

  async function getIslandWindow(): Promise<Window | null> {
    return Window.getByLabel('island').catch(() => null)
  }

  /** ─── 水平位置持久化 ─── */
  function persistDock() {
    try {
      localStorage.setItem(DOCK_KEY, JSON.stringify({ x: dockX.value }))
    } catch {
      // 忽略
    }
  }

  function loadDock(): { x: number } | null {
    try {
      const raw = localStorage.getItem(DOCK_KEY)
      if (!raw) return null
      const obj = JSON.parse(raw)
      if (typeof obj.x === 'number' && Number.isFinite(obj.x)) return { x: obj.x }
    } catch {
      // 解析失败使用默认
    }
    return null
  }

  /** ─── 原生层同步：胶囊窗口恒为顶部贴边 ─── */
  async function syncGeometryToBackend() {
    try {
      await invoke('set_capsule_geometry', { form: form.value, edge: 'top' })
    } catch (e) {
      console.warn('set_capsule_geometry failed:', e)
    }
  }

  /** 应用窗口位置：顶边贴边（x = 工作区 left + dockX），并同步原生命中区域 */
  async function applyWindowGeometry() {
    const dpr = window.devicePixelRatio || 1
    const monitor = await getMonitorBounds()
    const winW = Math.round(WINDOW_MORPH.capsuleW * dpr)
    let x = monitor.left + dockX.value
    const maxX = Math.max(monitor.left, monitor.right - winW)
    x = clamp(x, monitor.left, maxX)
    dockX.value = x - monitor.left
    await win.setPosition(new PhysicalPosition(x, monitor.top)).catch(() => {})
    currentWinX = x
    currentWinY = monitor.top
    await syncGeometryToBackend()
    persistDock()
  }

  /** 重置胶囊到顶边居中（托盘菜单“重置胶囊位置”触发） */
  async function resetToDefault() {
    try {
      localStorage.removeItem(DOCK_KEY)
    } catch {
      // 忽略
    }
    const dpr = window.devicePixelRatio || 1
    const monitor = await getMonitorBounds()
    dockX.value = Math.round((monitor.width - Math.round(WINDOW_MORPH.capsuleW * dpr)) / 2)
    form.value = 'pill'
    await applyWindowGeometry()
    await win.show().catch(() => {})
    console.log('[PonyClean] capsule reset to default top-center')
  }

  /** ─── 形态切换（胶囊 ⇄ 进度条） ─── */
  async function expandToPill() {
    if (form.value === 'pill') {
      resetBarTimer()
      return
    }
    form.value = 'pill'
    await syncGeometryToBackend()
    resetBarTimer()
  }

  async function collapseToBar() {
    if (form.value === 'bar' || islandState.value !== 'idle') return
    if (isDragging.value || scanning.value || capsuleHovered.value) return
    form.value = 'bar'
    await syncGeometryToBackend()
  }

  /** 胶囊无操作收起计时：island 收起后 10s 缩成贴边进度条 */
  function resetBarTimer() {
    if (barTimer) clearTimeout(barTimer)
    barTimer = null
    if (form.value !== 'pill' || islandState.value !== 'idle' || scanning.value) return
    barTimer = setTimeout(() => {
      barTimer = null
      if (isInsideIsland.value || scanning.value || isDragging.value || capsuleHovered.value) {
        resetBarTimer()
        return
      }
      collapseToBar()
    }, WINDOW_MORPH.barTimeout)
  }

  /** 用户活动刷新：同时刷新 island 收起计时与胶囊收起计时 */
  function notifyActivity() {
    resetIdleTimer()
    resetBarTimer()
  }

  /** ─── island 展开/收起（从胶囊正下方展开） ─── */
  async function positionIslandWindow(island: Window) {
    const dpr = window.devicePixelRatio || 1
    const monitor = await getMonitorBounds()
    const capsulePos = await win.outerPosition().catch(() => ({ x: currentWinX, y: currentWinY }))
    const capsuleSize = await win.outerSize().catch(() => ({
      width: Math.round(WINDOW_MORPH.capsuleW * dpr),
      height: Math.round(WINDOW_MORPH.capsuleH * dpr),
    }))
    const islandW = Math.round(WINDOW_MORPH.fullW * dpr)
    const edgePx = Math.round(WINDOW_MORPH.edgePadding * dpr)
    const centerX = capsulePos.x + Math.round(capsuleSize.width / 2)
    const x = clamp(
      centerX - Math.round(islandW / 2),
      monitor.left + edgePx,
      monitor.right - islandW - edgePx,
    )
    await island.setPosition(new PhysicalPosition(x, capsulePos.y)).catch(() => {})
  }

  async function showIsland() {
    if (islandState.value === 'visible' || islandState.value === 'entering') return
    if (islandState.value === 'leaving') {
      pendingShowAfterLeave = true
      return
    }
    const island = await getIslandWindow()
    if (!island) return
    if (form.value === 'bar') await expandToPill()
    // 先切到展开尺寸，再定位，避免展开后底部跑偏
    try {
      await invoke('set_island_expanded', { expanded: true })
    } catch {
      // 忽略
    }
    await positionIslandWindow(island)
    await emitTo('island', 'island-enter').catch(() => {})
    await new Promise<void>((resolve) => setTimeout(resolve, 0))
    await island.show().catch(() => {})
    islandState.value = 'entering'
  }

  function onCapsuleClick() {
    if (suppressNextCapsuleClick) {
      suppressNextCapsuleClick = false
      return
    }
    showIsland()
  }

  function hideIsland() {
    if (islandState.value === 'idle' || islandState.value === 'leaving') return
    win.show().catch(() => {})
    islandState.value = 'leaving'
    emitTo('island', 'island-leave').catch(() => {})
    // 加固（P2-1）：onLeaveDone 依赖 motion-v 的 complete 回调；在 entering 极早期点击
    // （胶囊淡出层目标态与当前态几乎无差）可能不派发 complete，leaving 会卡住。
    // 1.5s 看门狗强制收尾（onLeaveDone 幂等，动画若稍后完成再触发为 no-op）。
    if (leaveWatchdog) clearTimeout(leaveWatchdog)
    leaveWatchdog = setTimeout(() => {
      leaveWatchdog = null
      if (islandState.value === 'leaving') onLeaveDone()
    }, 1500)
  }

  function onEnterDone() {
    if (islandState.value === 'entering') {
      islandState.value = 'visible'
      startIdleDetection()
      win.hide().catch(() => {})
    }
  }

  async function onLeaveDone() {
    if (islandState.value === 'leaving') {
      const island = await getIslandWindow()
      if (island) {
        await island.hide().catch(() => {})
      }
      islandState.value = 'idle'
      stopIdleDetection()
      resetBarTimer()
      if (pendingShowAfterLeave) {
        pendingShowAfterLeave = false
        showIsland()
      }
    }
  }

  /** ─── island 无操作检测 ─── */
  function resetIdleTimer() {
    lastActivityMs = Date.now()
    if (idleTimer) clearTimeout(idleTimer)
    if (scanning.value || islandState.value !== 'visible') return
    idleTimer = setTimeout(() => {
      if (isInsideIsland.value) {
        resetIdleTimer()
        return
      }
      hideIsland()
    }, WINDOW_MORPH.idleTimeout)
  }

  async function pollIdle() {
    if (islandState.value !== 'visible') {
      scheduleNextPoll()
      return
    }
    if (scanning.value || isInsideIsland.value) {
      resetIdleTimer()
      scheduleNextPoll()
      return
    }
    const elapsed = Date.now() - lastActivityMs
    if (elapsed >= WINDOW_MORPH.idleTimeout) {
      try {
        if ((await invoke<number>('get_system_idle_ms')) >= WINDOW_MORPH.idleTimeout) hideIsland()
      } catch {
        // 查询失败走本地计时
      }
    }
    scheduleNextPoll()
  }

  function scheduleNextPoll() {
    idlePollTimer = setTimeout(
      pollIdle,
      WINDOW_MORPH.idlePollInterval,
    ) as unknown as ReturnType<typeof setTimeout>
  }

  function startIdleDetection() {
    resetIdleTimer()
    scheduleNextPoll()
  }

  function stopIdleDetection() {
    if (idleTimer) clearTimeout(idleTimer)
    if (idlePollTimer) clearTimeout(idlePollTimer)
    idleTimer = null
    idlePollTimer = null
  }

  /** ─── 鼠标事件 ─── */
  function onCapsuleEnter() {
    capsuleHovered.value = true
    resetBarTimer()
  }

  function onCapsuleLeave() {
    capsuleHovered.value = false
    resetBarTimer()
  }

  /** 进度条 hover：延迟展开为胶囊 */
  function onBarEnter() {
    capsuleHovered.value = true
    if (barHoverTimer) clearTimeout(barHoverTimer)
    barHoverTimer = setTimeout(() => {
      barHoverTimer = null
      if (form.value === 'pill' || islandState.value !== 'idle') return
      if (isDragging.value) return
      expandToPill()
    }, WINDOW_MORPH.barHoverDelay)
  }

  function onBarLeave() {
    capsuleHovered.value = false
    if (barHoverTimer) clearTimeout(barHoverTimer)
    barHoverTimer = null
    resetBarTimer()
  }

  function onIslandEnter() {
    isInsideIsland.value = true
    resetIdleTimer()
  }

  function onIslandLeave() {
    isInsideIsland.value = false
    capsuleHovered.value = false
    resetIdleTimer()
  }

  function onIslandUserActivity() {
    resetIdleTimer()
  }

  /** ─── 沿顶边水平拖动 ─── */
  function onCapsuleDragStart(e: MouseEvent) {
    if (islandState.value === 'entering' || islandState.value === 'visible') hideIsland()
    if (form.value === 'bar') expandToPill()
    e.preventDefault()
    e.stopPropagation()
    isDragging.value = true
    dragMoved = false
    capsuleHovered.value = false
    isInsideIsland.value = false
    if (barTimer) {
      clearTimeout(barTimer)
      barTimer = null
    }
    if (barHoverTimer) {
      clearTimeout(barHoverTimer)
      barHoverTimer = null
    }
    dragStartX = e.screenX
    dragStartWinX = currentWinX
    pendingDragX = null

    const onMove = (ev: MouseEvent) => {
      if (!isDragging.value) return
      const dx = ev.screenX - dragStartX
      if (!dragMoved && Math.abs(dx) >= WINDOW_MORPH.dragStartThreshold) dragMoved = true
      if (!dragMoved) return
      const dpr = window.devicePixelRatio || 1
      pendingDragX = dragStartWinX + Math.round(dx * dpr)
      if (dragRafId === null) {
        dragRafId = requestAnimationFrame(() => {
          dragRafId = null
          const targetX = pendingDragX
          pendingDragX = null
          if (targetX === null) return
          applyDragPosition(targetX)
        })
      }
    }

    const onUp = () => {
      isDragging.value = false
      if (dragMoved) suppressNextCapsuleClick = true
      if (dragRafId !== null) cancelAnimationFrame(dragRafId)
      dragRafId = null
      if (pendingDragX !== null) applyDragPosition(pendingDragX)
      pendingDragX = null
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
      onMoveRef = null
      onUpRef = null
      if (dragMoved) snapToTopEdge()
      else resetBarTimer()
    }

    onMoveRef = onMove
    onUpRef = onUp
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }

  /** 拖动中窗口沿顶边跟随光标（水平移动，Y 恒为工作区顶边） */
  async function applyDragPosition(targetX: number) {
    const dpr = window.devicePixelRatio || 1
    const monitor = await getMonitorBounds()
    const edgePx = Math.round(WINDOW_MORPH.edgePadding * dpr)
    const fullWPx = Math.round(WINDOW_MORPH.capsuleW * dpr)
    const minX = monitor.left + edgePx
    const maxX = monitor.right - fullWPx - edgePx
    const clampedX = clamp(targetX, minX, maxX)
    await win.setPosition(new PhysicalPosition(clampedX, monitor.top)).catch(() => {})
    currentWinX = clampedX
    currentWinY = monitor.top
  }

  /** 松手：吸附回顶边并保存水平位置 */
  async function snapToTopEdge() {
    const dpr = window.devicePixelRatio || 1
    const monitor = await getMonitorBounds()
    dockX.value = currentWinX - monitor.left
    await applyWindowGeometry()
    resetBarTimer()
  }

  function onBlur() {
    isInsideIsland.value = false
    capsuleHovered.value = false
    resetBarTimer()
  }

  /** ─── 生命周期 ─── */
  onMounted(async () => {
    console.log('[PonyClean] capsule window mounted, loading dock state...')
    const saved = loadDock()

    // 显式关闭装饰/阴影——部分 Tauri 2 版本在 Windows 上不完全遵循配置
    await win.setDecorations(false).catch(() => {})
    await win.setShadow(false).catch(() => {})
    await win.clearEffects().catch(() => {})

    try {
      if (saved) {
        dockX.value = saved.x
        console.log('[PonyClean] restoring dock x:', saved.x)
        await applyWindowGeometry()
      } else {
        const dpr = window.devicePixelRatio || 1
        const monitor = await getMonitorBounds()
        dockX.value = Math.round((monitor.width - Math.round(WINDOW_MORPH.capsuleW * dpr)) / 2)
        console.log('[PonyClean] initial dock x:', dockX.value)
        await applyWindowGeometry()
      }
    } catch (e) {
      // 定位失败不能阻止窗口显示：兜底用默认位置
      console.error('[PonyClean] capsule init positioning failed:', e)
    }

    await nextTick()
    await new Promise<void>((resolve) => setTimeout(resolve, 0))
    // 兜底：无论定位是否成功都确保窗口显示
    await win.show().catch((e) => console.error('[PonyClean] win.show failed:', e))
    console.log('[PonyClean] capsule window shown')

    window.addEventListener('blur', onBlur)

    try {
      unlistenIslandEnter = await listen('island-pointer-enter', onIslandEnter)
      unlistenIslandLeave = await listen('island-pointer-leave', onIslandLeave)
      unlistenIslandActivity = await listen('island-user-activity', onIslandUserActivity)
    } catch {
      // best-effort
    }

    resetBarTimer()
  })

  onUnmounted(() => {
    stopIdleDetection()
    if (barTimer) clearTimeout(barTimer)
    if (barHoverTimer) clearTimeout(barHoverTimer)
    if (leaveWatchdog) clearTimeout(leaveWatchdog)
    window.removeEventListener('blur', onBlur)
    unlistenIslandEnter?.()
    unlistenIslandLeave?.()
    unlistenIslandActivity?.()

    if (isDragging.value) {
      isDragging.value = false
      if (dragRafId !== null) cancelAnimationFrame(dragRafId)
      dragRafId = null
      pendingDragX = null
      if (onMoveRef) document.removeEventListener('mousemove', onMoveRef)
      if (onUpRef) document.removeEventListener('mouseup', onUpRef)
      onMoveRef = null
      onUpRef = null
    }
  })

  return {
    islandState,
    capsuleHovered,
    isInsideIsland,
    isDragging,
    form,
    onCapsuleEnter,
    onCapsuleLeave,
    onBarEnter,
    onBarLeave,
    onCapsuleDragStart,
    onCapsuleClick,
    onEnterDone,
    onLeaveDone,
    notifyActivity,
    resetToDefault,
    showIsland,
    hideIsland,
  }
}
