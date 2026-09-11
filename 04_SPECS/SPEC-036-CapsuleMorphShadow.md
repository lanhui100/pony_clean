# SPEC-036: 胶囊⇄贴边条 morph 期原生阴影贴合修复

- 关联任务：TASK-036
- 状态：Draft（双路对抗审核首轮不通过 → 已按裁决修订为方向化决策，待复审/终审）
- 复杂度：A

## 0. 修订记录（2026-09-06 首轮双审裁决）

首版方案（过渡期恒关：`!transitioning && form==Pill`）被 @architect 与 @reviewer-b
一致否决，核心 P0：TAO 多窗共享同一 Window Class，`CS_DROPSHADOW` 是进程级一位——
bar→点击→`showIsland→expandToPill` 链中过渡 OFF 恰落在 island entering 0.22s 动画窗内，
island 无影浮现、350ms 后才弹影（旧语义过渡恒 ON 则全程有影）。属单 class 位无法同时
满足 capsule-OFF/island-ON 的结构性冲突（详见 §10 两路评审记录）。
本修订采纳双方共同 M1：**方向化决策 `want_shadow = (form == Pill)`**——影子跟随目标形态，
与过渡标志解耦。`transitioning` 只控制 Region（并集 vs 精确），不再参与阴影决策。

## 1. 背景与目标

### 用户反馈
由胶囊态（pill）缩放到贴边进度条（bar）时，窗口阴影部分不贴合——收缩动画期间阴影轮廓与内容形状对不上。

### 现状（代码证据）
| 环节 | 行为 | 位置 |
|---|---|---|
| CSS 可视 | 入层从另一形态矩形 `translate+scale` 连续插值到自身，300ms `cubic-bezier(0.22,1,0.36,1)`，单圆角矩形轮廓 | `frontend/src/components/CapsuleWindow.vue` L251–267；`--from-*` 由 `pillLayerStyle`/`barLayerStyle` L41–78 注入 |
| 原生 Region（过渡期） | morph 起点立即应用 **pill∪bar 并集 Region**（`CombineRgn(RGN_OR)`），350ms 后延迟同步切精确 Region | `src-tauri/src/commands/window.rs` `apply_capsule_region` L433–493；前端串行定时器 `useWindowMorph.scheduleGeometrySync`（`morphDurationMs+50`，L195–201） |
| 原生阴影（过渡期） | 并集期**保留投影 ON**（`want_shadow = transitioning \|\| pill`），精确 bar 才关 | `window.rs` L475–479；分形态策略（未提交工作区变更 + ADR `2026-09-04-bar-state-disables-native-shadow`） |
| DWM 投影语义 | CS_DROPSHADOW 按**当前 Region 轮廓**投影，无渐隐、形状恒等于 Region | `window.rs` `set_native_shadow` 文档 L703–737 |

### 根因
DWM 投影轮廓 = 并集 Region 轮廓（两端点形状叠加的阶梯形），CSS 可视轮廓 = 单圆角矩形连续插值。两者在 morph 中间帧**必然错位**：保留投影的过渡期 = “阴影形状恒错 300ms”。静态 bar 污迹已由分形态策略解决，残留的是**过渡期动态错位**。
诚实注记（@architect P1-1，部分采纳）：本根因是几何推理，未做帧对比/Region dump 对照实验，
旧投影残留假说不能完全排除。但两者在收起路径上的修复动作一致（收起起点即关影：错位影与
残留影同灭）；若残留机制仍在，精确态切换复现即证伪（回滚条件见 §7）。

### 目标
收起（pill→bar）morph 全程阴影与内容严格贴合（起点即无影，无物可错）；展开（bar→pill）
保持过渡期有影（保 island 进入/浮起暗示，接受并集胖投影为已知残余）；精确态语义不变
（bar 关 / pill 开）；不复发“啃角”裁剪（CHANGELOG 0.3.4）；快速反向切换影子恒与末态形态一致。

## 2. 范围与非目标

