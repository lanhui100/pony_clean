# Agent Note: 胶囊态 hover/岛内标志在窗口显隐时 stuck，导致自动收缩永久失效

Status: implemented

## Problem

胶囊态（pill）按设计应在 island 收起后 10s 无操作自动收缩为贴边进度条（bar，
`WINDOW_MORPH.barTimeout`），但实际使用中一旦打开过面板，胶囊就再也不自动收缩。

根因是两处布尔标志在窗口显隐切换时收不到 `mouseleave`，永久 stuck 为 true，
而 `resetBarTimer` 的触发回调遇到任一标志为 true 就递归重排 10s 计时，
形成无限重排、永不 `collapseToBar`：

1. `capsuleHovered`：点击胶囊展开面板时光标正压在胶囊上；随后胶囊窗口
   `win.hide()`，pill 层收不到 `mouseleave`，标志 stuck true。
2. `isInsideIsland`：island 窗口 `hide()` 时若光标仍在其范围内，
   island 侧 motion 层的 `mouseleave` 同样不派发，标志 stuck true。

隐藏窗口不派发 mouseleave 是 WebView/浏览器固有行为，不是事件接线错误，
因此修复点必须在“已知过渡点”主动复位标志，而不能依赖事件补发。

## Decision

在 `useWindowMorph` 的确定性过渡点主动复位标志（现在时）：

- `showIsland()` 入口与 `onEnterDone()`（胶囊 `hide()` 后）：`capsuleHovered = false`。
  光标此时的真实位置在 island 侧，不在胶囊上；若之后回到胶囊上，
  `mouseenter` 会重新置 true。
- `hideIsland()` 入口：`capsuleHovered = false`（刚重现的胶囊不预设 hover，
  由后续 `mouseenter` 纠正）。
- `onLeaveDone()`（island `hide()` 后）：`isInsideIsland = false`、
  `capsuleHovered = false`，再 `resetBarTimer()`。这是本修复的核心：
  收起完成即认为“无处可 hover”，计时器从干净状态起算。
- `onBlur` 增加拖动兜底：若 `isDragging` 为 true，复用已注册的 `onUpRef`
  做一次 mouseup 等价收尾（防 alt-tab 等导致 mouseup 丢失、`isDragging`
  stuck 附带阻塞收缩）。
- `collapseToBar` / `expandToPill` 增加 `window.__ponyLog` info 日志，
  收缩行为可观测（Rust 终端可见），“靠 review”之外的机械自证。

扫描中（`scanning`）仍阻塞收缩，属既有设计，不变；`scan-state-changed`
事件丢失导致的 stuck 不在本条范围内（emitTo 可靠，窗口重载场景另议）。

## Alternatives considered

- island 侧在 `island-leave` 时主动补发 `island-pointer-leave`：
  能修 `isInsideIsland`，但修不了胶囊侧 `capsuleHovered`（胶囊窗口已隐藏，
  事件发了也无 hover 语义可言）；且把“复位”责任拆到两个窗口，时序更难推理。否决。
- 用 `document.elementFromPoint` / 光标轮询替代 hover 标志：
  引入轮询与 DPR 换算，复杂度远超收益；且 Tauri Region 裁剪区命中语义与
  DOM 不一致，易误判。否决。
- 去掉 hover 阻塞、固定 10s 硬收缩：
  光标停在胶囊上时被收缩打断是明确的 UX 回退（SPEC 既有语义）。否决。

## Consequences

- 打开面板 → 收起后，胶囊恢复 10s 自动收缩；机械验证：`collapseToBar`
  日志在 Rust 终端可观测（`[PonyClean] capsule collapsed to bar`），靠 review
  的仅为“视觉上确实变细条”这一项。
- `docs/DESIGN.md` 中 “10s 无操作自动收起” 描述恢复为真，无需改文档。
