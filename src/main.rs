#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use app::PonyCleanApp;
use tracing_subscriber::EnvFilter;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    // App 内部通过 self.rt.spawn() 使用 runtime，无需 enter guard

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 600.0])
            .with_always_on_top()
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "PonyClean",
        options,
        Box::new(|_cc| Box::new(PonyCleanApp::new(rt))),
    )
}
