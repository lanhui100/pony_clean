# Agent Note: 贴边进度条态关闭原生阴影（分形态阴影策略）

Status: implemented

## Problem

胶囊收缩为贴边进度条（bar）后，DWM 仍按 10px 细条 Region 投射
CS_DROPSHADOW：下沿一条模糊暗带 + 两侧阴影 blob。细条本应与屏幕边缘
齐平“贴住”，这圈投影让它看起来像浮起来的污迹——阴影没有随形态适配。

约束：阴影是原生 DWM 按 Region 投的，没有“只投一边”的 API；
`CS_DROPSHADOW` 是窗口类样式，而 tao 默认多窗共享同一窗口类，
一动俱动（含 island 窗口）。

## Decision

分形态阴影策略（`src-tauri/src/commands/window.rs`，现在时）：
`enable_native_shadow` 改为 `set_native_shadow(hwnd, enable)` 开关；
`apply_capsule_region` 内按形态决策——pill 与过渡并集期保留投影，
精确 bar 态关闭投影，样式变化由随后既有的 `refresh_window_frame`
（SWP_FRAMECHANGED）一次生效。切换仅在位实际变化时发生并记日志
（`native shadow enabled/disabled`，Rust 终端可观测）。

共享类安全性（本决策成立的前提，恢复条件见下）：关闭只发生在精确
bar 同步路径，而该路径仅在 island 隐藏时可达（前端 `collapseToBar`
守卫 `islandState==='idle'`）；每个 island 展开路径都经过阴影 ON
（bar 态先走 `expandToPill` 的过渡并集，或 pill 态本就 ON）；
`hideIsland` 的 ~220ms 双窗重叠期形态仍是 pill（ON）。
collapse 收尾（morph 结束 +50ms）阴影消失，视觉上是“落定”，
原生阴影无法渐隐，接受该一跳。

## Alternatives considered

- 保持常开、只修残留：现有 `refresh_window_frame` 已能清除旧形态残留，
  但 bar 态的问题不是残留而是“活的投影形状就不对”，重投影修不了。否决。
- CSS 阴影替代原生：窗口尺寸 = 面板尺寸、无阴影边距，CSS 阴影会被
  SetWindowRgn 裁掉；扩窗口留边距则命中区与 Region 边界回归，
  代价远超收益（SPEC-029 回退方案已评估过）。否决。
- DWMWA_NCRENDERING_POLICY 等逐窗开关：该属性关的是全部 DWM 渲染
  （含毛玻璃），且恢复会重新引入 Win11 标题栏问题（TASK-022 教训）。否决。
- 常驻关闭、全形态无阴影：pill 悬浮感依赖阴影（用户从未抱怨 pill 阴影），
  一刀切是回退。否决。

## Consequences

- bar 态：细条零阴影、干净贴边；pill/island：投影不变。
- 机械可查：`cargo clippy -p pony_clean` 零警告、`cargo build` 通过；
  视觉效果靠 review（需真机看收缩收尾一跳是否自然）。
- 恢复条件：若将来出现双窗同屏常驻且阴影需求相异（共享类冲突成真），
  本策略作废，需另寻逐窗投影方案；形态切换时 Rust 终端的
  enabled/disabled 日志是第一诊断位。
