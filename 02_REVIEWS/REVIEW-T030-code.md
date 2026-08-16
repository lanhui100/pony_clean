# REVIEW-T030-code: TASK-030 代码对抗审查记录

- 审查对象: `scripts/version-utils.mjs` / `scripts/bump-version.mjs` / `scripts/check-version.mjs` / `scripts/version-utils.test.mjs` / `ci.yml` version-sync job / 根 package.json / 相关文档（VERSIONING.md、ADR-012、RELEASE_CHECKLIST.md、README.md、AGENTS.md、SPEC-030）
- 审查方式: 2 路独立子智能体对抗审核（独立上下文并行）
  - Pass 1（@code-reviewer-a）: 正确性 / 边界 / 失败路径 / 回归风险（含隔离实验复验 cargo update 行为）
  - Pass 2（@code-reviewer-b）: 安全性 / git 操作边界 / 流程与文档一致性 / 运维体验
- 日期: 2026-08-16
- 结论: **有条件通过**（2 路均无 P0；4 个 P1 全部采纳修复后放行，无 consultant 升级）

## Pass 1（@code-reviewer-a）问题摘要

1. **[P1] 部分写入无事务**：先落盘四清单再跑 `cargo update`，update 失败（离线/锁冲突）时留"清单新、lock 旧"半态，无回滚。
2. **[P2] `rewriteJsonVersionLine` 改"首个 `"version"` 键"而非顶层**：嵌套键先出现时会静默错改（探针实证）。当前文件顶层恰为首个，不触发但属潜伏破坏。
3. **[P2] `archiveChangelog` 节边界 `/^##\s/m`**：`[Unreleased]` 内出现 level-2 `##` 标题会截断节（探针实证两种错误方向）。
4. **[P2] `spawnSync` 不解析 `.cmd`/`.bat`**：cargo/git 为 shim 时 ENOENT，报错信息含 `exit ?`。
5. **[P2] git 缺失时报误导信息**：`git show HEAD:Cargo.toml` 直接 spawn，status null 时误报"无法读取"。
6. **[P3]** porcelain 引号路径假设；`git add` 覆盖既有暂存意图；`isUnreleasedEmpty` 中 `###` 空子节算非空；`verifyAndTag` 并发 HEAD 过时（概率低）。

## Pass 2（@code-reviewer-b）问题摘要

1. **[P1] 单独 `--tag` 死锁**：`--tag` 前必先跑完整 bump，版本文件已是目标版本时命中"版本相同"报错；未 bump 时又命中"HEAD 版本旧"——两条路都堵死，`--tag` 单独用永远失败，VERSIONING.md "分步"承诺不可用。
2. **[P1] QA 后改动必然撞白名单**：文档顺序 bump → build → QA → commit/tag；QA 一旦需代码修复，`--commit` 白名单必中止，且版本已同步导致无法重跑 bump。
3. **[P1] 中途失败无回滚**：cargo update 失败 / 守卫中止后 6 文件已写未提交，CI 后续红、可能误打 tag。
4. **[P1] 首个发版被脏工作树阻塞**：当前工作树 30+ 已改 + 十余未跟踪文件，`--commit` 要求全树干净（设计意图，但文档低估清理量级）。
5. **[P2]** `git commit` 无 pathspec（预暂存文件混入风险）；push 漏 tag 无验证手段；"cargo update 无需网络"是假设非保证。
6. **[P3]** 提交体无中文（项目约定契合弱）；`.gitattributes eol=lf` 与"保字节"理论偏差；`isUnreleasedEmpty` 只认 5 种前缀未文档化；文档参数顺序与代码不一致。

## 采纳记录

| 意见 | 级别 | 采纳 | 处理 |
|---|---|---|---|
| P1 事务回滚（A/B 重合） | P1 | ✅ 采纳 | 快照 5 文件原内容；写文件/cargo update/断言任一失败自动还原 + 手动恢复命令提示；**实测 PATH 无 cargo 场景全量还原** |
| P1 单独 `--tag` 死锁（B） | P1 | ✅ 采纳 | **幂等模式**：版本已是目标版本且带 --commit/--tag 时跳过写文件与 cargo update，直接执行提交/打 tag |
| P1 QA 后改动撞白名单（B） | P1 | ✅ 采纳 | 幂等模式 + 文档明确"QA 修复先单独提交，再 `--commit`/`--tag` 分步收尾" |
| P1 首个发版阻塞（B） | P1 | ✅ 采纳 | 文档：发版第 0 步"工作树干净检查（git status --porcelain）+ git stash -u 提示"；RELEASE_CHECKLIST 同步 |
| P2 JSON 顶层锚定（A） | P2 | ✅ 采纳 | 正则限定缩进 0-2 空格；嵌套 `"version"` 不误改；无顶层则报错；补 2 个单测 |
| P2 changelog 节边界（A） | P2 | ✅ 采纳 | 节边界只认版本小节标题 `## [v] - 日期`；`## 说明` 不截断；补单测 |
| P2 .cmd shim（A） | P2 | ✅ 采纳 | `run`/`runCapture` 区分"二进制缺失"与"非零退出"，错误信息注明需真实 exe；VERSIONING.md 环境要求说明 |
| P2 git 缺失误导信息（A） | P2 | ✅ 采纳 | runCapture 统一处理 r.error |
| P2 commit pathspec（B） | P2 | ✅ 采纳 | `git commit -m ... -m <发布条目> -- <5 文件>` 限定提交范围 |
| P2 push tag 验证（B） | P2 | ✅ 采纳 | RELEASE_CHECKLIST/VERSIONING 增加 `git ls-remote --tags origin` 验证步骤 |
| P2 网络承诺（B） | P2 | ✅ 采纳 | 文档如实说明"lock 与清单不一致时可能联网刷新索引"；回滚兜底 |
| P3 提交体中文（B） | P3 | ✅ 采纳 | 提交体带本次发布条目（`extractReleaseEntries`，中文 changelog 内容） |
| P3 `###` 空子节（A/B） | P3 | ✅ 采纳 | `### Type` 子节需节内存在条目才非空；补单测 |
| P3 porcelain 引号路径（A） | P3 | ✅ 采纳（注释） | 代码注释说明引号路径判非预期属安全方向 |
| P3 文档参数顺序（B） | P3 | ✅ 采纳 | VERSIONING.md 统一为代码实际顺序 |
| P3 .gitattributes 偏差（B） | P3 | ⚠️ 部分采纳 | 记录为已知特性：脚本保工作区行尾，提交层由 git 归一 LF（仓库文件均为 LF，无实际影响） |
| P3 并发 HEAD 过时（A） | P3 | 不采纳 | 人为 CLI 调用，单进程无并发；--tag 守卫读 HEAD 时点即判定依据 |

## 修复后复验

- `node --test "scripts/*.test.mjs"` → **28/28 通过**（新增：JSON 嵌套键、`##` 说明标题、`-pre`+CRLF、无尾换行、lock 依赖列表不误匹配、`###` 空子节、发布条目提取）
- 真实 bump 0.1.1 往返 → 三处 + lock + CHANGELOG 一致（check-version exit 0）
- 幂等 `--commit`（工作树脏）→ 中止且零写入；幂等 `--tag`（HEAD 旧）→ 中止
- 回滚契约（PATH 无 cargo）→ 报错 + 5 文件自动还原
- 门禁：cargo check 两 crate / cargo test 98+6 / npm run build 全绿

## 结论

2 路独立对抗审核均通过（无 P0），P1 全部修复并复验，进入收口。
