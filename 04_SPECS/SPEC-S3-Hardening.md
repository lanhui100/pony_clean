# SPEC-S3: Phase 1 保护路径补充 + 路径匹配加固

> 对应 CLEAN_STRATEGY.md §3.7。在 S1 实现基础上进行专项安全验证 + 攻击面补充测试。
>
> **审查调优**: 经 3 路对抗审查发现 21 项问题，采纳 20 项。关键变更: 删除与 S1 重复的 11 个测试（仅保留 S1 未覆盖的攻击面）; 新增尾部空格/点号绕过测试; `temp_env` 加入 Cargo.toml; `#[cfg(windows)]` 保护 junction 测试; `TEMP=C:` 测试断言修正; 100K HashMap 移至 `tests/perf.rs`; PROTECTED_PREFIXES 完整性自动化验证; 工时重估 2h→6h。
>
> **工时**: 6h（新增 ~280 行，不含 S1 已覆盖的部分）
>
> **完成状态**: 尾部空格/点号、GLOBALROOT、前斜杠、Win32 命名空间等攻击面测试已完成。UWP junction 跳过测试已实现（`is_reparse_point`）。`verify_env_path_inner` 与 `is_path_protected` 的路径清洗已对齐。缺少 GLOBALROOT 专项单元测试。缺少系统性的负向渗透测试套件。

---

## 1. 加固范围

| # | 加固项 | 风险等级 | 测试状态 |
|---|--------|:--------:|:--------:|
| 1 | is_path_protected 尾部空格/点号 trim | CRITICAL | 新增 |
| 2 | is_path_allowed 分隔符边界 | CRITICAL | S1 已覆盖 |
| 3 | is_path_protected 分隔符边界 | HIGH | S1 已覆盖 |
| 4 | resolve_targets `..` 遍历防御 | CRITICAL | 新增 |
| 5 | verify_env_path TOCTOU 防御 | HIGH | 新增 |
| 6 | 8.3 短文件名保护 | HIGH | 新增 |
| 7 | DOS 设备命名空间变体 | MEDIUM | 新增 |
| 8 | system_drive() 使用 %SYSTEMDRIVE% | MEDIUM | 新增 |
| 9 | PROTECTED_PREFIXES 补充到 20+ | MEDIUM | 新增 |
| 10 | expand_env 大小写不敏感 | LOW | 新增 |
| 11 | 扫描阶段 per-file is_path_protected | HIGH | S1 已覆盖 |

---

## 2. 攻击面测试

### 2.1 is_path_protected 绕过

```rust
// 尾部空格/点号（Windows API 自动 trim，保护检查必须匹配）
#[test]
fn test_protected_trailing_space() {
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\kernel32.dll ")));
}
#[test]
fn test_protected_trailing_dot() {
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\kernel32.dll.")));
}
#[test]
fn test_protected_trailing_mixed() {
    assert!(is_path_protected(Path::new(r"C:\Windows\System32 ")));
    assert!(is_path_protected(Path::new(r"C:\Windows\System32.")));
}

// 8.3 短文件名（GetShortPathNameW → canonicalize 后可还原）
#[test]
#[cfg(windows)]
fn test_protected_short_name() {
    // 需要真实 Windows API: GetLongPathNameW
    // 跳过 if not admin
    assert!(is_path_protected(Path::new(r"C:\PROGRA~1\WindowsApps\test")));
}

// DOS 设备命名空间
#[test]
fn test_protected_win32_namespace_globalroot() {
    assert!(is_path_protected(Path::new(r"\\.\GLOBALROOT\Device\HarddiskVolume1\Windows\System32")));
}

// `..` 遍历（canonicalize 后重新检查）
#[test]
fn test_protected_dotdot_traversal() {
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\..\System32\kernel32.dll")));
}
```

### 2.2 is_path_allowed 绕过

