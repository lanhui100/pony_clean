import { ref, onMounted, onUnmounted, type Ref } from 'vue'
import { getCurrentWindow, PhysicalPosition } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { WINDOW_MORPH } from '@/lib/windowMorphConfig'

export type IslandState = 'idle' | 'entering' | 'visible' | 'leaving'

const win = getCurrentWindow()

interface EdgeCursorPayload {
  cursor_x: number
  cursor_y: number
  mon_left: number
  mon_top: number
  mon_right: number
  mon_bottom: number
}

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
  let isDragging = false
  let dragMoved = false
  let dragRafId: number | null = null
  let pendingDragX: number | null = null
  let hoverShowTimer: ReturnType<typeof setTimeout> | null = null
  let onMoveRef: ((e: MouseEvent) => void) | null = null
  let onUpRef: (() => void) | null = null
  let suppressNextCapsuleClick = false
  let suppressHoverUntilLeave = false

  // Timers
  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let idlePollTimer: ReturnType<typeof setTimeout> | null = null
  let edgeEnterTimer: ReturnType<typeof setTimeout> | null = null
  let unlistenEdgeEnter: UnlistenFn | null = null
  let unlistenEdgeLeave: UnlistenFn | null = null
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
    const { width: actualW } = await win.outerSize().catch(() => ({ width: WINDOW_MORPH.fullW }))
    const cx = monitor.left + Math.round((monitor.width - actualW) / 2)
    await win.setPosition(new PhysicalPosition(cx, monitor.top)).catch(() => {})
    currentWindowX = cx
    return cx
  }

  /** Set Win32 hit-test to capsule-only area (idle state).
   *  Outside the capsule, clicks pass through to windows beneath.
   *  No DPI conversion needed — WM_NCHITTEST subclass handles it. */
  async function setRegionCapsule(): Promise<boolean> {
    try {
      await invoke('set_hit_test_mode', { mode: 'capsule' })
      return true
    } catch (err) {
      console.warn('[PonyClean] Failed to switch window region to capsule', err)
      return false
    }
  }

  /** Set Win32 hit-test to full window area (visible/entering/leaving state).
   *  Entire window is interactive. */
  async function setRegionFull(): Promise<boolean> {
    try {
      await invoke('set_hit_test_mode', { mode: 'full' })
      return true
    } catch (err) {
      console.warn('[PonyClean] Failed to switch window region to full', err)
      return false
    }
  }

  /** ─── State transitions ─── */
  async function showIsland() {
    if (islandState.value === 'visible' || islandState.value === 'entering') return
    // During leaving animation: mark for retry after leave completes
    if (islandState.value === 'leaving') {
      pendingShowAfterLeave = true
      return
    }
    // Update region first so the island panel is fully visible when animation starts.
    if (!await setRegionFull()) return
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
    islandState.value = 'leaving'
    // Prefer a clipped native region while the island fades out, so the mostly
    // transparent full window does not keep blocking apps underneath.
    setRegionCapsule()
  }

  function onEnterDone() {
    if (islandState.value === 'entering') {
      islandState.value = 'visible'
      // Restart idle detection: pollIdle was stopped in onLeaveDone
      startIdleDetection()
    }
  }

  function onLeaveDone() {
    if (islandState.value === 'leaving') {
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
  function clearHoverShowTimer() {
    if (hoverShowTimer) clearTimeout(hoverShowTimer)
    hoverShowTimer = null
  }

  function clearEdgeEnterTimer() {
    if (edgeEnterTimer) clearTimeout(edgeEnterTimer)
    edgeEnterTimer = null
  }

  function scheduleHoverShow() {
    clearHoverShowTimer()
    hoverShowTimer = setTimeout(() => {
      hoverShowTimer = null
      if (!capsuleHovered.value || isDragging || suppressHoverUntilLeave) return
      showIsland()
      resetIdleTimer()
    }, WINDOW_MORPH.hoverShowDelay)
  }

  function onCapsuleEnter() {
    capsuleHovered.value = true
    isInsideIsland.value = true
    if (!isDragging && !suppressHoverUntilLeave) scheduleHoverShow()
    resetIdleTimer()
  }

  function onCapsuleLeave() {
    capsuleHovered.value = false
    suppressHoverUntilLeave = false
    clearHoverShowTimer()
    // Island mouseenter will take over
    if (islandState.value === 'idle' && !isDragging) isInsideIsland.value = false
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
    if (islandState.value !== 'idle') return
    e.preventDefault()
    e.stopPropagation()
    clearHoverShowTimer()
    clearEdgeEnterTimer()
    isDragging = true
    dragMoved = false
    suppressHoverUntilLeave = true
    capsuleHovered.value = false
    isInsideIsland.value = false
    dragStartX = e.screenX
    dragStartWinX = currentWindowX
    pendingDragX = null

    const onMove = (ev: MouseEvent) => {
      if (!isDragging) return
      const dpr = window.devicePixelRatio || 1
      const dx = ev.screenX - dragStartX          // delta in CSS logical pixels
      if (!dragMoved && Math.abs(dx) >= WINDOW_MORPH.dragStartThreshold) dragMoved = true
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
      isDragging = false
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
    const fullWPx = Math.round(WINDOW_MORPH.fullW * dpr)
    const minX = monitor.left + edgePx
    const maxX = monitor.right - fullWPx - edgePx
    const clampedX = Math.max(minX, Math.min(targetX, maxX))
    await win.setPosition(new PhysicalPosition(clampedX, monitor.top)).catch(() => {})
    currentWindowX = clampedX
  }

  function onBlur() {
    isInsideIsland.value = false
    capsuleHovered.value = false
    clearHoverShowTimer()
    resetIdleTimer()
  }

  /** ─── Lifecycle ─── */
  onMounted(async () => {
    await centerWindowX()

    // Explicitly disable decorations — some Tauri 2 versions on Windows don't
    // fully respect the `decorations: false` config key.
    win.setDecorations(false).catch(() => {})
    win.clearEffects().catch(() => {})

    // Start with capsule-only region so only the capsule is visible in idle state
    await setRegionCapsule()

    window.addEventListener('blur', onBlur)

    try {
      await invoke('start_edge_cursor_detect')
      unlistenEdgeEnter = await listen<EdgeCursorPayload>('edge-cursor-enter', () => {
        if (islandState.value === 'idle' && !isDragging && !suppressHoverUntilLeave) {
          clearEdgeEnterTimer()
          edgeEnterTimer = setTimeout(() => {
            edgeEnterTimer = null
            if (islandState.value === 'idle' && !isDragging && !suppressHoverUntilLeave) {
              showIsland()
            }
          }, 50)
        }
      })
      unlistenEdgeLeave = await listen<unknown>('edge-cursor-leave', () => {})
    } catch { /* best-effort */ }
  })

  onUnmounted(() => {
    stopIdleDetection()
    clearHoverShowTimer()
    clearEdgeEnterTimer()
    window.removeEventListener('blur', onBlur)
    invoke('stop_edge_cursor_detect').catch(() => {})
    unlistenEdgeEnter?.()
    unlistenEdgeLeave?.()

    // Clean up drag listeners if component unmounts during drag
    if (isDragging) {
      isDragging = false
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
    islandState, capsuleHovered, isInsideIsland,
    onCapsuleEnter, onCapsuleLeave,
    onCapsuleDragStart, onCapsuleClick,
    onIslandEnter, onIslandLeave, onIslandUserActivity,
    onEnterDone, onLeaveDone,
    showIsland, hideIsland,
  }
}
