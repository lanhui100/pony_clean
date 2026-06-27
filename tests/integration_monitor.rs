#[test]
fn test_monitor_module_is_accessible() {
    let pi = pony_clean::monitor::ProcessInfo {
        pid: 1,
        name: "test".into(),
        cpu: 0.0,
        mem_mb: 0.0,
        status: "Running".into(),
    };
    assert_eq!(pi.pid, 1);
    assert!(pi.to_string().contains("test"));

    let mut list = vec![pi];
    pony_clean::monitor::sort_processes(&mut list);
    assert_eq!(list.len(), 1);
}

#[test]
fn test_start_produces_snapshot() {
    let (tx, rx) = std::sync::mpsc::channel();
    let (_cmd_tx, _handle) = pony_clean::monitor::start(tx);
    let snapshot = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("should receive a snapshot within 5s");
    assert!(
        snapshot.summary.process_count > 0,
        "should see at least one process on a running system"
    );
}

#[test]
fn test_start_shutdown() {
    let (tx, rx) = std::sync::mpsc::channel();
    let (cmd_tx, _handle) = pony_clean::monitor::start(tx);
    let _ = cmd_tx.send(pony_clean::monitor::MonitorCommand::Shutdown);
    // 发送 Shutdown 后 channel 应关闭，recv 返回 Err
    use std::sync::mpsc::RecvTimeoutError;
    let result = rx.recv_timeout(std::time::Duration::from_secs(3));
    assert!(
        matches!(result, Err(RecvTimeoutError::Disconnected)),
        "channel should be disconnected after shutdown, got: {result:?}"
    );
}