### 范围
- `src-tauri/src/commands/window.rs`：`apply_capsule_region` 阴影决策一行 + 注释更新；
  附带同函数 `SetWindowRgn` 失败分支补 `return`（@architect P2-2：失败后旧 Region 配新阴影撕裂；
  与本决策同函数、同一原子性，顺带修复；恢复条件见 ADR）。
- `04_SPECS/SPEC-029-VisualPolish.md` §“分形态阴影策略”：追加方向化语义修订段。
- `CHANGELOG.md` Unreleased：追加 Fixed 条目。
- `.agents/notes/implemented/bug-fix/2026-09-06-capsule-morph-shadow-by-form.md`：ADR。

### 非目标
- 前端零改动（`CapsuleWindow.vue` / `useWindowMorph.ts` / `windowMorphConfig.ts` 不动：morph 曲线、时长、延迟同步、串行定时器全部保留）。
- 不改形态状态机、精确态开关语义、`pony_core`。
- 不引入逐帧 Region 插值、不重启 CSS 阴影边距（理由见 §5）。

## 3. 用户/系统行为（方向化决策：影子跟随目标 `form`，与 `transitioning` 解耦）

| 场景 | 行为（变更后） | 与旧语义差 |
|---|---|---|
| pill→bar 收缩 | morph 起点（过渡并集调用，`form=bar`）即关阴影 → 300ms 全程无影 → 延迟同步切精确 bar（保持关，幂等）→ 细条干净落定 | 起点 ON→OFF：错位影/残留影同灭（本 bug 修复点） |
| bar→pill 展开 | 过渡并集调用（`form=pill`）保持开 → 全程有影（并集胖投影为已知残余，低于 island 无影 350ms 的显著性）→ 延迟同步切精确 pill（保持开） | 无变化（旧语义过渡恒 ON，本就如此） |
| bar→点击→showIsland 进入 | `expandToPill`（`form=pill`→ON）→ island entering 全程有影 → 350ms 后精确 pill（ON，无跳变） | 无变化：island 路径零回归（首版 P0 的消除项） |
| 快速反向（<350ms 内来回） | 每次过渡调用影子=其目标形态；末次精确同步恒等于末态形态（串行定时器已有语义） | 中间态影子随目标跳（collapse→expand：OFF→ON 各一次；旧语义恒 ON 无跳）。残余时序风险见 §7 |
| 拖动抓取（`onCapsuleDragStart→expandToPill`） | `form=pill`→ON，全程有影，lift 暗示保留 | 无变化（首版 P0-2 的消除项） |
| `snapToTopEdge`（`morphInFlight` 保持并集） | 影子=当前 `form`，与精确态同值：无“滞留并集+错影”组合 | 旧语义过渡恒 ON：pill 拖动无差；bar 态拖动先 expandToPill 故同为 ON |
| `resetToDefault` | `form=pill`：wasBar 时过渡+延迟同步皆 ON，无双跳 | 无变化（@reviewer-b P2-3 在方向化下自然消除：两次调用同值，幂等无跳） |
| island 收起重叠期（~220ms 双窗） | capsule 侧：pill 态 ON（`hideIsland` 不调几何，形态仍 pill）→ 与旧一致 | 无变化 |
| unmount 中断 morph | 影子=末次调用的目标形态；精确同步 timer 被 clear 则形态=目标形态（前端 `form` 已翻转，后端 geo 已写）——影子与形态一致，无“永久无影”（首版 P0-4 在方向化下自然消除：无 OFF 滞留态） | — |

## 4. 技术方案

### 变更 1（`window.rs`，一行 + 注释）：方向化阴影决策
```rust
// 方向化阴影（SPEC-036）：影子跟随目标形态，与过渡标志解耦。
// 收起（form=bar）：起点即关——并集 Region 轮廓与 CSS morph 中间帧必然错位，
//   全程无影则无物可错；精确同步保持关（幂等）。
// 展开（form=pill）：过渡期保持开——island 进入/island 路径/拖动 lift 暗示
//   依赖过渡期有影（单 class 位无法同时满足 capsule-OFF/island-ON，见 §6）。
// `transitioning` 只控制 Region（并集 vs 精确），不再参与阴影决策。
let want_shadow = geo.form == CapsuleForm::Pill;
```
调用方 `refresh_window_frame` / `schedule_delayed_frame_refresh` 逻辑不变。

