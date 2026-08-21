#!/bin/bash
# 实机取证启动脚本：独立 WebView2 UDF 强制新建实例 + CDP 调试端口 + 后端诊断日志
# 用法: bash scripts/diag-launch-app.sh
set -e
cd "$(dirname "$0")/.."

EXE=target/debug/pony_clean.exe
UDF="${TEMP:-/tmp}/pony_diag_udf"
LOG=target/scan_diag.log

# 杀掉已有实例，避免 WebView2 单例复用旧进程（调试端口参数被忽略）
taskkill //IM pony_clean.exe //F 2>/dev/null || true
sleep 1

# debug 构建走 devUrl（vite 5183）：未运行则后台拉起
if ! curl -s -o /dev/null http://127.0.0.1:5183/; then
  echo "[diag] vite 未运行，后台启动（frontend/）..."
  (cd frontend && npm run dev >/dev/null 2>&1 &)
  for i in $(seq 1 30); do
    curl -s -o /dev/null http://127.0.0.1:5183/ && { echo "[diag] vite 已就绪"; break; }
    sleep 1
  done
fi

mkdir -p "$UDF"

echo "[diag] 启动 $EXE"
echo "[diag] WEBVIEW2_USER_DATA_FOLDER=$UDF"
echo "[diag] 后端日志 → $LOG （RUST_LOG=pony_core=debug 可见 per-target 扫描统计）"

WEBVIEW2_USER_DATA_FOLDER="$(cygpath -w "$UDF")" \
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223" \
RUST_LOG="pony_core=debug" \
"$EXE" >"$LOG" 2>&1 &

APP_PID=$!
echo "[diag] app pid=$APP_PID，等待调试端口..."
for i in $(seq 1 30); do
  if curl -s http://127.0.0.1:9223/json/version >/dev/null 2>&1; then
    echo "[diag] 9223 端口已生效"
    curl -s http://127.0.0.1:9223/json | head -40
    exit 0
  fi
  sleep 1
done
echo "[diag] ❌ 30s 内 9223 未生效，日志尾部："
tail -20 "$LOG"
exit 1
