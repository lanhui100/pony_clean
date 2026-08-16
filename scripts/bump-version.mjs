#!/usr/bin/env node
// bump-version.mjs — 版本唯一变更点（零依赖，Node >= 20）
// 用法: node scripts/bump-version.mjs <新版本> [--commit] [--tag] [--dry-run]
// 流程: semver 校验 → 同步三处清单（文本精改保字节）→ cargo update 刷新 Cargo.lock + 回读断言
//       → CHANGELOG [Unreleased] 归档（唯一且非空校验）→ 可选 --commit / --tag（含守卫）
// 幂等: 版本已是目标版本时（QA 修复后补提交 / 分步打 tag），--commit/--tag 跳过写文件直接执行收尾
// 事务: 写文件或 cargo update 失败 → 自动还原全部 5 个版本文件，不留半态

import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
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

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const FILES = {
  cargoToml: path.join(ROOT, 'Cargo.toml'),
  cargoLock: path.join(ROOT, 'Cargo.lock'),
  packageJson: path.join(ROOT, 'frontend', 'package.json'),
  tauriConf: path.join(ROOT, 'src-tauri', 'tauri.conf.json'),
  changelog: path.join(ROOT, 'CHANGELOG.md'),
};
/** --commit 白名单：仅允许这 5 个版本文件被提交 */
const COMMIT_FILES = ['Cargo.toml', 'Cargo.lock', 'frontend/package.json', 'src-tauri/tauri.conf.json', 'CHANGELOG.md'];
/** 还原提示用路径（与 COMMIT_FILES 一致） */
const RELEASE_FILE_LABELS = COMMIT_FILES.join(' ');

function parseArgs(argv) {
  const args = { version: null, commit: false, tag: false, dryRun: false };
  for (const a of argv) {
    if (a === '--commit') args.commit = true;
    else if (a === '--tag') args.tag = true;
    else if (a === '--dry-run') args.dryRun = true;
    else if (a.startsWith('-')) throw new Error(`未知选项: ${a}`);
    else if (args.version === null) args.version = a;
    else throw new Error(`多余的位置参数: ${a}`);
  }
  if (args.version === null) {
    throw new Error('用法: node scripts/bump-version.mjs <新版本> [--commit] [--tag] [--dry-run]');
  }
  return args;
}

