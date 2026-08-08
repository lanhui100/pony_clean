# PonyClean 项目总控面板

## 项目信息
- 名称: PonyClean
- 描述: Windows 极简桌面小组件 — 进程监控报警 + C盘安全分析清理 + 内存整理
- 技术栈: Rust (Tauri v2) + Vue 3 + shadcn-vue + Tailwind + sysinfo + jwalk
- 负责人: @self
- 当前阶段: Wave 3 — 托盘图标、系统通知、配置持久化（代码完成，待手动 QA）
- 更新日期: 2026-08-08

## 整体目标
1. 实时监控 Windows 进程，对 CPU/内存异常超高的进程报警，支持一键 kill
2. 安全扫描 C盘，分级展示可清理文件，支持勾选后批量清理
3. 一键整理内存（EmptyWorkingSet），恢复系统流畅度
4. 极简半透明悬浮窗 UI（胶囊灵动岛），美观、流畅、低资源占用

## 当前状态
- 最关键任务: **TASK-013/015 手动 QA** — 灵动岛面板接入 + Wave 3 功能验证
- 已完成: ✅ Wave 1 核心模块（monitor + cleaner + memory）开发完成，71 项测试通过
- 已完成: ✅ UI 重构（Tauri v2 + Vue 3 + shadcn-vue）+ 双窗口灵动岛
- 已完成: ✅ 主链路打通（面板接入 + 动态高度）+ 内存整理 + 托盘/通知/自启/配置
- 最大阻塞: 无（待真实窗口手动 QA）
- 下一步最小动作: 启动 `npm run dev:tauri` 手动 QA 灵动岛交互与托盘/通知

## 任务概览

| ID | 标题 | 状态 | 优先级 | 负责人 | 下一步 |
|---|---|---|---|---|---|
| TASK-001 | 项目脚手架搭建 | Done | P0 | @self | ✅ 已完成 |
| TASK-002 | 进程监控模块 | Done | P0 | @self | ✅ 已完成 |
| TASK-003 | C盘扫描与安全清理模块 | Done | P0 | @self | ✅ 已完成 |
| TASK-004 | UI 集成与数据流 | Done | P1 | @self | ✅ 已完成 |
| TASK-012 | 双窗口灵动岛毛玻璃实现 | Done | P1 | @self | ✅ 已完成 |
| TASK-014 | 内存整理（EmptyWorkingSet） | Done | P1 | @self | ✅ 已完成 |
| **TASK-013** | **灵动岛面板接入 + 动态高度** | **Validation** | **P0** | **@self** | **手动 QA** |
| **TASK-015** | **Wave 3 托盘/通知/自启/配置** | **Validation** | **P1** | **@self** | **手动 QA** |
| TASK-011 | UI 设计规范制定与对抗式审核 | In Progress | P1 | @self | 补产物（SPEC-011 缺失） |

## 里程碑
- **M1 (Wave 1)**: 核心功能完成（monitor + cleaner 模块） ✅
- **M2 (Wave 2)**: Tauri v2 版本 UI 重构完成，功能等价，UI 质量显著提升 ✅
- **M3 (Wave 3)**: 托盘图标、系统通知、开机自启、配置持久化 — 代码完成，待 QA
- **M4 (Wave 4)**: 自定义阈值 UI、清理路径规则可配、动效打磨
