#!/usr/bin/env bash
# =============================================================================
# gen-self-signed-cert.sh — 生成自签名代码签名证书（初期方案）
#
# 背景：PonyClean 分发 .exe 安装包时，未签名会触发浏览器/SmartScreen 警告。
#       正式发版需 OV/EV 权威 CA 证书（见 docs/RELEASE_BUILD.md），但内测阶段
#       可用"自签名证书 + 安装说明"缓解：签名后可消除本地"文件已损坏/发布者未知"
#       的红色拦截（黄色 SmartScreen 警告仍在，需用户在 README 指引下放行）。
#
# 产物：
#   build/sign/ponyclean-selfsigned.pfx  自签名证书（PFX，含私钥，导入/签名用）
#   build/sign/ponyclean-selfsigned.cer  公钥证书（CER，供用户信任导入）
#
# 用法：
#   bash scripts/sign/gen-self-signed-cert.sh [CN] [输出目录]
#   示例：
#     bash scripts/sign/gen-self-signed-cert.sh                # 默认 CN=PonyClean
#     bash scripts/sign/gen-self-signed-cert.sh "PonyClean Dev" build/sign
#
# 说明：
#   - 需要 openssl（Linux/macOS 自带；Windows 用 Git Bash/WSL 或 chocolatey 装）。
#   - 自签名证书仅适合内测/内部分发，无法通过浏览器信任链与 SmartScreen 信誉分。
#   - 生成后请妥善保管 pfx 密码，私钥泄露等同于可代签你的程序。
# =============================================================================
set -euo pipefail

CN="${1:-PonyClean}"
OUT_DIR="${2:-build/sign}"
PFX_PASS="${PONY_SIGN_PFX_PASS:-PonyClean@SelfSigned}"

mkdir -p "$OUT_DIR"

echo "==> 生成自签名证书（CN=$CN）"
openssl req -x509 -newkey rsa:2048 -sha256 -days 825 -nodes \
  -keyout "$OUT_DIR/ponyclean-selfsigned.key" \
  -out "$OUT_DIR/ponyclean-selfsigned.cer" \
  -subj "/CN=$CN/O=PonyClean/OU=Internal"

echo "==> 打包为 PFX（导入证书库/签名用，密码：$PFX_PASS）"
openssl pkcs12 -export \
  -out "$OUT_DIR/ponyclean-selfsigned.pfx" \
  -inkey "$OUT_DIR/ponyclean-selfsigned.key" \
  -in "$OUT_DIR/ponyclean-selfsigned.cer" \
  -passout "pass:$PFX_PASS"

echo "==> 清理中间密钥文件"
rm -f "$OUT_DIR/ponyclean-selfsigned.key"

echo ""
echo "✅ 完成，产物："
echo "   PFX: $OUT_DIR/ponyclean-selfsigned.pfx（密码：$PFX_PASS）"
echo "   CER: $OUT_DIR/ponyclean-selfsigned.cer"
echo ""
echo "下一步："
echo "   1) 用 sign-exe.sh 对安装包签名："
echo "        PONY_SIGN_PFX_PASS='$PFX_PASS' bash scripts/sign/sign-exe.sh $OUT_DIR/ponyclean-selfsigned.pfx <你的.exe>"
echo "   2) 把 CER 提供给内测用户，让其信任（见 README 下载安装指引）。"
