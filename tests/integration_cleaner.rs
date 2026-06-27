mod common;

#[test]
fn test_cleaner_module_is_accessible() {
    common::init_logging();
    let msg = pony_clean::cleaner::placeholder();
    assert!(!msg.is_empty());
    assert!(msg.contains("cleaner"));
}
