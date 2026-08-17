# SPEC-028: 空间面板傻瓜式重构 — 一键扫描 → 一键清理

- 状态: Done（2 路对抗审查 + consultant 裁决 + 双代码审核两轮闭环，门禁全绿；手动 QA 留待用户）
- 关联: TASK-028
- 日期: 2026-08-08

## 1. 背景与目标

现状问题（代码证据）：

| 问题 | 位置 |
|---|---|
| 大文件阈值三档按钮（≥100/500/1000MB）暴露给用户 | `SpacePanel.vue:265-270, 665-679` |
| 目录占用展示"用户目录 · 3 层"工程文案 | `SpacePanel.vue:787` |
| 底部「全选 / ⚡一键清理 / 🗑清理选中」语义重叠 | `SpacePanel.vue:620-651` |
| 大文件删除 3 秒倒计时双点（两套确认范式并存） | `SpacePanel.vue:295-356, 733-741` |
| 回收站命令已实现但 UI 无入口 | `commands/cleaner.rs:213-218` |
| 失败错误英文原文直达 UI | `cleaner.rs:1537, 1547, 1556` |
| 扫描参数由前端透传（minMb、深度 3） | `SpacePanel.vue:374`、`useDisk.ts:87-96` |
| **P0（现存缺陷）**：Safe/Confirm 判断大小写错误 —— 前端用 `'Confirm'` 比较，后端序列化为 `'confirm'`，判断恒真 → 现行"一键清理"会删除 Confirm 级（含 Downloads 90 天以上用户文件） | `SpacePanel.vue:134,162,184,218,222` vs `cleaner.rs:27-33` |
| **P2（现存缺陷）**：删除校验用 `get_clean_targets()`，自定义 target 扫出的文件在删除时因不在内置目标集而被拒绝 | `cleaner.rs:1501, 1555` |

目标：用户只需两次点击——「一键扫描」（打开空间面板自动触发，5 分钟冷却）→「一键清理」。所有工程参数、高风险项目、辅助分析信息从主流程剥离，同时修复两个现存缺陷（P0 大小写 + 自定义 target 删除）。

## 2. 范围与非目标

范围：
- `frontend/src/views/SpacePanel.vue`（主流程重构 + P0 大小写修复）
- `frontend/src/composables/useDisk.ts`（listenersReady 守卫 + 结果状态提升模块级）
- `frontend/src/composables/useCleaner.ts`（结果状态提升模块级 + 清空回收站封装）
- `frontend/src/lib/scanSession.ts`（新增：会话级自动扫描标记，模块级非 per-instance）
- `frontend/src/views/SettingsPanel.vue`（新增「扫描与清理参数」区）
- `frontend/e2e/ui-check.mjs`（mock 更新到新命令 + 自动扫描调用次数断言）
- `crates/pony_core/src/cleaner.rs`（`PonyConfig` 增加 `disk_scan`；移除 `recycle_bin` 扫描目标；`delete_files_with_progress` 改用 `get_filtered_targets`）
- `src-tauri/src/commands/disk.rs`（`start_user_scan` 参数 Option 化、读配置、clamp 校验）

非目标：免确认删除（回收站也一样走一次确认）；后端删除安全逻辑（受保护路径/服务停启）改动；真实释放量统计；监控面板改动；内置 target 禁用 UI（P2 后续）。

## 3. 用户/系统行为

1. **自动扫描**：窗口会话内首次打开空间面板自动开始扫描（此后 5 分钟冷却内再次打开不重扫）；结果状态提升为模块级，切 tab 再切回**结果保留**；扫描按钮空闲=扫描、扫描中=取消
2. **垃圾区 done 态**：Safe 级按**分类行**展示（一行一类：checkbox + 色点 + 类名 + 字节数），不渲染逐文件列表；**开发工具缓存类（dev_cache）默认不勾选**（"Safe"≠"删了零成本"，consultant 裁决），其余 Safe 分类默认勾选；主按钮「一键清理」清理**勾选中的 Safe 分类** → 确认弹窗（分类明细、文件数、释放量、不可撤销提示）→ 执行 → 中文 toast；清理成功后清空 Safe 项并显示「重新扫描」CTA
3. **「高级」折叠区**（默认收起）：Confirm 级项目（如 Downloads 90 天以上旧文件），默认全不选；展开后逐项勾选清理（复用分类折叠勾选交互）
4. **「空间分析」折叠区**（默认收起）：大文件 Top 列表 + 目录占用 Top 10
   - 大文件删除（单行垃圾桶按钮与底部批量按钮一致）= 选入确认弹窗，弹窗**列出文件名**（前 5 个 + "等 N 项"）与类型/大小 → 确认 → 执行 → 中文 toast；废除 3 秒双点
   - installer/AppData 类（level=confirm）默认不勾选，弹窗对其显示"可能是正在使用的程序"警告
