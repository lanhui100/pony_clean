// version-utils.mjs — 版本管理纯函数（零依赖，Node >= 20）
// 供 bump-version.mjs / check-version.mjs 共用，全部为无副作用纯函数，可被 node --test 直接测试。

export const SEMVER_RE = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

/** 校验并返回规范化版本号（原样返回），非法则抛错 */
export function parseVersion(raw) {
  if (typeof raw !== 'string' || !SEMVER_RE.test(raw)) {
    throw new Error(`非法版本号 "${raw}"：必须为 X.Y.Z 或 X.Y.Z-pre（语义化版本）`);
  }
  return raw;
}

/** 取 TOML 中指定段的文本（不含段头，截止下一个段头或文件尾）；段不存在返回 null */
function extractTomlSection(text, name) {
  const re = new RegExp(`^\\[${name}\\]\\s*$`, 'm');
  const m = re.exec(text);
  if (!m) return null;
  const start = m.index + m[0].length;
  const next = /^\[/m.exec(text.slice(start));
  const end = next ? start + next.index : text.length;
  return text.slice(start, end);
}

/** 读取 [workspace.package] 的 version */
export function readWorkspaceVersion(cargoToml) {
  const section = extractTomlSection(cargoToml, 'workspace.package');
  if (section === null) throw new Error('Cargo.toml 缺少 [workspace.package] 段');
  const m = /^version\s*=\s*"([^"]+)"$/m.exec(section);
  if (!m) throw new Error('Cargo.toml [workspace.package] 段缺少 version = "..." 字段');
  return m[1];
}

/** 读取 [workspace] members 路径列表 */
export function readWorkspaceMembers(cargoToml) {
  const section = extractTomlSection(cargoToml, 'workspace');
  if (section === null) throw new Error('Cargo.toml 缺少 [workspace] 段');
  const m = /^members\s*=\s*\[([^\]]*)\]/m.exec(section);
  if (!m) throw new Error('Cargo.toml [workspace] 段缺少 members 列表');
  const members = [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
  if (members.length === 0) throw new Error('Cargo.toml [workspace] members 列表为空');
  return members;
}

/** 读取成员 Cargo.toml [package] 的 name */
export function readPackageName(memberCargoToml) {
  const section = extractTomlSection(memberCargoToml, 'package');
  if (section === null) throw new Error('成员 Cargo.toml 缺少 [package] 段');
  const m = /^name\s*=\s*"([^"]+)"$/m.exec(section);
  if (!m) throw new Error('成员 Cargo.toml [package] 段缺少 name 字段');
  return m[1];
}

/** 读取 JSON 文本的顶层 version（同时校验 JSON 可解析） */
export function readJsonVersion(jsonText, fileLabel) {
  let obj;
  try {
    obj = JSON.parse(jsonText);
  } catch (e) {
    throw new Error(`${fileLabel} 不是合法 JSON：${e.message}`);
  }
  if (!obj || typeof obj.version !== 'string') {
    throw new Error(`${fileLabel} 缺少字符串 version 字段`);
  }
  return obj.version;
}

