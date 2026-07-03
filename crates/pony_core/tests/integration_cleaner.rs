mod common;

use pony_core::cleaner::{self, Category, SafetyLevel, ScanTarget};

/// 测试 resolve_targets 在有 mock 环境时不 panic，能正确展开路径
#[test]
fn test_resolve_targets_basic() {
    let env = common::TestEnv::new();
    env.create_windows_tree();
    env.apply_env();

    let targets = vec![
        ScanTarget::new("t1", "%TEMP%", SafetyLevel::Safe, Category::Temp, ""),
        ScanTarget::new(
            "t2",
            "%WINDIR%\\Temp",
            SafetyLevel::Confirm,
            Category::Temp,
            "",
        ),
    ];
    let resolved = cleaner::resolve_targets(&targets);
    assert!(!resolved.is_empty(), "should resolve at least TEMP");
    assert!(
        resolved
            .iter()
            .any(|(p, _)| p.to_string_lossy().to_lowercase().contains("temp"))
    );
}

/// 测试 resolve_targets 排除 Forbidden 级别
#[test]
fn test_resolve_targets_excludes_forbidden() {
    let targets = vec![
        ScanTarget::new("t1", "%TEMP%", SafetyLevel::Safe, Category::Temp, ""),
        ScanTarget::new(
            "t2",
            "%WINDIR%\\System32",
            SafetyLevel::Forbidden,
            Category::Temp,
            "",
        ),
    ];
    let resolved = cleaner::resolve_targets(&targets);
    assert!(!resolved.is_empty());
    assert!(
        !resolved
            .iter()
            .any(|(p, _)| p.to_string_lossy().contains("System32"))
    );
}

/// 测试 get_clean_targets 返回 60 个目标
#[test]
fn test_get_clean_targets_count() {
    let targets = cleaner::get_clean_targets();
    assert_eq!(
        targets.len(),
        60,
        "should have 60 targets (43 original + 17 new)"
    );
}

/// 测试 target id 唯一性
#[test]
fn test_target_ids_unique() {
    let targets = cleaner::get_clean_targets();
    let ids: Vec<&str> = targets.iter().map(|t| t.id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "target ids must be unique");
}

/// 测试 load_config 返回默认值且不 panic
#[test]
fn test_load_config_default() {
    let config = cleaner::load_config();
    // 默认配置应与 v3 格式一致
    assert!(config.version.is_none() || config.version == Some(3));
}

/// 测试 delete_files 拒绝保护路径
#[test]
fn test_delete_rejects_protected() {
    let result =
        cleaner::delete_files(&[std::path::PathBuf::from(r"C:\Windows\System32\config\SAM")]);
    assert_eq!(result.success, 0);
    assert!(
        result
            .errors
            .iter()
            .any(|e| e.contains("Protected") || e.contains("SAM"))
    );
}
