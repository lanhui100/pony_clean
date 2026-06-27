#[allow(dead_code)]
pub fn init_logging() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
}
