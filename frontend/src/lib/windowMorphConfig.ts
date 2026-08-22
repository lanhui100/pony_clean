/**
 * 窗口形态配置（胶囊 ⇄ 贴边进度条 ⇄ 灵动岛）
 *
 * - `pill`：胶囊态，显示 CPU/MEM 数字
 * - `bar`：贴边进度条态，10s 无操作后自动收起为贴边细条
 * - 胶囊仅支持**顶部贴边**（左右/底部贴边已按需求移除，后续需要可扩展）
 *
 * 面板即窗口（SPEC-029 二次修订）：窗口尺寸 = 面板尺寸，CSS 面板占满窗口，
 * SWCA Acrylic 毛玻璃铺满整个窗口（不被 Region 裁剪），因此无阴影边距、无
 * 外圈直角毛玻璃；阴影改由原生 DWM（CS_DROPSHADOW）按圆角 Region 投影。
 * 注意：尺寸常量必须与 `src-tauri/src/commands/window.rs` 同步。
 */
export const WINDOW_MORPH = {
  fullW: 315, // island 窗口宽 = 内容宽
  fullH: 100, // island 概要态高度
  expandedH: 480, // island 展开态高度
  capsuleW: 166, // 胶囊窗口宽（含 3px 左右抗锯齿余量）
  capsuleH: 44, // 胶囊窗口高（含 2px 上下余量）
  pillW: 160, // 胶囊视觉宽度
  pillH: 40, // 胶囊视觉高度
  stripThick: 10, // 贴边进度条厚度
  edgePadding: 8,
  idleTimeout: 5_000, // island 无操作收起时间
  barTimeout: 10_000, // pill 无操作收起为进度条的时间
  barHoverDelay: 500, // 进度条 hover 后展开为胶囊的延迟
  idlePollInterval: 1000,
  dragStartThreshold: 4,
  /**
   * pill⇄bar CSS morph 时长（与 CapsuleWindow 的 transition 一致）。
   * 同时界定过渡并集 Region 的时窗：形态切换瞬间应用 pill∪bar 并集，
   * 此时长 +50ms 后由延迟同步切到目标形态精确 Region（useWindowMorph）。
   */
  morphDurationMs: 300,
} as const

export type CapsuleForm = 'pill' | 'bar'

/**
 * 内容矩形（CSS 逻辑像素，相对窗口左上角）。
 * - pill：居中（横向留 ~3px 抗锯齿余量）
 * - bar：顶部贴边细条（满窗口宽）
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
