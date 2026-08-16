#!/usr/bin/env node
// check-version.mjs — 四处版本一致性校验（零依赖，Node >= 20）
// 校验: workspace Cargo.toml / frontend/package.json / src-tauri/tauri.conf.json
//       + Cargo.lock 中每个 workspace 成员条目版本
// 用法: node scripts/check-version.mjs   （一致 exit 0，不一致 exit 1 并输出差异）

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  readWorkspaceVersion,
  readWorkspaceMembers,
  readPackageName,
  readJsonVersion,
  extractLockVersion,
} from './version-utils.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function main() {
  const cargoToml = readFileSync(path.join(ROOT, 'Cargo.toml'), 'utf8');
  const baseline = readWorkspaceVersion(cargoToml);

  const entries = [
    ['Cargo.toml [workspace.package]', baseline],
    ['frontend/package.json', readJsonVersion(readFileSync(path.join(ROOT, 'frontend', 'package.json'), 'utf8'), 'frontend/package.json')],
    ['src-tauri/tauri.conf.json', readJsonVersion(readFileSync(path.join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'), 'src-tauri/tauri.conf.json')],
  ];

  const lockText = readFileSync(path.join(ROOT, 'Cargo.lock'), 'utf8');
  for (const member of readWorkspaceMembers(cargoToml)) {
    const name = readPackageName(readFileSync(path.join(ROOT, member, 'Cargo.toml'), 'utf8'));
    const v = extractLockVersion(lockText, name);
    entries.push([`Cargo.lock（${name}）`, v === null ? '<条目缺失>' : v]);
  }

  const bad = entries.filter(([, v]) => v !== baseline);
  if (bad.length > 0) {
    console.error(`版本不一致：基准 ${baseline}，以下位置与基准不符：`);
    for (const [label, v] of entries) {
      console.error(`  ${v === baseline ? '  ' : '✗'} ${label}: ${v}`);
    }
    process.exit(1);
  }
  console.log(`✅ 版本一致: ${baseline}（三处清单 + ${entries.length - 3} 个 Cargo.lock 成员条目）`);
}

try {
  main();
} catch (e) {
  console.error(`[check-version] ${e.message}`);
  process.exit(1);
}
