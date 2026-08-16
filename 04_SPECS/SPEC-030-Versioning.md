# SPEC-030: 项目版本管理体系建设

- 状态: In Progress（2 路对抗审核通过 + 采纳修订完成 → 实现中）
- 关联: TASK-030
- 日期: 2026-08-15
- 复杂度: B（跨清单/脚本/CI/文档，设计决策明确后实现风险低）
- 审核: `02_REVIEWS/REVIEW-030-spec.md`（P0×1 + P1×4 全部采纳，详见第 10 节）

## 1. 背景与目标

现状问题（仓库证据）：

| 问题 | 证据 |
|---|---|
| 版本号 `0.1.0` 手工散落三处，无同步机制 | `Cargo.toml`（workspace.package）、`frontend/package.json`、`src-tauri/tauri.conf.json` |
| 无 changelog，历史变更无版本归档入口 | 仓库无 `CHANGELOG.md` |
| 无版本 bump 工具，发版靠手工改三处 + Cargo.lock | `scripts/` 为空目录 |
| CI 不校验版本一致性，改漏一处静默漂移 | `.github/workflows/ci.yml`、`test.yml` 均无版本检查 |
| 无版本 tag，无法回溯"某版本包含哪些变更" | `git tag` 为空；main 已领先 origin 80 提交 |
| 发布检查清单无版本步骤 | `RELEASE_CHECKLIST.md` 仅含手动 QA 项 |

目标：建立轻量的**单一变更点 + 自动同步 + CI 强制一致 + changelog + 语义化 tag + 文档化流程**的版本管理体系，使发版成为一条可复现命令而非手工多文件编辑。

## 2. 范围与非目标

范围：

- `scripts/bump-version.mjs` — 版本唯一变更点（校验 semver → 同步三处清单 → 刷新 Cargo.lock 并断言 → 归档 CHANGELOG [Unreleased]（空节拒绝）→ 可选 `--commit` / `--tag`，支持 `--dry-run`）
- `scripts/check-version.mjs` — 四处版本一致性校验（三处清单 + Cargo.lock 成员条目，CI 门禁 + 本地自查）
- `scripts/version-utils.mjs` + `scripts/version-utils.test.mjs` — 纯函数抽离 + `node --test` 契约测试（Node 内置测试器，保持零依赖）
- `CHANGELOG.md` — Keep a Changelog 风格（中文条目），含 `[Unreleased]` 与 `[0.1.0]` 基线
- `.github/workflows/ci.yml` — 新增 `version-sync` job（windows-latest，与现有 CI 一致；运行 check + node --test）
- 根 `package.json` — 新增 `check:version` / `bump:version` 脚本入口
- 文档：`docs/VERSIONING.md`（发版流程与规范）、`docs/DESIGN.md`（ADR-012）、`docs/README.md`（链接）、`RELEASE_CHECKLIST.md`（版本步骤）、`README.md`（项目结构）、`AGENTS.md`（快速命令表补 2 行）
- 任务系统：`03_TASKS/TASK-030.md`、`02_REVIEWS/REVIEW-030-spec.md`、`02_REVIEWS/REVIEW-T030-code.md`

非目标：

- 自动生成 changelog（引入 conventional commits 解析器，收益低）
- UI 内版本展示（后续单独任务）
- npm / crates.io 发布、GitHub Release 自动发布（仓库无远端推送权限流程，tag 由人工推送）
- 创建 `Makefile.toml`（当前不存在，AGENTS.md 与其不一致属既有问题，不在本任务范围）
- 不改变现有构建、打包、CI 测试逻辑

## 3. 用户/系统行为（开发与发版流程）

1. **日常开发**：功能合并后向 `CHANGELOG.md` 的 `## [Unreleased]` 小节追加条目（类型前缀：`Added` / `Changed` / `Fixed` / `Removed` / `Security`，中文描述）
2. **发版**：`node scripts/bump-version.mjs 0.2.0` →
   - 校验 semver 且与当前版本不同
   - 同步 `Cargo.toml` / `frontend/package.json` / `src-tauri/tauri.conf.json` 三处版本（文本精改，保留字节与行尾）
   - 刷新 `Cargo.lock`：`cargo update -p pony_clean -p pony_core -w`（限定 workspace 成员，最小 lock diff）→ **断言** lock 中两成员条目版本 == 新版本，失败即退出
   - 校验 `[Unreleased]` 节存在且唯一、**非空**（空节拒绝发版）→ 改名为 `## [0.2.0] - YYYY-MM-DD`（本地日期），上方插入新 `[Unreleased]` 小节
   - 打印汇总 diff，不自动提交
