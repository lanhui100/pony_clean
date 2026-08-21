# CNB 构建 Windows 安装包方案

> 调研日期：2026-08-19
> 状态：已实施方案 A（官方构建机交叉编译 NSIS `.exe`）并**验证通过**（2026-08-19）
> 双架构验证通过（2026-08-19）：x64 + ARM64 双安装包已上线
> 自动更新（tauri-plugin-updater）验证通过（2026-08-19）：v0.1.2 含签名 + latest.json 全链路
> GitHub Actions 流水线上线（2026-08-20）：与 CNB 并行构建，见 §5.6/§5.7
> 更新源双端点（2026-08-21，v0.2.1 起）：GitHub（主）+ CNB（备），GitHub Release 附带流水线生成的 `latest.json`
> 最新产物：`PonyClean_0.2.1_x64-setup.exe` + `PonyClean_0.2.1_arm64-setup.exe` + `latest.json`，见 [v0.2.1 Release](https://github.com/lanhui100/pony_clean/releases/tag/v0.2.1)

## 1. 背景

PonyClean 是 Windows 桌面应用（Rust + Tauri 2 + Vue 3），需要在 CNB（cnb.cool）流水线中产出**可供用户直接下载安装**的安装包。

## 2. 结论：安装包格式是 `.exe`

Tauri 2 在 Windows 上打包产出两种安装包，均支持"直接下载安装"：

| 格式 | 安装技术 | 产物文件名（以 0.1.0 为例） | 说明 |
|------|---------|---------------------------|------|
| **`.exe`（主推，x64）** | NSIS | `PonyClean_0.1.0_x64-setup.exe` | 单文件安装器，双击即装 |
| **`.exe`（ARM64）** | NSIS | `PonyClean_0.1.0_arm64-setup.exe` | Windows on ARM 设备（NSIS 本体经模拟运行，应用为原生 ARM64） |
| `.msi`（可选） | WiX / MSI | `PonyClean_0.1.0_x64_en-US.msi` | 企业分发常用，仅 Windows 可构建 |

> 参考：[Tauri Windows Installer](https://v2.tauri.app/distribute/windows-installer/)

### 2.1 构建平台约束（关键调研结论）

- **`.msi` 只能在 Windows 上构建** —— WiX Toolset 仅支持 Windows。
- **`.exe`（NSIS）可以在 Linux/macOS 上交叉编译**，但 Tauri 官方明确标注：
  > Cross compiling Windows apps on Linux hosts is possible with caveats... should only be used as **a last resort**.
- CNB **官方构建机是 Linux 容器**，无法原生构建 Windows 安装包；CNB **自托管构建机支持 Windows（win32/x86_64）**，是官方针对"Windows 桌面应用打包"场景提供的能力。

## 3. 两种方案对比

| 维度 | 方案 A：官方 Linux 构建机交叉编译 | 方案 B：Windows 自托管构建机 |
|------|----------------------------------|------------------------------|
| 产出 `.exe` | ✅ | ✅ |
| 产出 `.msi` | ❌（WiX 仅 Windows） | ✅ |
| 流水线复杂度 | 需在脚本中装工具链（rustup target + cargo-xwin + NSIS/llvm） | 构建机预装好工具链后，流水线极简 |
| 风险 | 官方标注"last resort"，未充分测试，报错概率高、难排查 | 原生构建，与本地 `cargo tauri build` 一致，零风险 |
| 成本 | 0（官方核时） | 需一台 Windows 机器接入 runner |
| 附加收益 | 无 | 真机环境可跑验证/签名/自动更新，一次配置长期受益 |

## 4. 决策记录（ADR）

- **当前采用：方案 A**（官方构建机交叉编译 NSIS `.exe`）。
  - 理由：当前阶段目标是先产出可下载安装的 `.exe`，零基础设施成本。
- **触发切换条件（升级到方案 B）**：
  1. 正式发版（需要 `.msi` / 代码签名 / 自动更新）；
  2. 交叉编译遇到无法解决的问题。
- 切换时依据本仓库 [RELEASE_CHECKLIST.md](../RELEASE_CHECKLIST.md) 的 Windows 验证要求，并参考下文第 6 节。

## 5. 方案 A 实施细节

流水线配置见仓库根目录 `.cnb.yml`（触发事件：`tag_push`，即推送 `vX.Y.Z` tag 自动构建）。

### 5.1 构建环境

- 镜像：`node:20-bookworm`（Node 20 LTS，满足前端构建与 Tauri CLI）
- 不配置 cargo/rustup 数据卷缓存（跨构建机复用权限异常，见 §5.4）
- 工具链安装脚本（stage 1）：
  - `rustup`（stable，满足 workspace `rust-version = 1.85`）
  - `rustup target add x86_64-pc-windows-msvc aarch64-pc-windows-msvc`
  - `cargo install cargo-xwin`（Tauri 的交叉编译 runner，自动下载 Windows SDK）
  - `apt-get install nsis lld llvm clang libayatana-appindicator3-dev`（NSIS 打包 + lld 链接器 + llvm-rc 资源编译 + clang-cl + tray 检查）
- 签名密钥：`imports` 从 CNB 密钥仓库注入 `TAURI_SIGNING_PRIVATE_KEY`（见 §9.3）

### 5.2 构建命令（stage 2，双架构）

```bash
npm ci --prefix frontend      # 安装前端依赖
npm run build                 # 产出 frontend/dist
# 必须在仓库根目录执行（tauri CLI 需在子目录中找到 src-tauri/tauri.conf.json）

# x64：用 cargo-xwin runner（已验证通过）
npx --prefix frontend tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc

# ARM64：手动交叉编译（绕开 cargo-xwin runner，详见 §5.4 坑 7）
# 需先设置 CC/CXX/CFLAGS/AR 及 CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_RUSTFLAGS 等环境变量
npx --prefix frontend tauri build --target aarch64-pc-windows-msvc
```

> 完整的 ARM64 环境变量配置见 `.cnb.yml` 的「构建前端并交叉编译安装包」阶段（含 xwin SDK 库路径动态探测）。

### 5.3 产物与分发（双架构）

- 产物路径（注意：因 workspace 共享 target（见 AGENTS.md），产物在仓库根 `target/`，**不是** `src-tauri/target/`）：
  - x64：`target/x86_64-pc-windows-msvc/release/bundle/nsis/PonyClean_<version>_x64-setup.exe`
  - ARM64：`target/aarch64-pc-windows-msvc/release/bundle/nsis/PonyClean_<version>_arm64-setup.exe`
- updater 产物（因 `createUpdaterArtifacts: true`，**默认模式**）：
  - updater 直接复用 `..._x64-setup.exe` + `..._x64-setup.exe.sig`（安装器 + 签名）
  - `..._arm64-setup.exe` + `..._arm64-setup.exe.sig`（ARM64 同）
  - 均上传 Release 附件；`latest.json` 的 `url` 指向 `setup.exe`、`signature` 取 `.exe.sig`
  - 注：不生成 `.nsis.zip`（那是 `v1Compatible` 旧模式的产物，本项目未启用）
- ARM64 说明：NSIS 安装器本体为 x86（在 ARM 机器上经模拟运行），应用二进制为原生 ARM64，用户安装体验无差异
- 命名规则：Tauri bundler 固定为 `{productName}_{version}_{arch}-setup.exe`，架构短名 `x64`/`arm64`/`x86`；`x64` = `x86_64`，为 Windows 生态通行叫法，无需改成 `x86_64`
- 分发链路：
  1. `git:release` 内置任务为当前 tag 创建 Release；
  2. `cnbcool/attachments` 插件把 `setup.exe` + `.sig` 上传为 Release 附件（官方构建机有 Docker，可运行该插件）；
  3. 「生成并提交 latest.json」阶段把更新清单提交到 main（git raw 固定 URL），供 app 内自动更新检查；
  4. 用户在 CNB Release 页面直接下载安装，或通过 app 内「软件更新」自动升级。

### 5.4 实施中踩过的坑（2026-08-19 测试）

1. **tauri CLI 目录识别**：必须在仓库根目录执行 `npx --prefix frontend tauri build`，不能 `cd frontend`（否则找不到 `src-tauri/tauri.conf.json`）。
2. **数据卷缓存权限**：`/root/.cargo`、`/root/.rustup` 配数据卷缓存后，跨构建机复用时出现 `settings.toml: Permission denied`。**当前方案不加 volumes 缓存**，工具链每次重装（约 2-3 分钟）。
3. **cc-rs 需 clang**：本项目依赖编译 C/C++，交叉编译报 `failed to find tool "clang-cl"`，需 `apt install clang`。
4. **tray-icon 特性检查**：应用启用 `tray-icon`，tauri-cli 打包阶段在 Linux 宿主机检测 appindicator 库并 panic，需 `apt install libayatana-appindicator3-dev`。
5. **产物路径**：workspace 共享 target，安装包在仓库根 `target/`（`target/x86_64-pc-windows-msvc/release/bundle/nsis/`），**不在** `src-tauri/target/`；附件上传 glob 必须写 `./target/**/bundle/nsis/*-setup.exe`。
6. **删除 tag 的限制**：tag 一旦创建 Release，CNB 禁止直接删除 tag 重建；需先用 `cnb releases delete-release` 删除 Release 再重建 tag。
7. **ARM64 交叉编译（ring 依赖，重点坑）**：
   - **cargo-xwin runner 只对 x64 可靠**。对 `aarch64-pc-windows-msvc`，`--runner cargo-xwin` 会强制注入 `CC=clang-cl` + MSVC 风格 `/imsvc` 参数，而 Linux 上 clang-cl 家族检测失败回退 GNU clang，GNU clang 不识别 `/imsvc`（clang 19 连 `-imsvc` 也不支持，必须用 `-isystem`）→ ring 0.17 编译失败。
   - **解决方案**：ARM64 不用 cargo-xwin runner，改用普通 cargo + 手动环境变量：
     - `CC_aarch64_pc_windows_msvc="clang"` + `CFLAGS_*` 用 `-isystem` 指向 `xwin/crt/include`、`xwin/sdk/include/{ucrt,um,shared,winrt}`
     - `AR_aarch64_pc_windows_msvc` 指向 `llvm-lib-<ver>`（并 `ln -s` 成 `lib.exe` 供 cc-rs 查找）
     - `CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_RUSTFLAGS="-Lnative=<um/aarch64> -Lnative=<crt/lib/aarch64> -Lnative=<ucrt/aarch64> -C linker-flavor=lld-link"`
     - **架构目录是 `aarch64` 不是 `arm64`**；路径必须动态探测（`find /root/.cache/cargo-xwin -name kernel32.lib -path "*aarch64*"`），避免 x64 构建残留干扰。
   - **CNB script 陷阱**：`$VAR` 在 RUSTFLAGS 变量值内不展开（字面保留），需用命令替换/写死路径。设置 target 专属 `CARGO_TARGET_*_RUSTFLAGS` 时**不能**再设全局 `RUSTFLAGS`（cargo 优先全局，导致 `-Lnative` 丢失）。

### 5.5 已知限制与优化方向

- `apt install` 每次流水线重复执行：可改为 `docker.build` 自定义镜像预装工具链（见 [CNB 构建环境文档](https://docs.cnb.cool/zh/build/build-env.md)），并顺带解决缓存卷权限问题（把 rustup/cargo 装进镜像）。
- 仅产出 NSIS `.exe`（x64 + arm64），无 `.msi`。
- 双架构构建比单架构多编译一次，流水线耗时约增加 2-3 分钟。

### 5.6 GitHub Actions 构建（windows-latest 原生 + ARM64 交叉）

> 2026-08-20 新增。仓库同时提供 GitHub Actions 流水线（`.github/workflows/build-installers.yml`），
> 与 CNB 方案 A 互为备份，产物一致（x64 + arm64 NSIS `.exe`）。

| 维度 | CNB 方案 A | GitHub Actions |
|------|-----------|----------------|
| 构建机 | Linux 容器（node:20-bookworm） | windows-latest（Windows Server 2022 x64） |
| x64 | cargo-xwin 交叉编译 | **原生构建**（MSVC，零交叉） |
| arm64 | 手动 clang + lld-link + xwin SDK 探测 | **MSVC amd64_arm64 交叉环境**（VS 2022 自带 ARM64 工具集） |
| 工具链 | 每次 apt 安装 rustup/nsis/llvm/clang | 预装 VS 2022 + LLVM + rustup，tauri 自动下载 NSIS |
| 签名 | CNB 密钥仓库 imports | GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY`（同一把密钥） |
| 分发 | CNB Release + latest.json 提交 main | GitHub Release（含流水线生成的 `latest.json` 附件）+ workflow artifact |

**触发方式**：推送 `vX.Y.Z` tag 自动构建并创建 GitHub Release；`workflow_dispatch` 手动触发（仅构建 + artifact）。

**关键步骤**：
1. `dtolnay/rust-toolchain` 安装 `x86_64-pc-windows-msvc` + `aarch64-pc-windows-msvc` 双 target；
2. x64：`npx --prefix frontend tauri build --ci`（原生，host target）；
3. arm64：`ilammy/msvc-dev-cmd@v1`（`arch: amd64_arm64`）设置 MSVC 交叉环境后
   `npx --prefix frontend tauri build --ci --target aarch64-pc-windows-msvc --bundles nsis`；
   - ring 0.17 在 aarch64 上用 clang 汇编，需 LLVM 在 PATH（windows-latest 预装）；
   - `--bundles nsis`：NSIS 是 ARM64 唯一支持的 bundle（Tauri 官方建议显式指定）；
4. 签名：`TAURI_SIGNING_PRIVATE_KEY`（+ 可选 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`）注入 tauri build；
   **未配置时 workflow 自动临时关闭 `createUpdaterArtifacts`**，产物仅安装包（无 `.sig`）；
5. 生成 updater 清单：用双架构 `.sig` 生成 `latest.json`（`version`/`pub_date`(RFC 3339)/
   `platforms.{windows-x86_64,windows-aarch64}.{signature,url}`，url 指向 GitHub Release 下载地址）；
6. 分发：`softprops/action-gh-release` 上传 `setup.exe` + `.sig` + `latest.json` 到 GitHub Release；`upload-artifact` 始终上传。

**与 CNB 方案的差异与注意**：
- Windows 原生构建质量更好、无需 cargo-xwin 与手动 NSIS 安装（tauri 自动下载）；
- **updater/latest.json 由本流水线生成并作为 Release 附件上传**（2026-08-21 起，v0.2.1）：app 内更新源为
  GitHub（主）+ CNB（备），固定 URL `https://github.com/lanhui100/pony_clean/releases/latest/download/latest.json`
  （`latest` 自动指向最新正式 Release）。清单用 PowerShell `ConvertTo-Json` 生成——**勿用 heredoc 手写 JSON**
  （YAML 块标量与 `@"..."@` 冲突曾致工作流 0s 失败）；`pub_date` 用 `(Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")`；
- 签名私钥需配置到 GitHub Secrets（与 CNB 密钥仓库 `lanhui100/pony_clean-secrets` 同一把，见 §9.3）。

### 5.7 GitHub 发版流程（与 CNB 并行）

推送 `vX.Y.Z` tag 后，GitHub Actions 与 CNB 流水线**并行自动构建**，无需手动上传：

```bash
node scripts/bump-version.mjs 0.2.0 --commit --tag   # 同步版本 + 提交 + 打 tag
git push origin main --follow-tags                    # 推送 main 与 tag（触发两条流水线）
```

- GitHub 侧：`Build Windows Installers` 自动构建双架构 → 创建 GitHub Release（附件 = `setup.exe` + `.sig` + `latest.json`）；
- CNB 侧：自动构建 → CNB Release + `updater/latest.json` 提交 main（备用更新源）；
- 完整发版流程与注意事项（tag 指向、版本守卫、签名密钥、tag 冲突）见 [VERSIONING.md「双平台发版」](VERSIONING.md#双平台发版github--cnb)。

## 6. 代码签名（解决浏览器/SmartScreen 警告）

> 背景：Issue #7 — `.exe` 安装包在浏览器下载/运行时弹警告。结论：**zip 无法绕开**，根源是"未签名"，正确解法是代码签名。

### 6.1 签名方案分层

| 阶段 | 方案 | 效果 | 成本 |
|---|---|---|---|
| **初期（已实施）** | 自签名证书 + 安装说明 | 消除本地"文件已损坏/发布者未知"红色拦截；SmartScreen 黄色警告仍在 | 0 |
| 正式发版 | OV 代码签名证书 | 消除黄色警告，进入信任链 | 数百~上千/年 |
| 最优 | EV 代码签名证书 | 首次下载即获 SmartScreen 信任 | 数千/年 |

> 自签名证书**无法进入浏览器权威信任链**，因此浏览器下载 `.exe` 拦截与 SmartScreen 黄色警告依然存在，需配合 README 安装说明让用户放行。

### 6.2 初期方案实现（已落地 `scripts/sign/`）

| 脚本 | 作用 | 依赖 |
|---|---|---|
| `gen-self-signed-cert.sh` | 生成自签名证书（PFX + CER） | openssl |
| `sign-exe.sh` | 对 `.exe` 打上数字签名 | osslsigncode |

osslsigncode 可在 **Linux 上对交叉编译的 Windows .exe 直接签名**，无需 Windows 机器，适合方案 A（官方 Linux 构建机）内测分发。快速开始见 `scripts/sign/README.md`。

### 6.3 正式发版接入权威签名

- 方案 B（Windows 自托管构建机）+ 权威 OV/EV 证书；
- 用 `sign-exe.sh` 换成权威 `pfx`（时间戳默认 `http://timestamp.digicert.com`，可用位置参数或 `PONY_TIMESTAMP_URL` 覆盖为 HTTPS 等其它端点），或在 `tauri.conf.json` 配置 `bundle.windows.signCommand` 让打包阶段自动签名；
- 也可在 `.cnb.yml` 流水线的 release 阶段接入签名。

## 7. 未来升级：方案 B 前提清单（备忘）

接入 Windows 自托管构建机需要：

1. 管理端开启"允许添加自定义 Runner"；
2. Windows x86_64 机器上执行接入脚本，节点标签打 `windows`、`x86_64`；
3. 构建机预装：Rust（MSVC 工具链）、Node.js 20+、npm；WebView2 运行时（Win10/11 自带）；
4. NSIS/WiX 由 tauri bundler 自动下载，无需预装；
5. 若需流水线内自动上传附件（`cnbcool/attachments` 为 Docker 镜像插件），构建机需装 Docker Desktop；否则在 Release 页面手动上传 `.exe`；
6. 切换时 `.cnb.yml` 增加 `runner.namespace: group` + `runner.tags` 调度段，构建命令去掉 `--runner cargo-xwin --target`。

## 8. 参考

- [Tauri Windows Installer](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri Build Windows apps on Linux and macOS](https://v2.tauri.app/distribute/windows-installer/#build-windows-apps-on-linux-and-macos)
- [Tauri Updater 插件](https://v2.tauri.app/plugin/updater/)
- [CNB 自定义构建机（自托管 Runner）](https://docs.cnb.cool/zh/build/build-node.md)
- [CNB 附件插件 cnbcool/attachments](https://cnb.cool/cnb/plugins/market)
- [CNB 密钥仓库](https://docs.cnb.cool/zh/repo/secret.md)

## 9. 自动更新（tauri-plugin-updater）

应用内"设置 → 软件更新"可检查新版本并静默下载安装（Windows `installMode: passive`，小窗口进度条）。

### 9.1 更新链路

1. **发版**：推送 tag → 流水线构建双架构 → 自动签名（`TAURI_SIGNING_PRIVATE_KEY`）→ 上传 `setup.exe` + `.exe.sig` 到 Release 附件
2. **清单**（双端点，客户端按序尝试）：
   - **GitHub（主）**：GitHub 流水线生成 `latest.json` 并作为 Release 附件上传，固定 URL：
     `https://github.com/lanhui100/pony_clean/releases/latest/download/latest.json`（`latest` 自动指向最新正式 Release）
   - **CNB（备）**：CNB 流水线生成 `updater/latest.json` 提交到 main，固定 URL：
     `https://cnb.cool/lanhui100/pony_clean/-/git/raw/main/updater/latest.json`
   两份清单的 `url` 分别指向各自平台的 `setup.exe` 下载地址
3. **客户端**：app 启动/手动检查时按端点顺序请求 → 对比版本 → 下载对应架构 `setup.exe` → 校验签名 → 被动安装（`installMode: passive`）→ 重启

### 9.2 配置

- `tauri.conf.json`：`bundle.createUpdaterArtifacts: true` + `plugins.updater.{pubkey,endpoints,windows.installMode}`
- `capabilities/default.json`：`updater:default`、`updater:allow-check`、`updater:allow-download-and-install`
- Rust：`tauri_plugin_updater::Builder::new().build()` 注册插件
- 前端：`@tauri-apps/plugin-updater` 的 `check()` + `downloadAndInstall()`（SettingsPanel.vue）

### 9.3 签名密钥管理（重要）

- 密钥对：`npx tauri signer generate -w <path>` 生成（已生成 `/tmp/ponyclean_updater.key` + `.pub`，2026-08-19）
- **公钥**：写入 `tauri.conf.json` 的 `plugins.updater.pubkey`
- **私钥**：存放 CNB **密钥仓库** `lanhui100/pony_clean-secrets`（Web 创建：cnb.cool/new/repos → 选"密钥仓库"），文件 `updater.yml`：
  ```yaml
  TAURI_SIGNING_PRIVATE_KEY: "dW50cnVzdGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5..."
  ```
- **私钥绝不可提交到业务仓库**（本仓库已通过 `.gitignore` 意识防护 + imports 机制隔离）
- 密钥丢失 = 无法再签名更新包，需重新生成密钥对并更新 `pubkey`（会失效已装客户端的自动更新，直到用户重装）

### 9.4 发版注意

- CNB 每次发版会**自动提交 `updater/latest.json` 到 main**，因此 CNB 侧 main 会比 tag 提交多 1 个 docs 提交（属正常，由 `CNB_TOKEN` 推送）；GitHub 侧无此行为（清单是 Release 附件，不进 git）
- 若签名失败（私钥未配置），**CNB 流水线**的 `tauri build` 会因 `createUpdaterArtifacts` 报错——需先完成密钥仓库配置；**GitHub 流水线**（§5.6）会自动用 `--config` 临时关闭 `createUpdaterArtifacts`，产物仅安装包（无 `.sig`），不报错
- **endpoint 编译进二进制**：修改 `tauri.conf.json` 的 `plugins.updater.endpoints` 只影响之后的构建；已装旧版客户端仍用编译时的端点，直到升级到含新端点的版本
