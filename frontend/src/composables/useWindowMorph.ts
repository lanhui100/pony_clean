import { ref, onMounted, onUnmounted, type Ref } from 'vue'
import { getCurrentWindow, type Monitor } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'

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
  const userDockPref = ref<string | null>(localStorage.getItem('pony_dock_pref'))

  let idleTimer: ReturnType<typeof setTimeout> | null = null
  let idlePollTimer: ReturnType<typeof setInterval> | null = null
  let pauseTimer: ReturnType<typeof setTimeout> | null = null
  let monitorInfo: Monitor | null = null

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
    morphState.value = 'shrinking'
    showCapsule.value = true
  }

  /** Called by @transitionend on panel-layer after scale(1→0.3) completes */
  function onShrinkAnimEnd() {
    if (morphState.value !== 'shrinking') return
    morphState.value = 'capsule'
    // Remove DWM shadow so the 315×340 window area has no visible border/shadow remnant
    clearWindowEffects()
    win.setShadow(false).catch(() => {})
    pauseTimer = setTimeout(() => {
      if (morphState.value !== 'capsule') return
      startDocking()
    }, PAUSE_AFTER_SHRINK)
  }

  async function startDocking() {
    if (morphState.value !== 'capsule') return
    morphState.value = 'docking'

    const pos = await win.outerPosition()
    const x = pos.x; const y = pos.y
    const sw = getSw(); const sh = getSh()
    const cx = x + FULL_W / 2
    const cy = y + FULL_H / 2
    const pref = userDockPref.value
    let targetX: number, targetY: number, side: 'top' | 'left' | 'right'

    if (pref === 'left') {
      side = 'left'
      targetX = EDGE_PADDING - (FULL_W - CAPSULE_W)
      targetY = clamp(EDGE_PADDING, y, sh - FULL_H - EDGE_PADDING)
    } else if (pref === 'right') {
      side = 'right'
      targetX = sw - CAPSULE_W - EDGE_PADDING
      targetY = clamp(EDGE_PADDING, y, sh - FULL_H - EDGE_PADDING)
    } else if (pref === 'none') {
      morphState.value = 'docked'; dockSide.value = 'top'; return
    } else {
      // Euclidean distance from capsule center to each edge
      const topD = Math.sqrt((cx - sw / 2) ** 2 + (cy - 0) ** 2)
      const leftD = Math.sqrt((cx - 0) ** 2 + (cy - sh / 2) ** 2)
      const rightD = Math.sqrt((cx - sw) ** 2 + (cy - sh / 2) ** 2)
      const minD = Math.min(topD, leftD, rightD)
      if (minD === topD) side = 'top'
      else if (minD === leftD) side = 'left'
      else side = 'right'

      if (side === 'top') {
        targetX = clamp(EDGE_PADDING, cx - CAPSULE_W / 2, sw - FULL_W - EDGE_PADDING)
        targetY = EDGE_PADDING - (FULL_H - CAPSULE_H)
      } else if (side === 'left') {
        targetX = EDGE_PADDING - (FULL_W - CAPSULE_W)
        targetY = clamp(EDGE_PADDING, y, sh - FULL_H - EDGE_PADDING)
      } else {
        targetX = sw - CAPSULE_W - EDGE_PADDING
        targetY = clamp(EDGE_PADDING, y, sh - FULL_H - EDGE_PADDING)
      }
    }

    dockSide.value = side
    await win.setPosition({ x: targetX, y: targetY })
    morphState.value = 'docked'
    if (isFirstDock.value) {
      localStorage.setItem(MORPH_ONBOARDED_KEY, '1')
      isFirstDock.value = false
    }
  }

  async function expandToFull() {
    if (morphState.value !== 'docked' && morphState.value !== 'capsule') return
    if (pauseTimer) clearTimeout(pauseTimer); pauseTimer = null

    const pos = await win.outerPosition()
    const x = pos.x; const y = pos.y
    const sw = getSw()

    let targetX: number, targetY: number
    if (x < sw / 3) targetX = EDGE_PADDING
    else if (x > sw * 2 / 3) targetX = sw - FULL_W - EDGE_PADDING
    else targetX = clamp(EDGE_PADDING, x + FULL_W / 2 - FULL_W / 2, sw - FULL_W - EDGE_PADDING)
    targetY = clamp(EDGE_PADDING, y + FULL_H / 2 - FULL_H / 2, getSh() - FULL_H - EDGE_PADDING)

    await win.setPosition({ x: targetX, y: targetY })
    // Keep capsule visible, mount panel hidden, then trigger transition
    morphState.value = 'expanding'
    // shadow + effects restored on transitionend
  }

  function onExpandAnimEnd() {
    if (morphState.value !== 'expanding') return
    win.setShadow(true).catch(() => {})
    restoreWindowEffects()
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

  function onCapsuleHover() {
    if (morphState.value === 'docked') return
    if (morphState.value === 'shrinking' || morphState.value === 'capsule' || morphState.value === 'docking') {
      abortAndRestore()
    }
  }

  function onCapsuleDragStart() {
    // Undock first so window moves freely
    if (morphState.value === 'docked') {
      morphState.value = 'capsule'
    }
    win.startDragging().catch(() => {})
  }

  function onCapsuleClick() { expandToFull() }

  function setDockPref(pref: string | null) {
    userDockPref.value = pref
    if (pref) localStorage.setItem('pony_dock_pref', pref)
    else localStorage.removeItem('pony_dock_pref')
  }

  onMounted(() => { getMonitor(); startIdleDetection() })
  onUnmounted(() => { stopIdleDetection(); if (pauseTimer) clearTimeout(pauseTimer) })

  return {
    morphState, showCapsule, isFirstDock, dockSide, userDockPref,
    onUserActivity, onCapsuleHover, onCapsuleDragStart, onCapsuleClick,
    onShrinkAnimEnd, onExpandAnimEnd, expandToFull, setDockPref,
  }
}

function clamp(min: number, val: number, max: number) {
  return Math.max(min, Math.min(val, max))
}