3. **可选收尾**：`--commit` 生成 `chore(release): v0.2.0` 提交（白名单 = 5 个版本文件，存在任何其他已修改/未跟踪文件即中止并列出，提示先提交）；`--tag` 生成 annotated tag `v0.2.0`（消息 `v0.2.0 发布`），前置守卫：HEAD 的 `Cargo.toml` 必须已含目标版本、同名 tag 不存在
4. **CI**：每次 push/PR 运行 `node scripts/check-version.mjs`（四处不一致即红）+ `node --test scripts/`
5. **人工改漏**：本地 `node scripts/check-version.mjs` 立即报错并指出差异文件
6. **发版流程文档**：`docs/VERSIONING.md` 规定 bump → 构建 → 手动 QA（RELEASE_CHECKLIST）→ commit/tag → `git push --follow-tags` 的完整顺序

## 4. 技术方案与替代

### 4.1 采纳方案：三处同步 + 脚本唯一变更点 + CI 校验

- **bump-version.mjs**（Node ≥ 20，纯 stdlib，无依赖）：
  - 参数：`<new-version>`；选项：`--commit`、`--tag`、`--dry-run`
  - semver 校验：`^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$`，与当前版本相同且无 --commit/--tag 则报错退出
  - `Cargo.toml`：正则替换 `[workspace.package]` 段内 `version = "x.y.z"`（保留文件其余字节不变）
  - `package.json` / `tauri.conf.json`：**正则精改顶层 `"version"` 行**（缩进 0-2 空格锚定，不误改嵌套键；保留字节/行尾，CRLF 安全）；读取时 `JSON.parse` 校验可解析
  - `Cargo.lock`：写入清单后执行 `cargo update -p pony_core -p pony_clean -w`（隔离实验实证：`cargo metadata --no-deps` **不会**刷新 lock；`cargo update` / `cargo check` 可以；`-p` 限定两成员减少连带 diff）→ **回读断言** lock 中 `pony_clean` / `pony_core` 条目版本 == 新版本，否则 exit 1
  - CHANGELOG：定位 `## [Unreleased]` 小节（必须**恰好一个**，缺失/多个均报错）；校验节内非空（`- Type: 内容` 条目，或 `### Type` 子节且节内存在条目，注释不算）；**节边界只认版本小节标题 `## [v] - 日期`**（`## 说明` 等不会截断节）；改名为 `## [x.y.z] - <本地日期>`，并在其上方插入新 `## [Unreleased]` + 注释模板
  - **事务与幂等**：写文件 / cargo update / 断言任一失败 → 自动还原全部 5 个版本文件（不留半态），并给出手动恢复命令；版本已是目标版本时 `--commit`/`--tag` 跳过写文件直接收尾（支持"QA 修复后补提交、分步打 tag"两种真实流程）
  - `--commit`：守卫**前置到写文件之前**——`git status --porcelain` 校验已修改 tracked 文件 ⊆ 5 个版本文件且无任何 untracked 文件，不满足则中止并列出；通过后 `git add` 五文件 + `git commit -m "chore(release): vX.Y.Z" -m <发布条目> -- <5 文件>`（pathspec 限定提交范围；幂等：无改动则跳过）
  - `--tag`：校验 `git show HEAD:Cargo.toml` 已含目标版本（HEAD 确为该版本提交）+ `git tag` 无同名；通过后 `git tag -a vX.Y.Z -m "vX.Y.Z 发布"`
  - `--dry-run`：全部步骤只计算不落盘、不执行 git/cargo，输出将要写入的值
- **check-version.mjs**：读取 workspace `Cargo.toml` 版本（断言 `[workspace.package]` 存在）+ `members` 列表 → 逐成员读其 `Cargo.toml` 的 `[package] name` → 在 `Cargo.lock` 中提取对应条目版本；与 `package.json` / `tauri.conf.json` 版本共**四处**比较；不一致打印差异并 `process.exit(1)`；文件损坏（JSON 解析失败、缺段）明确报错。纯 stdlib，无依赖
- **version-utils.mjs**：纯函数（版本解析/校验、文本精改、lock 成员提取、changelog 归档、发布条目提取、本地日期），供两个脚本共用；`version-utils.test.mjs` 用 `node --test`（Node 20 内置，零依赖）覆盖：非法/相同版本拒绝、JSON 顶层精改（嵌套键不误改）、lock 成员提取（依赖列表不误匹配）、changelog 归档（正常/缺失/多段/空节/`##` 说明标题/`-pre` 版本/CRLF/无尾换行）、发布条目提取
- **ci.yml** 新增 job：`version-sync`，`runs-on: windows-latest`（与现有 CI 一致），`node scripts/check-version.mjs` + `node --test "scripts/*.test.mjs"`

### 4.2 替代方案（不采纳）

