import { ref, onMounted, onUnmounted, type Ref } from 'vue'
import { getCurrentWindow, PhysicalPosition } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type MorphState = 'full' | 'shrinking' | 'capsule' | 'docking' | 'docked' | 'expanding'

const FULL_W = 315
const FULL_H = 340
const CAPSULE_W = 160
const CAPSULE_H = 40
const EDGE_PADDING = 8
const IDLE_TIMEOUT = 5_000
const IDLE_POLL_INTERVAL = 1000
const PAUSE_AFTER_SHRINK = 800
const MORPH_ONBOARDED_KEY = 'pony_morph_onboarded'

const win = getCurrentWindow()
const ACRYLIC_COLOR = { r: 30, g: 28, b: 26, a: 200 }

async function clearWindowEffects() {
  try { await win.setEffects({ effects: [] }) }
  catch { /* best-effort */ }
}

async function restoreWindowEffects() {
  try {
    await win.setEffects({
      effects: [{ effect: 'acrylic' }],
      color: ACRYLIC_COLOR,
    })
  } catch { /* best-effort */ }
}

/** DPI-aware capsule rect in physical pixels.
 *  Enlarged to include CSS box-shadow (0 4px 8px) so DWM doesn't clip it. */
function getCapsuleRectPhysical(): { x: number; y: number; w: number; h: number } {
  const dpr = window.devicePixelRatio || 1
  // CapsuleBar box-shadow: 0 4px 8px → 8px blur all sides, +4px y-offset downward
  const SHADOW_PAD = 12
  const leftRaw = ((FULL_W - CAPSULE_W) / 2 - SHADOW_PAD) * dpr
  const rightRaw = ((FULL_W - CAPSULE_W) / 2 + CAPSULE_W + SHADOW_PAD) * dpr
  const x = Math.floor(leftRaw)
  const w = Math.ceil(rightRaw) - x
  const h = Math.ceil((CAPSULE_H + SHADOW_PAD + 4) * dpr)  // +4 for y-offset
  return { x, y: 0, w, h }
}

