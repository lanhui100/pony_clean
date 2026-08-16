# TASK-030: 项目版本管理体系建设

## Basic Info
- Status: Done
- Validated: 2026-08-16
- Priority: P1
- Owner: @self（agent team 编排）
- Created: 2026-08-15
- Estimated: 3h
- Depends: 无
- Complexity: B（跨清单/脚本/CI/文档，需明确"唯一变更点"设计）
- Spec: `04_SPECS/SPEC-030-Versioning.md`

## Goal
建立"单一变更点 + 自动同步 + CI 强制一致 + changelog + 语义化 tag + 文档化流程"的版本管理体系：`scripts/bump-version.mjs` 一键发版（三处清单 + Cargo.lock + CHANGELOG 同步），`scripts/check-version.mjs` 一致性门禁，CI 自动校验，文档覆盖发版流程。

## 背景
版本 `0.1.0` 散落三处（workspace Cargo.toml / frontend/package.json / tauri.conf.json）无同步机制；无 changelog、无 bump 工具、无版本 tag（git tag 为空）；CI 不校验版本一致性；RELEASE_CHECKLIST 无版本步骤。

## Acceptance
1. `node scripts/bump-version.mjs <v>` 一键同步 Cargo.toml / package.json / tauri.conf.json / Cargo.lock / CHANGELOG（[Unreleased] 归档为新版本节）
2. `node scripts/check-version.mjs` 不一致 exit 1 并输出差异；一致 exit 0
3. ci.yml 含 version-sync job（push/PR 自动校验）
4. CHANGELOG.md 含 [Unreleased] 与 [0.1.0] 基线节
5. docs/VERSIONING.md 含发版流程；DESIGN.md 含 ADR-012；RELEASE_CHECKLIST 含版本步骤
6. bump --dry-run 不落盘；--commit/--tag 对非预期改动中止；非法/相同版本、缺 [Unreleased] 均报错退出
7. 门禁全绿：`cargo check -p pony_core -p pony_clean`、`cargo test -p pony_core`、`npm run build`

## Non-Goal
自动 changelog 生成、UI 版本展示、npm/crates 发布、GitHub Release 自动发布、Makefile.toml 创建。

## Validation Evidence

- 规格审核：2 路独立对抗审核有条件通过（P0×1/P1×6 全部采纳），记录 `02_REVIEWS/REVIEW-030-spec.md`
- 代码审核：2 路独立对抗审核有条件通过（P1×5 全部采纳），记录 `02_REVIEWS/REVIEW-T030-code.md`
- `node --test "scripts/*.test.mjs"` → 28/28 通过（版本校验/JSON 顶层精改/lock 提取/changelog 归档/发布条目提取）
- `node scripts/check-version.mjs` → exit 0（四处一致）；手工改版本/lock → exit 1 报差异
- 真实 bump 0.1.1 往返 → 三处清单 + Cargo.lock + CHANGELOG 同步，lock diff 仅成员版本行
- 守卫实测：非法/相同版本、空 [Unreleased]、--commit 脏工作树中止、--tag HEAD 版本校验中止、--dry-run 零写入、幂等模式跳过写文件
- 回滚契约实测：PATH 无 cargo → 报错 + 5 文件自动还原
- 门禁：`cargo check -p pony_core -p pony_clean` ✅、`cargo test -p pony_core` 98+6 ✅、`npm run build` ✅

## Next Action

无（待用户提交；可选：基线 tag v0.1.0）

## Resume Hint

发版：`node scripts/bump-version.mjs 0.2.0 [--commit] [--tag]`，流程见 `docs/VERSIONING.md`。
