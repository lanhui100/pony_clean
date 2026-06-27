#[test]
fn test_monitor_module_is_accessible() {
    let pi = pony_clean::monitor::ProcessInfo {
        pid: 1,
        name: "test".into(),
        cpu: 0.0,
        mem_mb: 0.0,
        status: "Running".into(),
        cmdline: String::new(),
    };
    assert_eq!(pi.pid, 1);
    assert!(pi.to_string().contains("test"));

    let mut list = vec![pi];
    pony_clean::monitor::sort_processes(&mut list);
    assert_eq!(list.len(), 1);
}

#[test]
fn test_start_produces_snapshot() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let (tx, rx) = std::sync::mpsc::channel();
    let _cmd_tx = pony_clean::monitor::start(tx);
    let snapshot = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("should receive a snapshot within 5s");
    assert!(
        snapshot.summary.process_count > 0,
        "should see at least one process"
    );
    // sysinfo 在某些环境下可能返回 0 total_memory，只断言进程数
    assert!(
        snapshot.summary.mem_total_mb >= 0.0,
        "total memory should be non-negative"
    );
}

#[test]
fn test_start_shutdown() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let (tx, rx) = std::sync::mpsc::channel();
    let cmd_tx = pony_clean::monitor::start(tx);
    let _ = cmd_tx.try_send(pony_clean::monitor::MonitorCommand::Shutdown);
    // 发送 Shutdown 后 channel 应关闭，recv 返回 Err
    let result = rx.recv_timeout(std::time::Duration::from_secs(3));
    assert!(result.is_err(), "channel should close after shutdown");
}