export function useWindowMorph(scanning: Ref<boolean>) {
  const morphState = ref<MorphState>('full')
  const showCapsule = ref(false)
  const isFirstDock = ref(!localStorage.getItem(MORPH_ONBOARDED_KEY))
  const dockSide = ref<'top' | 'left' | 'right'>('top')
  const isMouseInside = ref(false)

  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let idlePollTimer: ReturnType<typeof setInterval> | null = null
  let pauseTimer: ReturnType<typeof setTimeout> | null = null
  let unlistenEdgeEnter: UnlistenFn | null = null
  let unlistenEdgeLeave: UnlistenFn | null = null
  let dockAborted = false
  let lastActivityMs = Date.now()
  let isExpanding = false

  async function getSw(): Promise<number> {
    try {
      const m = await win.currentMonitor()
      return m?.size.width ?? Math.round(window.screen.width * (window.devicePixelRatio || 1))
    } catch {
      return Math.round(window.screen.width * (window.devicePixelRatio || 1))
    }
  }

  async function getSh(): Promise<number> {
    try {
      const m = await win.currentMonitor()
      return m?.size.height ?? Math.round(window.screen.height * (window.devicePixelRatio || 1))
    } catch {
      return Math.round(window.screen.height * (window.devicePixelRatio || 1))
    }
  }

  function resetIdleTimer() {
    lastActivityMs = Date.now()
    if (idleTimer) clearTimeout(idleTimer)
    if (scanning.value || morphState.value !== 'full' || isMouseInside.value) return
    idleTimer = setTimeout(startShrinking, IDLE_TIMEOUT)
  }

  async function startIdleDetection() {
    resetIdleTimer()
    idlePollTimer = setInterval(async () => {
      if (morphState.value !== 'full') return
      if (scanning.value || isMouseInside.value) { resetIdleTimer(); return }
      const elapsed = Date.now() - lastActivityMs
      if (elapsed < IDLE_TIMEOUT) return
      try {
        if (await invoke<number>('get_system_idle_ms') >= IDLE_TIMEOUT) startShrinking()
      } catch { /* fallback: WebView events */ }
    }, IDLE_POLL_INTERVAL)
  }

  function stopIdleDetection() {
    if (idleTimer) clearTimeout(idleTimer)
    if (idlePollTimer) clearInterval(idlePollTimer)
    idleTimer = null; idlePollTimer = null
  }

  function onUserActivity() { resetIdleTimer() }

  function onFullWindowMouseEnter() {
    isMouseInside.value = true
    resetIdleTimer()
  }

  function onFullWindowMouseLeave() {
    isMouseInside.value = false
    resetIdleTimer()
  }

  function onWindowBlur() {
    isMouseInside.value = false
    resetIdleTimer()
  }

  function onWindowFocus() {
    if (morphState.value === 'full') {
      resetIdleTimer()
    }
  }

  function startShrinking() {
    if (morphState.value !== 'full') return
    clearWindowEffects()
    win.setShadow(false).catch(() => {})
    morphState.value = 'shrinking'
    showCapsule.value = true
    // Install capsule-shaped hit-test region (physical pixel coordinates)
    const { x, y, w, h } = getCapsuleRectPhysical()
    invoke('set_capsule_hit_rect', { x, y, w, h }).catch(() => {})
  }

  function onShrinkAnimEnd() {
    if (morphState.value !== 'shrinking') return
    morphState.value = 'capsule'
    pauseTimer = setTimeout(() => {
      if (morphState.value !== 'capsule') return
      startDocking()
    }, PAUSE_AFTER_SHRINK)
  }

  async function startDocking() {
    if (morphState.value !== 'capsule') return
    morphState.value = 'docking'
    dockAborted = false

    let pos
    try { pos = await win.outerPosition() } catch { return }
    if (dockAborted) return
    const sw = await getSw()
    const { width: actualWinW } = await win.outerSize().catch(() => ({ width: FULL_W }))
    const targetX = Math.round((sw - actualWinW) / 2)
    const targetY = 0

    dockSide.value = 'top'
    await win.setPosition(new PhysicalPosition(targetX, targetY)).catch(() => {})
    if (dockAborted) return
    morphState.value = 'docked'
    if (isFirstDock.value) {
      localStorage.setItem(MORPH_ONBOARDED_KEY, '1')
      isFirstDock.value = false
    }
  }

  async function expandToFull() {
    if (isExpanding) return
    if (morphState.value !== 'docked' && morphState.value !== 'capsule' && morphState.value !== 'docking') return
    isExpanding = true
    if (pauseTimer) clearTimeout(pauseTimer); pauseTimer = null
    dockAborted = true

    // Restore full hit-test region first
    invoke('set_capsule_hit_rect', { x: 0, y: 0, w: 0, h: 0 }).catch(() => {})

    const sw = await getSw()
    const { width: actualWinW } = await win.outerSize().catch(() => ({ width: FULL_W }))
    const targetX = Math.round((sw - actualWinW) / 2)
    const targetY = -1

    const currentPos = await win.outerPosition().catch(() => ({ x: targetX, y: targetY }))
    const startY = currentPos.y

    await win.setPosition(new PhysicalPosition(targetX, startY)).catch(() => {})
    win.setShadow(true).catch(() => {})
    restoreWindowEffects()
    morphState.value = 'expanding'

    if (startY !== targetY) {
      const duration = 250
      const startTime = performance.now()
      let expandRafId: number | null = null
      const animateY = () => {
        const elapsed = performance.now() - startTime
        const t = Math.min(elapsed / duration, 1)
        const ease = t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t
        const y = Math.round(startY + (targetY - startY) * ease)
        win.setPosition(new PhysicalPosition(targetX, y)).catch(() => {})
        if (t < 1) { expandRafId = requestAnimationFrame(animateY) }
        else { expandRafId = null; isExpanding = false }
      }
      requestAnimationFrame(animateY)
    } else {
      isExpanding = false
    }
  }

  function onExpandAnimEnd() {
    if (morphState.value !== 'expanding') return
    showCapsule.value = false
    morphState.value = 'full'
    isMouseInside.value = false
    isExpanding = false
    resetIdleTimer()
  }

  function abortAndRestore() {
    if (pauseTimer) clearTimeout(pauseTimer); pauseTimer = null
    const wasInCapsule = morphState.value !== 'full'
    showCapsule.value = false
    morphState.value = 'full'
    isExpanding = false
    if (wasInCapsule) {
      invoke('set_capsule_hit_rect', { x: 0, y: 0, w: 0, h: 0 }).catch(() => {})
      win.setShadow(true).catch(() => {})
      restoreWindowEffects()
    }
    resetIdleTimer()
  }

  let hoverTimer: ReturnType<typeof setTimeout> | null = null
  let hoverIntent = false

  function cancelHoverTimer() {
    if (hoverTimer) clearTimeout(hoverTimer)
    hoverTimer = null
    hoverIntent = false
  }

  function onCapsuleHover() {
    if (morphState.value !== 'capsule' && morphState.value !== 'docked' && morphState.value !== 'docking') return
    cancelHoverTimer()
    hoverIntent = true
    hoverTimer = setTimeout(() => {
      hoverTimer = null
      if (morphState.value !== 'capsule' && morphState.value !== 'docked' && morphState.value !== 'docking') return
      if (!hoverIntent) return
      expandToFull().catch(() => {})
    }, 2000)
  }

  function onCapsuleDragStart(e: MouseEvent) {
    cancelHoverTimer()
    if (morphState.value !== 'docked' && morphState.value !== 'capsule' && morphState.value !== 'docking') return
    e.preventDefault()

    if (morphState.value === 'docked') {
      morphState.value = 'capsule'
    }

    const startMouseX = e.screenX
    let startWinX: number | null = null
    let rafId: number | null = null
    let pendingX: number | null = null
    let dragEnded = false

    const onMove = (ev: MouseEvent) => {
      if (dragEnded) return
      const dx = ev.screenX - startMouseX
      if (startWinX === null) {
        pendingX = dx
        return
      }
      pendingX = startWinX + Math.round(dx)
      if (rafId === null) {
        rafId = requestAnimationFrame(() => {
          if (pendingX !== null) {
            win.setPosition(new PhysicalPosition(pendingX, 0)).catch(() => {})
          }
          rafId = null
          pendingX = null
        })
      }
    }

    const onUp = () => {
      dragEnded = true
      if (rafId !== null) cancelAnimationFrame(rafId)
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)

      getSw().then(sw => {
        const centerX = Math.round((sw - CAPSULE_W) / 2)
        win.setPosition(new PhysicalPosition(centerX, 0)).catch(() => {})
      })
      dockSide.value = 'top'
      if (morphState.value === 'capsule' || morphState.value === 'docking') {
        morphState.value = 'docked'
      }
    }

    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)

    win.outerPosition().then(pos => {
      if (dragEnded) return
      startWinX = pos.x
      if (pendingX !== null) {
        const applyX = startWinX + Math.round(pendingX)
        win.setPosition(new PhysicalPosition(applyX, 0)).catch(() => {})
        pendingX = null
      }
    }).catch(() => {})
  }

  function onCapsuleLeave() {
    cancelHoverTimer()
  }

  function onCapsuleClick() {
    cancelHoverTimer()
    dockAborted = true
    expandToFull().catch(() => {})
  }

  onMounted(async () => {
    startIdleDetection()
    window.addEventListener('blur', onWindowBlur)
    window.addEventListener('focus', onWindowFocus)
    try {
      await invoke('start_edge_cursor_detect')
      unlistenEdgeEnter = await listen<{
        cursor_x: number
        cursor_y: number
        mon_left: number
        mon_top: number
        mon_right: number
        mon_bottom: number
      }>('edge-cursor-enter', async (event) => {
        if (morphState.value !== 'docked' && morphState.value !== 'capsule' && morphState.value !== 'docking') return
        try {
          const winMon = await win.currentMonitor()
          if (winMon) {
            const pos = winMon.position
            const size = winMon.size
            const { mon_left, mon_top, mon_right, mon_bottom } = event.payload
            // Both sides derive from GetMonitorInfoW — exact comparison is correct
            if (mon_left !== pos.x || mon_top !== pos.y ||
                mon_right !== pos.x + size.width || mon_bottom !== pos.y + size.height) {
              return
            }
          }
        } catch {
          return
        }
        cancelHoverTimer()
        await expandToFull().catch(() => {})
      })
      unlistenEdgeLeave = await listen<unknown>('edge-cursor-leave', () => {})
    } catch (e) {
      console.warn('[PonyClean] edge cursor detection not available:', e)
    }
  })
  onUnmounted(() => {
    stopIdleDetection()
    if (pauseTimer) clearTimeout(pauseTimer)
    cancelHoverTimer()
    window.removeEventListener('blur', onWindowBlur)
    window.removeEventListener('focus', onWindowFocus)
    invoke('stop_edge_cursor_detect').catch(() => {})
    unlistenEdgeEnter?.()
    unlistenEdgeLeave?.()
  })

  return {
    morphState, showCapsule, isFirstDock, dockSide,
    onUserActivity, onFullWindowMouseEnter, onFullWindowMouseLeave,
    onCapsuleHover, onCapsuleLeave, onCapsuleDragStart, onCapsuleClick,
    onShrinkAnimEnd, onExpandAnimEnd, expandToFull, abortAndRestore,
  }
}
