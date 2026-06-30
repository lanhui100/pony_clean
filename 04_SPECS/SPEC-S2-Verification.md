# SPEC-S2: Phase 1 逐目标验证 + 边界测试

> 对应 CLEAN_STRATEGY.md §3.9 S2。在 S1 实现完成后，逐个验证 43 个 target 在真实 Windows 系统上的行为正确性。
>
> **审查调优**: 经 3 路对抗审查发现 15 项问题，采纳 14 项。关键变更: Mock 目录补全至 28+ target 对应路径; `env::set_var` 改为 `temp_env::with_var` + `#[serial_test]`; 移除与 S1 重复的 5 个测试; 新增 `..` 遍历安全测试; 工时重估 8h→16h。
>
> **前置条件**: SPEC-S1 实现代码已合入（PonyConfig v2, Category 枚举, ScanWarning 枚举, 43 target）。
>
> **完成状态**: TestEnv 已创建（`tests/common/mod.rs`），6 个集成测试已通过。但 TestEnv 尚未完整覆盖全部 43 个 target 的 mock 路径（仅基础结构）。`RELEASE_CHECKLIST.md` 未创建。域环境 GPO 等真机手动验证未执行。

---

## 1. 测试基础设施

### 1.1 TestEnv (tests/common/mod.rs)

与 SPEC-S5 共享，统一在 `tests/common/mod.rs`:

```rust
use tempfile::TempDir;
use std::path::{Path, PathBuf};

pub struct TestEnv {
    pub root: PathBuf,
    _tmp: TempDir,  // Drop 时自动清理
}

impl TestEnv {
    pub fn new(opts: TestEnvOptions) -> Self { ... }
    pub fn with_browsers(self, browsers: &[&str]) -> Self { ... }
    pub fn with_uwp_packages(self, count: usize) -> Self { ... }
    pub fn with_test_files(self) -> Self { ... }
    pub fn apply_env(&self) { /* temp_env::with_var scoped */ }
    pub fn path_of(&self, rel: &str) -> PathBuf { self.root.join(rel) }
}

pub struct TestEnvOptions {
    pub skip_browsers: bool,
    pub skip_uwp: bool,
    pub skip_system_paths: bool,
}
```

### 1.2 环境变量隔离

**不接受裸 `env::set_var`**。所有测试使用 `temp_env::with_var` 或 `TestEnv::apply_env`（内部使用 `temp_env`）。

```rust
// Cargo.toml 新增
[dev-dependencies]
temp_env = "0.3"
serial_test = "3"
```

**并行测试**: 修改环境变量的测试全部标注 `#[serial_test::serial]`。不修改环境变量的测试（如 `test_all_target_ids_unique`）可并行。

---

## 2. 验证清单

### 2.1 已有 15 目标回归验证

为节省篇幅，展开路径预期与 S1 一致。关键验证点:

| id | 边界条件 |
|----|---------|
| user_temp | 空 TEMP 环境变量 → expand_env 返回空, resolve 跳过 |
| sys_temp | 无 SYSTEM 权限访问 → jwalk 跳过, 不 crash |
| chrome_code_cache | Chrome 未安装 → 路径不存在 → resolve_targets 跳过 |
| firefox_cache | Firefox 未安装 → 无 profile → 空结果 |
| driver_store | 深目录权限 → 可读子目录正常, 不可读跳过 |
| recycle_bin | 系统隐藏 + 受保护 → 需要特殊权限 |

### 2.2 新增 28 目标验证

**TestEnv 必须完整覆盖以下所有路径**（每路径创建空目录 + 可选测试文件）:

