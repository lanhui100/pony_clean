use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui::{self, Color32, Rounding, Frame, Stroke};

use pony_clean::cleaner;
use pony_clean::monitor;

const CARD_BG: Color32 = Color32::from_rgba_premultiplied(28, 32, 38, 200);
const CARD_ROUNDING: f32 = 12.0;
const TEXT_MAIN: Color32 = Color32::from_rgb(220, 222, 228);
const TEXT_MUTED: Color32 = Color32::from_rgb(148, 155, 164);
const COLOR_ALERT: Color32 = Color32::from_rgb(207, 102, 102);

#[derive(PartialEq)]
enum Tab { Monitor, Cleaner }

enum ScanState {
    Idle,
    Scanning { cancel_token: tokio_util::sync::CancellationToken, scanned: u64, current: String },
    Done { items: Vec<cleaner::CleanItem>, checked: HashSet<PathBuf>, total_bytes: u64 },
    Cancelled,
    Error(String),
}

pub struct PonyCleanApp {
    _rt: tokio::runtime::Runtime,

    monitor_rx: Option<mpsc::Receiver<monitor::Snapshot>>,
    monitor_cmd_tx: Option<tokio::sync::mpsc::Sender<monitor::MonitorCommand>>,
    latest_snapshot: Option<monitor::Snapshot>,
    pending_kill: Option<tokio::sync::oneshot::Receiver<Result<(), String>>>,
    kill_feedback: Option<String>,

    clean_cmd_tx: Option<mpsc::Sender<cleaner::CleanCommand>>,
    clean_rx: Option<mpsc::Receiver<cleaner::ScanEvent>>,
    scan_state: ScanState,
    scan_start_time: Option<Instant>,

    selected_tab: Tab,
}

impl PonyCleanApp {
    pub fn new(rt: tokio::runtime::Runtime) -> Self {
        let _guard = rt.enter();

        let (monitor_tx, monitor_rx) = mpsc::channel();
        let cmd_tx = monitor::start(monitor_tx);

        Self {
            _rt: rt,
            monitor_rx: Some(monitor_rx),
            monitor_cmd_tx: Some(cmd_tx),
            latest_snapshot: None,
            pending_kill: None,
            kill_feedback: None,
            clean_cmd_tx: None,
            clean_rx: None,
            scan_state: ScanState::Idle,
            scan_start_time: None,
            selected_tab: Tab::Monitor,
        }
    }

    fn drain_channels(&mut self, ctx: &egui::Context) -> bool {
        let mut new_data = false;

        if let Some(rx) = &self.monitor_rx {
            while let Ok(snapshot) = rx.try_recv() {
                self.latest_snapshot = Some(snapshot);
                new_data = true;
            }
        }

        // 提取 cleaner 事件再处理，避免借用冲突
        let events: Vec<cleaner::ScanEvent> = self.clean_rx.as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            self.handle_scan_event(event);
            new_data = true;
        }

        if let Some(rx) = &mut self.pending_kill {
            if let Ok(result) = rx.try_recv() {
                self.kill_feedback = Some(match result {
                    Ok(()) => "✓ 进程已终止".to_string(),
                    Err(e) => format!("✗ {e}"),
                });
                self.pending_kill = None;
                new_data = true;
            }
        }

