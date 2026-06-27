//! cleaner_probe: 验证 C盘清理模块的独立入口
//!
//! 扫描安全路径并打印可清理项列表。dry-run 模式，不执行删除。

use std::sync::mpsc;
use pony_clean::cleaner::{self, ScanEvent};

fn main() {
    let (tx, rx) = mpsc::channel::<ScanEvent>();
    let _rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");

    match cleaner::start_scan(tx) {
        Err(e) => {
            eprintln!("Failed to start scan: {e}");
            return;
        }
        Ok((_cmd_tx, _cancel_token)) => {
            println!("PonyClean Cleaner Probe — 扫描安全路径...\n");
            for event in rx {
                match event {
                    ScanEvent::Progress { scanned, current } => {
                        println!("  Scanned {scanned} files... ({current})");
                    }
                    ScanEvent::ItemsFound { items, .. } => {
                        for item in &items {
                            let mb = item.size_bytes as f64 / (1024.0 * 1024.0);
                            println!("  [{:>8.2}MB] {}", mb, item.path.display());
                        }
                    }
                    ScanEvent::Done { total_items, total_bytes } => {
                        let mb = total_bytes as f64 / (1024.0 * 1024.0);
                        println!("\nScan complete: {total_items} files, {mb:.2}MB found");
                        break;
                    }
                    ScanEvent::Cancelled => {
                        println!("\nScan cancelled");
                        break;
                    }
                    ScanEvent::Warning(msg) => {
                        eprintln!("Warning: {msg}");
                    }
                }
            }
        }
    }
}
