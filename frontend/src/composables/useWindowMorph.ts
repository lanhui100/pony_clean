import { ref, onMounted, onUnmounted, type Ref } from 'vue'
import { getCurrentWindow, PhysicalPosition, type Monitor } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type MorphState = 'full' | 'shrinking' | 'capsule' | 'docking' | 'docked' | 'expanding'

const FULL_W = 315
const FULL_H = 340
const CAPSULE_W = 160
const CAPSULE_H = 40
const EDGE_PADDING = 8
const IDLE_TIMEOUT = 10_000
const IDLE_POLL_INTERVAL = 500
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

export function useWindowMorph(scanning: Ref<boolean>) {
  const morphState = ref<MorphState>('full')
  const showCapsule = ref(false)
  const isFirstDock = ref(!localStorage.getItem(MORPH_ONBOARDED_KEY))
  const dockSide = ref<'top' | 'left' | 'right'>('top')

  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let idlePollTimer: ReturnType<typeof setInterval> | null = null
  let pauseTimer: ReturnType<typeof setTimeout> | null = null
  let monitorInfo: Monitor | null = null
  let unlistenEdgeEnter: UnlistenFn | null = null
  let unlistenEdgeLeave: UnlistenFn | null = null
  let dockAborted = false

  async function getMonitor() {
    try { monitorInfo = await win.currentMonitor() }
    catch { monitorInfo = null }
  }

  function getSw() {
    return monitorInfo?.size.width ?? window.screen.width
  }

  function getSh() {
    return monitorInfo?.size.height ?? window.screen.height
  }

  function resetIdleTimer() {
    if (idleTimer) clearTimeout(idleTimer)
    if (scanning.value || morphState.value !== 'full') return
    idleTimer = setTimeout(startShrinking, IDLE_TIMEOUT)
  }

  async function startIdleDetection() {
    resetIdleTimer()
    idlePollTimer = setInterval(async () => {
      if (morphState.value !== 'full') return
      if (scanning.value) { resetIdleTimer(); return }
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

  function startShrinking() {
    if (morphState.value !== 'full') return
    clearWindowEffects()
    win.setShadow(false).catch(() => {})
    morphState.value = 'shrinking'
    showCapsule.value = true
  }

  /** Called by @transitionend on panel-layer after scale(1→0.3) completes */
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

    const pos = await win.outerPosition()
    if (dockAborted) return
    const sw = getSw()
    const targetX = Math.round((sw - FULL_W) / 2)
    const targetY = 0

    dockSide.value = 'top'
    await win.setPosition(new PhysicalPosition(targetX, targetY))
    if (dockAborted) return
    morphState.value = 'docked'
    if (isFirstDock.value) {
      localStorage.setItem(MORPH_ONBOARDED_KEY, '1')
      isFirstDock.value = false
    }
  }

  async function expandToFull() {
    if (morphState.value !== 'docked' && morphState.value !== 'capsule' && morphState.value !== 'docking') return
    if (pauseTimer) clearTimeout(pauseTimer); pauseTimer = null
    dockAborted = true

    const sw = getSw()
    const targetX = Math.round((sw - FULL_W) / 2)
    const targetY = -1

    // Animate Y from current position to target smoothly
    const currentPos = await win.outerPosition()
    const startY = currentPos.y

    // Set X immediately, restore shadow+effects, then start CSS animation
    await win.setPosition(new PhysicalPosition(targetX, startY))
    win.setShadow(true).catch(() => {})
    restoreWindowEffects()
    morphState.value = 'expanding'

    if (startY !== targetY) {
      const duration = 250
      const startTime = performance.now()
      const animateY = () => {
        const elapsed = performance.now() - startTime
        const t = Math.min(elapsed / duration, 1)
        const ease = t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t
        const y = Math.round(startY + (targetY - startY) * ease)
        win.setPosition(new PhysicalPosition(targetX, y)).catch(() => {})
        if (t < 1) requestAnimationFrame(animateY)
      }
      requestAnimationFrame(animateY)
    }
  }

  function onExpandAnimEnd() {
    if (morphState.value !== 'expanding') return
    showCapsule.value = false
    morphState.value = 'full'
    resetIdleTimer()
  }

  function abortAndRestore() {
    if (pauseTimer) clearTimeout(pauseTimer); pauseTimer = null
    const wasInCapsule = morphState.value !== 'full'
    showCapsule.value = false
    morphState.value = 'full'
    if (wasInCapsule) {
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
    let scaleFactor = 1
    let rafId: number | null = null
    let pendingX: number | null = null

    const onMove = (ev: MouseEvent) => {
      const dx = ev.screenX - startMouseX
      if (startWinX === null) {
        pendingX = dx
        return
      }
      pendingX = startWinX + Math.round(dx * scaleFactor)
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
      if (rafId !== null) cancelAnimationFrame(rafId)
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)

      const sw = getSw()
      const centerX = Math.round((sw - CAPSULE_W) / 2)
      win.setPosition(new PhysicalPosition(centerX, 0)).catch(() => {})
      dockSide.value = 'top'
      if (morphState.value === 'capsule' || morphState.value === 'docking') {
        morphState.value = 'docked'
      }
    }

    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)

    Promise.all([
      win.outerPosition(),
      win.scaleFactor().catch(() => 1),
    ]).then(([pos, sf]) => {
      startWinX = pos.x
      scaleFactor = sf
      if (pendingX !== null) {
        const applyX = startWinX + Math.round(pendingX * scaleFactor)
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
    getMonitor()
    startIdleDetection()
    // Start Rust-side GetCursorPos polling for reliable edge detection
    try {
      await invoke('start_edge_cursor_detect')
    unlistenEdgeEnter = await listen<void>('edge-cursor-enter', async () => {
        if (morphState.value === 'docked' || morphState.value === 'capsule' || morphState.value === 'docking') {
          cancelHoverTimer()
          await expandToFull().catch(() => {})
        }
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
    invoke('stop_edge_cursor_detect').catch(() => {})
    unlistenEdgeEnter?.()
    unlistenEdgeLeave?.()
  })

  return {
    morphState, showCapsule, isFirstDock, dockSide,
    onUserActivity, onCapsuleHover, onCapsuleLeave, onCapsuleDragStart, onCapsuleClick,
    onShrinkAnimEnd, onExpandAnimEnd, expandToFull,
  }
}