### 变更 2（同函数，原子性顺带修复，@architect P2-2）
`SetWindowRgn` 失败分支补 `return`：失败时 region 所有权仍在调用方（已有 DeleteObject），
但旧代码继续执行 `set_native_shadow` + `refresh_window_frame`，造成“旧 Region 配新阴影”撕裂。
成功/其他失败 early-return 分支（`capsule_form_region` None、`CombineRgn` 失败）均已 return，
本分支补齐即一致。

### 时序（调用链不变；展开/island/拖动/重置路径影子零跳变）
1. `collapseToBar` → `set_capsule_geometry(form=bar, transitioning=true)` → 并集 Region + 阴影 OFF + 即时刷新。
2. 350ms 后 → `set_capsule_geometry(form=bar, transitioning=false)` → 精确 bar Region + OFF（幂等，无跳）+ 即时刷新 + 24ms 延迟二次刷新。
3. `expandToPill`（任意调用方）→ 两次调用皆 `form=pill` → ON/ON，无跳变。

### 任务拆解
1. spec 双审 → 修订（本文件）。
2. 实现：`window.rs` 一行 + 注释；SPEC-029 追加段；CHANGELOG 条目；ADR。
3. 代码双审 → 回改。
4. 门禁：`cargo fmt --check`、`cargo clippy -p pony_clean`、`cargo check -p pony_clean`、`npx vue-tsc --noEmit`。
5. 用户真机 QA（`npm run dev:tauri`，收缩/展开/快速反向三场景）。

### 并行边界
实现与文档可同批；双审必须并行独立；门禁串行收尾。

## 5. 技术方案与替代方案

| 候选 | 结论 | 理由 |
|---|---|---|
| **A. 方向化决策 `form==Pill`（本方案，修订后）** | 采纳 | 收起错位消除 + 展开/island/拖动零回归；单跳变只发生在收起起点（OFF），展开零跳变；unmount/snap/reset 自然一致（§3）；回滚=一行 revert |
| A0. 过渡期恒关（首版，已否决） | 否决（双审 P0） | 单 class 位结构性冲突：island 进入 350ms 无影 pop-in、拖动去影反模式、unmount 锁死无影。详见 §10 |
| B. 前端 rAF 逐帧 invoke 更新原生 Region（插值 Region 贴合 CSS） | 否决 | 300ms×60fps ≈ 18 次跨边界 `SetWindowRgn` + `SWP_FRAMECHANGED`，DWM 重投影跟不上帧率且开销大；invoke 往返抖动致 Region 落后 CSS 可视，错位从“形状错”变“时序错”；复杂度与本 A 级任务不匹配 |
| C. 过渡期改用 CSS 阴影、精确态再切原生 | 否决 | 窗口尺寸 = 面板尺寸、无阴影边距，CSS 阴影被 `SetWindowRgn` 裁掉（SPEC-029 已评估）；扩窗口留边距则命中区/Region 边界回归，代价远超 300ms 收益 |
| D. 缩短 morph 到 ~120ms 让错位“看不见” | 否决 | 掩盖而非修复；SPEC-029 验收要求过渡 200–350ms，动效语言不为单个 bug 让路；低速眼仍可见 |
| E. 保持过渡期开影、仅修“收尾一跳” | 否决 | 收尾一跳是原生阴影无渐隐的固有语义（既有接受项）；本 bug 是收起全程形状错位，与收尾正交 |

## 6. 影响面和依赖（方向化后重写；首版论证作废）

- `set_native_shadow` 共享类安全（tao 多窗同类，单 class 位）：方向化决策下**过渡调用的影子值恒等于其目标形态的精确值**，
  故过渡调用相对“紧随其后的精确同步”永远无跳变；相对“上一个精确态”至多一次跳变（且只发生在收起起点：pill-ON→bar-OFF）。
  island 进入链（`showIsland→expandToPill(form=pill)`）全程 ON，与旧语义逐点一致——island 路径**零回归**（首版 P0 的消除证明）。
  拖动/重置/收起重叠期同理逐点一致。结论：共享类无新增冲突；远期双窗同屏常驻且影需求相异仍走既有恢复条件（另起 per-window 阴影 ADR，@reviewer-b 建议，本次记为 rejected-with-condition 见 §10）。