/** 前台执行（继承 stdio），失败抛错；区分"二进制缺失"与"非零退出" */
function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { cwd: ROOT, stdio: 'inherit', ...opts });
  if (r.error) {
    throw new Error(`无法执行 ${cmd}：${r.error.message}（需要真实可执行文件；.cmd/.bat shim 不被 Node spawn 解析）`);
  }
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} 失败（exit ${r.status}）`);
  return r;
}

/** 捕获式执行（返回 stdout），失败抛错；调用方自行检查 status */
function runCapture(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', ...opts });
  if (r.error) throw new Error(`无法执行 ${cmd}：${r.error.message}`);
  return r;
}

/** --commit 白名单守卫：已修改的 tracked 文件必须 ⊆ 白名单，且不允许任何 untracked 文件 */
function checkCommitWhitelist() {
  const r = runCapture('git', ['status', '--porcelain']);
  if (r.status !== 0) throw new Error(`git status 执行失败（exit ${r.status}）`);
  const unexpected = [];
  for (const line of r.stdout.split(/\r?\n/).filter(Boolean)) {
    // porcelain 格式: "XY PATH"（X=暂存区状态，Y=工作区状态，'??'=未跟踪）。
    // 路径含特殊字符时 git 会 C 转义并加引号，与 ASCII 白名单不匹配 → 判为非预期 → 中止（安全方向）。
    const flags = line.slice(0, 2);
    const rel = line.slice(3);
    if (flags === '??') {
      unexpected.push(`未跟踪文件: ${rel}`);
    } else if (!COMMIT_FILES.includes(rel)) {
      unexpected.push(`已修改文件: ${rel}`);
    }
  }
  if (unexpected.length > 0) {
    throw new Error(
      `--commit 中止：工作树存在版本文件之外的改动\n${unexpected.join('\n')}\n` +
        '请先提交/暂存这些改动（WIP 可用 git stash -u 暂存），或先不带 --commit 运行 bump 后手工提交。',
    );
  }
}

/** 提交版本文件；提交体带本次发布条目（幂等：无改动则跳过） */
function verifyAndCommit(newVersion) {
  checkCommitWhitelist();
  const status = runCapture('git', ['status', '--porcelain', '--', ...COMMIT_FILES]);
  if (!status.stdout.trim()) {
    console.log('（版本文件无待提交改动，跳过 git commit）');
    return;
  }
  const releaseNotes = extractReleaseEntries(readFileSync(FILES.changelog, 'utf8'), newVersion);
  const commitArgs = ['commit', '-m', `chore(release): v${newVersion}`];
  if (releaseNotes) commitArgs.push('-m', releaseNotes);
  run('git', ['add', ...COMMIT_FILES]);
  // pathspec 限定提交范围：只提交白名单文件（纵深防御，防预暂存的其他文件混入）
  run('git', [...commitArgs, '--', ...COMMIT_FILES]);
  console.log(`✅ git commit: chore(release): v${newVersion}`);
}

/** tag 守卫：HEAD 必须已含目标版本（防 tag 到错误提交），且同名 tag 不存在 */
function verifyAndTag(newVersion) {
  const tagName = `v${newVersion}`;
  const exists = runCapture('git', ['tag', '-l', tagName]);
  if (exists.stdout.trim() === tagName) throw new Error(`--tag 中止：tag ${tagName} 已存在`);
  const headToml = runCapture('git', ['show', 'HEAD:Cargo.toml']);
  if (headToml.status !== 0) throw new Error(`无法读取 HEAD:Cargo.toml（exit ${headToml.status}）`);
  const headVer = readWorkspaceVersion(headToml.stdout);
  if (headVer !== newVersion) {
    throw new Error(`--tag 中止：HEAD 的版本为 ${headVer}，不是 ${newVersion}（tag 会打到错误提交，先提交本次 bump）`);
  }
  run('git', ['tag', '-a', tagName, '-m', `${tagName} 发布`]);
  console.log(`✅ git tag: ${tagName}`);
}

/** 事务回滚：还原 5 个版本文件到 bump 前内容（尽力而为） */
function rollback(snapshot) {
  const restores = [
    [FILES.cargoToml, snapshot.cargoToml],
    [FILES.packageJson, snapshot.pkgJson],
    [FILES.tauriConf, snapshot.tauriConf],
    [FILES.changelog, snapshot.changelog],
    [FILES.cargoLock, snapshot.lockText],
  ];
  for (const [file, content] of restores) {
    try {
      writeFileSync(file, content);
    } catch {
      /* 尽力而为：还原失败时外层错误信息会附手动恢复命令 */
    }
  }
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const newVersion = parseVersion(args.version);

  const cargoToml = readFileSync(FILES.cargoToml, 'utf8');
  const current = readWorkspaceVersion(cargoToml);
  const alreadyBumped = newVersion === current;

  // 幂等模式：版本已是目标版本时，--commit/--tag 直接执行收尾（QA 修复后补提交、分步打 tag 均走此路径）
  if (alreadyBumped && !args.commit && !args.tag) {
    throw new Error(`新版本与当前版本相同（${current}），无需 bump`);
  }

  if (!alreadyBumped) {
    const pkgJson = readFileSync(FILES.packageJson, 'utf8');
    const tauriConf = readFileSync(FILES.tauriConf, 'utf8');
    const changelog = readFileSync(FILES.changelog, 'utf8');
    const lockText = readFileSync(FILES.cargoLock, 'utf8');

    // JSON 合法性前置校验（精改前确保文件可解析）
    readJsonVersion(pkgJson, 'frontend/package.json');
    readJsonVersion(tauriConf, 'src-tauri/tauri.conf.json');

    // CHANGELOG 归档计算（缺节/多节/空节在此拒绝）
    const dateStr = todayLocalDate();
    const archived = archiveChangelog(changelog, newVersion, dateStr);
    if (!archived.ok) throw new Error(archived.error);

    const updates = [
      { file: FILES.cargoToml, rel: 'Cargo.toml', content: rewriteTomlVersion(cargoToml, newVersion) },
      { file: FILES.packageJson, rel: 'frontend/package.json', content: rewriteJsonVersionLine(pkgJson, newVersion) },
      { file: FILES.tauriConf, rel: 'src-tauri/tauri.conf.json', content: rewriteJsonVersionLine(tauriConf, newVersion) },
      { file: FILES.changelog, rel: 'CHANGELOG.md', content: archived.text },
    ];

    console.log(`bump ${current} → ${newVersion}（归档日期 ${dateStr}）`);
    for (const u of updates) {
      console.log(`  ${u.rel}: ${current} → ${newVersion}${u.rel === 'CHANGELOG.md' ? '（[Unreleased] 归档为新版本节）' : ''}`);
    }
    console.log('  Cargo.lock: cargo update（限 workspace 成员）+ 回读断言');
    if (args.commit) console.log('  git: commit chore(release): v' + newVersion);
    if (args.tag) console.log(`  git: tag -a v${newVersion}`);

    if (args.dryRun) {
      console.log('（--dry-run：未写入任何文件，未执行 git/cargo）');
      return;
    }

    // 白名单守卫前置：守卫失败时不产生任何写入（不留"已写未提交"半态）
    if (args.commit) checkCommitWhitelist();

    // 事务：写文件 / cargo update / 断言任一失败 → 还原全部版本文件
    const snapshot = { cargoToml, pkgJson, tauriConf, changelog, lockText };
    try {
      for (const u of updates) writeFileSync(u.file, u.content);

      const members = readWorkspaceMembers(cargoToml);
      const names = members.map((p) => readPackageName(readFileSync(path.join(ROOT, p, 'Cargo.toml'), 'utf8')));
      const specArgs = names.flatMap((n) => ['-p', n]);
      run('cargo', ['update', ...specArgs, '-w']);

      // 回读断言：lock 中成员条目版本必须等于新版本
      const newLock = readFileSync(FILES.cargoLock, 'utf8');
      const stale = names.filter((n) => extractLockVersion(newLock, n) !== newVersion);
      if (stale.length > 0) {
        throw new Error(`Cargo.lock 断言失败：${stale.join(', ')} 条目版本未更新为 ${newVersion}（cargo update 未生效，请检查 cargo 环境）`);
      }
    } catch (e) {
      rollback(snapshot);
      throw new Error(
        `${e.message}\n` +
          `（已自动还原全部版本文件；若还原失败，请手动执行: git checkout -- ${RELEASE_FILE_LABELS}）`,
      );
    }
  } else {
    console.log(`版本已是 ${current}，跳过写文件与 cargo update`);
    if (args.dryRun) {
      console.log('（--dry-run：未执行 git 操作）');
      return;
    }
  }

  if (args.commit) verifyAndCommit(newVersion);
  if (args.tag) verifyAndTag(newVersion);

  if (!args.commit && !args.tag) {
    console.log(`✅ bump 完成: ${current} → ${newVersion}（建议 git diff 审查后提交）`);
  }
}

try {
  main();
} catch (e) {
  console.error(`[bump-version] 错误: ${e.message}`);
  process.exit(1);
}