5. **清空回收站**：唯一入口 = 主面板按钮 → 轻量确认弹窗（"回收站中所有文件将被永久删除"）→ 执行 → toast；回收站不再作为扫描目标参与一键清理
6. **设置面板**：「扫描与清理参数」区：大文件阈值（100/500/1000MB）、目录占用分解层数（1-5，文案注明"仅影响目录占用分解粒度，不影响扫描范围"）；保存后下次扫描生效；旧配置 null 显示为默认 100/3

## 4. 技术方案与替代

### 4.1 后端

**P0 修复（大小写 + 编译期回归门禁）**：前端所有 CleanItem level 比较统一小写 `'confirm'`（与 `SafetyLevel` 的 `serde(rename_all = "lowercase")` 一致）；同时将 `useCleaner.ts` 的 `CleanItem.level` 类型化为 `'safe' | 'confirm' | 'forbidden'` 联合类型——大写 `'Confirm'` 比较会触发 TS2367"类型无重叠"编译错误，vue-tsc 成为该缺陷的**永久编译期回归门禁**。磁盘侧比较已是小写，无需改动。QA 专项验证：一键清理后 Downloads 90 天以上文件必须仍然存在。

**P2 修复（自定义 target 删除）**：`delete_files_with_progress` 的校验目标集从 `get_clean_targets()` 改为 `get_filtered_targets(&load_config())`，使自定义 target 扫描出的文件可删除；受保护路径检查不变。补测试：自定义 target 下文件删除成功。

**回收站单通道**：`get_clean_targets()` 移除 `recycle_bin` 目标（`$Recycle.Bin` 目录扫描基本权限失败、价值低）；回收站唯一入口为 `empty_recycle_bin` 按钮 + 确认弹窗。目标数 55→54，同步更新测试断言（`cleaner.rs` 单测与 `tests/integration_cleaner.rs`）。`Category::RecycleBin` 枚举保留（配置兼容）。

