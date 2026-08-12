# SPEC-024: 一键清理 Safe 级 + 释放量反馈

- 状态: Draft（待对抗审查）
- 关联: TASK-024
- 日期: 2026-08-08

## 1. 背景与目标
清理页需逐项勾选才能执行，操作成本高。目标：提供"一键清理（仅 Safe 级）"，并在完成 toast 展示本次释放量，强化完成感（策略清单 P0 体验项）。

## 2. 范围与非目标
- 范围：`frontend/src/views/SpacePanel.vue`（垃圾区块 done 态）
- 非目标：不做免确认删除；不做真实磁盘差值统计（P2）；不改后端

## 3. 用户/系统行为
- done 态且存在 Safe 项时，底部操作栏出现"一键清理"按钮（与"清理选中"并列，Safe 项 > 0 时可用）
- 点击 → 自动全选 Safe 项 → 弹确认框（复用现有 confirmClean）→ executeClean
- 完成后 toast：`✓ 清理完成 X 项成功，释放约 Y GB`（Y = 选中项字节预估值 `selectedBytes`）

## 4. 技术方案与替代
- **一键清理**：构建临时待删集合 `safePaths`（`level !== 'Confirm'` 的 item.path），**不修改 selectedPaths 用户勾选态**；将 `safePaths` 作为确认弹窗数据源 → 确认 → `executeClean(safePaths)`
  - 实现：确认弹窗从"全局 selectedPaths"改为接受"待删集合"参数（一键清理传入 safePaths，普通清理传入 selectedPaths 快照），清理完成后清空
- 释放量：展示 `formatBytes(待删集合字节)`，文案用"约 X"（失败项未删时略有偏差，可接受）
- Safe 项为 0 时隐藏一键清理按钮
- **替代方案**：后端统计实际释放字节 → 改动大且收益低，否决（近似值足够）

## 5. 影响面与依赖
- 仅前端单文件；与 TASK-026 的 SpacePanel 改动（进度联动）同文件 —— 需串行或合并提交，避免冲突

## 6. 任务拆解与并行边界
- 本任务修改 SpacePanel.vue；TASK-026 也改 SpacePanel.vue → 两个任务对同一文件，**不可盲目并行**，编排为串行（先 026 或先 024，最后统一验证）

## 7. 风险、回滚与迁移
- 低风险；回滚 = 移除按钮与 toast 文案

## 8. 测试计划
- `vue-tsc --noEmit`、`npm run build`
- 手动 QA：有 Safe 项时按钮可用；Confirm 项不进入一键

## 9. 验收标准
见 TASK-024 Acceptance。

## 10. 审核记录
（审查后填写）
