import { ref, onMounted, onUnmounted, nextTick, type Ref } from 'vue'
import { getCurrentWindow, PhysicalPosition, Window } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { WINDOW_MORPH } from '@/lib/windowMorphConfig'

export type IslandState = 'idle' | 'entering' | 'visible' | 'leaving'

const win = getCurrentWindow()

interface MonitorBounds {
  left: number
  top: number
  right: number
  bottom: number
  width: number
}

export function useWindowMorph(scanning: Ref<boolean>) {
  // ─── All state lives inside function scope ───
  const islandState = ref<IslandState>('idle')
  const capsuleHovered = ref(false)
  const isInsideIsland = ref(false)

  // Horizontal drag state
  let currentWindowX = 0
  let dragStartX = 0
  let dragStartWinX = 0
  const isDragging = ref(false)
  let dragMoved = false
  let dragRafId: number | null = null
  let pendingDragX: number | null = null
  let onMoveRef: ((e: MouseEvent) => void) | null = null
  let onUpRef: (() => void) | null = null
  let suppressNextCapsuleClick = false

  // Timers
  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let idlePollTimer: ReturnType<typeof setTimeout> | null = null
  let unlistenIslandEnter: UnlistenFn | null = null
  let unlistenIslandLeave: UnlistenFn | null = null
  let unlistenIslandActivity: UnlistenFn | null = null
  let lastActivityMs = Date.now()

  // Re-entry guard: set when showIsland() is called during leaving animation
  let pendingShowAfterLeave = false

  /** ─── Window positioning ─── */
  async function getMonitorBounds(): Promise<MonitorBounds> {
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
      // Fall through to the browser screen fallback below.
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

  async function centerWindowX(): Promise<number> {
    const monitor = await getMonitorBounds()
    const { width: actualW } = await win.outerSize().catch(() => ({ width: WINDOW_MORPH.capsuleW }))
    const cx = monitor.left + Math.round((monitor.width - actualW) / 2)
    await win.setPosition(new PhysicalPosition(cx, monitor.top)).catch(() => {})
    currentWindowX = cx
    return cx
  }

  async function getIslandWindow(): Promise<Window | null> {
    return Window.getByLabel('island').catch(() => null)
  }

  async function positionIslandWindow(island: Window) {
    const dpr = window.devicePixelRatio || 1
    const monitor = await getMonitorBounds()
    const capsulePos = await win.outerPosition()
    const capsuleSize = await win.outerSize().catch(() => ({
      width: Math.round(WINDOW_MORPH.capsuleW * dpr),
      height: Math.round(WINDOW_MORPH.capsuleH * dpr),
    }))
    const islandW = Math.round(WINDOW_MORPH.fullW * dpr)
    const edgePx = Math.round(WINDOW_MORPH.edgePadding * dpr)
    const centerX = capsulePos.x + Math.round(capsuleSize.width / 2)
    const x = Math.max(
      monitor.left + edgePx,
      Math.min(centerX - Math.round(islandW / 2), monitor.right - islandW - edgePx),
    )
    await island.setPosition(new PhysicalPosition(x, capsulePos.y)).catch(() => {})
  }

  /** ─── State transitions ─── */
  async function showIsland() {
    if (islandState.value === 'visible' || islandState.value === 'entering') return
    // During leaving animation: mark for retry after leave completes
    if (islandState.value === 'leaving') {
      pendingShowAfterLeave = true
      return
    }
    const island = await getIslandWindow()
    if (!island) return
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
  }

  function onEnterDone() {
    if (islandState.value === 'entering') {
      islandState.value = 'visible'
      // Restart idle detection: pollIdle was stopped in onLeaveDone
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
      // Check for re-entry request made during leaving
      if (pendingShowAfterLeave) {
        pendingShowAfterLeave = false
        showIsland()
      }
    }
  }

  /** ─── Idle detection ─── */
  function resetIdleTimer() {
    lastActivityMs = Date.now()
    if (idleTimer) clearTimeout(idleTimer)
    if (scanning.value || islandState.value !== 'visible') return
    idleTimer = setTimeout(() => {
      if (isInsideIsland.value) { resetIdleTimer(); return }
      hideIsland()
    }, WINDOW_MORPH.idleTimeout)
  }

  async function pollIdle() {
    if (islandState.value !== 'visible') { scheduleNextPoll(); return }
    if (scanning.value || isInsideIsland.value) { resetIdleTimer(); scheduleNextPoll(); return }
    const elapsed = Date.now() - lastActivityMs
    if (elapsed >= WINDOW_MORPH.idleTimeout) {
      try {
        if (await invoke<number>('get_system_idle_ms') >= WINDOW_MORPH.idleTimeout) hideIsland()
      } catch { /* fallback */ }
    }
    scheduleNextPoll()
  }

  function scheduleNextPoll() {
    idlePollTimer = setTimeout(pollIdle, WINDOW_MORPH.idlePollInterval) as unknown as ReturnType<typeof setTimeout>
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

  /** ─── Mouse event handlers ─── */
  function onCapsuleEnter() {
    capsuleHovered.value = true
  }

  function onCapsuleLeave() {
    capsuleHovered.value = false
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

  /** ─── Capsule horizontal drag ─── */
  function onCapsuleDragStart(e: MouseEvent) {
    // If island is entering or visible, cancel it first so drag can proceed.
    if (islandState.value === 'entering' || islandState.value === 'visible') {
      hideIsland()
    }
    e.preventDefault()
    e.stopPropagation()
    isDragging.value = true
    dragMoved = false
    capsuleHovered.value = false
    isInsideIsland.value = false
    dragStartX = e.screenX
    dragStartWinX = currentWindowX
    pendingDragX = null

    const onMove = (ev: MouseEvent) => {
      if (!isDragging.value) return
      const dpr = window.devicePixelRatio || 1
      const dx = ev.screenX - dragStartX          // delta in CSS logical pixels
      if (!dragMoved && Math.abs(dx) >= WINDOW_MORPH.dragStartThreshold) dragMoved = true
      if (!dragMoved) return
      // Convert logical delta to physical pixels to match PhysicalPosition
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
    }

    onMoveRef = onMove
    onUpRef = onUp
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }

  async function applyDragPosition(targetX: number) {
    const dpr = window.devicePixelRatio || 1
    const monitor = await getMonitorBounds()
    const edgePx = Math.round(WINDOW_MORPH.edgePadding * dpr)
    const fullWPx = Math.round(WINDOW_MORPH.capsuleW * dpr)
    const minX = monitor.left + edgePx
    const maxX = monitor.right - fullWPx - edgePx
    const clampedX = Math.max(minX, Math.min(targetX, maxX))
    await win.setPosition(new PhysicalPosition(clampedX, monitor.top)).catch(() => {})
    currentWindowX = clampedX
  }

  function onBlur() {
    isInsideIsland.value = false
    capsuleHovered.value = false
    resetIdleTimer()
  }

  /** ─── Lifecycle ─── */
  onMounted(async () => {
    await centerWindowX()

    // Explicitly disable decorations — some Tauri 2 versions on Windows don't
    // fully respect the `decorations: false` config key.
    await win.setDecorations(false).catch(() => {})
    await win.setShadow(false).catch(() => {})
    await win.clearEffects().catch(() => {})
    await nextTick()
    await new Promise<void>((resolve) => setTimeout(resolve, 0))
    await win.show().catch(() => {})

    window.addEventListener('blur', onBlur)

    try {
      unlistenIslandEnter = await listen('island-pointer-enter', onIslandEnter)
      unlistenIslandLeave = await listen('island-pointer-leave', onIslandLeave)
      unlistenIslandActivity = await listen('island-user-activity', onIslandUserActivity)
    } catch { /* best-effort */ }
  })

  onUnmounted(() => {
    stopIdleDetection()
    window.removeEventListener('blur', onBlur)
    unlistenIslandEnter?.()
    unlistenIslandLeave?.()
    unlistenIslandActivity?.()

    // Clean up drag listeners if component unmounts during drag
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
    islandState, capsuleHovered, isInsideIsland, isDragging,
    onCapsuleEnter, onCapsuleLeave,
    onCapsuleDragStart, onCapsuleClick,
    onIslandEnter, onIslandLeave, onIslandUserActivity,
    onEnterDone, onLeaveDone,
    showIsland, hideIsland,
  }
}