- 跨窗刷新传播（@architect P2-1，部分采纳→显式残余）：`SetClassLong` 改共享类后 `refresh_window_frame` 只发给 capsule hwnd，
  island 自身 frame 未刷、其 DWM 投影何时跟随未实测。本次收起起点 OFF 是否“误伤可见 island”：可达性上收起过渡恒源于
  `collapseToBar`（守卫 `islandState==='idle'`，island 隐藏）——无可见 island 可误伤。展开/island 路径影子值与旧逐点一致，
  故 P2-1 在本方案下无新增暴露面；记为残余风险，靠真机 review（双窗重叠期专项）。
- `EdgeCursorState.geo`：只读形态字段，不受开关语义影响。
- 日志可观测：收起起点一条 `disabled`；展开/精确同值幂等无日志（@architect P3-1 部分采纳：collapse 全程静默
  恰因“起点关、落定关”无变化——排障时以形态日志 `set_capsule_geometry: form=...` 为准，ADR 注明；不为本单加日志噪音）。
- Windows-only 编译（@architect P3-2，采纳）：门禁必须在 Windows 跑 `cargo clippy/check -p pony_clean`（§8 已点名；CI 矩阵见仓库既有配置）。

## 7. 风险、回滚与迁移

- 风险 1（残余）：个别 Windows 版本收起起点 OFF 后 `SWP_FRAMECHANGED` 未即时清除投影（与既有残留机制同源）。
  缓解：调用链即时刷新仍在；精确路径 24ms 延迟二次刷新仍在。若复现，对称地给过渡路径加延迟刷新（预留，不先行）。
- 风险 2（残余，@architect P1-3 部分采纳）：浮动 invoke（`syncGeometryToBackend(true)` 未 await）+ 线程池无序投递，
  快速反向时旧过渡调用可能覆盖新 geo/Region。但影子值=目标形态，错序的影响面从“影+形双错”收敛为“形错影对”
  （影子恒与所应用 Region 的形态一致——两者同一次调用写入）。Region 错序本身是既有语义（串行 timer 只保精确末次），
  本次不扩大修复面；`snapToTopEdge` 无 timer 滞留（只发 true 不排精确）在方向化下影子=当前 form，
  最坏是“并集 Region + 对应形态影”，无 OFF 锁死（首版 P0-4 消除）。记为残余，前端 await 化列为后续项（ADR）。
- 风险 3（@reviewer-b P1-3，部分采纳→不先行）：收起起点 OFF + 落定（同值幂等，无第二次类样式写）= 单次 FRAMECHANGED，
  与旧语义次数一致（旧：起点 ON 无变化→落定 OFF 一次）。展开零跳变。故无新增双跳抖动；低端机回归面不变。
- **回滚条件立法**（@architect M4，采纳）：island 进入无影 / expand 落定 pop-in / 收起后残留影复现，任现其一即
  revert 到 `transitioning || pill`（一行）+ ADR 标注。无迁移、无数据格式变化。

## 8. 测试计划（Windows 真机；@architect T1–T5 与 @reviewer-b 遗漏测试折叠）

1. `cargo fmt --check`（工作区根）。
2. `cargo clippy -p pony_clean` 零警告（Windows）。
3. `cargo check -p pony_core -p pony_clean`。
4. `npx vue-tsc --noEmit`（前端零改动，防误触回归）。
5. 真机手动（用户，`npm run dev:tauri`，60fps 录屏备查）：
   a. pill→bar 收缩：T+0/T+150/T+350 无阶梯错位影、无残留；Rust 终端 `set_capsule_geometry: form=bar transitioning=true` 后一条 `native shadow disabled`。
   b. bar→pill 展开：过渡期有影（与旧一致），落定无 pop-in（ON/ON 无跳）。
   c. bar 态点击展开：island entering 全程有影（首版 P0 专项回归，120fps 逐帧前 400ms）。
   d. <350ms 快速反向 3 次：最终影子=末态形态，无 stuck/残留。
   e. 抓取拖动 + 快拖松手（`morphInFlight` 分支）：抓取全程有影。
   f. 双窗重叠期（hideIsland ~220ms）：island 折叠动画阴影无异常闪。
   g. DPR 125%/150% 各一遍 a–b。