/** 仅改 [workspace.package] 段内的 version 行，其余字节原样保留 */
export function rewriteTomlVersion(text, newVersion) {
  const sectionRe = /^\[workspace\.package\]\s*$/m;
  const m = sectionRe.exec(text);
  if (!m) throw new Error('Cargo.toml 缺少 [workspace.package] 段');
  const start = m.index + m[0].length;
  const next = /^\[/m.exec(text.slice(start));
  const end = next ? start + next.index : text.length;
  const section = text.slice(start, end);
  const v = /^(\s*version\s*=\s*")[^"]*(".*)$/m.exec(section);
  if (!v) throw new Error('Cargo.toml [workspace.package] 段缺少 version = "..." 字段');
  const newSection =
    section.slice(0, v.index) + v[1] + newVersion + v[2] + section.slice(v.index + v[0].length);
  return text.slice(0, start) + newSection + text.slice(end);
}

/** 仅改 JSON 文件顶层 "version" 行（缩进 0-2 空格锚定，不误改嵌套键），保留字节与行尾（CRLF 安全） */
export function rewriteJsonVersionLine(text, newVersion) {
  const re = /^( {0,2}"version"\s*:\s*")[^"]*("\s*,?\s*)$/m;
  const m = re.exec(text);
  if (!m) throw new Error('JSON 文件缺少顶层 "version" 字段（嵌套 version 键不会被修改）');
  return text.slice(0, m.index) + m[1] + newVersion + m[2] + text.slice(m.index + m[0].length);
}

function escapeRegExp(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** 从 Cargo.lock 中提取指定包条目的版本；条目不存在返回 null */
export function extractLockVersion(lockText, packageName) {
  const nameRe = new RegExp(`^name = "${escapeRegExp(packageName)}"$`, 'm');
  const m = nameRe.exec(lockText);
  if (!m) return null;
  const after = lockText.slice(m.index + m[0].length);
  const v = /^version = "([^"]+)"$/m.exec(after);
  return v ? v[1] : null;
}

function detectEol(text) {
  return text.includes('\r\n') ? '\r\n' : '\n';
}

/** 版本小节标题：`## [X.Y.Z] - YYYY-MM-DD`（archiveChangelog 写入的格式） */
const VERSION_HEADING_RE = /^## \[[^\]]+\] - \d{4}-\d{2}-\d{2}[ \t]*$/m;

function isUnreleasedEmpty(section) {
  const entryRe = /^-\s*(Added|Changed|Fixed|Removed|Security)\s*[:：]/m;
  const subsectionRe = /^###\s*(Added|Changed|Fixed|Removed|Security)\b/m;
  if (entryRe.test(section)) return false;
  // `### Type` 子节只有在节内确实存在条目时才视为非空（防空子节产出空发布节）
  if (subsectionRe.test(section)) {
    return !/^-\s/m.test(section);
  }
  return true;
}

/**
 * 归档 CHANGELOG 的 [Unreleased] 小节为 `## [version] - date`，并在其上方插入新的空 [Unreleased]。
 * 守卫：必须恰好一个 [Unreleased]；节内必须有真实条目（- Type: 或 ### Type + 条目），注释不算。
 * 节边界只认版本小节标题（`## [v] - 日期`），其他 `##` 标题不会截断节。
 * 返回 { ok, text? , error? }；失败不修改输入。
 */
export function archiveChangelog(text, version, dateStr) {
  const headingRe = /^## \[Unreleased\][ \t]*$/gm;
  const matches = [...text.matchAll(headingRe)];
  if (matches.length === 0) {
    return { ok: false, error: 'CHANGELOG.md 缺少 ## [Unreleased] 小节' };
  }
  if (matches.length > 1) {
    return { ok: false, error: `CHANGELOG.md 存在 ${matches.length} 个 ## [Unreleased] 小节（应恰好一个）` };
  }
  const idx = matches[0].index;
  const headLen = matches[0][0].length;
  const rest = text.slice(idx + headLen);
  // 只认版本小节标题为边界；[Unreleased] 内的 `## 说明` 等不会截断
  const nextHeading = VERSION_HEADING_RE.exec(rest);
  const sectionEnd = nextHeading ? idx + headLen + nextHeading.index : text.length;
  const section = text.slice(idx, sectionEnd);
  if (isUnreleasedEmpty(section)) {
    return { ok: false, error: '## [Unreleased] 小节为空：发版前必须先记录本次变更条目（- Added: / - Fixed: ...）' };
  }
  const eol = detectEol(text);
  const renamed = section.replace(headingRe, `## [${version}] - ${dateStr}`);
  const fresh =
    `## [Unreleased]${eol}${eol}` +
    `<!-- 合并新变更后在此按类型追加条目：- Added: / - Changed: / - Fixed: / - Removed: / - Security:（中文描述） -->${eol}${eol}`;
  return { ok: true, text: text.slice(0, idx) + fresh + renamed + text.slice(sectionEnd) };
}

/**
 * 提取指定版本小节的条目文本（供 release 提交体使用）；小节不存在返回 null。
 * 返回节内条目行（去掉小节标题后的内容，保留原始换行）。
 */
export function extractReleaseEntries(text, version) {
  const headingRe = new RegExp(`^## \\[${escapeRegExp(version)}\\] - \\d{4}-\\d{2}-\\d{2}[ \\t]*$`, 'm');
  const m = headingRe.exec(text);
  if (!m) return null;
  const rest = text.slice(m.index + m[0].length);
  const next = VERSION_HEADING_RE.exec(rest);
  const end = next ? m.index + m[0].length + next.index : text.length;
  return text.slice(m.index + m[0].length, end).trim();
}

/** 本地日期 YYYY-MM-DD（非 UTC，避免时区跨日写错日期） */
export function todayLocalDate() {
  const d = new Date();
  const y = d.getFullYear();
  const mo = String(d.getMonth() + 1).padStart(2, '0');
  const da = String(d.getDate()).padStart(2, '0');
  return `${y}-${mo}-${da}`;
}
