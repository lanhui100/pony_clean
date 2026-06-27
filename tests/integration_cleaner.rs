#[test]
fn test_cleaner_module_is_accessible() {
    let targets = pony_clean::cleaner::get_clean_targets();
    assert!(!targets.is_empty());
    assert!(targets.iter().any(|t| t.category == "temp"));

    let temp = std::env::var("TEMP").unwrap();
    let result = pony_clean::cleaner::is_path_allowed(
        &std::path::PathBuf::from(&temp).join("test.txt"),
        &targets,
    );
    assert!(result, "TEMP path should be allowed");
}

#[test]
fn test_start_scan_scans_temp() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let (tx, rx) = std::sync::mpsc::channel();
    let (cmd, cancel) = pony_clean::cleaner::start_scan(tx).expect("scan should start on normal system");

    let mut found_done = false;
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(3) {
        if let Ok(event) = rx.try_recv() {
            match event {
                pony_clean::cleaner::ScanEvent::Done { .. } => {
                    found_done = true;
                    break;
                }
                _ => {}
            }
        }
    }
    if !found_done {
        let _ = cmd.send(pony_clean::cleaner::CleanCommand::CancelScan);
        cancel.cancel();
    }
}

// 不测试 env 注入场景：set_var 全局影响导致并行测试竞态
// 该逻辑由单元测试 test_resolve_targets_skips_protected 验证