| 方案 | 不采纳理由 |
|---|---|
| B: beforeBuildCommand 从 package.json 注入 tauri 版本 | 构建期耦合；Cargo/Rust 侧仍需版本，维护面未减少 |
| C: 单一来源 workspace Cargo.toml，打包时读取 | Tauri v2 不支持直接引用清单版本，仍需注入，同 B |
| D: release-plz / cargo-bump 等工具 | 引入外部工具链与远端集成；本项目发版频率低，脚本已足够 |

## 5. 影响面和依赖

- 改动文件：3 份清单、`Cargo.lock`、`ci.yml`、5 份文档、2 个新脚本、任务系统 4 个文件
- 无运行时代码（Rust/Vue 源码）改动；版本号变更后由门禁 `cargo check` 验证 lock 与编译
- 依赖：Node ≥ 20（仓库已有 node v24）、cargo（`cargo update` 刷新 lock，需真实可执行文件；lock 与清单一致时通常无需网络，不一致时可能联网刷新索引）、git（--commit/--tag 时，需真实可执行文件）
- 不依赖：第三方 npm 包；远端仓库（push 为人工步骤）

## 6. 任务拆解与并行边界

| # | 任务 | 并行性 |
|---|---|---|
| 1 | 两个脚本（bump / check） | 串行实现，脚本间共享版本提取逻辑 |
| 2 | CHANGELOG.md 基线 | 可与 1 并行（独立文件） |
| 3 | ci.yml version-sync job | 依赖 1（脚本存在） |
| 4 | 文档（VERSIONING / ADR-012 / 链接 / RELEASE_CHECKLIST / README） | 可与 1、2 并行 |
| 5 | 任务系统收口 | 依赖全部 |

## 7. 风险、回滚与迁移

| 风险 | 缓解 |
|---|---|
| Cargo.lock 版本陈旧导致构建漂移 | bump 内 `cargo update -p pony_clean -p pony_core -w` 刷新 + **回读断言**成员版本 + 门禁 `cargo check` 三重保险 |
| JSON 精改破坏格式 | 两文件已确认为纯 JSON；仅正则改 `"version"` 行保留字节/行尾；读取时 `JSON.parse` 校验可解析；git diff 可审 |
| CHANGELOG 误改 | `[Unreleased]` 缺失/多个/空节均中止；变更全部为文本替换，git 可回滚 |
| `--commit` 误带无关改动 | 白名单 + 无 untracked 校验；不满足则中止并列出全部非预期项 |
| tag 打到错误提交 | `git show HEAD:Cargo.toml` 版本校验 + tag 同名不存在校验 |
| `cargo update` 连带依赖升级 | `-p` 限定两 workspace 成员（非 `-w` 全量）；lock diff 纳入验收检查 |

迁移：无数据迁移；新增文件全部向后兼容，旧流程（手工改版本）仍可用但 CI 会立即提示不一致。

## 8. 测试计划

1. `node --test "scripts/*.test.mjs"` → 契约测试全绿（版本校验、JSON 顶层精改、lock 提取、changelog 归档、发布条目提取五类，28 项）
2. `node scripts/check-version.mjs` → exit 0（当前四处一致）
3. 手工改一处版本 → check → exit 1 且报出差异（验证后还原）
4. 手工改 Cargo.lock 成员版本 → check → exit 1（三处一致但 lock 旧必须报红，验证后还原）
5. `node scripts/bump-version.mjs --dry-run` → 不落盘、输出目标版本
6. `node scripts/bump-version.mjs 0.1.1` 实测 → 三处 + lock + CHANGELOG 全部更新；`git diff` 审查确认 **lock 仅成员版本行变化**、无连带依赖 diff → 验证后还原（git checkout 清单/lock + 备份还原 CHANGELOG）
7. 非法版本 / 相同版本 / 缺 [Unreleased] / 空 [Unreleased] / 多段 [Unreleased] → 报错退出
8. `--commit` 守卫：工作树含非预期文件时中止并列出（当前实现期状态即满足该场景，天然可测）
9. **幂等模式**：bump 到 0.1.1 后再次 `bump 0.1.1 --commit`（工作树脏 → 守卫中止，不写文件）/ `bump 0.1.1 --tag`（HEAD 版本旧 → 中止）
10. **回滚契约**：PATH 移除 cargo 后 bump → 报错且**全部 5 个版本文件自动还原**（无半态）
11. 门禁：`cargo check -p pony_core -p pony_clean`、`cargo test -p pony_core`、`npm run build` 全绿

## 9. 验收标准

