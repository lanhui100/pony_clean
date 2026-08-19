#!/usr/bin/env bash
# =============================================================================
# sign-exe.sh — 用自签名/权威证书对 Windows 安装包签名（初期方案）
#
# 用法：
#   bash scripts/sign/sign-exe.sh <证书.pfx> <exe文件> [时间戳服务器URL]
#
# 示例：
#   # 自签名证书（密码在 gen-self-signed-cert.sh 输出，或用 PONY_SIGN_PFX_PASS）
#   PONY_SIGN_PFX_PASS='PonyClean@SelfSigned' \
#     bash scripts/sign/sign-exe.sh build/sign/ponyclean-selfsigned.pfx \
#       target/x86_64-pc-windows-msvc/release/bundle/nsis/PonyClean_0.1.1_x64-setup.exe
#
#   # 权威 OV/EV 证书：换成自己的 pfx + 时间戳服务器
#   bash scripts/sign/sign-exe.sh /secure/codesign.pfx ./PonyClean-setup.exe \
#     http://timestamp.digicert.com
#
# 说明：
#   - 需要 osslsigncode。安装：
#       Ubuntu/Debian:  sudo apt install osslsigncode
#       macOS:          brew install osslsigncode
#   - osslsigncode 可在 Linux 上对交叉编译的 Windows .exe 直接签名，无需 Windows。
#   - 自签名只能消除本地"文件已损坏/发布者未知"红色拦截；SmartScreen 黄色
#     警告仍需用户在 README 指引下放行（内测阶段接受）。
#   - 权威 OV/EV 证书签名后才能彻底通过浏览器/SmartScreen 信任链（正式发版）。
# =============================================================================
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "用法: bash scripts/sign/sign-exe.sh <证书.pfx> <exe文件> [时间戳URL]" >&2
  exit 1
fi

PFX="$1"
EXE="$2"
TS_URL="${3:-http://timestamp.digicert.com}"
PFX_PASS="${PONY_SIGN_PFX_PASS:-}"

if ! command -v osslsigncode >/dev/null 2>&1; then
  echo "错误：未找到 osslsigncode。请先安装（Ubuntu: sudo apt install osslsigncode）" >&2
  exit 1
fi
if [ ! -f "$PFX" ]; then echo "错误：证书不存在：$PFX" >&2; exit 1; fi
if [ ! -f "$EXE" ]; then echo "错误：exe 不存在：$EXE" >&2; exit 1; fi

OUT="${EXE%.exe}-signed.exe"

echo "==> 签名中：$EXE"
if [ -n "$PFX_PASS" ]; then
  osslsigncode sign -pkcs12 "$PFX" -pass "$PFX_PASS" \
    -n "PonyClean" -t "$TS_URL" \
    -in "$EXE" -out "$OUT"
else
  # 无密码时交互式提示输入
  osslsigncode sign -pkcs12 "$PFX" \
    -n "PonyClean" -t "$TS_URL" \
    -in "$EXE" -out "$OUT"
fi

echo "==> 校验签名"
osslsigncode verify "$OUT"

echo ""
echo "✅ 签名完成：$OUT"
echo "   该文件签名后，本地红色拦截已消除；SmartScreen 若仍提示，请在 README 指引下点"
echo "   '更多信息 → 仍要运行'（内测阶段可接受），或用权威证书正式签名。"
