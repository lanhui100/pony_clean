export const WINDOW_MORPH = {
  fullW: 315,
  fullH: 100,
  capsuleW: 166,   // window width (extra 6px for pill anti-aliasing breathing room)
  capsuleH: 44,    // window height (extra 4px)
  pillW: 160,      // visual capsule pill width
  pillH: 40,       // visual capsule pill height
  edgePadding: 8,
  idleTimeout: 5_000,
  idlePollInterval: 1000,
  dragStartThreshold: 4,
} as const

export const capsuleOffsetX = (WINDOW_MORPH.fullW - WINDOW_MORPH.pillW) / 2
