# TASK-006: Rust 后端 Tauri 命令封装

## Basic Info
- Status: Backlog
- Priority: P0
- Owner: @self
- Created: 2026-06-27
- Estimated: 6h
- Depends: TASK-005

## Goal
将 `pony_core` 库的 monitor / cleaner 功能暴露为 Tauri commands + events，替换原有的 mpsc 通道通信。前端通过 `@tauri-apps/api` 调用。

## Output
- `src-tauri/src/commands/mod.rs` — 模块组织
- `src-tauri/src/commands/monitor.rs` — get_processes / kill_process
- `src-tauri/src/commands/cleaner.rs` — start_scan / execute_clean / empty_recycle_bin
- `src-tauri/src/commands/events.rs` — scan-progress Tauri Event
- `pony_core` monitor::start 适配为 Tauri 命令可调用的同步/异步接口

## Acceptance
1. `invoke('get_processes')` 返回进程列表 JSON
2. `invoke('kill_process', { pid, name })` 成功终止进程
3. `invoke('start_scan')` 启动扫描，scan-progress event 实时推送进度
4. `invoke('execute_clean', { paths })` 删除后返回 DeleteResult
5. 所有原有单元测试通过
6. 前后端 IPC 错误处理完整（前端能拿到 Rust 端错误信息）

## Spec
详见 `04_SPECS/SPEC-006-Tauri-Commands.md`
