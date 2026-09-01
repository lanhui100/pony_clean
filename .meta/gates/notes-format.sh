#!/usr/bin/env bash
# 门禁：决策记录格式（承诺"ADR 形状合法" → 非零退出，P2）
# 代理执行框架交付的 .agents/skills/write-adr/verify-note.sh（唯一真源，避免复制漂移）
# 对应判据：verify_ladder 1.1-1.8（路径两轴/文件名/日期/Status/骨架标题）
set -u
cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)" || exit 1
exec bash .agents/skills/write-adr/verify-note.sh
