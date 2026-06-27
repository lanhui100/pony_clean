# PonyClean 项目总控面板

## 项目信息
- 名称: PonyClean
- 描述: Windows 极简桌面小组件 — 进程监控报警 + C盘安全分析清理
- 技术栈: Rust (Tauri v2) + Vue 3 + shadcn-vue + Tailwind + sysinfo + jwalk
- 负责人: @self
- 当前阶段: Wave 2 — UI 重构（egui → Tauri v2 + Vue 3 + shadcn-vue）
- 更新日期: 2026-06-27

## 整体目标
1. 实时监控 Windows 进程，对 CPU/内存异常超高的进程报警，支持一键 kill
2. 安全扫描 C盘，分级展示可清理文件，支持勾选后批量清理
3. 极简半透明悬浮窗 UI，美观、流畅、低资源占用

## 当前状态
- 最关键任务: **TASK-005** Tauri v2 脚手架搭建 — 迁移的第一步
- 已完成: ✅ Wave 1 核心模块（monitor + cleaner）开发完成，29 项单元测试通过
- 已完成: ✅ egui UI 全面优化（设计系统、监控/清理面板重设计）
- 关键决策: 从 egui 迁移到 Tauri v2 + Vue 3 + shadcn-vue（[迁移方案](docs/MIGRATION_PLAN.md)）
- 最大阻塞: 无（业务逻辑 100% 无需改动）
- 下一步最小动作: TASK-005 — 搭建 Tauri v2 项目骨架 + shadcn-vue 集成

## 任务概览

| ID | 标题 | 状态 | 优先级 | 负责人 | 下一步 |
|---|---|---|---|---|---|
| TASK-001 | 项目脚手架搭建 | Done | P0 | @self | ✅ 已完成 |
| TASK-002 | 进程监控模块 | Done | P0 | @self | ✅ 已完成 |
| TASK-003 | C盘扫描与安全清理模块 | Done | P0 | @self | ✅ 已完成 |
| TASK-004 | egui UI 集成与数据流 | Done | P1 | @self | ✅ 已完成 |
| **TASK-005** | **Tauri v2 + shadcn-vue 脚手架** | **Ready** | **P0** | **@self** | **开始搭建** |
| **TASK-006** | **Rust 后端 Tauri 命令封装** | **Backlog** | **P0** | **@self** | **等待 TASK-005** |
| **TASK-007** | **Vue 设计系统 + 窗口布局** | **Backlog** | **P0** | **@self** | **等待 TASK-005** |
| **TASK-008** | **Vue 监控面板** | **Backlog** | **P1** | **@self** | **等待 TASK-006/007** |
| **TASK-009** | **Vue 清理面板** | **Backlog** | **P1** | **@self** | **等待 TASK-006/007** |
| **TASK-010** | **集成测试 + 旧代码清理** | **Backlog** | **P1** | **@self** | **等待 TASK-008/009** |

## 里程碑
- **M1 (Wave 1)**: egui 版本核心功能完成 ✅
- **M2 (Wave 2)**: Tauri v2 版本 UI 重构完成，功能等价，UI 质量显著提升
- **M3 (Wave 3)**: 托盘图标、系统通知、开机自启、配置持久化
- **M4 (Wave 4)**: 自定义阈值、清理路径规则可配、动效打磨
