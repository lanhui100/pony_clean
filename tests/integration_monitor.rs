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
