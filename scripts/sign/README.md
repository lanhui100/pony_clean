# 代码签名（初期方案）

> 解决 ISSUE #7：`.exe` 安装包在浏览器下载/运行时的警告。
> 结论：**zip 无法绕开警告，根源是"未签名"**，正确解法是代码签名。

## 方案分层

| 阶段 | 方案 | 效果 | 成本 |
|---|---|---|---|
| **初期（本方案）** | **自签名证书 + 安装说明** | 消除本地"文件已损坏/发布者未知"红色拦截；SmartScreen 黄色警告仍在（内测可接受） | 0 |
| 正式发版 | OV 代码签名证书 | 消除黄色警告，进入浏览器/SmartScreen 信任链 | ¥ 数百~上千/年 |
| 最优 | EV 代码签名证书 + 云端签名 | 首次下载即获 SmartScreen 信任，浏览器不再拦 | ¥ 数千/年 |

> 自签名证书**无法进入浏览器权威信任链**，因此浏览器下载 `.exe` 的拦截和
> SmartScreen 黄色警告依然存在——这正是内测阶段需要配合**安装说明**（README）的原因。

## 脚本

| 脚本 | 作用 | 依赖 |
|---|---|---|
| `gen-self-signed-cert.sh` | 生成自签名证书（PFX + CER） | openssl |
| `sign-exe.sh` | 对 `.exe` 打上数字签名 | osslsigncode |

### 快速开始（内测分发）

```bash
# 1. 生成自签名证书（CN 建议用你的组织/项目名）
bash scripts/sign/gen-self-signed-cert.sh

# 2. 对交叉编译出的安装包签名
PONY_SIGN_PFX_PASS='PonyClean@SelfSigned' \
  bash scripts/sign/sign-exe.sh \
    build/sign/ponyclean-selfsigned.pfx \
    target/x86_64-pc-windows-msvc/release/bundle/nsis/PonyClean_*_x64-setup.exe
# 产物为同目录下 *-signed.exe，用这个文件分发
```

### 内测用户侧需要做的

1. 下载 `PonyClean_*_x64-setup-signed.exe`；
2. 若浏览器提示"保留"，点**保留**；
3. 若 SmartScreen 弹"Windows 已保护你的电脑"→ 点**更多信息 → 仍要运行**；
4. 也可双击 `ponyclean-selfsigned.cer` → 安装证书 → 选择"受信任的根证书颁发机构"，
   导入后红色拦截即消失。

## 正式发版切换

正式发版按 `docs/RELEASE_BUILD.md` 切换到**方案 B（Windows 自托管构建机）**并接入
权威 OV/EV 证书。届时：

- `sign-exe.sh` 仍可用，换成你的权威 `pfx` 即可（时间戳服务器建议
  `http://timestamp.digicert.com`）；
- 或在 `tauri.conf.json` 配置 `bundle.windows.signCommand` 让打包阶段自动签名；
- 也可在 `.cnb.yml` 流水线的 release 阶段接入签名（见 `docs/RELEASE_BUILD.md` 第 6 节）。
