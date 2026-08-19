# Session Log 004 — CNB Windows 安装包构建流水线（方案 A 交叉编译）

- 日期: 2026-08-19
- 项目: PonyClean
- 目标: 在 CNB（cnb.cool）流水线产出可供 Windows 用户直接下载安装的 `.exe`，验证全链路

## 本次完成

### 1. 调研与决策
- 结论：Tauri 2 打包格式为 NSIS `.exe`（`PonyClean_<ver>_x64-setup.exe`，主推）与 WiX `.msi`
- 约束：`.msi` 仅 Windows 可构建；NSIS `.exe` 可 Linux 交叉编译但 Tauri 官方标注 "last resort"
- 决策：**方案 A**（官方 Linux 构建机交叉编译），触发条件（正式发版 / 编译报错无法解决）再切方案 B（Windows 自托管构建机）
- 产出文档：`docs/RELEASE_BUILD.md`（含方案对比、ADR、切换条件、方案 B 前提清单）

### 2. 流水线配置 `.cnb.yml`
- 触发：`$ → tag_push`（推送 vX.Y.Z tag，配合 `node scripts/bump-version.mjs <ver> --commit --tag`）
- 环境：`node:20-bookworm`；4 个 stage：装工具链 → 交叉编译 → 建 Release → 传附件
- 工具链：rustup + `x86_64-pc-windows-msvc` target + `cargo-xwin` + `nsis`/`lld`/`llvm`/`clang`/`libayatana-appindicator3-dev`
- 构建：`npx --prefix frontend tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc`
- 分发：`git:release` 建 Release + `cnbcool/attachments` 上传 `.exe`
- `src-tauri/tauri.conf.json` 补 `bundle` 段（此前缺失，无法打包）：`active/targets:["nsis"]/icon`

### 3. 测试历程（共 6 次触发，最终通过）
| 构建 | 结果 | 失败原因 |
|------|------|---------|
| cnb-rbo-1k0bq5bt3 | ❌ | `cd frontend` 后 tauri CLI 找不到 `tauri.conf.json` |
| cnb-f4g-1k0bqhv73 | ❌ | cargo/rustup 数据卷缓存跨构建机复用权限异常（`settings.toml: Permission denied`） |
| cnb-mio-1k0bqo9j3 | ❌ | cc-rs 缺 `clang-cl`（依赖含 C 编译） |
| cnb-oto-1k0bre20i | ❌ | tray-icon 特性导致 tauri-cli panic（`Can't detect any appindicator library`） |
| cnb-fg8-1k0bs32ed | ⚠️ | 6 阶段全绿但附件上传静默失败（glob 写错 `src-tauri/target/`，实际产物在仓库根 `target/`） |
| **cnb-pho-1k0bt3dvt** | ✅ | **全链路成功，附件已上传** |

### 4. 最终产物
- 文件名：`PonyClean_0.1.0_x64-setup.exe`（1.9 MB，NSIS）
- SHA256：`6c66ce90e1ee1886d94a5a86dcb969aa596b0e1635a2afd2f51f8d1a543336f9`
- 下载：https://cnb.cool/lanhui100/pony_clean/-/releases/tag/v0.1.0
- 构建耗时：约 6 分钟（含工具链重装约 2-3 分钟）

### 5. 踩坑备忘（已入 docs/RELEASE_BUILD.md §5.4）
1. tauri CLI 必须在仓库根目录执行（不能 `cd frontend`）
2. cargo/rustup 数据卷缓存跨构建机权限异常 → 当前不加 volumes
3. cc-rs 需 `clang`
4. tray-icon 需 `libayatana-appindicator3-dev`
5. workspace 共享 target → 产物在 `target/` 不在 `src-tauri/target/`，附件 glob 为 `./target/**/bundle/nsis/*-setup.exe`
6. tag 关联 Release 后禁止删除重建，需先 `cnb releases delete-release`

## 改动文件
- `.cnb.yml`（新增，流水线）
- `src-tauri/tauri.conf.json`（补 bundle 段）
- `docs/RELEASE_BUILD.md`（新增调研/决策/踩坑文档）、`docs/README.md`（索引）
- `CHANGELOG.md`（Unreleased 记录）

## 当前结果
- 方案 A 验证通过；`v0.1.0` Release 已发布首个 `.exe`
- git 状态：main `46fc8d8`；tag `v0.1.0` → `ed25d39`

## 下一步动作
- 待用户真机验证安装包（安装、启动、托盘、扫描清理全链路）
- 后续发版：`node scripts/bump-version.mjs <新版本> --commit --tag && git push --follow-tags` 自动产出安装包
- 优化方向（非阻塞）：`docker.build` 预装工具链镜像，消除每次 apt install 与缓存卷权限问题

## Resume Hint
打开 docs/RELEASE_BUILD.md → 方案 A 已验证；若交叉编译再出问题或正式发版（需 .msi/签名/自动更新）→ 按文档 §6 切方案 B（Windows 自托管构建机）。