（S1 已覆盖分隔符边界和尾部 `\`。S3 补充:）

```rust
#[test]
fn test_allowed_dotdot_within_temp() {
    // TEMP\..\Windows\System32 — starts_with 会匹配 TEMP, 但 canonicalize 后不应通过
    temp_env::with_var("TEMP", Some("C:\\Users\\T\\AppData\\Local\\Temp"), || {
        let targets = vec![ScanTarget::new("t", "%TEMP%", Safe, Category::Temp, "")];
        let bad = Path::new("C:\\Users\\T\\AppData\\Local\\Temp\\..\\..\\..\\Windows\\System32\\config\\SAM");
        assert!(!is_path_allowed(bad, &targets));
    });
}
```

### 2.3 环境变量注入

```rust
#[test]
fn test_env_injection_temp_is_c_drive() {
    // TEMP=C: 展开后与 system_drive() 前缀匹配
    // is_path_protected(C:\) 通过盘根保护检查
    // 注意: Path::new("C:") ≠ Path::new("C:\\"), 但 canonicalize("C:") 返回当前 dir
    temp_env::with_var("TEMP", Some("C:"), || {
        let expanded = expand_env("%TEMP%");
        // C: → 应被 is_path_protected 拦截或 canonicalize 后匹配盘根
        let canonical = std::fs::canonicalize(&expanded).unwrap_or_default();
        let protected = is_path_protected(Path::new(&expanded))
            || is_path_protected(&canonical);
        assert!(protected, "TEMP=C: must be rejected: expanded={expanded:?}");
    });
}

#[test]
fn test_env_injection_temp_is_system32_named() {
    // 标准命名字段测试
    temp_env::with_var("TEMP", Some(r"C:\Windows\System32"), || {
        let expanded = expand_env("%TEMP%");
        assert!(is_path_protected(Path::new(&expanded)));
    });
}

#[test]
fn test_env_injection_temp_non_existent() {
    // verify_env_path 对不存在的路径应返回 false（不放过）
    temp_env::with_var("TEMP", Some(r"C:\DoesNotExist"), || {
        let expanded = expand_env("%TEMP%");
        assert!(!verify_env_path(Path::new(&expanded), "%TEMP%"));
    });
}

#[test]
fn test_expand_env_case_insensitive() {
    // Windows 环境变量名大小写不敏感
    temp_env::with_var("TEMP", Some("C:\\Temp"), || {
        assert_eq!(expand_env("%temp%\\foo"), r"C:\Temp\foo");
        assert_eq!(expand_env("%Temp%\\foo"), r"C:\Temp\foo");
    });
}
```

### 2.4 Junction 逃逸（预枚举阶段）

```rust
#[test]
#[cfg(windows)]
fn test_uwp_junction_skipped() {
    let packages = tempfile::tempdir().unwrap();
    let junction_path = packages.path().join("FakePkg");
    let target = tempfile::tempdir().unwrap();
    std::os::windows::fs::symlink_dir(target.path(), &junction_path)
        .expect("junction creation (need admin or developer mode)");
    let resolved = resolve_uwp_packages_inner(packages.path());
    assert!(!resolved.iter().any(|p| p.ends_with("FakePkg")));
}
```

### 2.5 SleepStudy 保护验证

（注意: 路径在 PROTECTED_PREFIXES 之外, 代码级单独处理）

```rust
#[test]
fn test_sleepstudy_dir_protected() {
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\sleepstudy")));
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\sleepstudy\")));
}
#[test]
fn test_sleepstudy_subfile_not_protected() {
    // 子文件不通过 is_path_protected 保护, 靠 mtime 90d 过滤
    assert!(!is_path_protected(Path::new(r"C:\Windows\System32\sleepstudy\sub.etl")));
}
```

---

## 3. PROTECTED_PREFIXES 完整性

### 3.1 自动化验证

```rust
#[test]
fn test_all_protected_prefixes_normalized() {
    for prefix in PROTECTED_PREFIXES {
        let expanded = prefix.replace("%SYSTEMDRIVE%", &system_drive());
        assert!(!expanded.starts_with('\\'));
        assert!(!expanded.ends_with('\\'));
    }
}

#[test]
fn test_protected_prefixes_no_duplicates() {
    let prefixes: Vec<String> = PROTECTED_PREFIXES.iter()
        .map(|p| p.replace("%SYSTEMDRIVE%", "c:").to_lowercase()).collect();
    let mut seen = std::collections::HashSet::new();
    for p in &prefixes {
        assert!(seen.insert(p), "duplicate: {p}");
    }
}

