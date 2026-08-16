// version-utils.test.mjs — 版本管理纯函数契约测试（node --test，零依赖）
// 运行: node --test scripts/

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  parseVersion,
  readWorkspaceVersion,
  readWorkspaceMembers,
  readPackageName,
  readJsonVersion,
  rewriteTomlVersion,
  rewriteJsonVersionLine,
  extractLockVersion,
  archiveChangelog,
  extractReleaseEntries,
  todayLocalDate,
} from './version-utils.mjs';

// ---------- parseVersion ----------

test('parseVersion: 接受合法版本', () => {
  assert.equal(parseVersion('0.2.0'), '0.2.0');
  assert.equal(parseVersion('1.0.0'), '1.0.0');
  assert.equal(parseVersion('0.2.0-rc.1'), '0.2.0-rc.1');
});

test('parseVersion: 拒绝非法版本', () => {
  for (const bad of ['', '0.1', 'v0.2.0', '0.2.0.1', '0.2.0 ', 'abc', '1.2', '0.2.0-']) {
    assert.throws(() => parseVersion(bad), undefined, `应拒绝: "${bad}"`);
  }
});

// ---------- TOML 读取 ----------

const SAMPLE_CARGO = `[workspace]
resolver = "2"
members = [
    "crates/pony_core",
    "src-tauri",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
`;

const MEMBER_CARGO = `[package]
name = "pony_core"
version.workspace = true
edition.workspace = true
`;

test('readWorkspaceVersion: 读取 workspace.package 版本', () => {
  assert.equal(readWorkspaceVersion(SAMPLE_CARGO), '0.1.0');
});

test('readWorkspaceVersion: 缺段/缺字段报错', () => {
  assert.throws(() => readWorkspaceVersion('[workspace]\nmembers = []\n'));
  assert.throws(() => readWorkspaceVersion('[workspace.package]\nedition = "2024"\n'));
});

test('readWorkspaceMembers: 读取成员路径', () => {
  assert.deepEqual(readWorkspaceMembers(SAMPLE_CARGO), ['crates/pony_core', 'src-tauri']);
});

test('readPackageName: 读取成员包名', () => {
  assert.equal(readPackageName(MEMBER_CARGO), 'pony_core');
});

// ---------- 文本精改 ----------

test('rewriteTomlVersion: 只改 workspace.package 段，保留其余字节', () => {
  const input = `[workspace.package]\nversion = "0.1.0"\n\n[package]\nname = "x"\nversion = "9.9.9"\n`;
  const out = rewriteTomlVersion(input, '0.2.0');
  assert.match(out, /\[workspace\.package\]\nversion = "0\.2\.0"/);
  assert.match(out, /\[package\]\nname = "x"\nversion = "9\.9\.9"/); // 其他段不受影响
});

test('rewriteJsonVersionLine: 精改 version 行并保留行尾（CRLF）', () => {
  const input = '{\r\n  "name": "p",\r\n  "version": "0.1.0",\r\n  "scripts": {}\r\n}\r\n';
  const out = rewriteJsonVersionLine(input, '0.2.0');
  assert.equal(out, '{\r\n  "name": "p",\r\n  "version": "0.2.0",\r\n  "scripts": {}\r\n}\r\n');
  assert.ok(out.includes('\r\n')); // CRLF 保留
});

test('rewriteJsonVersionLine: 缺 version 行报错', () => {
  assert.throws(() => rewriteJsonVersionLine('{"name":"x"}', '0.2.0'));
});

test('rewriteJsonVersionLine: 嵌套 version 键先出现时不误改（顶层锚定）', () => {
  const input = '{\n  "scripts": {\n    "bump:version": "node scripts/bump-version.mjs",\n    "nested": {"version": "9.9.9"}\n  },\n  "version": "0.1.0"\n}\n';
  const out = rewriteJsonVersionLine(input, '0.2.0');
  assert.match(out, /"version": "0\.2\.0"/); // 顶层被改
  assert.match(out, /"version": "9\.9\.9"/); // 嵌套未动
});

test('rewriteJsonVersionLine: 无顶层 version（仅嵌套）时报错', () => {
  const input = '{\n  "scripts": {\n    "version": "9.9.9"\n  }\n}\n';
  assert.throws(() => rewriteJsonVersionLine(input, '0.2.0'), undefined, '嵌套 version 不应被改写');
});

// ---------- Cargo.lock ----------

const SAMPLE_LOCK = `version = 4

[[package]]
name = "pony_core"
version = "0.1.0"
dependencies = [
 "tokio",
]

[[package]]
name = "pony_clean"
version = "0.1.0"
`;

test('extractLockVersion: 提取成员条目版本', () => {
  assert.equal(extractLockVersion(SAMPLE_LOCK, 'pony_core'), '0.1.0');
  assert.equal(extractLockVersion(SAMPLE_LOCK, 'pony_clean'), '0.1.0');
});

test('extractLockVersion: 条目缺失返回 null', () => {
  assert.equal(extractLockVersion(SAMPLE_LOCK, 'no_such_pkg'), null);
});

test('extractLockVersion: 依赖列表中出现的包名不误匹配', () => {
  const lock = `[[package]]
name = "some_dep"
version = "1.0.0"
dependencies = [
 "pony_core",
]

[[package]]
name = "pony_core"
version = "0.1.0"
`;
  assert.equal(extractLockVersion(lock, 'pony_core'), '0.1.0');
});

// ---------- CHANGELOG 归档 ----------

const SAMPLE_CHANGELOG = `# Changelog

## [Unreleased]

- Added: 版本管理体系建设

## [0.1.0] - 2026-08-15

- Added: 初始版本
`;

