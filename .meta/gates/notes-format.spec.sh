#!/usr/bin/env bash
# 负样本 spec：构造非法决策样本，断言 notes-format 门以非零退出拒绝。
# 用法：bash .meta/gates/notes-format.spec.sh  （全部断言通过 → exit 0）
set -u
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"; cd "$ROOT" || exit 1
GATE=".meta/gates/notes-format.sh"
FAIL=0
bad() { echo "FAIL  $*" >&2; FAIL=1; }
ok()  { echo "ok    $*"; }

# 正样本：合法决策 → 门应 exit 0
mkdir -p .agents/notes/implemented/feature
printf "# Agent Note: good\n\nStatus: implemented\n\n## Problem\nx\n\n## Decision\ny\n\n## Alternatives considered\nz\n" > .agents/notes/implemented/feature/2026-09-01-good.md
bash "$GATE" >/dev/null 2>&1 && ok "正样本 exit 0" || bad "正样本被拒（误报）"
rm -f .agents/notes/implemented/feature/2026-09-01-good.md

# 负样本 1：文件名缺日期前缀（1.6）→ 应 exit 1
printf "# Agent Note: bad\n\nStatus: implemented\n\n## Problem\nx\n\n## Decision\ny\n\n## Alternatives considered\nz\n" > .agents/notes/implemented/feature/badname.md
bash "$GATE" >/dev/null 2>&1 && bad "负样本1（坏文件名）未被拒" || ok "负样本1 exit 1"
rm -f .agents/notes/implemented/feature/badname.md

# 负样本 2：implemented 含提案时代标题（1.8）→ 应 exit 1
printf "# Agent Note: bad2\n\nStatus: implemented\n\n## Problem\nx\n\n## Proposal\ny\n\n## Alternatives considered\nz\n" > .agents/notes/implemented/feature/2026-09-01-bad2.md
bash "$GATE" >/dev/null 2>&1 && bad "负样本2（implemented 含 Proposal）未被拒" || ok "负样本2 exit 1"
rm -f .agents/notes/implemented/feature/2026-09-01-bad2.md

# 负样本 3：Status 与目录不符（1.7）→ 应 exit 1
mkdir -p .agents/notes/proposed/feature
printf "# Agent Note: bad3\n\nStatus: implemented\n\n## Problem\nx\n\n## Proposal\ny\n\n## Alternatives considered\nz\n" > .agents/notes/proposed/feature/2026-09-01-bad3.md
bash "$GATE" >/dev/null 2>&1 && bad "负样本3（Status 不符）未被拒" || ok "负样本3 exit 1"
rm -f .agents/notes/proposed/feature/2026-09-01-bad3.md

[ "$FAIL" = "0" ] && echo "spec: 全部通过（正 1 / 负 3）" && exit 0
echo "spec: 有断言失败" >&2; exit 1