**扫描参数配置化**：`PonyConfig` 增加（`#[serde(default)]`，向后兼容）：

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiskScanConfig {
    pub min_bytes_mb: Option<u64>,   // None = 100，合法范围 50..=10000
    pub dir_depth: Option<usize>,    // None = 3，合法范围 1..=5
}
```

`start_user_scan` 参数保持 Option，None 时读 `cleaner::load_config().disk_scan` 并 clamp（防手改配置文件恶意值）；`min_bytes = clamp(min_bytes_mb.unwrap_or(100), 50, 10000) * 1MB`、`depth = clamp(dir_depth.unwrap_or(3), 1, 5)`。跨模块读配置（disk → cleaner）的轻耦合在 spec 中注明可接受。

### 4.2 前端

**P1-1 修复（监听竞态）**：`useDisk.startScan` 复刻 `useCleaner` 的 `listenersReadyPromise` 守卫（`useCleaner.ts:156-158` 模式），确保 invoke 发生在事件监听注册完成之后。

**P1-2 修复（自动扫描 + 状态持久）**：
- 新建 `frontend/src/lib/scanSession.ts`：`export const scanSession = { lastAutoScanAt: 0 }` —— 独立模块保证真正模块级（`<script setup>` 内变量是 per-instance，会随 v-if 重挂载重置，文件头注释说明）
- `useCleaner`/`useDisk` 的**结果状态提升为模块级 ref**（state、items、totalBytes、skippedSmall、largeFiles、dirUsage、errorMessage、deleteResult 等）：v-if 重挂载后结果保留
- **监听器同样模块级注册一次**（`ensureListeners()` 幂等，返回共享 Promise；`startScan` await 之），不再随实例注销——consultant 裁决：若监听器仍随实例注册/注销，扫描中切 tab 会丢 `scan-done` 事件，`state` 卡在 `'scanning'` 且无法自愈；模块级监听器在整个窗口生命周期存活，与模块级状态生命周期一致
- 自动扫描策略：SpacePanel `onMounted` 时若 `Date.now() - scanSession.lastAutoScanAt > 5*60_000` 且当前无结果 → `startAllScan()` 并记录时间戳

**主区 Safe 分类行**：分类级 checkbox（默认全选）+ 色点 + 类名 + 字节数；「一键清理」取勾选分类的 Safe 项快照走 `pendingClean` 确认弹窗机制；「全选/取消全选」保留为分类级操作；**删除「清理选中」按钮与逐文件列表**（Confirm 区除外）。

**大文件确认弹窗**：新增 `pendingLargeFiles` 快照（单行删除与批量删除统一入口）；弹窗列出文件名（前 5 + 等 N 项）、类型、大小、风险提示（installer/AppData）；废除 `confirmPaths`/`batchConfirm`/`confirmTimer`。

**错误中文化（完整清单 + 路径剥离）**：先剥离路径（正则 `[A-Za-z]:[\\/][^\s:]*` → `…`），再前缀映射：

| 后端原文（前缀） | 中文 |
|---|---|
| `Cannot resolve path` | 文件无法访问，已跳过 |
| `Protected path` | 受保护路径，已跳过 |
| `Path not in scan scope` | 不在可清理范围，已跳过 |
| `Path outside scan root` | 超出扫描范围，已跳过 |
| `System file not deletable` | 系统文件不可删除 |
| `Path contains null byte` | 非法路径，已跳过 |
| `MoveFileExW failed` | 延迟删除失败 |
| `Scan already in progress` | 扫描已在进行中 |
| `No scan in progress` | 没有进行中的扫描 |
| `No scan targets available` | 没有可扫描的目标 |
| `文件被进程占用，延迟删除失败`（已中文） | 直接展示 |
| `服务无法停止，跳过删除`（已中文） | 直接展示 |
| `无法停止服务` / `删除后无法恢复服务`（已中文） | 直接展示 |
| 其他未知 | 截断至 60 字符保留原文 |

**清理后状态**：一键清理成功后清空 Safe 项（items 过滤掉已删集合），主区显示「重新扫描」CTA；Confirm 区保留但提示需重新扫描刷新。

**minMb 残留**：大文件空态文案改为「未发现大文件」；移除 `MIN_OPTIONS` 与 `minMb` ref。

### 4.3 替代方案（否决/裁决）

- 后端合并为单命令单事件流：事件协议与双状态机重写成本高，收益低 —— 否决
- 错误中文化放后端：波及日志脱敏与测试断言，前端映射成本最低 —— 否决
- 回收站体积预估（SHQueryRecycleBinW）：额外 Win32 面，P2 再做
- Safe 级完全无列表 / 保留逐文件列表：均否决，取分类级勾选折中（consultant 裁决见 §10）
- 自动扫描改用 v-show 常驻：三面板常驻改变全部面板生命周期（设置面板 onMounted 读配置时机、监控面板轮询），风险高 —— 否决，取模块级状态（consultant 裁决见 §10）

## 5. 影响面与依赖

- `SpacePanel.vue` 950 行集中重构，风险需双代码审核
- `useCleaner`/`useDisk` 状态提升模块级：仅 SpacePanel 使用这两个 composable，无其他消费方
- `PonyConfig` 结构扩展 → serde 兼容测试必补
- 移除 `recycle_bin` target → 更新 `docs/CLEAN_STRATEGY.md` 3.8 类别表
- 无与其他进行中任务的文件冲突

## 6. 任务拆解与并行边界

顺序实施，每一步门禁全绿（消除 step2→step3 编译断裂的中间态）：

1. **后端一步到位**：`DiskScanConfig` + clamp + `start_user_scan` 读配置 + 移除 recycle_bin target + `delete_files_with_progress` 改 filtered targets + 全部测试更新/新增 → `cargo check` + `cargo test`
2. **前端 composables + 状态提升 + SpacePanel 重构合并一次提交**：`scanSession.ts` + `useDisk` 守卫 + 状态提升 + P0 大小写修复 + 主流程/高级区/空间分析/回收站/文案 + 调用点同步 → `vue-tsc` + `build`
3. `SettingsPanel.vue` 参数区（null→默认值映射）→ `vue-tsc`
4. `e2e/ui-check.mjs` mock 更新 + 自动扫描调用次数断言
5. 门禁全量 + 收口文档

## 7. 风险、回滚与迁移

| 风险 | 缓解 |
|---|---|
| SpacePanel 重构破坏胶囊扫描状态联动 | 保留 isBusy watch 不动；手动 QA 清单覆盖 |
| 模块级状态引入跨实例污染 | 仅 SpacePanel 消费；startScan 显式 reset；监听器仍按实例注册/注销 |
| 自动扫描在弱机耗时 | 5 分钟冷却、随时可取消、事件流不阻塞 UI |
| 一键清理误删 | 仅勾选的 Safe 分类 + 确认弹窗；后端校验零改动；P0 大小写修复后 Confirm 级严格排除 |
| 旧配置文件不兼容 | serde default + 部分字段/缺失字段测试 |
| 移除 recycle_bin target 影响已有用户配置 | `disabled_target_ids` 含该 id 仅被忽略，无迁移成本 |

回滚：git 按文件回滚；配置向前兼容无需迁移。

## 8. 测试计划

- 后端 `cargo test -p pony_core`：
  - 新增：DiskScanConfig 缺失字段默认值、仅设 min_bytes_mb 部分字段、clamp 越界（0 / 巨大值）、自定义 target 文件删除成功、`get_filtered_targets` 包含自定义 target
  - 更新：目标数断言 55→54（两处）
- 前端 `npx vue-tsc --noEmit`、`npm run build`
- e2e（`node e2e/ui-check.mjs`，需完整环境）：mock 更新 + 断言切到 cleaner tab 后 `start_scan`/`start_user_scan` 各调用一次、切走再切回不重复调用
- 手动 QA 清单：
  1. 首次打开空间面板自动扫描；切 monitor 再切回：结果保留且 5 分钟内不重扫
  2. 一键清理仅删勾选的 Safe 分类；**Downloads 90 天以上文件不被删除**（P0 回归）
  3. 取消勾选某 Safe 分类（如开发工具缓存）后一键清理不删该类
  4. 高级区 Confirm 项勾选清理可用；确认弹窗分类明细正确
  5. 空间分析：单行删除与批量删除都走确认弹窗且列文件名；installer 默认不勾选
  6. 清空回收站：确认弹窗 → 执行 → toast
  7. 设置面板改阈值/层数 → 保存 → 重新扫描生效
  8. 胶囊扫描状态联动正常（扫描中不收起）

## 9. 验收标准

见 TASK-028 Acceptance。

## 10. 审核记录

### 10.1 架构 reviewer（@architect）意见采纳表

| 编号 | 意见 | 处置 |
|---|---|---|
| P1-1 | useDisk 缺 listenersReady 守卫，自动扫描与监听注册竞态可卡死状态机 | ✅ 采纳：复刻 useCleaner 守卫（§4.2） |
| P1-2 | sessionAutoScanned 易误实现为 per-instance；v-if 重挂载丢状态未承认 | ✅ 采纳：独立 `scanSession.ts` + 结果状态提升模块级（§4.2） |
| P2-1 | step2→step3 存在编译断裂中间态 | ✅ 采纳：composables 与 SpacePanel 合并为一次提交（§6） |
| P2-2 | Safe 不渲染列表剥夺分类排除能力 | ✅ 采纳（经 consultant 修改）：分类级 checkbox，dev_cache 默认不勾选 |
| P2-3 | 错误映射表不完整 + toast 透出全路径 | ✅ 采纳：全量清单 + 路径剥离（§4.2 表） |
| P2-4 | 回收站双通道冗余 | ✅ 采纳（consultant 维持）：移除扫描目标、按钮 + 确认弹窗单通道 |
| P2-5 | 验收#1 无自动化、e2e mock 过时 | ✅ 采纳：更新 mock + 调用次数断言（§8） |
| P3 | 大文件弹窗列文件名 / 清理后状态 / minMb 残留 / depth 文案 / 回收站免确认矛盾 / 单行删除交互 | ✅ 全部采纳（§3.4、§4.2） |

### 10.2 安全 reviewer（@security-reviewer）意见采纳表

| 编号 | 意见 | 处置 |
|---|---|---|
| P0-1 | 前端 `'Confirm'` 大写比较恒真 → 一键清理实际删 Confirm 级（Downloads 90 天+） | ✅ 采纳：5 处比较改小写 + `level` 类型化编译期门禁（§4.2） |
| P1 | 回收站 Safe 归类导致一键清空、与免确认矛盾 | ✅ 采纳：移除扫描目标、确认弹窗（§4.1/§4.2） |
| P1 | 大文件双点改单确认对 installer 防护不足 | ✅ 部分采纳：installer/AppData 默认不勾选 + 弹窗列文件名与警告 |
| P2 | 错误中文化掩盖安全信息 | ✅ 采纳：失败计数与结构化原因保留，路径剥离（§4.2） |
| P2 | `is_path_allowed` 目标集不一致导致自定义 target 删不掉 | ✅ 采纳：`delete_files_with_progress` 改用 `get_filtered_targets`（§4.1） |
| P3 | DiskScanConfig.dir_depth 无校验 | ✅ 采纳：clamp 50..=10000 / 1..=5（§4.1） |

### 10.3 Consultant 裁决

| 分歧 | 裁决 | 落点 |
|---|---|---|
| Safe 级展示粒度 | 分类级 checkbox，但 dev_cache 默认不勾选 | §3.2 |
| 状态持久方案 | 模块级单例状态 + **监听器模块级注册一次**（防扫描中切 tab 丢 done 事件卡死） | §4.2 P1-2 |
| 回收站通道 | 单通道 + 一次轻量确认（SHEmptyRecycleBinW 唯一官方入口，SHERB_NOCONFIRMATION 仅抑制系统双弹窗） | §3.5、§4.1 |
| P0-1 加固 | `level` 类型化 + 编译期回归门禁 | §4.2 |

Consultant 补充意见处置：P1-2 监听器生命周期 → 已按"模块级注册一次"实现（§4.2）；P1-3 recycle_bin target 移除 → 已写入 §2 范围与 §4.1；P1-4 dev_cache 默认不勾选 → 已写入 §3.2 与 QA 清单；P2-5 进度流实例级 → 实现中已升级为模块级节流（超出 spec 要求）；P2-6 invoke 错误前缀 → 映射采用正则 contains 匹配，包装前缀不影响命中；P3-7 recycle_bin 枚举/前端映射 → **不采纳移除**：保留 `Category::RecycleBin` 与前端 label 是旧 config.json 的 serde 兼容要求（删除枚举会使旧配置反序列化失败、用户设置静默归零），非死代码。

审查结论：2 路有条件通过 + consultant 修改后**同意进入实施**，无需重走对抗审查。

### 10.4 代码审核记录（实施后）

**正确性 reviewer**：不通过（3 P1）→ 全部修复后待复评：
- P1-1 复选框双触发（Checkbox 内部 click 冒泡与行级 @click 叠加）→ 三处改 `@click.stop` ✅
- P1-2 重挂载后默认勾选丢失（watch 无 immediate）→ `{ immediate: true }` ✅
- P1-3 清理后 state=idle 致 Confirm 区/CTA 消失 → executeClean 成功回 `done` + `justCleaned` 空态分支 ✅
- P2-1 失败项移除/释放量口径 → 部分采纳：DeleteResult 无逐路径结果（协议改动被 §4.3 否决），代码注释说明"失败项经 toast 反馈、重扫可重试"，释放量保持快照口径
- P2-2 toast 定时器跨实例 → 采纳：lastCleanBytes + 关闭定时器提升模块级 ✅
- P3 全部采纳：弹窗取消清 pendingClean、陈旧注释更新（54 target）、e2e 事件总线（start_* 派发 done，断言覆盖真实冷却路径）✅
- 其余核对（P0 大小写、saturating_mul、HMR 注释、invokeSeq、delete_files_with_targets 逐行对比、SettingsPanel 保存、clamp）全部通过 ✅

**边界 reviewer**：不通过（2 P1）→ 全部修复后待复评：
- P1-1 清理后状态与 done 分支脱节 → 同正确性 P1-3 修复 ✅
- P1-2 取消/重扫数据混叠（旧扫描迟到批次/Done 混入新扫描）→ useCleaner/useDisk 增加扫描代际守卫（activeScanDone：startScan 前 await 旧事件流收尾，终态事件无条件 markScanDone 防死锁）✅

**复评（定向复审）**：
- 正确性 reviewer：**通过** ✅ —— 3 个 P1 全部修复确认；残留处理：Confirm 条目恢复整行可点（div + @click，与 Safe 行同构）、items 过滤移入 executeClean（置 done 前，消除中间渲染帧）、activeScanDone 超时兜底
- 边界 reviewer：**通过** ✅ —— 2 个 P1 修复确认；残留处理：justCleaned 提升模块级、disk 监听器补 state 守卫（数据事件非 scanning 丢弃、终态事件无条件 markScanDone）、useDisk 错误中文化、auto-scan 条件放宽（error/cancelled 后重进可重试，冷却防死循环）、selectedCategories 初始化改用 categoriesInitialized 标志（消除 size===0 与「取消全选」冲突）、清理 invoke 级失败改走合成 DeleteResult toast（不抛 unhandled rejection、不误用扫描失败文案）
- 边界 reviewer 复审终稿：有条件通过 → 其 3 处残留（justCleaned 模块级 / activeScanDone 超时兜底 / disk 监听器 state 守卫）**已逐项核对落定**（useCleaner.ts:86,177、useDisk.ts:48-67,110），闭环
- 最终门禁：`cargo check` ✅ / `cargo test -p pony_core` ✅（88 单测 + 6 集成）/ `cargo fmt --check` ✅ / `npx vue-tsc --noEmit` ✅ / `npm run build` ✅
