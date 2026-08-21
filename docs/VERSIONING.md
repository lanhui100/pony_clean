# 版本管理（Versioning）

PonyClean 的版本管理体系：**单一变更点 + 自动同步 + CI 强制一致 + 语义化 tag**。
设计决策见 [ADR-012](DESIGN.md#adr-012-版本管理--三处清单同步--脚本唯一变更点--ci-强制一致)。

## 版本号规则

- 采用[语义化版本](https://semver.org/lang/zh-CN/) `X.Y.Z`（可带 `-pre` 预发布后缀）。
- 0.x 阶段：minor 增加 = 新功能，patch 增加 = 缺陷修复。
- **版本唯一权威**：以下四处必须一致，手工改任何一处都会被门禁拦截：

| 位置 | 说明 |
|---|---|
| `Cargo.toml` `[workspace.package] version` | Rust 侧唯一来源 |
| `frontend/package.json` `version` | 前端清单 |
| `src-tauri/tauri.conf.json` `version` | 构建产物版本 —— **`cargo tauri build` 产出的 MSI 产品版本（file version）即此值** |
| `Cargo.lock` 中 `pony_core` / `pony_clean` 条目 | workspace 成员版本 |

## 日常开发

合并任何用户可见变更后，向 `CHANGELOG.md` 的 `## [Unreleased]` 小节追加条目：

```markdown
- Added: 新功能（中文描述）
- Fixed: 缺陷修复
```

- 类型前缀：`Added` / `Changed` / `Fixed` / `Removed` / `Security`。
- **只有这 5 种类型的条目算有效内容**；空节、纯注释、其他前缀（如 `- TODO:`）不满足发版门禁。
- **发版前 `[Unreleased]` 必须非空**（bump 脚本强制校验，空节拒绝发版）。

## 发版流程

```bash
# 0. 确保工作树干净（只有版本文件允许有改动；WIP 用 git stash -u 暂存）
git status --porcelain

# 1. 确认四处版本一致
node scripts/check-version.mjs            # 或 npm run check:version

# 2. 确认 CHANGELOG [Unreleased] 已记录本次全部变更（bump 会强制校验非空）

# 3. 同步版本：三处清单 + Cargo.lock + CHANGELOG 归档（不提交）
node scripts/bump-version.mjs 0.2.0       # 或 npm run bump:version -- 0.2.0

# 4. 构建 + 手动 QA（按 RELEASE_CHECKLIST.md）
cargo tauri build

# 5. 提交与打 tag（二选一）
#    a) 原子方式（QA 无代码改动时）：
node scripts/bump-version.mjs 0.2.0 --commit --tag
#    b) 分步方式（QA 后改过代码时）：先提交代码修复，再执行：
node scripts/bump-version.mjs 0.2.0 --commit   # 幂等：版本已同步，只提交版本文件
node scripts/bump-version.mjs 0.2.0 --tag      # 幂等：只打 annotated tag v0.2.0

# 6. 推送（tag 随提交推送，勿漏）
git push --follow-tags
git ls-remote --tags origin | findstr v0.2.0   # 验证 tag 已推（可选）

# 7. 流水线自动构建与发布（推送后自动触发，无需手动操作）
#    - GitHub Actions：推 v* tag 自动构建 x64 + arm64 安装包 → 创建 GitHub Release（含 latest.json 更新清单）
#    - CNB 流水线：同样推 tag 触发，产出到 CNB Release + 更新 updater/latest.json
```

> **QA 后需要代码修复**：先 `git commit` 代码修复（单独提交），再执行上面第 5 步 b) 的幂等收尾。
> 不要在版本文件已同步后继续改代码再直接 `--commit`（白名单会拦截非版本文件改动，这是设计意图）。
>
> **首次使用提示**：`--commit` 要求工作树除版本文件外无任何改动（含未跟踪文件）。
> 若版本管理功能本身（scripts/、CHANGELOG.md 等）尚未提交，先提交它；有 WIP 用 `git stash -u` 暂存。

## 双平台发版（GitHub + CNB）

推送 tag 后两条流水线**并行自动构建**，无需手动上传产物：

| 平台 | 触发 | 产物 | Release | updater 清单 |
|---|---|---|---|---|
| **GitHub Actions**（`.github/workflows/build-installers.yml`） | push `v*` tag | x64 + arm64 NSIS `.exe` + `.sig` + `latest.json` | GitHub Release（自动创建并上传附件） | 流水线生成 `latest.json`（url 指向 GitHub Release 下载地址）并随 Release 上传 |
| **CNB**（`.cnb.yml`） | tag_push | x64 + arm64 NSIS `.exe` + `.sig` | CNB Release（自动创建并上传附件） | 生成 `latest.json` 并提交 main |

### 注意事项

1. **tag 必须指向包含 workflow 文件的提交**：GitHub 流水线文件是 `.github/workflows/build-installers.yml`，tag 指向的提交若不含它则不会触发构建（v0.1.2 曾踩坑：cnb 旧 tag 指向无 workflow 的提交，需 `git tag -f v0.1.2 main` 重指向后推送）。
2. **版本守卫**：GitHub 流水线校验 tag 与 `tauri.conf.json` version 一致（`vX.Y.Z` == `X.Y.Z`），不一致直接失败——必须用 `bump-version.mjs` 发版，禁止手动打 tag。
3. **签名密钥**：GitHub 需在仓库 Secrets 配置 `TAURI_SIGNING_PRIVATE_KEY`（与 CNB 密钥仓库 `lanhui100/pony_clean-secrets` 同一把，密钥带密码时还需 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`）。未配置时：GitHub 流水线自动关闭 updater 产物（仅安装包，无 `.sig`，不报错）；CNB 流水线直接报错。
4. **tag 冲突**：同一 tag 在两个平台指向不同提交时，后续 push 会冲突（v0.1.2 本地 tag 已重指向 GitHub main，与 cnb 指向不同；如需 `git push cnb` 同步需先处理）。
5. **updater 分发**：v0.2.1 起 app 内更新源为 **GitHub（主）+ CNB（备）** 双端点。GitHub 的 `latest.json` 由流水线生成并作为 Release 附件上传（固定 URL：`https://github.com/lanhui100/pony_clean/releases/latest/download/latest.json`）；CNB 继续维护 git raw 清单作备用。注意 endpoint 编译进二进制——v0.2.0 及更早的已装客户端仍只走 CNB，升级到 v0.2.1+ 后才启用 GitHub 源。
6. **构建验证**：推送后到 GitHub Actions 页面确认 `Build Windows Installers` 全绿，Release 附件含双架构 `setup.exe` + `.sig`（约 8 分钟，缓存生效后）。

## bump-version.mjs 行为与守卫

`node scripts/bump-version.mjs <新版本> [--commit] [--tag] [--dry-run]`

| 场景 | 行为 |
|---|---|
| 非法 semver / 与当前版本相同（且无 --commit/--tag） | 报错退出 |
| CHANGELOG 缺 `[Unreleased]` 或存在多个 | 报错退出 |
| `[Unreleased]` 为空（注释/非 5 种前缀不算条目） | 报错退出，禁止空 changelog 发版 |
| 版本同步 | 文本精改三处清单（保留字节/行尾）；`cargo update -p pony_core -p pony_clean -w` 刷新 Cargo.lock（限定成员，最小 diff）并**回读断言**，断言失败退出 |
| 写文件 / cargo update / 断言失败 | **自动还原全部 5 个版本文件**（事务回滚），并给出手动恢复命令 |
| 版本已是目标版本 + `--commit`/`--tag` | 幂等：跳过写文件与 cargo update，直接执行提交/打 tag |
| `--dry-run` | 只预览不落盘、不执行 git/cargo |
| `--commit` | 白名单 = 5 个版本文件；存在任何其他已修改/未跟踪文件 → 中止并列出；提交带本次发布条目作为提交体，`git commit -- <5 文件>` 限定范围 |
| `--tag` | 校验 HEAD 的 Cargo.toml 已含目标版本（防 tag 到错误提交）+ tag 同名不存在；tag 消息 `vX.Y.Z 发布` |

> **环境要求**：bump 需要 `cargo` 与 `git` 的**真实可执行文件**（rustup 的 `cargo.exe`、Git for Windows 的 `git.exe` 满足）；
> Node `spawn` 不解析 `.cmd`/`.bat` shim。`cargo update` 在 lock 与清单不一致时可能需要联网刷新 registry 索引。

## check-version.mjs

`node scripts/check-version.mjs`（或 `npm run check:version`）—— 校验上述四处版本一致：
一致 exit 0；不一致 exit 1 并逐处列出差异。CI（`.github/workflows/ci.yml` 的 `version-sync` job）
在每次 push/PR 自动执行，同时运行版本工具的契约测试（`node --test "scripts/*.test.mjs"`）。

## 其他约定

- CHANGELOG 归档日期以 **bump 当天本地日期**为准（非 UTC）。
- tag 命名 `vX.Y.Z`（如 `v0.2.0`），必须指向包含该版本代码的提交（脚本守卫）。
- 版本工具脚本均为纯 Node stdlib（零第三方依赖），Node ≥ 20。
