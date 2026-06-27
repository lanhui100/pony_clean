use std::sync::OnceLock;

static TRACING_INIT: OnceLock<()> = OnceLock::new();

pub fn init_logging() {
    TRACING_INIT.get_or_init(|| {
        tracing_subscriber::fmt().with_test_writer().init();
    });
}
