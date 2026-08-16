# REVIEW-030: SPEC-030 对抗审查记录

- 审查对象: `04_SPECS/SPEC-030-Versioning.md`（TASK-030）
- 审查方式: 2 路独立子智能体对抗审核（独立上下文并行）
  - Pass 1（@code-reviewer-a）: 正确性 / 边界 / 失败路径 / 可维护性
  - Pass 2（@code-reviewer-b）: 架构一致性 / 简化方案 / 运维发版体验 / 项目约定契合
- 日期: 2026-08-15
- 结论: **有条件通过**（P0 事实错误 + P1 修复后放行；均无需升级 consultant）

## Pass 1（@code-reviewer-a）问题摘要

1. **[P1] lock 刷新机制不可靠**：`cargo metadata --no-deps` 的 lock 写入是 resolver 副作用而非契约，版本变化触发重解析可能联网，与 spec"不依赖网络"声明矛盾。
2. **[P1] check-version.mjs 不校验 Cargo.lock**：人工只改三处清单漏改 lock → check 报绿 → lock 陈旧漂移，读写不对称。
3. **[P2] --commit 白名单死锁**：首次使用（版本管理功能自身未提交）时新增文件全部判为非预期 → 中止。spec 中 `--include-extra` 括号设计未完成。
4. **[P2] --tag 流程欠定义**：单独 `--tag` 会 tag 到旧版本 HEAD，无"HEAD 含该版本"守卫。
5. **[P2] push --tags 缺失**：文档未写 tag 推送步骤，存在"tag 打了没推"。
6. **[P3] 日期 UTC/本地未明**；JSON 重写破坏字节/行尾（CRLF）；多段 [Unreleased] 未防御；check 与 workspace.package 布局硬耦合。

## Pass 2（@code-reviewer-b）问题摘要

1. **[P0] `cargo metadata --no-deps` 不会刷新 Cargo.lock（隔离实验实证）**：改 workspace 包版本后执行 metadata，lock 仍为旧版本；对照 `cargo check` / `cargo update -w` 均能刷新。后果：bump 后 lock 陈旧、CI 门禁全绿放行、本地/CI lock 分叉。
2. **[P1] CHANGELOG 无强制机制会退化**：check 只比版本号，不拦空 [Unreleased] 归档；无任何机制阻止空 changelog 发版。
3. **[P1] --commit 白名单与 tag 顺序间隙**：`cargo update -w` 全量可能连带升级依赖 → lock 非预期 diff；--tag 两步序列无守卫。
4. **[P2] CI runner 不统一**：version-sync 用 ubuntu 而现有 CI 全 windows；test.yml 与 ci.yml 两个 `name: CI` 重复（既有坏味道，范围外）。
5. **[P2] 脚本可测性**：零依赖脚本正确性全凭手工实测，建议 `node --test`（Node 内置，仍零依赖）为纯函数写契约测试。
6. **[P2] tauri.conf.json 版本 ↔ MSI 产品版本关系未文档化**。
7. **[P3] tag 消息建议中文**（项目提交体全中文）；AGENTS.md 快速命令表未列新脚本；changelog 日期漂移；ADR-012 编号确认无冲突（DESIGN.md 现有至 011）。

## 采纳记录

| 意见 | 级别 | 采纳 | 处理 |
|---|---|---|---|
| P0: lock 刷新命令错误（B 实证） | P0 | ✅ 采纳 | lock 刷新改为 `cargo update -p pony_clean -p pony_core -w`（限定 workspace 成员，减少连带 diff）+ bump 末尾**显式断言** lock 成员版本 = 新版本，失败即退出 |
| P1: check 不校验 Cargo.lock（A） | P1 | ✅ 采纳 | check-version.mjs 增加第 4 处校验：按 workspace members 逐成员读取其 Cargo.toml `[package] name`，在 Cargo.lock 中提取对应条目版本比对 |
| P1: CHANGELOG 空归档无门禁（B） | P1 | ✅ 采纳（增强版） | bump 归档前校验 [Unreleased] 非空，空则中止并提示先补 changelog；check-version **不**拦空节（避免阻塞日常 push），差异见下 |
| P1: --commit 白名单（A P2-1 / B P1-2） | P1 | ✅ 采纳 | 白名单 = 5 个版本文件；存在任何其他已修改/未跟踪文件 → 中止并列出，提示先提交或不用 --commit；VERSIONING.md 写明"首次使用先提交版本管理功能本身" |
| P1: --tag 守卫（A P2-2 / B P1-2） | P1 | ✅ 采纳 | --tag 前校验 `git show HEAD:Cargo.toml` 含目标版本 + tag 不存在；文档推荐原子用法 `bump <v> --commit --tag` |
| P2: push --tags（A） | P2 | ✅ 采纳 | VERSIONING.md / RELEASE_CHECKLIST 增加 `git push --follow-tags` |
| P2: 脚本可测性（B） | P2 | ✅ 采纳 | 纯函数抽到 `scripts/version-utils.mjs`，`node --test` 契约测试（仍零依赖） |
| P2: tauri.conf.json ↔ MSI 版本（B） | P2 | ✅ 采纳 | VERSIONING.md 说明关系 |
| P3: 本地日期（A） | P3 | ✅ 采纳 | changelog 日期取本地日期（非 UTC） |
| P3: JSON 字节保真（A） | P3 | ✅ 采纳 | JSON 文件改为正则精改 `"version"` 行，不整体重写（保行尾/字节，CRLF 安全） |
| P3: 多段 [Unreleased] 防御（A） | P3 | ✅ 采纳 | 多于一个 `## [Unreleased]` 报错 |
| P3: workspace 布局耦合（A） | P3 | ✅ 采纳 | 读取时断言 `[workspace.package]` / `[workspace] members` 存在，缺则明确报错 |
| P3: tag 消息中文（B） | P3 | ✅ 采纳 | tag message 默认 `vX.Y.Z 发布`（tag 名保持英文） |
| P3: AGENTS.md 补命令（B） | P3 | ✅ 采纳 | 快速命令表补 bump/check 两行 |
| P3: changelog 日期漂移（B） | P3 | ✅ 采纳 | 文档规定"归档日期以 bump 当天为准" |
| P2: CI runner 统一（B） | P2 | ⚠️ 部分采纳 | version-sync job 改用 windows-latest（与现有 CI 一致）；**不采纳**合并 test.yml/ci.yml（既有重复属范围外，记为后续任务候选） |
| P2: cargo update 连带 diff 提示（B） | P2 | ✅ 采纳 | `-p` 限定两成员 + 断言；lock diff 纳入验收检查项 |

## 不采纳说明

- **check-version 拦空 [Unreleased]**（B 的 P1-1 简单版）：CI 每次 push 都跑 check，日常开发期 [Unreleased] 为空是常态，会误伤所有无关推送；空 changelog 拦截属于**发版动作时点**的职责，放在 bump 归档前（增强版），逻辑自洽。
- **合并 test.yml 与 ci.yml**（B P2-1 顺带项）：既有重复与本次版本管理无直接关系，避免扩大 diff，记为后续任务。

## 结论

有条件通过 → 已按采纳记录修订 SPEC-030（4.1 技术方案、8 测试计划、9 验收标准、10 审核记录），进入实现。
