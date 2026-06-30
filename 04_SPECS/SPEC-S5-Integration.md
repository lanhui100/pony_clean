# SPEC-S5: Phase 1 集成测试全量覆盖

> 对应 CLEAN_STRATEGY.md §3.9 S5。覆盖 S1-S4 所有变更的端到端集成测试。
>
> **审查调优**: 经对抗审查发现 13 项问题，采纳 12 项。关键变更: `TestEnv` 与 S2 统一为 `tests/common/mod.rs`; 新增 ENV 注入攻击测试; 验证取消后锁释放; 配置迁移保留 `custom_exclude_paths`; 标注 `tauri::test::mock_app` Windows 已知 bug; 工时重估 4h→8h。
>
> **前置条件**: SPEC-S1/S2/S3/S4 代码已全部合入。
>
> **工时**: 8h（~510 行）
>
> **完成状态**: TestEnv 已创建（`tests/common/mod.rs`），6 个集成测试已通过。Tauri 命令 E2E 测试因 tauri#13419 已知 bug 跳过。CI workflow（`.github/workflows/test.yml`）未创建。更多 S2 规格中的边界测试未覆盖。

---

## 1. 测试架构

```
tests/
  common/
    mod.rs          ← TestEnv（与 S2 共享，唯一 mock 环境）
  integration_s1.rs  ← 7 个端到端测试（串行）
  config_migration.rs ← 配置迁移测试（可并行）
```

**`tests/common/mod.rs`** — 由 S2 和 S5 共享:

```rust
pub struct TestEnv { ... }        // 完整模拟 Windows 目录结构
pub struct TestEnvOptions { ... } // 可配置跳过浏览器/UWP/系统路径
```

---

## 2. 集成测试

### 2.1 扫描全流程（integration_s1.rs）

```rust
mod common;

#[serial_test::serial]
#[test]
fn test_scan_all_targets() {
    let env = common::TestEnv::new(TestEnvOptions::default().with_test_files(true));
    env.apply_env();
    let (tx, rx) = std::sync::mpsc::channel();
    let (cmd_tx, _) = cleaner::start_scan(tx).unwrap();
    let mut all_items = Vec::new();
    loop {
        match rx.recv().unwrap() {
            ScanEvent::ItemsFound { items, .. } => all_items.extend(items),
            ScanEvent::Done { total_items, .. } => {
                assert!(total_items > 0);
                break;
            }
            ScanEvent::Cancelled => panic!("unexpected cancel"),
            _ => {}
        }
    }
    let _ = cmd_tx.send(CleanCommand::Shutdown);
    // 验证发现各类测试文件
    assert!(all_items.iter().any(|i| i.path.to_string_lossy().contains("test.tmp")));
    assert!(all_items.iter().any(|i| i.path.to_string_lossy().contains("trace.etl")));
}

#[serial_test::serial]
#[test]
fn test_scan_excludes_protected_paths() {
    let env = common::TestEnv::new(TestEnvOptions::default());
    env.apply_env();
    // 在 mock 的 winevt\Logs 下创建文件（该路径在 PROTECTED_PREFIXES 中）
    let protected_file = env.path_of("Windows/System32/winevt/Logs/Security.evtx");
    std::fs::create_dir_all(protected_file.parent().unwrap()).unwrap();
    std::fs::write(&protected_file, vec![0u8; 100]).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let (cmd_tx, _) = cleaner::start_scan(tx).unwrap();
    let mut all_items = Vec::new();
    loop {
        match rx.recv().unwrap() {
            ScanEvent::ItemsFound { items, .. } => all_items.extend(items),
            ScanEvent::Done { .. } => break,
            _ => {}
        }
    }
    let _ = cmd_tx.send(CleanCommand::Shutdown);
    assert!(!all_items.iter().any(|i| i.path.to_string_lossy().contains("Security.evtx")));
}

#[test]
fn test_delete_and_log() {
    let env = common::TestEnv::new(TestEnvOptions::default());
    let test_file = env.path_of("Users/Test/AppData/Local/Temp/delete_me.tmp");
    std::fs::write(&test_file, vec![0u8; 4096]).unwrap();
    let result = cleaner::delete_files(&[test_file.clone()]);
    assert_eq!(result.success, 1);
    assert!(!test_file.exists());
    // 日志中应有记录
    let logs = cleaner::get_clean_logs(10).unwrap();
    assert!(logs.entries.iter().any(|e| e.success >= 1));
}

#[test]
fn test_protected_delete_rejected() {
    let result = cleaner::delete_files(&[PathBuf::from(r"C:\Windows\System32\kernel32.dll")]);
    assert_eq!(result.success, 0);
    assert!(result.errors.iter().any(|e| e.contains("Protected")));
}

#[serial_test::serial]
#[test]
fn test_scan_cancel_and_restart() {
    let env = common::TestEnv::new(TestEnvOptions::default());
    env.apply_env();
    let (tx, rx) = std::sync::mpsc::channel();
    let (cmd_tx, cancel_token) = cleaner::start_scan(tx).unwrap();
    let mut received_items = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(ScanEvent::ItemsFound { .. }) => { received_items = true; cancel_token.cancel(); }
            Ok(ScanEvent::Cancelled) => { assert!(received_items); break; }
            Ok(ScanEvent::Done { .. }) => break,
            Err(_) => break,
            _ => {}
        }
    }
    let _ = cmd_tx.send(CleanCommand::Shutdown);
    // 验证取消后可重新发起扫描
    let (tx2, _) = std::sync::mpsc::channel();
    let r2 = cleaner::start_scan(tx2);
    assert!(r2.is_ok(), "should be able to restart after cancel");
}

#[test]
fn test_double_scan_rejected() {
    let env = common::TestEnv::new(TestEnvOptions::default());
    env.apply_env();
    let (tx1, _) = std::sync::mpsc::channel();
    let (tx2, _) = std::sync::mpsc::channel();
    let r1 = cleaner::start_scan(tx1);
    assert!(r1.is_ok());
    let r2 = cleaner::start_scan(tx2);
    assert!(r2.is_err());
    assert!(r2.unwrap_err().contains("already in progress"));
}

#[serial_test::serial]
#[test]
fn test_env_injection_attack() {
    // 模拟攻击者设置 SystemRoot 为 Temp 目录 → 尝试扫描 System32
    let env = common::TestEnv::new(TestEnvOptions::default());
    let fake_sys = env.path_of("FakeWindows");
    std::fs::create_dir_all(&fake_sys).unwrap();
    temp_env::with_var("SystemRoot", Some(fake_sys.to_str().unwrap()), || {
        temp_env::with_var("TEMP", Some(env.path_of("Users/Test/AppData/Local/Temp").to_str().unwrap()), || {
            let (tx, rx) = std::sync::mpsc::channel();
            let (cmd_tx, _) = cleaner::start_scan(tx).unwrap();
            let mut all_items = Vec::new();
            loop {
                match rx.recv() {
                    Ok(ScanEvent::ItemsFound { items, .. }) => all_items.extend(items),
                    Ok(ScanEvent::Done { .. }) | Err(_) => break,
                    _ => {}
                }
            }
            let _ = cmd_tx.send(CleanCommand::Shutdown);
            // 不应发现真实 Windows 文件
            assert!(!all_items.iter().any(|i| i.path.to_string_lossy().contains("System32")));
        });
    });
}
```