6. 不跑：`cargo test -p pony_core`（`pony_core` 未动；`window.rs` 处 `#[cfg(windows)]` 无可移植单测——决策纯函数抽取留待后续，显式标注）。
7. reduced-motion：morph CSS 暂无 reduced-motion 分支（既有缺口，非本次引入；将来补 0ms 分支时 350ms 锚点需同步改立即——恢复条件立法，ADR 注明）。

## 9. 验收标准

对应 TASK-036 Acceptance（修订后，方向化）：
1. pill→bar 收缩：起点即无影，全程无错位/残留（录屏）。
2. bar→pill 展开与 island 进入：与旧语义逐点一致（全程有影，落定无 pop-in）。
3. 快速反向/拖动/重置：影子恒等于末次目标形态，无 stuck、无 OFF 锁死。
4. 门禁全绿（§8 1–4，Windows）。
5. 视觉项靠 review + 用户真机确认（§8 5），显式标注。

## 10. 审核记录与采纳/不采纳说明

### 首轮（2026-09-06）：@architect + @reviewer-b 并行对抗 → 一致**不通过**（对象：首版恒关方案）

@architect（窗口层/竞态）：
- P0-1 共享类冲突（过渡恒 OFF 落在 island entering 窗内）→ **采纳**，方向化后消除（§3 进入行逐点一致）。
- P0-2 回滚立法缺失 → **采纳**（§7 回滚条件）。
- P1-1 根因未证伪（残留假说）→ **部分采纳**（§1 诚实注记 + §7 回滚条件；对照实验本次不做，理由：收起动作对两种机制一致，证伪锚点在精确态复现）。
- P1-2 一刀切过度关（影子先跳 `form==Pill` 同成本更小）→ **采纳**（即本修订 M1）。
- P1-3 最终一致性不成立（浮动 invoke 无序/snap 滞留/unmount clear）→ **部分采纳**（§7 风险 2 收敛论证 + 前端 await 化列后续；snap/unmount 在方向化下无 OFF 锁死，§3 已逐项）。
- P1-4 门禁不可执行 → **采纳**（§8 帧级标准/DPR 矩阵/专项回归）。
- P2-1 跨窗刷新传播未验证 → **部分采纳**（§6 残余 + 可达性论证；consultant 待真机验证，真机前不阻塞合入——可达性上无可见 island 可误伤）。
- P2-2 `SetWindowRgn` 失败分支撕裂 → **采纳**（变更 2 同单修复）。
- P2-3 文档分叉 → **采纳**（实现同单更新 `set_native_shadow` 文档 + SPEC-029 追加段）。
- P3-1/P3-2 可观测/门禁点名 → **部分采纳**（§6 日志说明 + §8 Windows 点名；不加噪音日志）。