test('archiveChangelog: 归档 [Unreleased] 并插入新节', () => {
  const r = archiveChangelog(SAMPLE_CHANGELOG, '0.2.0', '2026-08-15');
  assert.equal(r.ok, true);
  assert.match(r.text, /## \[Unreleased\]/); // 新空节在最前
  assert.match(r.text, /## \[0\.2\.0\] - 2026-08-15\n\n- Added: 版本管理体系建设/); // 原节归档
  assert.match(r.text, /## \[0\.1\.0\] - 2026-08-15\n\n- Added: 初始版本/); // 更早节不受影响
  assert.ok(r.text.indexOf('## [Unreleased]') < r.text.indexOf('## [0.2.0]'));
});

test('archiveChangelog: 缺 [Unreleased] 拒绝', () => {
  const r = archiveChangelog('# Changelog\n\n## [0.1.0] - 2026-08-15\n\n- Added: x\n', '0.2.0', '2026-08-15');
  assert.equal(r.ok, false);
  assert.match(r.error, /缺少/);
});

test('archiveChangelog: 多个 [Unreleased] 拒绝', () => {
  const r = archiveChangelog('# C\n\n## [Unreleased]\n- Added: a\n## [Unreleased]\n- Fixed: b\n', '0.2.0', '2026-08-15');
  assert.equal(r.ok, false);
  assert.match(r.error, /2 个/);
});

test('archiveChangelog: 空 [Unreleased] 拒绝（注释不算条目）', () => {
  const r = archiveChangelog('# C\n\n## [Unreleased]\n\n<!-- 模板注释 -->\n', '0.2.0', '2026-08-15');
  assert.equal(r.ok, false);
  assert.match(r.error, /为空/);
});

test('archiveChangelog: CRLF 输入保持 CRLF', () => {
  const input = '# C\r\n\r\n## [Unreleased]\r\n\r\n- Added: x\r\n';
  const r = archiveChangelog(input, '0.2.0', '2026-08-15');
  assert.equal(r.ok, true);
  assert.ok(r.text.includes('## [Unreleased]\r\n'));
  assert.ok(r.text.includes('## [0.2.0] - 2026-08-15\r\n'));
});

test('archiveChangelog: 预发布版本（-pre）+ CRLF', () => {
  const input = '# C\r\n\r\n## [Unreleased]\r\n\r\n- Added: x\r\n';
  const r = archiveChangelog(input, '0.3.0-rc.1', '2026-08-15');
  assert.equal(r.ok, true);
  assert.ok(r.text.includes('## [0.3.0-rc.1] - 2026-08-15\r\n'));
});

test('archiveChangelog: [Unreleased] 内的 `##` 说明标题不截断节', () => {
  const input = '# C\n\n## [Unreleased]\n\n## 说明\n\n- Added: x\n\n## [0.1.0] - 2026-08-15\n\n- Added: init\n';
  const r = archiveChangelog(input, '0.2.0', '2026-08-15');
  assert.equal(r.ok, true);
  assert.ok(r.text.includes('## [0.2.0] - 2026-08-15\n\n## 说明\n\n- Added: x'));
});

test('archiveChangelog: 仅含 `###` 子节无条目视为空', () => {
  const r = archiveChangelog('# C\n\n## [Unreleased]\n\n### Added\n\n## [0.1.0] - 2026-08-15\n\n- Added: init\n', '0.2.0', '2026-08-15');
  assert.equal(r.ok, false);
  assert.match(r.error, /为空/);
});

test('archiveChangelog: `###` 子节 + 条目视为非空', () => {
  const r = archiveChangelog('# C\n\n## [Unreleased]\n\n### Added\n- 新功能\n\n## [0.1.0] - 2026-08-15\n\n- Added: init\n', '0.2.0', '2026-08-15');
  assert.equal(r.ok, true);
});

test('archiveChangelog: version 行在文件末无尾换行时正常归档', () => {
  const input = '# C\n\n## [Unreleased]\n\n- Added: x';
  const r = archiveChangelog(input, '0.2.0', '2026-08-15');
  assert.equal(r.ok, true);
  assert.ok(r.text.includes('## [0.2.0] - 2026-08-15\n\n- Added: x'));
});

// ---------- 发布条目提取 ----------

test('extractReleaseEntries: 提取版本小节条目', () => {
  const text = '# C\n\n## [Unreleased]\n\n- Added: 新\n\n## [0.2.0] - 2026-08-15\n\n- Added: a\n- Fixed: b\n\n## [0.1.0] - 2026-08-15\n\n- Added: init\n';
  const entries = extractReleaseEntries(text, '0.2.0');
  assert.ok(entries.includes('- Added: a'));
  assert.ok(entries.includes('- Fixed: b'));
  assert.ok(!entries.includes('- Added: init'));
});

test('extractReleaseEntries: 小节不存在返回 null', () => {
  assert.equal(extractReleaseEntries('# C\n\n## [0.1.0] - 2026-08-15\n', '9.9.9'), null);
});

// ---------- 杂项 ----------

test('readJsonVersion: 解析并校验 JSON', () => {
  assert.equal(readJsonVersion('{"version": "0.1.0"}', 'test'), '0.1.0');
  assert.throws(() => readJsonVersion('{broken', 'test'), undefined, '非法 JSON 应报错');
  assert.throws(() => readJsonVersion('{"name":"x"}', 'test'), undefined, '缺 version 应报错');
});

test('todayLocalDate: YYYY-MM-DD 格式', () => {
  assert.match(todayLocalDate(), /^\d{4}-\d{2}-\d{2}$/);
});
