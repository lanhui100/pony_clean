# TASK-028: 空间面板傻瓜式重构 — 一键扫描 → 一键清理

## Basic Info
- Status: Done（代码与门禁完成；手动 QA 留待用户）
- Priority: P0
- Owner: @self（agent team 编排）
- Created: 2026-08-08
- Estimated: 8h
- Depends: 无（承接 TASK-024/026/027 已完成的扫描与一键清理能力）
- Complexity: B
- Spec: `04_SPECS/SPEC-028-SimpleSpace.md`

## Goal
空间面板从"三区块 + 三按钮 + 工程参数"重构为傻瓜式两键流程：一键扫描（首次打开自动扫）→ 一键清理（Safe 级 + 一次确认弹窗）。大文件/目录占用合并为默认折叠的「空间分析」区块；Confirm 级项目收进折叠「高级」区；大文件阈值与扫描深度移入设置面板；大文件删除废除 3 秒双点，统一一次确认弹窗；补齐「清空回收站」入口；文案去工程化。

## 背景
用户反馈：扫描/清理应"傻瓜式"，文件深度等工程参数增加使用难度。现状（代码证据）：
- `SpacePanel.vue` 大文件区暴露 ≥100/500/1000MB 三档阈值按钮、目录占用区展示"用户目录 · 3 层"
- 底部操作栏「全选 / ⚡一键清理 / 🗑清理选中」三按钮语义重叠
- 大文件删除走 3 秒倒计时双点确认（confirmPaths/batchConfirm），与垃圾清理的弹窗确认是两套范式
- 后端 `empty_recycle_bin` 命令已实现但 UI 无入口
- 失败错误展示英文原文

## Acceptance
1. 空间面板仅一个扫描按钮：空闲=扫描、扫描中=取消；窗口会话内首次打开 cleaner tab 自动开始扫描（仅一次）
2. 垃圾区 done 态：显示可释放总量 + 主按钮「一键清理」（仅 Safe 级）→ 确认弹窗（分类明细/文件数/释放量）→ 执行 → 中文 toast
3. Confirm 级项目默认收在折叠「高级」区，不参与一键清理，可展开逐项勾选清理
4. 大文件 + 目录占用合并为默认折叠「空间分析」区块；大文件删除 = 勾选 + 一次确认弹窗，无双点倒计时
5. 大文件阈值、扫描深度不出现在空间面板；设置面板新增「扫描与清理」参数区（阈值 100/500/1000MB、深度 1-5 层），保存后下次扫描生效
6. 「清空回收站」入口在主面板可用，成功后 toast 反馈
7. 失败错误前端映射为中文
8. 门禁全绿：`npx vue-tsc --noEmit`、`npm run build`、`cargo check -p pony_core -p pony_clean`、`cargo test -p pony_core`

## Non-Goal
- 不做免确认删除（安全底线：仍保留一次确认弹窗）
- 不改后端删除安全逻辑（受保护路径/扫描范围校验/服务停启零改动）
- 不做真实释放量的磁盘差值统计（预估值即可）
- 不重做监控面板与设置面板其他区域

## Validation Evidence
- 门禁全绿：`cargo check -p pony_core -p pony_clean` ✅、`cargo test -p pony_core` ✅（88 单测 + 6 集成）、`cargo fmt --check` ✅、`npx vue-tsc --noEmit` ✅、`npm run build` ✅
- 审查闭环：spec 2 路对抗审查（架构/安全）+ consultant 裁决 + 双代码审核两轮（首轮 5 个 P1 全部回改，复审双双通过）
- 顺带修复现存缺陷：P0 大小写误删 Confirm 级（level 类型化编译期门禁）、自定义 target 文件删除被拒
- 手动 QA：留待用户（清单见 spec §8）

## Next Action
用户手动 QA（spec §8 清单，重点：自动扫描一次、一键清理不含 Confirm、回收站确认、胶囊联动）。
