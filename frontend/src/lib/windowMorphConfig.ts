export const WINDOW_MORPH = {
  fullW: 315,
  fullH: 100,
  capsuleW: 160,
  capsuleH: 40,
  edgePadding: 8,
  idleTimeout: 5_000,
  idlePollInterval: 1000,
  hoverShowDelay: 140,
  dragStartThreshold: 4,
} as const

export const capsuleOffsetX = (WINDOW_MORPH.fullW - WINDOW_MORPH.capsuleW) / 2