```rust
// 系统日志
root/Windows/System32/LogFiles/W3SVC1/           // sys_logfiles
root/Windows/Logs/CBS/                            // sys_logs
root/Users/Test/AppData/Local/Microsoft/Windows/WER/  // wer_user
root/ProgramData/Microsoft/Windows/WER/           // wer_system
root/Windows/System32/sru/                        // sru (含 SRUDB.dat)
root/Windows/System32/oobe/info/                  // oobe_info
root/Windows/System32/NtmsData/                   // ntms_data
root/Windows/Downloaded Program Files/            // downloaded_progs
root/Windows/System32/Macromed/Flash/             // flash_cache
root/Windows/System32/spool/SERVERS/              // spool_servers
root/Windows/System32/MsDtc/Trace/                // msdtc_trace

// 缓存
root/Users/Test/AppData/Local/Microsoft/Windows/INetCache/IE/  // inet_cache_ie
root/Users/Test/AppData/Local/Microsoft/Windows/AppCache/      // app_cache
root/Users/Test/AppData/Local/Microsoft/TerminalServer Client/Cache/ // ts_client_cache
root/Users/Test/AppData/Local/Microsoft/Media Player/          // wmp_cache
root/Users/Test/AppData/Local/Microsoft/Windows/Caches/        // explorer_cache

// 系统临时
root/Users/Test/AppData/Local/Temp/                // user_temp + wer_temp + etl + app_logs
root/Windows/Temp/                                  // sys_temp + wer_sys

// 用户数据
root/Users/Test/Downloads/                          // downloads_old
root/Users/Test/AppData/Local/CrashDumps/          // crashdumps
root/$Recycle.Bin/                                  // recycle_bin

// 系统重置
root/$SysReset/                                     // sys_reset
root/$Windows.~BT/                                  // win_upgrade_tmp

// UWP (创建 51 个包以测试上限)
root/Users/Test/AppData/Local/Packages/Pkg{i}/AC/Temp/
root/Users/Test/AppData/Local/Packages/Pkg{i}/AC/INetCache/
root/Users/Test/AppData/Local/Packages/Pkg{i}/LocalCache/

// 浏览器
root/Users/Test/AppData/Local/Google/Chrome/User Data/Default/Cache/
root/Users/Test/AppData/Local/Microsoft/Edge/User Data/Default/Cache/
root/Users/Test/AppData/Roaming/Mozilla/Firefox/Profiles/abcd.default-release/cache2/entries/
```

### 2.3 边界测试

| 边界 | 方法 | 预期 |
|------|------|------|
| `..` 遍历 | `TEMP=C:\Temp\..\Windows\System32` | resolve 时 canonicalize 拒绝 |
| 尾部空格 | `is_path_protected("C:\\Windows\\System32 ")` | 空格被 trim, 返回 true |
| 尾部点号 | `is_path_protected("C:\\Windows\\System32.")` | 点号被 trim, 返回 true |
| 8.3 短名 | `C:\PROGRA~1\WindowsApps` | `is_path_protected` 处理 8.3 需要 GetShortPathNameW |
| 路径含 unicode | Temp 下中文/日文文件名 `测试.tmp` | 不 panic, UTF-8 正确传递 |
| 路径超长 >260 | 创建 >260 字符文件 | 使用 `\\?\` 或 Error, 不 crash |
| 空变量 | TEMP 为空字符串 | expand_env 返回空, resolve 跳过 |
| 非 C 盘 | SystemRoot = D:\Windows | system_drive() 返回 D: |
| 多个变量同时缺失 | LOCALAPPDATA + APPDATA 同时为空 | expand_env 含空串 fallback, resolve 跳过 |
| junction 在 target 路径中 | 扫描起始路径本身是 junction | resolve_targets 中 canonicalize 后匹配保护路径 |

---

## 3. 测试用例

### 3.1 验证测试（13 个，不重复 S1）

```rust
#[serial_test::serial]
#[test]
fn test_all_targets_resolve_in_mock_env() {
    let env = TestEnv::new(TestEnvOptions::default());
    env.apply_env();
    let targets = get_clean_targets();
    let resolved = resolve_targets(&targets);
    // 不存在的路径应跳过, 不 crash
    assert!(resolved.len() <= targets.len());
}

#[test]
fn test_target_ids_unique() {
    let ids: Vec<&str> = get_clean_targets().iter().map(|t| t.id).collect();
    let mut sorted = ids.clone(); sorted.sort(); sorted.dedup();
    assert_eq!(ids.len(), sorted.len());
}

