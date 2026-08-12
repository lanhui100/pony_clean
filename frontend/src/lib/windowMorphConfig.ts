/**
 * 窗口形态配置（胶囊 ⇄ 贴边进度条 ⇄ 灵动岛）
 *
 * - `pill`：胶囊态，显示 CPU/MEM 数字
 * - `bar`：贴边进度条态，10s 无操作后自动收起为贴边细条
 * - 胶囊仅支持**顶部贴边**（左右/底部贴边已按需求移除，后续需要可扩展）
 */
export const WINDOW_MORPH = {
  fullW: 315, // island 展开宽度
  fullH: 100, // island 概要态高度（仅显示摘要条）
  expandedH: 480, // island 展开态高度（显示监控/清理面板）
  capsuleW: 166, // 胶囊窗口宽度（逻辑 px，额外 6px 抗锯齿余量）
  capsuleH: 44, // 胶囊窗口高度（逻辑 px，额外 4px）
  pillW: 160, // 胶囊视觉宽度
  pillH: 40, // 胶囊视觉高度
  stripThick: 10, // 贴边进度条厚度
  edgePadding: 8,
  idleTimeout: 5_000, // island 无操作收起时间
  barTimeout: 10_000, // pill 无操作收起为进度条的时间
  barHoverDelay: 500, // 进度条 hover 后展开为胶囊的延迟
  idlePollInterval: 1000,
  dragStartThreshold: 4,
} as const

export type CapsuleForm = 'pill' | 'bar'

/**
 * 内容矩形（CSS 逻辑像素，相对窗口左上角）。
 * - pill：窗口内居中
 * - bar：顶部贴边细条（满宽）
 */
export function contentRectFor(form: CapsuleForm): { x: number; y: number; w: number; h: number } {
  if (form === 'pill') {
    return {
      x: Math.round((WINDOW_MORPH.capsuleW - WINDOW_MORPH.pillW) / 2),
      y: Math.round((WINDOW_MORPH.capsuleH - WINDOW_MORPH.pillH) / 2),
      w: WINDOW_MORPH.pillW,
      h: WINDOW_MORPH.pillH,
    }
  }
  return { x: 0, y: 0, w: WINDOW_MORPH.capsuleW, h: WINDOW_MORPH.stripThick }
}
