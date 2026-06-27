# TASK-009: Vue 清理面板

## Basic Info
- Status: Backlog
- Priority: P1
- Owner: @self
- Created: 2026-06-27
- Estimated: 6h
- Depends: TASK-006, TASK-007

## Goal
实现 C盘安全清理面板全部 UI，功能等价于当前 egui 版本，增加删除确认弹窗、删除结果反馈等 UX 提升。

## Output
- `frontend/src/views/CleanerPanel.vue` — 清理面板主视图
- `frontend/src/components/ScanProgress.vue` — 扫描进度组件
- `frontend/src/components/CleanCategory.vue` — 分类折叠列表
- `frontend/src/composables/useCleaner.ts` — Tauri IPC 封装
- 扫描 → 展示 → 确认 → 删除 → 结果反馈完整流程

## Acceptance
1. Idle: 居中 "开始扫描" 按钮（蓝色，prominent）
2. Scanning: 不确定进度条 + 已扫描文件数 + 当前路径
3. Done: 总计可释放空间 + 分类图例（颜色 dot + 标签 + 大小）
4. 分类折叠列表（shadcn-vue Collapsible），每项带 checkbox
5. 全选/取消全选 per category + global
6. 底部操作栏：已选计数 + [全选] + [清理选中]（红色，disabled 逻辑）
7. 删除前确认弹窗（shadcn-vue AlertDialog）
8. 删除结果反馈（成功数/失败数，失败可展开错误列表）
9. 空结果状态
10. 扫描进度实时从 Tauri Event 推送

## Spec
详见 `04_SPECS/SPEC-009-Cleaner-Panel.md`