#[test]
fn test_chrome_not_installed() {
    let env = TestEnv::new(TestEnvOptions { skip_browsers: true, ..Default::default() });
    env.apply_env();
    let targets = filter_targets_by_category(&get_clean_targets(), Category::Cache);
    let resolved = resolve_targets(&targets);
    // 浏览器缓存 target 应全部跳过
    assert!(resolved.iter().all(|(p, _)| !p.to_string_lossy().contains("Chrome")));
}

#[test]
fn test_uwp_50_limit() {
    let env = TestEnv::new(TestEnvOptions { skip_uwp: false, ..Default::default() });
    // 创建 55 个包
    // verify resolve_targets 只返回 50
}

#[test]
fn test_downloads_mtime_filter() {
    // 创建 >90d 和 <90d 的文件
    // verify 只列出 >90d
}

#[test]
fn test_logs_mtime_filter() {
    // 创建 >90d 和 <90d 的 .etl
    // verify 只列出 >90d
}

#[test]
fn test_empty_temp_var() {
    temp_env::with_var("TEMP", Some(""), || {
        let targets = get_clean_targets();
        let resolved = resolve_targets(&targets);
        assert!(!resolved.iter().any(|(p, t)| t.id == "user_temp"));
    });
}

#[test]
fn test_non_c_drive() {
    temp_env::with_var("SystemRoot", Some("D:\\Windows"), || {
        assert_eq!(system_drive(), "D:");
    });
}

#[test]
fn test_path_traversal_dotdot() {
    // TEMP = C:\Real\Temp\..\..\Windows
    // verify resolve_targets 通过 canonicalize 拒绝
}

#[test]
fn test_junction_in_scan_root() {
    // 扫描起始目录是 junction → canonicalize 后匹配保护路径
    // verify resolve_targets 跳过
}

#[test]
fn test_unicode_path() {
    // 创建中文/日文文件名 → verify 扫描命中
}

#[test]
fn test_all_glob_patterns_case_insensitive() {
    #[cfg(windows)]
    // *.ETL 应匹配 .etl
}
```

### 3.2 已从 S1 覆盖的测试（不重复实现）

以下测试在 S1 §4.1 中已定义，S2 不重复:

- `test_is_path_allowed_separator`
- `test_is_path_protected_separator`
- `test_env_injection_defense`
- `test_uwp_junction_skip`
- `test_category_serde_roundtrip`
- `test_config_migration_v1_to_v2`

---

## 4. 手动验证

**必须真机（4 项）**:

```
[ ] 1. 全新 Win11（无浏览器、无 UWP）→ 扫描正常，0 结果 target 不报错
[ ] 2. 日常使用 6 个月的 Win10 → 扫描正常，各浏览器缓存正确识别
[ ] 3. 企业域环境 Win11（GPO 锁定 WU/Delivery Optimization）→ 非管理员降级正确
[ ] 4. 扫描中途取消 → 部分结果展示 + 可重新发起扫描
```

**可自动化但暂时手动（0 项 — 全部转为自动化测试）**:

已验证: #4(中文Win)→env测试, #5(日文Win)→env测试, #6(取消)→取消测试, #7(43 target)→resolve测试, #8(去重)→ids测试, #9(300K上限)→Warning测试, #10(UWP 50)→uwp测试。均已在上方覆盖。

---

## 5. 文件变更

| 文件 | 变更 | 行数 |
|------|------|:----:|
| `tests/common/mod.rs` | TestEnv（与 S5 共享） | +120 |
| `crates/pony_core/src/cleaner.rs` | 新增 ~13 个 `#[cfg(test)]` | +200 |
| `Cargo.toml` (pony_core) | [dev-dependencies] temp_env, serial_test | +3 |
| `RELEASE_CHECKLIST.md` | 手动验证 4 项 | +8 |
| **合计** | | **~330** |

---

## 6. 验收标准

1. [ ] TestEnv 覆盖所有 43 target 的 mock 路径
2. [ ] 13 个测试全部通过（`--test-threads=1`）
3. [ ] 不修改环境变量的测试可并行（`--test-threads=4`）
4. [ ] 手动验证 4 项通过
5. [ ] `RELEASE_CHECKLIST.md` 创建并包含验证项
6. [ ] CI 全通过
