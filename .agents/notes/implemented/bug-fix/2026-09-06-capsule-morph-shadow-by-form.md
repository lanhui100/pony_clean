# Agent Note: 胶囊 morph 阴影方向化（影子跟随目标形态）

Status: implemented

## Problem

胶囊态（pill）缩放到贴边进度条（bar）的 300ms morph 期间，窗口阴影轮廓与内容不贴合。
机制：morph 起点原生侧立即应用 pill∪bar 并集 Region（防“啃角”裁剪，CHANGELOG 0.3.4），
而 DWM CS_DROPSHADOW 按 Region 轮廓投影——并集阶梯轮廓 vs CSS 单圆角矩形连续插值，
中间帧必然错位。旧语义（过渡并集期保留投影 ON）让该错位影持续整个过渡期。

约束：TAO 多窗共享同一 Window Class，`CS_DROPSHADOW` 是进程级一位，
capsule 关影的同时也会关掉 island 的影子。

## Decision

方向化阴影（`src-tauri/src/commands/window.rs`，现在时）：影子跟随目标形态，
与过渡标志解耦——`CapsuleForm::wants_shadow()`（pill→true / bar→false，
无 `#[cfg(windows)]` 门控，全平台单测 `wants_shadow_follows_form` 覆盖），
`apply_capsule_region` 内 `let want_shadow = geo.form.wants_shadow();`。
收起（form=bar）过渡起点即关（错位影/残留影同灭）；展开（form=pill）过渡期保持开
（island 进入动画与拖动 lift 暗示依赖过渡期有影）。`transitioning` 只控制
Region（并集 vs 精确），不再参与阴影决策。

同单附带（同一原子性，共同前提，见 Alternatives）：
`apply_capsule_region` / `apply_island_region` 的 `SetWindowRgn` 失败分支补
early-return（旧 Region 配新阴影撕裂）；`set_island_expanded` 对 island 显式
`set_native_shadow(true)`（island 有影此前依赖 capsule 侧把共享位留在 ON，
而 capsule 过渡 invoke 是 fire-and-forget，ON 可能晚于 island 首绘）。

共享类安全：关闭只发生在目标 form=bar 的调用，而收起过渡仅在 island 隐藏时可达
（前端 `collapseToBar` 守卫 `islandState==='idle'`）；island 可见时 capsule 必隐藏
（pill），显式 ON 无冲突；每个 island 展开路径的影子值皆为 ON。
恢复条件：双窗同屏常驻且影子需求相异 → 本策略作废，需 per-window 阴影
（rejected-with-condition：DWM 扩展帧/分窗口类，成本未评估）。

回滚条件：island 进入无影 / expand 落定 pop-in / 收起残留影复现，任现其一即
revert 到 `transitioning || pill`（一行）。

后续（本次不做，显式立法）：前端浮动 invoke（`syncGeometryToBackend(true)` 未 await）
改 await/排队；morph CSS 补 reduced-motion 分支时 350ms 延迟锚点同步改立即；
`hit_test_subclass` 过渡期“耳朵”区吞点击（约 400ms）注释一句。

## Alternatives considered

- 首版过渡期恒关（`!transitioning && form==Pill`）：被 @architect + @reviewer-b 双路
  对抗一致否决（P0 结构性冲突：island entering 0.22s 动画窗内被关影 350ms、
  拖动抓取去影反模式、unmount 锁死无影）。否决。
- B 逐帧 invoke 插值 Region：18 次跨边界 SetWindowRgn + FRAMECHANGED 跟不上帧率，
  错位从形状错变时序错。否决。
- C CSS 阴影替代：无阴影边距会被 SetWindowRgn 裁掉；扩窗口代价远超 300ms 收益
  （SPEC-029 已评估）。否决。
- D 缩短 morph 掩盖：动效语言不为单个 bug 让路。否决。
- E 保持开影只修收尾：收尾一跳是固有语义，与本 bug（全程形状错位）正交。否决。
- 变更 2 拆单：否决——旧 enable-only 下影子单调开，撕裂窗口不可达；正是本次引入
  OFF 翻转才让它变活，属同一原子性的共同前提（@code-reviewer-b 确认）。
- 过渡路径延迟刷新先行：否决（预留）——不加的代价上确界是 350ms 内残影
  （精确同步 + 24ms 二次刷新必随后覆盖）；先行加引入确定性闪烁。触发条件已立法。

## Consequences

- 收起 morph 全程无影（无物可错），展开/island/拖动/重置与旧语义逐点一致（零回归面见 SPEC-036 §3 矩阵）。
- 机械可查：`cargo fmt --check`、`cargo clippy -p pony_clean` 零警告、
  `cargo check -p pony_core -p pony_clean`、`cargo test -p pony_clean wants_shadow_follows_form`、
  `npx vue-tsc --noEmit`；视觉效果靠 review（真机清单 SPEC-036 §8.5 a–g）。
- 排障：收起起点一条 `native shadow disabled`；collapse 全程静默恰因起点关落定关无变化，
  以 `set_capsule_geometry: form=...` 形态日志为准。