        if new_data {
            ctx.request_repaint();
        }
        new_data
    }

    fn handle_scan_event(&mut self, event: cleaner::ScanEvent) {
        match event {
            cleaner::ScanEvent::Progress { scanned: n, current: c } => {
                if let ScanState::Scanning { ref mut scanned, ref mut current, .. } = self.scan_state {
                    *scanned = n;
                    *current = c;
                }
                self.scan_start_time = Some(Instant::now());
            }
            cleaner::ScanEvent::ItemsFound { items: new_items, .. } => {
                if let ScanState::Scanning { .. } = self.scan_state {
                    // 合并到 Done 中暂存，批次自动 append
                    let old_checked = if let ScanState::Done { checked, .. } =
                        std::mem::replace(&mut self.scan_state, ScanState::Idle)
                    {
                        checked
                    } else {
                        HashSet::new()
                    };

                    let bytes: u64 = new_items.iter().map(|i| i.size_bytes).sum();
                    self.scan_state = ScanState::Done {
                        items: new_items,
                        checked: old_checked,
                        total_bytes: bytes,
                    };
                }
            }
            cleaner::ScanEvent::Done { total_items: _total_items, total_bytes } => {
                if let ScanState::Done { items, checked, .. } =
                    std::mem::replace(&mut self.scan_state, ScanState::Idle)
                {
                    self.scan_state = ScanState::Done {
                        items,
                        checked,
                        total_bytes,
                    };
                }
                self.scan_start_time = None;
            }
            cleaner::ScanEvent::Cancelled => {
                self.scan_state = ScanState::Cancelled;
                self.scan_start_time = None;
            }
            cleaner::ScanEvent::Warning(msg) => {
                tracing::warn!("Scan warning: {msg}");
            }
        }
    }

    fn render_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(Frame::none())
            .show(ctx, |ui| {
                ui.style_mut().visuals.window_fill = Color32::TRANSPARENT;
                ui.style_mut().visuals.panel_fill = Color32::TRANSPARENT;

                // 标题栏 + 拖动
                let title_bar = egui::TopBottomPanel::top("title_bar")
                    .frame(Frame::none())
                    .min_height(32.0);
                title_bar.show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("PonyClean").color(TEXT_MAIN).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("×").clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    });
                    // 拖动
                    let resp = ui.interact(
                        ui.min_rect(),
                        ui.id().with("drag"),
                        egui::Sense::click(),
                    );
                    if resp.drag_started() || resp.dragged() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                });

                // Tab 切换
                egui::TopBottomPanel::top("tab_bar")
                    .frame(Frame::none())
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            let tabs = [("进程监控", Tab::Monitor), ("C盘清理", Tab::Cleaner)];
                            for (label, tab) in &tabs {
                                let selected = self.selected_tab == *tab;
                                if ui.selectable_label(selected, *label).clicked() {
                                    self.selected_tab = Tab::Monitor;
                                    if *tab == Tab::Cleaner {
                                        self.selected_tab = Tab::Cleaner;
                                    }
                                }
                            }
                        });
                    });

                // 内容区
                Frame::none()
                    .fill(CARD_BG)
                    .rounding(Rounding::same(CARD_ROUNDING))
                    .stroke(Stroke::NONE)
                    .show(ui, |ui| {
                        ui.set_min_height(ui.available_height() - 16.0);
                        match self.selected_tab {
                            Tab::Monitor => self.render_monitor_panel(ui),
                            Tab::Cleaner => self.render_cleaner_panel(ui),
                        }
                    });
            });
    }

    fn render_monitor_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(snapshot) = &self.latest_snapshot {
            let s = &snapshot.summary;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!(
                    "CPU: {:.1}%  内存: {:.1}GB / {:.1}GB",
                    s.cpu_total, s.mem_used_mb / 1024.0, s.mem_total_mb / 1024.0,
                )).color(TEXT_MAIN));
            });
        }

        if let Some(feedback) = &self.kill_feedback {
            ui.label(egui::RichText::new(feedback).color(COLOR_ALERT));
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("process_grid")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Name").color(TEXT_MUTED).strong());
                    ui.label(egui::RichText::new("CPU%").color(TEXT_MUTED).strong());
                    ui.label(egui::RichText::new("Mem").color(TEXT_MUTED).strong());
                    ui.label(egui::RichText::new("").color(TEXT_MUTED));
                    ui.end_row();

                    if let Some(snapshot) = &self.latest_snapshot {
                        let threshold_cpu = 80.0;
                        let threshold_mem = 500.0;
                        for p in &snapshot.processes {
                            let is_high = p.cpu > threshold_cpu || p.mem_mb > threshold_mem;
                            let name_color = if is_high { COLOR_ALERT } else { TEXT_MAIN };

                            ui.label(egui::RichText::new(&p.name).color(name_color));
                            ui.label(egui::RichText::new(format!("{:.1}", p.cpu)).color(name_color));
                            ui.label(egui::RichText::new(format!("{:.0}MB", p.mem_mb)).color(name_color));

                            if ui.button("✕").clicked() {
                                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                                if let Some(tx) = &self.monitor_cmd_tx {
                                    let _ = tx.try_send(monitor::MonitorCommand::Kill {
                                        pid: p.pid,
                                        name: p.name.clone(),
                                        resp: resp_tx,
                                    });
                                }
                                self.pending_kill = Some(resp_rx);
                                self.kill_feedback = None;
                            }
                            ui.end_row();
                        }
                    }
                });
        });
    }

    fn render_cleaner_panel(&mut self, ui: &mut egui::Ui) {
        // 判断当前状态并渲染对应的 UI，用 flag 避免在闭包中 borrow self
        let mut should_start_scan = false;
        let mut should_cancel_scan = false;
        let mut should_reset = false;
        let mut execute_delete = Vec::new();

        match &mut self.scan_state {
            &mut ScanState::Idle => {
                if ui.button("开始扫描").clicked() {
                    should_start_scan = true;
                }
            }
            &mut ScanState::Scanning { ref scanned, ref current, ref cancel_token } => {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("扫描中... ({scanned} files)")).color(TEXT_MUTED));
                    if ui.button("取消").clicked() {
                        should_cancel_scan = true;
                    }
                });
                ui.label(egui::RichText::new(current.as_str()).color(TEXT_MUTED));
                if should_cancel_scan {
                    cancel_token.cancel();
                }
            }
            &mut ScanState::Done { ref items, ref mut checked, ref total_bytes } => {
                let mb = *total_bytes as f64 / (1024.0 * 1024.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("总计可释放: {mb:.1}MB")).color(TEXT_MAIN));
                    if ui.button("清理选中").clicked() {
                        execute_delete = items.iter()
                            .filter(|i| checked.contains(&i.path))
                            .map(|i| i.path.clone())
                            .collect();
                    }
                    if ui.button("重新扫描").clicked() {
                        should_start_scan = true;
                    }
                });

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for item in items {
                        let is_checked = checked.contains(&item.path);
                        let mb2 = item.size_bytes as f64 / (1024.0 * 1024.0);
                        let text = format!("{:.1}MB  {}", mb2, item.path.display());

                        ui.horizontal(|ui| {
                            let mut checked_state = is_checked;
                            ui.checkbox(&mut checked_state, "");
                            ui.label(egui::RichText::new(&text).color(TEXT_MAIN));

                            if checked_state && !is_checked {
                                checked.insert(item.path.clone());
                            } else if !checked_state && is_checked {
                                checked.remove(&item.path);
                            }
                        });
                    }
                });
            }
            &mut ScanState::Cancelled => {
                ui.label(egui::RichText::new("扫描已取消").color(TEXT_MUTED));
                if ui.button("重新扫描").clicked() {
                    should_start_scan = true;
                }
            }
            &mut ScanState::Error(ref msg) => {
                ui.label(egui::RichText::new(format!("扫描失败: {msg}")).color(COLOR_ALERT));
                if ui.button("重试").clicked() {
                    should_start_scan = true;
                }
            }
        }

        if !execute_delete.is_empty() {
            cleaner::delete_files(&execute_delete);
            self.scan_state = ScanState::Idle;
            should_reset = true;
        }
        if should_cancel_scan {
            // cancel was already done above via cancel_token
        }
        if should_start_scan {
            self.start_scan();
        }
        if should_reset {
            self.scan_state = ScanState::Idle;
        }
    }

    fn start_scan(&mut self) {
        let (tx, rx) = mpsc::channel();
        match cleaner::start_scan(tx) {
            Ok((cmd, cancel_token)) => {
                self.clean_cmd_tx = Some(cmd);
                self.clean_rx = Some(rx);
                self.scan_state = ScanState::Scanning {
                    cancel_token,
                    scanned: 0,
                    current: "Starting...".into(),
                };
                self.scan_start_time = Some(Instant::now());
            }
            Err(e) => {
                self.scan_state = ScanState::Error(e);
            }
        }
    }
}

impl eframe::App for PonyCleanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 检查 scan timeout (120s)
        if let ScanState::Scanning { .. } = &self.scan_state {
            if let Some(start) = self.scan_start_time {
                if start.elapsed().as_secs() > 120 {
                    self.scan_state = ScanState::Error("扫描超时".into());
                    self.scan_start_time = None;
                    ctx.request_repaint();
                }
            }
        }

        self.drain_channels(ctx);
        self.render_ui(ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}

impl Drop for PonyCleanApp {
    fn drop(&mut self) {
        if let ScanState::Scanning { cancel_token, .. } = &self.scan_state {
            cancel_token.cancel();
        }
        if let Some(tx) = &self.monitor_cmd_tx {
            let _ = tx.try_send(monitor::MonitorCommand::Shutdown);
        }
        if let Some(tx) = &self.clean_cmd_tx {
            let _ = tx.send(cleaner::CleanCommand::Shutdown);
        }
        // rt drop 会终止所有 spawned tasks
    }
}