### 2.2 配置迁移（config_migration.rs，可并行）

```rust
#[test]
fn test_config_migration_v1_to_v2() {
    let dir = tempfile::tempdir().unwrap();
    let v1_config = r#"{
        "disabled_targets": ["%TEMP%", "%WINDIR%\\Temp"],
        "custom_exclude_paths": ["C:\\MyData"]
    }"#;
    std::fs::write(dir.path().join("config.json"), v1_config).unwrap();
    let config = cleaner::load_config_in_dir(dir.path());
    assert_eq!(config.version, Some(2));
    assert!(config.disabled_target_ids.contains(&"user_temp".to_string()));
    assert!(config.disabled_target_ids.contains(&"sys_temp".to_string()));
    // custom_exclude_paths 保留（不丢失）
    assert!(config.custom_exclude_paths.contains(&"C:\\MyData".to_string()));
}

#[test]
fn test_get_filtered_targets_uses_id() {
    let config = PonyConfig {
        disabled_target_ids: vec!["firefox_cache".into(), "wu_download".into()],
        ..Default::default()
    };
    let filtered = get_filtered_targets(&config);
    assert!(!filtered.iter().any(|t| t.id == "firefox_cache"));
}
```

### 2.3 Tauri 命令测试（tauri_commands.rs，可选）

⚠ **已知限制**: `tauri::test::mock_app` 在 Windows 上有 `ENTRYPOINT_NOT_FOUND` 已知 bug（tauri#13419）。需要 `tauri = { features = ["test"], default-features = false }`。

如果条件允许:

```rust
#[cfg(feature = "e2e")]
#[cfg(target_os = "windows")]
#[test]
fn test_start_scan_tauri_command() {
    // 使用 tauri::test::mock_app 验证事件发射
}

#[cfg(feature = "e2e")]
#[cfg(target_os = "windows")]
#[test]
fn test_get_clean_logs_command() {
    // invoke get_clean_logs → 验证返回结构
}
```

---

## 3. CI 集成

```yaml
jobs:
  unit:
    runs-on: windows-latest
    steps:
      - run: cargo test -p pony_core -- --test-threads=4
        name: "Unit tests (parallel, non-env tests)"
  integration:
    runs-on: windows-latest
    steps:
      - run: cargo test -p pony_core --test integration_s1 -- --test-threads=1
        name: "Integration tests (serial, env-sensitive)"
      - run: cargo test -p pony_core --test config_migration -- --test-threads=4
        name: "Config migration tests (parallel)"
  e2e:
    if: false  # disabled until tauri#13419 fixed
    runs-on: windows-latest
    steps:
      - run: cargo test -p pony_clean --features e2e -- --test-threads=1
```

---

## 4. 文件变更

| 文件 | 行数 |
|------|:----:|
| `tests/common/mod.rs` | +120（与 S2 共享） |
| `tests/integration_s1.rs` | +200 |
| `tests/config_migration.rs` | +60 |
| `tests/tauri_commands.rs` | +60 |
| `.github/workflows/test.yml` | +20 |
| **合计** | **~460** |

---

## 5. 验收标准

1. [ ] 全部 7 个集成测试通过（`--test-threads=1`）
2. [ ] 配置迁移: v1→v2, disabled_targets 迁移, custom_exclude_paths 保留
3. [ ] 保护路径: 扫描不发现, 删除被后端拒绝
4. [ ] 取消后: 锁释放, 可重新发起扫描
5. [ ] 重入: 第二次 start_scan 返回 Err
6. [ ] ENV 注入: 伪造 SystemRoot 不泄露真实系统文件
7. [ ] 操作日志: 每次清理后 clean_log.jsonl 追加 + stats 更新
8. [ ] Tauri 命令测试: 标注 feature flag + 已知 bug 注释
9. [ ] CI: unit + integration 分 workflow 并行