#[test]
fn test_critical_paths_protected() {
    // CRITICAL 级别路径
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\winevt\Logs\Security.evtx")));
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\catroot2\some.db")));
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\spool\drivers\some.dll")));
    assert!(is_path_protected(Path::new(r"C:\Windows\System32\config\SAM")));
    // 盘根保护
    assert!(is_path_protected(Path::new(r"C:\")));
    // System Volume Information
    assert!(is_path_protected(Path::new(r"C:\System Volume Information\some")));
}

#[test]
fn test_protected_prefixes_meet_20_plus() {
    assert!(PROTECTED_PREFIXES.len() >= 20, "need 20+ prefixes");
}
```

### 3.2 PROTECTED_PREFIXES 补充（共 20+）

原有 12 条 + 补充:

```rust
"%SYSTEMDRIVE%\\Windows\\System32\\winevt\\Logs",
"%SYSTEMDRIVE%\\Windows\\System32\\catroot2",
"%SYSTEMDRIVE%\\Windows\\System32\\catroot",
"%SYSTEMDRIVE%\\Windows\\System32\\spool\\drivers",
"%SYSTEMDRIVE%\\Windows\\System32\\Tasks",
"%SYSTEMDRIVE%\\Windows\\System32\\Tasks\\MICROSOFT",
"%SYSTEMDRIVE%\\Windows\\System32\\drivers\\etc",
"%SYSTEMDRIVE%\\Windows\\System32\\CodeIntegrity",
"%SYSTEMDRIVE%\\Windows\\System32\\Licensing",
"%SYSTEMDRIVE%\\Windows\\System32\\config",
"%SYSTEMDRIVE%\\Windows\\System32\\config\\RegBack",
"%SYSTEMDRIVE%\\Windows\\System32\\GroupPolicy",
"%SYSTEMDRIVE%\\Windows\\System32\\SMI\\Store\\Machine",
"%SYSTEMDRIVE%\\System Volume Information",
"%SYSTEMDRIVE%\\Windows\\CSC",
"%SYSTEMDRIVE%\\Windows\\Registration",
"%SYSTEMDRIVE%\\Config.Msi",
"%SYSTEMDRIVE%\\ProgramData\\USOShared",
"%SYSTEMDRIVE%\\Recovery",
// S3 新增:
"%SYSTEMDRIVE%\\Boot",
"%PROGRAMDATA%\\Microsoft\\Windows\\Containers",
```

---

## 4. 性能测试（独立文件）

```rust
// tests/perf.rs (手动运行: cargo test -- --ignored)
#[test]
#[ignore]
fn test_target_map_lookup_perf() {
    let targets = get_clean_targets();
    let resolved = resolve_targets(&targets);
    let map: HashMap<PathBuf, &ScanTarget> = resolved.into_iter().collect();
    let key = Path::new(r"C:\Users\Test\AppData\Local\Temp\file.tmp");
    // warm-up
    for _ in 0..1000 { let _ = map.get(key); }
    let start = std::time::Instant::now();
    for _ in 0..20_000 { let _ = map.get(key); }
    let elapsed = start.elapsed();
    assert!(elapsed < std::time::Duration::from_millis(200), "too slow: {elapsed:?}");
}
```

---

## 5. 文件变更

| 文件 | 变更 | 行数 |
|------|------|:----:|
| `crates/pony_core/src/cleaner.rs` | 安全测试（~16 新增）+ `PROTECTED_PREFIXES` 补充 | +250 |
| `tests/perf.rs` | `#[ignore]` 性能测试 | +30 |
| `Cargo.toml` (pony_core) | [dev-dependencies] `temp_env = "0.3"`（如 S2 尚未添加） | +1 |
| **合计** | | **~280** |

---

## 6. 验收标准

1. [ ] 尾部空格/点号: `is_path_protected` trim 后正确保护
2. [ ] `..` 遍历: resolve_targets 通过 canonicalize 拒绝
3. [ ] 8.3 短名: 测试通过（需 Windows + API 调用）
4. [ ] DOS GLOBALROOT: `is_path_protected` 处理 `\\.\GLOBALROOT`
5. [ ] `TEMP=C:` / `TEMP=C:\Windows\System32` → 被保护拒绝
6. [ ] `expand_env` 大小写不敏感（`%temp%` = `%TEMP%`）
7. [ ] UWP junction 在预枚举阶段被 `is_symlink` 检测跳过
8. [ ] SleepStudy 目录保护 + 子文件不保护（mtime 过滤）
9. [ ] PROTECTED_PREFIXES ≥ 20 条, 无重复
10. [ ] `PROTECTED_PREFIXES` 包含 System Volume Information / Boot
11. [ ] 全部安全测试通过（`cargo test -p pony_core -- --ignored` 可选 perf）
12. [ ] `system_drive()` 优先读 `%SYSTEMDRIVE%` 再 fallback 到 `SystemRoot`
