#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod theme;

use app::PonyCleanApp;
use tracing_subscriber::EnvFilter;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {info}");
        tracing::error!("PANIC: {info}");
    }));

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let rt = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to create tokio runtime: {e}");
            eprintln!("FATAL: Failed to create tokio runtime: {e}");
            std::process::exit(1);
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 600.0])
            .with_always_on_top()
            .with_transparent(true),
        ..Default::default()
    };

    match eframe::run_native(
        "PonyClean",
        options,
        Box::new(|cc| {
            app::setup_fonts(&cc.egui_ctx);
            Box::new(PonyCleanApp::new(rt))
        }),
    ) {
        Ok(()) => tracing::info!("PonyClean exited normally"),
        Err(e) => {
            tracing::error!("eframe error: {e}");
            eprintln!("FATAL: eframe error: {e}");
            std::process::exit(1);
        }
    }
}