1. `node scripts/bump-version.mjs <v>` 一键同步 Cargo.toml / package.json / tauri.conf.json / Cargo.lock（**含 lock 成员版本断言**，不一致自动失败）/ CHANGELOG
2. `node scripts/check-version.mjs` 四处（三清单 + Cargo.lock）不一致时 exit 1 并输出差异；一致时 exit 0
3. `ci.yml` 含 `version-sync` job（windows-latest），push/PR 自动校验版本一致性 + 运行脚本测试
4. `CHANGELOG.md` 含 `[Unreleased]` 与 `[0.1.0]` 基线节；空 `[Unreleased]` 无法通过 bump 归档
5. `docs/VERSIONING.md` 含完整发版流程（含 `git push --follow-tags` 与 tag 验证）；`docs/DESIGN.md` 含 ADR-012；`RELEASE_CHECKLIST.md` 含版本步骤；`AGENTS.md` 快速命令表含 bump/check
6. bump `--dry-run` 不落盘；`--commit` 对任何非预期改动（含 untracked）中止并列出；`--tag` 对"HEAD 版本 ≠ 目标版本 / tag 已存在"中止
7. **幂等**：版本已是目标版本时 `--commit`/`--tag` 跳过写文件直接收尾；**回滚**：cargo/git 失败后 5 个版本文件无半态
8. 门禁：`cargo check` 两 crate、`cargo test -p pony_core`、`npm run build` 全绿

## 10. 审核记录

**Spec 审核**（2026-08-15，详见 `02_REVIEWS/REVIEW-030-spec.md`）：

- **Pass 1**（@code-reviewer-a，正确性/边界）: 有条件通过。P1×2（lock 刷新机制不可靠且与"无网络依赖"矛盾；check 不校验 Cargo.lock）+ P2×2 + P3×4。
- **Pass 2**（@code-reviewer-b，架构/简化）: 有条件通过。**P0×1（隔离实验实证 `cargo metadata --no-deps` 不会刷新 Cargo.lock，应改用 `cargo update`）** + P1×2（changelog 空归档无门禁；--commit 白名单与 --tag 顺序间隙）+ P2×3 + P3×3。

采纳结果（全部 P0/P1 采纳；完整逐条记录见 REVIEW-030-spec.md）:

| 类别 | 采纳处理 |
|---|---|
| lock 刷新 | `cargo update -p pony_clean -p pony_core -w` + 回读断言（替代 `cargo metadata`） |
| check 覆盖 lock | 第 4 处校验：workspace members → 成员 Cargo.toml name → lock 条目版本 |
| changelog 门禁 | bump 归档前校验 [Unreleased] 唯一且非空；check 不拦空节（避免阻塞日常 push，详见不采纳说明） |
| --commit / --tag 守卫 | 白名单 + 无 untracked 校验；HEAD 版本校验 + tag 同名校验 |
| 脚本可测性 | version-utils.mjs 纯函数 + `node --test`（零依赖） |
| 其他 | 本地日期、JSON 正则精改保字节、tag 消息中文、`git push --follow-tags`、AGENTS.md 补命令、CI runner 统一 windows-latest |

不采纳：check 拦空 [Unreleased]（误伤日常 push）；合并 test.yml/ci.yml（范围外，后续任务候选）。

**代码审核**（2026-08-16，详见 `02_REVIEWS/REVIEW-T030-code.md`）：

- **Pass 1**（@code-reviewer-a，正确性/边界）: 有条件通过，无 P0。P1×1（写文件后 cargo update 失败留半态）+ P2×4（JSON 精改锚定首个而非顶层键；changelog 节边界 `##` 截断；spawn 不解析 .cmd shim；git 缺失误导信息）+ P3×4 + 测试缺口。
- **Pass 2**（@code-reviewer-b，流程/安全）: 有条件通过，无 P0。P1×4（单独 `--tag` 死锁、文档"分步"承诺不可用；QA 后改动必然撞白名单；bump 中途失败无回滚；首个发版被脏工作树阻塞）+ P2×3 + P3×4。

采纳结果（全部 P1 采纳；完整逐条记录见 REVIEW-T030-code.md）:

| 类别 | 采纳处理 |
|---|---|
| 事务回滚 | 写文件 / cargo update / 断言失败自动还原 5 文件 + 手动恢复命令提示（实测 PATH 无 cargo 场景） |
| 幂等 commit/tag | 版本已是目标版本时跳过写文件直接收尾 → 单独 `--tag`、"QA 修复后补提交"两条流程跑通 |
| 守卫前置 | `--commit` 白名单校验移到写文件之前，守卫失败零写入 |
| git 纵深 | `git commit -m ... -m <发布条目> -- <5 文件>` pathspec 限定提交范围 |
| JSON 顶层锚定 | 缩进 0-2 空格锚定，嵌套 `"version"` 不误改（含单测） |
| changelog 节边界 | 只认版本小节标题 `## [v] - 日期`，`## 说明` 不截断（含单测） |
| 空节判定收紧 | `### Type` 子节需节内存在条目（含单测） |
| 文档 | 发版流程重写（原子/分步双路径 + QA 修复规则 + 干净工作树前置 + tag 推送验证）；网络/cargo 环境要求如实说明 |