@reviewer-b（动效/UX）：
- P0-1 共享类误伤 island → **采纳**（同上）。
- P0-2 拖动去影反模式 → **采纳**（方向化后拖动全程 ON，§3）。
- P0-3 reduced-motion 前提假 → **采纳**（§8.7 立法；本次不引入 morph 无障碍分支——范围外，显式声明）。
- P0-4 unmount 锁死无影/starvation → **采纳**（方向化后无 OFF 滞留态，§3 unmount 行）。
- P1-1 落定弹影 → **不采纳**（方向化后展开零跳变、收起为单调去影；弹影比较对象消失）。
- P1-2 hover 外发光论断（Region 裁掉外发光）→ **采纳为模型修正**（不影响决策：外发光既被裁，影子开关更无叠加态可论）。
- P1-3 双跳 FRAMECHANGED → **不采纳**（§7 风险 3：次数与旧一致，无新增）。
- P1-4 collapse/expand 不对称 → **采纳为方向化依据**（收窄到仅 collapse 起效正是裁决）。
- P2-1 双层影子模型误用 → **接受批评**（单 HWND 单影子；§3 重写后无双层表述）。
- P2-2 `--shape` 跳变位置 → **接受批评**（§3 不再 claim 落定打架）。
- P2-3 `resetToDefault` 双同步 → **采纳验证**（方向化下同值幂等，§3 已列）。
- consultant 升级请求 → **部分采纳**：Windows/DWM 跨窗传播确认（P2-1）与展开 A/B（影子先跳 vs 无影）——后者在方向化后已不存在（展开保持有影，无 A/B 可做）；前者降级为真机验证项（§8.5c/f），不阻塞合入。per-window 阴影长远方案记 rejected-with-condition（ADR）。

### 复审
- （待）方向化修订版请两路 reviewer 复审（重点：§3 进入/拖动/unmount 行、§6 共享类论证、变更 2 原子性）。

### 代码双审（2026-09-06，对象：方向化实现）：@code-reviewer-a + @code-reviewer-b → 一致**有条件通过**，无 P0

@code-reviewer-a（正确性/回归）：
- 方向化“治本修法”、失败 return“消半更新撕裂”正确；6 问逐项核查通过（顺序正确/无泄漏/错序有界/
  初始化 ON/无死参/clippy 合规）；错序分析确认影子与 Region 永不跨调用撕裂（同调用原子），
  且本次少一个信任输入（不再信任 `transitioning`）。
- P2-1（必须修其一）：island 显示路径影子 ON 无显式保障，与文档断言矛盾
  （`apply_island_region`/`set_island_expanded` 从不调 ON；capsule 过渡 invoke fire-and-forget，
  ON 可能晚于 island 首绘且无纠正）→ **采纳 (a) 修法**：`set_island_expanded` 显式 ON+refresh
  （island 可见时 capsule 必隐藏/pill，无共享类冲突）。
- P3-1 island 路径失败泄漏 → **采纳**（对称补 Delete+return+eprintln）。
- P3-2 过渡期命中死区 → **接受为已知**（pre-existing，本次不扩面）。
- P3-3 文档补“影子不依赖 transitioning” → **采纳**（`set_capsule_geometry` 文档已补）。

@code-reviewer-b（边界/失败路径）：
- 9 路调用矩阵逐条影子值与 §3 一致，无意外 OFF；`resetToDefault` ON/ON 幂等无跳；
  bar+transitioning 假设组合不可达（前端时序保证，后端 P3-1 声明不变式）。
- P1-1 SPEC-029 段落 + CHANGELOG 仍写旧语义 → **采纳**（已改方向化表述）。
- P1-2 TASK-036 Resume Hint 仍写恒关 → **采纳**（已更新）。
- P1-3 缺 ADR → **采纳**（`2026-09-06-capsule-morph-shadow-by-form.md` 已补）。
- P2-1（单测锚定）→ **采纳**：`CapsuleForm::wants_shadow()` 纯函数（无 cfg，全平台可跑）
  + `wants_shadow_follows_form` 单测，调用点改用；防一行 revert 无声回退。
- P2-2 refresh 失败残留 → **接受残余**（机制与旧一致；触发频率放大但精确路径 24ms 二次刷新自愈）。
- 变更 2 同单正当 → **确认保留**（OFF 翻转使撕裂窗口变活，属同一原子性；commit/ADR 已显式记录）。
- 过渡延迟刷新 → **同意预留不先行**（触发条件已立法）。
- 前端零改动成立 → **确认**（唯一可观测变化即 bug 修复本身；<350ms re-hover OFF→ON 闪一次语义正确）。

不采纳项：无（两路必修/建议全部采纳或书面接受残余；B-P3-1 后端不变式声明以 A-P3-3 文档补句覆盖，不另加注释——避免同一不变式两处表述分叉）。
