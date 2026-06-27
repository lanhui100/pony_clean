mod common;

#[test]
fn test_monitor_module_is_accessible() {
    common::init_logging();
    let msg = pony_clean::monitor::placeholder();
    assert!(!msg.is_empty());
    assert!(msg.contains("monitor"));
}
