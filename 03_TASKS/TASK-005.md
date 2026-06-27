# TASK-005: Tauri v2 + Vue 3 + shadcn-vue 脚手架

## Basic Info
- Status: Ready
- Priority: P0
- Owner: @self
- Created: 2026-06-27
- Estimated: 4h

## Goal
在现有 `pony_clean` 仓库中搭建 Tauri v2 项目骨架，集成 Vue 3 + shadcn-vue + Tailwind + motion-vue。与现有 egui 代码共存，不破坏已有构建。

## Output
- `src-tauri/` — Tauri Rust 后端骨架（Cargo.toml、tauri.conf.json、main.rs）
- `frontend/` — Vue 3 + Vite + shadcn-vue + Tailwind 前端工程
- `crates/pony_core/` — 业务核心 lib crate（从 src/ 迁出）
- `Cargo.toml` — 改为 workspace 结构

## Acceptance
1. `cd frontend && npm install && npm run dev` 能启动 Vite dev server
2. `cargo tauri dev` 能弹出 Tauri 窗口（空白 Vue 页面）
3. shadcn-vue `npx shadcn-vue@latest add button` 能添加组件
4. 已有 `cargo build` / `cargo test` / `cargo clippy` 仍通过
5. Tailwind CSS 正确编译，shadcn-vue 组件正确渲染

## Spec
详见 `04_SPECS/SPEC-005-Tauri-Scaffold.md`
