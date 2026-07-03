import { ref, onMounted, onUnmounted, type Ref } from 'vue'
import { getCurrentWindow, PhysicalPosition } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type IslandState = 'idle' | 'entering' | 'visible' | 'leaving'

const FULL_W = 315
const FULL_H = 100            // Window height = island height
const CAPSULE_W = 160
const CAPSULE_H = 40
const ISLAND_H = 100
const EDGE_PADDING = 8
const IDLE_TIMEOUT = 5_000
const IDLE_POLL_INTERVAL = 1000

const win = getCurrentWindow()

interface EdgeCursorPayload {
  cursor_x: number
  cursor_y: number
  mon_left: number
  mon_top: number
  mon_right: number
  mon_bottom: number
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
  let dragRafId: number | null = null
  let pendingDragX: number | null = null
  let onMoveRef: ((e: MouseEvent) => void) | null = null
  let onUpRef: (() => void) | null = null

  // Timers
  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let idlePollTimer: ReturnType<typeof setTimeout> | null = null
  let unlistenEdgeEnter: UnlistenFn | null = null
  let unlistenEdgeLeave: UnlistenFn | null = null
  let lastActivityMs = Date.now()

  // Re-entry guard: set when showIsland() is called during leaving animation
  let pendingShowAfterLeave = false

  /** ─── Window positioning ─── */
  async function getSw(): Promise<number> {
    try {
      const m = await win.currentMonitor()
      return m?.size.width ?? Math.round(window.screen.width * (window.devicePixelRatio || 1))
    } catch {
      return Math.round(window.screen.width * (window.devicePixelRatio || 1))
    }
  }

  async function centerWindowX(): Promise<number> {
    const sw = await getSw()
    const { width: actualW } = await win.outerSize().catch(() => ({ width: FULL_W }))
    const cx = Math.round((sw - actualW) / 2)
    await win.setPosition(new PhysicalPosition(cx, 0)).catch(() => {})
    currentWindowX = cx
    return cx
  }

  /** Set Win32 hit-test to capsule-only area (idle state).
   *  Outside the capsule, clicks pass through to windows beneath.
   *  No DPI conversion needed — WM_NCHITTEST subclass handles it. */
  async function setRegionCapsule() {
    await invoke('set_hit_test_mode', { mode: 'capsule' }).catch(() => {})
  }

  /** Set Win32 hit-test to full window area (visible/entering/leaving state).
   *  Entire window is interactive. */
  async function setRegionFull() {
    await invoke('set_hit_test_mode', { mode: 'full' }).catch(() => {})
  }

  /** ─── State transitions ─── */
  async function showIsland() {
    if (islandState.value === 'visible' || islandState.value === 'entering') return
    // During leaving animation: mark for retry after leave completes
    if (islandState.value === 'leaving') {
      pendingShowAfterLeave = true
      return
    }
    // Update region FIRST so the island panel is fully visible when animation starts
    await setRegionFull()
    islandState.value = 'entering'
  }

  function onCapsuleClick() {
    showIsland()
  }

  function hideIsland() {
    if (islandState.value === 'idle' || islandState.value === 'leaving') return
    islandState.value = 'leaving'
  }

  function onEnterDone() {
    if (islandState.value === 'entering') {
      islandState.value = 'visible'
      // Restart idle detection: pollIdle was stopped in onLeaveDone
      startIdleDetection()
    }
  }

  async function onLeaveDone() {
    if (islandState.value === 'leaving') {
      // Switch region FIRST to capsule-only before marking idle,
      // so there's no window where region (full) > visual content (capsule)
      await setRegionCapsule()
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
    }, IDLE_TIMEOUT)
  }

  async function pollIdle() {
    if (islandState.value !== 'visible') { scheduleNextPoll(); return }
    if (scanning.value || isInsideIsland.value) { resetIdleTimer(); scheduleNextPoll(); return }
    const elapsed = Date.now() - lastActivityMs
    if (elapsed >= IDLE_TIMEOUT) {
      try {
        if (await invoke<number>('get_system_idle_ms') >= IDLE_TIMEOUT) hideIsland()
      } catch { /* fallback */ }
    }
    scheduleNextPoll()
  }

  function scheduleNextPoll() {
    idlePollTimer = setTimeout(pollIdle, IDLE_POLL_INTERVAL) as unknown as ReturnType<typeof setTimeout>
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
    isInsideIsland.value = true
    showIsland()
    resetIdleTimer()
  }

  function onCapsuleLeave() {
    capsuleHovered.value = false
    // Island mouseenter will take over
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
    isDragging = true
    dragStartX = e.screenX
    dragStartWinX = currentWindowX
    pendingDragX = null

    const onMove = (ev: MouseEvent) => {
      if (!isDragging) return
      const dpr = window.devicePixelRatio || 1
      const dx = ev.screenX - dragStartX          // delta in CSS logical pixels
      // Convert logical delta to physical pixels to match PhysicalPosition
      pendingDragX = dragStartWinX + Math.round(dx * dpr)
      if (dragRafId === null) {
        // Capture the latest value sync, then apply async outside RAF
        const targetX = pendingDragX
        pendingDragX = null
        dragRafId = requestAnimationFrame(() => {
          dragRafId = null
          applyDragPosition(targetX)
        })
      }
    }

    const onUp = () => {
      isDragging = false
      if (dragRafId !== null) cancelAnimationFrame(dragRafId)
      dragRafId = null
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
    const sw = await getSw()
    const edgePx = Math.round(EDGE_PADDING * dpr)
    const fullWPx = Math.round(FULL_W * dpr)
    const clampedX = Math.max(edgePx, Math.min(targetX, sw - fullWPx - edgePx))
    await win.setPosition(new PhysicalPosition(clampedX, 0)).catch(() => {})
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
    win.setDecorations(false).catch(() => {})
    win.clearEffects().catch(() => {})

    // Start with capsule-only region so only the capsule is visible in idle state
    await setRegionCapsule()

    startIdleDetection()
    window.addEventListener('blur', onBlur)

    try {
      await invoke('start_edge_cursor_detect')
      let edgeEnterTimer: ReturnType<typeof setTimeout> | null = null
      unlistenEdgeEnter = await listen<EdgeCursorPayload>('edge-cursor-enter', () => {
        if (islandState.value === 'idle') {
          if (edgeEnterTimer) clearTimeout(edgeEnterTimer)
          edgeEnterTimer = setTimeout(() => { showIsland(); edgeEnterTimer = null }, 50)
        }
      })
      unlistenEdgeLeave = await listen<unknown>('edge-cursor-leave', () => {})
    } catch { /* best-effort */ }
  })

  onUnmounted(() => {
    stopIdleDetection()
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
