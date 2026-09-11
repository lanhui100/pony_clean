# TASK-036: 胶囊⇄贴边条 morph 期原生阴影错位修复

## Basic Info
- Status: Validation（代码 + spec 双审 + 代码双审闭环 + 门禁全绿；真机视觉 QA 留待用户）
- Priority: P0（用户可视回归：收缩动画 300ms 内阴影轮廓与内容错位）
- Owner: @self（agent team 编排）
- Created: 2026-09-06
- Estimated: 2h
- Depends: TASK-029（SPEC-029 分形态阴影策略为直接前置）
- Complexity: A（单模块 bugfix：`window.rs` 阴影决策语义 + 同函数原子性顺带修复；前端零改动）
- Spec: `04_SPECS/SPEC-036-CapsuleMorphShadow.md`（Rev.2 方向化；首版恒关被双审否决见 SPEC §10）

## Goal
胶囊态缩放到贴边进度条（及反向展开）300ms morph 期间，原生 DWM 阴影轮廓与 CSS 插值内容严格贴合：无“阴影大一圈 / 形状对不上 / 收尾一跳带污迹”的观感。

## 背景
- 前端 `CapsuleWindow.vue` morph：入层从另一形态矩形 `translate+scale` 插值到自身（300ms `cubic-bezier(0.22,1,0.36,1)`），出层仅淡出。
- 原生 `apply_capsule_region(hwnd, transitioning=true)` 在 morph 起点立即应用 **pill∪bar 并集 Region** 并**保留阴影 ON**，350ms 后延迟同步切精确 Region（`useWindowMorph.scheduleGeometrySync`，`morphDurationMs+50`）。
- 并集 Region 的 DWM 投影是“两端点形状叠加”的阶梯轮廓，而 CSS 可视是单圆角矩形的连续插值：两者在中间帧必然错位。用户反馈即此：“由胶囊态缩放到贴边进度条状态时，窗口阴影部分不贴合”。
- 前置工作（未提交，工作区现存）：分形态阴影策略（精确 bar 态关阴影、pill/过渡期开阴影）解决了“静态 bar 污迹”，但把错位留在了 300ms 过渡期。

## Acceptance（可验证；Rev.2 方向化：影子跟随目标 form）
1. pill→bar 收缩全程：起点即无影，无错位/残留投影（录屏；`refresh_window_frame` 仍生效）。
2. bar→pill 展开与 island 进入：与旧语义逐点一致（过渡期有影，落定无 pop-in；island entering 全程有影专项回归）。
3. morph 期内容无裁剪回归：并集 Region 保留（CHANGELOG 0.3.4 修过的“啃角”不复发）。
4. 快速反向/拖动/重置/unmount：影子恒等于末次目标形态，无 stuck、无 OFF 锁死（方向化下无滞留态）。
5. 门禁（Windows）：`cargo fmt --check`、`cargo clippy -p pony_clean` 零警告、`cargo check -p pony_core -p pony_clean`、`npx vue-tsc --noEmit`。
6. 视觉项靠 review + 用户真机确认（§8 真机清单 a–g），显式标注。

## Non-Goal
- 不引入逐帧原生 Region 插值（rAF→invoke 洪流，见 SPEC §5 否决理由）。
- 不恢复 CSS 阴影边距方案（SPEC-029 已裁决否决，不重启）。
- 不改变形态状态机、morph 时长/曲线、精确态阴影语义（bar 关 / pill 开）。
- 不改动 `pony_core`。

## Review Records
- （待）Spec 双路对抗：@architect（窗口层/竞态）+ @reviewer-b（前端动效/UX）。
- （待）代码双审：@code-reviewer-a + @code-reviewer-b。

## Validation Evidence
- `cargo fmt --check -p pony_clean` ✅ exit 0
- `cargo clippy -p pony_clean` ✅ 零警告（11.29s）
- `cargo test -p pony_clean wants_shadow` ✅ 1 passed（`wants_shadow_follows_form`；编译 2m21s，二进制可编即 check 语义覆盖）
- `npx vue-tsc --noEmit` ✅ exit 0（前端零改动）
- 审查闭环：spec 双路对抗（首版恒关一致不通过→修订方向化）+ 代码双审（A/B 一致有条件通过，要求项全部落实见 SPEC §10）
- 变更文件：`src-tauri/src/commands/window.rs`（决策行 + `wants_shadow()` 纯函数 + 单测 + island 显式 ON + 双失败 return + 3 处文档）、`04_SPECS/SPEC-036-CapsuleMorphShadow.md`（新建 Rev.2）、`04_SPECS/SPEC-029-VisualPolish.md`（方向化追加段）、`CHANGELOG.md`（2 条 Fixed）、`.agents/notes/implemented/bug-fix/2026-09-06-capsule-morph-shadow-by-form.md`（ADR）
- 未跑：`cargo test -p pony_core`（`pony_core` 未动，SPEC §8 立法）
- 手动 QA（留给用户）：SPEC-036 §8.5 a–g（收缩/展开/island 进入逐帧、快速反向、拖动、双窗重叠、DPR 125%/150%）

## Next Action
用户在真实 Windows 会话运行 `npm run dev:tauri` 按 SPEC-036 §8.5 真机 QA；回滚线：island 进入无影 / expand 落定 pop-in / 收起残留影复现任一即 revert一行 + ADR 标注。

## Resume Hint
核心改动点（Rev.2 方向化，首版“过渡期恒关”已否决）：`src-tauri/src/commands/window.rs`
`apply_capsule_region` 内 `let want_shadow = geo.form.wants_shadow();`
（`CapsuleForm::wants_shadow`：pill→true / bar→false），`transitioning` 只控制 Region。
附带：`SetWindowRgn` 失败 early-return、`set_island_expanded` 显式开影。
实现前重读 SPEC-036 §3–§7。
