# TASK-010: 集成测试 + 旧代码清理 + ADR 更新

## Basic Info
- Status: Backlog
- Priority: P1
- Owner: @self
- Created: 2026-06-27
- Estimated: 3h
- Depends: TASK-008, TASK-009

## Goal
端到端验证 Tauri 版本功能完整性，清理废弃的 egui 代码，更新架构文档和 ADR。

## Output
- 删除了 `src/app.rs`、`src/theme.rs`（egui 相关，不再需要）
- `src/main.rs` 简化移除 eframe 引用
- 更新 `docs/ARCHITECTURE.md` 反映 Tauri 架构
- 更新 `docs/DESIGN.md` 添加 ADR-007（egui → Tauri 迁移决策）
- 更新 `AGENTS.md` 反映新技术栈
- 清理 Cargo.toml 中 egui/eframe/wgpu 依赖
- 手动 E2E 验证清单

## Acceptance
1. `cargo build` 通过（无 egui/eframe 依赖）
2. `cargo tauri build` 通过（release 二进制）
3. `cargo test` 全部通过（pony_core 单元测试）
4. E2E 验证清单全部通过监控 + 清理功能
5. 架构文档和 ADR 一致
6. AGENTS.md 命令更新为新技术栈

## Spec
详见 `04_SPECS/SPEC-010-Cleanup-Test.md`
