# Session Log 005 — 应用内自动更新（tauri-plugin-updater）落地与发版验证

- 日期: 2026-08-19
- 项目: PonyClean
- 目标: 集成 tauri-plugin-updater，打通「发版 → 签名 → 更新清单 → 应用内检查更新」全链路；排障 ARM64 交叉编译 ring 依赖问题

## 本次完成

### 1. 代码集成（提交 `e1aed96`，后经 rebase 拆分为 3 个原子提交）
- **Rust 侧**：`src-tauri/Cargo.toml` 加 `tauri-plugin-updater 2.10.1`；`main.rs` 注册 `.plugin(tauri_plugin_updater::Builder::new().build())`；`capabilities/default.json` 授权 `updater:default/allow-check/allow-download-and-install`
- **前端**：`SettingsPanel.vue` 新增「软件更新」区域（`check()` + `downloadAndInstall()`，`installMode: passive` 被动安装，完成后自动重启）；`vue-tsc` + 构建均通过
- **配置**：`tauri.conf.json` 加 `bundle.createUpdaterArtifacts: true` + `plugins.updater.{pubkey,endpoints,windows.installMode}`；与并行改动（NSIS 中文化）合并，无冲突

### 2. 密钥管理
- 生成 ed25519 签名密钥对（`tauri signer generate`）→ `/tmp/ponyclean_updater.key` + `.pub`
- 公钥写入 `tauri.conf.json`；私钥存放 CNB 密钥仓库 `lanhui100/pony_clean-secrets/updater.yml`（Web 创建，含 `allow_slugs` + `allow_events` 最小权限）
- **坑**：密钥仓库须 Web 界面创建（CLI `create-repo` 404）；`allow_branches` 会拒绝 tag_push 事件（branch=tag 名非 main），需移除

### 3. 流水线改造 `.cnb.yml`
- `imports` 注入签名私钥 → 构建阶段 `TAURI_SIGNING_PRIVATE_KEY`
- 上传 `setup.exe` + `.exe.sig`（4 个附件）
- 新增「生成并提交 latest.json」阶段：生成清单 → `CNB_TOKEN` 推送 main（git raw 固定 URL 作更新端点）
- **关键澄清**：`createUpdaterArtifacts: true` 默认模式下 updater 直接复用 `setup.exe` + `.exe.sig`，**不产生 `.nsis.zip`**（那是 v1Compatible 旧模式产物）

### 4. ARM64 交叉编译排障（v0.1.2 发版验证，7 次触发最终通过）
| 构建 | 失败原因 | 修复 |
|------|---------|------|
| cnb-f38-1k0cg2sol | `allow_branches` 拒绝 tag_push | 移除该规则（用户改密钥仓库） |
| cnb-di8-1k0cgcugm | cargo-xwin 注入 clang-cl + `/imsvc`，GNU clang 不识别 → ring 编译失败 | ARM64 放弃 runner，改手动 env |
| cnb-ug3-1k0chf0j8 | CC env 被 runner 覆盖未生效 | 手动 env 方案（写死路径） |
| cnb-2ag-1k0cid95a | 同上（env 未穿过 runner） | 不用 runner |
| cnb-fjo-1k0cjo8u1 | `failed to find lib.exe` | 动态探测 `llvm-lib-*` + `ln -s` |
| cnb-0to-1k0cke2cs | 链接缺系统库（路径假设错误） | 精确匹配 `aarch64` 目录 + 动态探测 |
| cnb-klo-1k0cliago | `$VAR` 在 RUSTFLAGS 内未展开 + 全局 RUSTFLAGS 干扰 | 写死路径 + target 专属 RUSTFLAGS |
| cnb-heo-1k0co95pm | 同上（继续） | — |
| cnb-rbg-1k0cppej3 | CRT 库路径探测匹配错误（arm64 vs aarch64） | find 精确匹配 `aarch64` |
| **cnb-qh3-1k0cr6a22** | ✅ **全部 6 阶段成功** | — |

### 5. 发版验证结果
- v0.1.2 Release 附件：`x64/arm64` 的 `setup.exe`（2.79/2.51 MB）+ `.sig`
- `latest.json` 自动提交到 main（`32d4b26`），git raw URL 实测返回正确 JSON（双架构签名 + 下载地址）
- 更新端点：`https://cnb.cool/lanhui100/pony_clean/-/git/raw/main/updater/latest.json`

## 改动文件
- `.cnb.yml`（imports 密钥注入 + 签名 + 上传 sig + latest.json 阶段 + ARM64 手动编译）
- `src-tauri/tauri.conf.json`（createUpdaterArtifacts + plugins.updater）
- `src-tauri/Cargo.toml` / `Cargo.lock`（tauri-plugin-updater）
- `src-tauri/capabilities/default.json`（updater 权限）
- `src-tauri/src/main.rs`（注册插件）
- `frontend/package.json` / `package-lock.json` / `SettingsPanel.vue`（检查更新 UI）
- `docs/RELEASE_BUILD.md`（§9 自动更新 + §5.4 坑 7 ARM64）
- `CHANGELOG.md`（v0.1.2 记录）

## 当前结果
- 自动更新全链路验证通过；v0.1.2 双架构安装包 + 签名 + 更新清单已上线
- git 状态：main `6091b09`（含 5 个 ARM64 排障 fix + 2 个 docs 同步提交）

## 下一步动作
- **待用户**：Windows 真机验证「0.1.1 → 0.1.2」自动更新实际安装（确认 passive 体验）
- 工作区有并行会话的 icon 改动（`src-tauri/icons/*`、`main.rs`）未提交，勿动
- 优化方向：`docker.build` 预装工具链镜像（消除每次 apt install + 规避 ARM64 手动 env 复杂度）

## Resume Hint
打开 docs/RELEASE_BUILD.md → §9 自动更新链路；ARM64 排障要点见 §5.4 坑 7（手动 env + 动态探测 aarch64 路径）。
