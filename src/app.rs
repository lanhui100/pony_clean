use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui::{self, Color32, Frame, Stroke};

use pony_clean::cleaner;
use pony_clean::monitor;

use crate::theme::Theme;

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let font_dir = format!("{windir}\\Fonts");

    let cjk_candidates = [
        ("msyh.ttc", "msyh-regular"),
        ("simsun.ttc", "simsun"),
        ("simhei.ttf", "simhei"),
    ];

    for (file, key) in &cjk_candidates {
        let path = format!("{font_dir}\\{file}");
        if let Ok(data) = std::fs::read(&path) {
            fonts
                .font_data
                .insert(key.to_string(), egui::FontData::from_owned(data));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push(key.to_string());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(key.to_string());
            ctx.set_fonts(fonts);
            return;
        }
    }

    tracing::warn!("No CJK font found, Chinese text may show as boxes");
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Monitor,
    Cleaner,
}

enum ScanState {
    Idle,
    Scanning {
        cancel_token: tokio_util::sync::CancellationToken,
        scanned: u64,
        current: String,
        accumulated_items: Vec<cleaner::CleanItem>,
        accumulated_bytes: u64,
        checked: HashSet<PathBuf>,
    },
    Deleting {
        rx: std::sync::mpsc::Receiver<()>,
    },
    Done {
        items: Vec<cleaner::CleanItem>,
        checked: HashSet<PathBuf>,
        total_bytes: u64,
    },
    Cancelled,
    Error(String),
}

enum MonitorState {
    Connected,
    Disconnected,
}

#[derive(Clone, Copy, PartialEq)]
enum SortField {
    Name,
    Cpu,
    Mem,
}

struct SortState {
    field: SortField,
    ascending: bool,
}

pub struct PonyCleanApp {
    theme: Theme,
    _rt: tokio::runtime::Runtime,

    monitor_rx: Option<mpsc::Receiver<monitor::Snapshot>>,
    monitor_cmd_tx: Option<mpsc::Sender<monitor::MonitorCommand>>,
    #[allow(dead_code)]
    monitor_thread: Option<std::thread::JoinHandle<()>>,
    monitor_state: MonitorState,
    latest_snapshot: Option<monitor::Snapshot>,
    pending_kill: Option<tokio::sync::oneshot::Receiver<Result<(), String>>>,
    kill_feedback: Option<String>,

    clean_cmd_tx: Option<mpsc::Sender<cleaner::CleanCommand>>,
    clean_rx: Option<mpsc::Receiver<cleaner::ScanEvent>>,
    scan_state: ScanState,
    scan_start_time: Option<Instant>,

    selected_tab: Tab,

    process_search: String,
    sort_state: SortState,
}

impl PonyCleanApp {
    pub fn new(rt: tokio::runtime::Runtime) -> Self {
        let (monitor_tx, monitor_rx) = mpsc::channel();
        let (cmd_tx, handle) = monitor::start(monitor_tx);

        Self {
            theme: Theme::dark(),
            _rt: rt,
            monitor_rx: Some(monitor_rx),
            monitor_cmd_tx: Some(cmd_tx),
            monitor_thread: Some(handle),
            monitor_state: MonitorState::Connected,
            latest_snapshot: None,
            pending_kill: None,
            kill_feedback: None,
            clean_cmd_tx: None,
            clean_rx: None,
            scan_state: ScanState::Idle,
            scan_start_time: None,
            selected_tab: Tab::Monitor,

            process_search: String::new(),
            sort_state: SortState {
                field: SortField::Cpu,
                ascending: false,
            },
        }
    }

    fn drain_channels(&mut self, ctx: &egui::Context) -> bool {
        let mut new_data = false;

        if let Some(rx) = &self.monitor_rx {
            while let Ok(snapshot) = rx.try_recv() {
                self.latest_snapshot = Some(snapshot);
                self.monitor_state = MonitorState::Connected;
                new_data = true;
            }
            // 检查 channel 是否断开（monitor task 已退出）
            if matches!(rx.try_recv(), Err(mpsc::TryRecvError::Disconnected)) {
                if let MonitorState::Connected = self.monitor_state {
                    self.monitor_state = MonitorState::Disconnected;
                    new_data = true;
                }
            }
        }

        // 提取 cleaner 事件再处理，避免借用冲突
        let events: Vec<cleaner::ScanEvent> = self
            .clean_rx
            .as_ref()
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
            cleaner::ScanEvent::Progress {
                scanned: n,
                current: c,
            } => {
                if let ScanState::Scanning {
                    ref mut scanned,
                    ref mut current,
                    ..
                } = self.scan_state
                {
                    *scanned = n;
                    *current = c;
                }
                self.scan_start_time = Some(Instant::now());
            }
            cleaner::ScanEvent::ItemsFound { items, .. } => {
                if let ScanState::Scanning {
                    ref mut accumulated_items,
                    ref mut accumulated_bytes,
                    ..
                } = self.scan_state
                {
                    let batch_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();
                    accumulated_items.extend(items);
                    *accumulated_bytes += batch_bytes;
                }
            }
            cleaner::ScanEvent::Done {
                total_items: _total_items,
                total_bytes,
            } => {
                match std::mem::replace(&mut self.scan_state, ScanState::Idle) {
                    ScanState::Done {
                        items, mut checked, ..
                    } => {
                        let valid: HashSet<PathBuf> =
                            items.iter().map(|i| i.path.clone()).collect();
                        checked.retain(|p| valid.contains(p));
                        self.scan_state = ScanState::Done {
                            items,
                            checked,
                            total_bytes,
                        };
                    }
                    ScanState::Scanning {
                        accumulated_items,
                        mut checked,
                        ..
                    } => {
                        let valid: HashSet<PathBuf> =
                            accumulated_items.iter().map(|i| i.path.clone()).collect();
                        checked.retain(|p| valid.contains(p));
                        self.scan_state = ScanState::Done {
                            items: accumulated_items,
                            checked,
                            total_bytes,
                        };
                    }
                    _ => {
                        self.scan_state = ScanState::Done {
                            items: vec![],
                            checked: HashSet::new(),
                            total_bytes: 0,
                        };
                    }
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
        egui::TopBottomPanel::top("title_bar")
            .frame(Frame::none())
            .min_height(32.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("PonyClean")
                            .color(self.theme.text_primary)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("×").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
                let resp = ui.interact(
                    ui.min_rect(),
                    ui.id().with("drag"),
                    egui::Sense::click_and_drag(),
                );
                if resp.drag_started() || resp.dragged() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            });

        egui::TopBottomPanel::top("tab_bar")
            .frame(Frame::none())
            .min_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let tabs = [("进程监控", Tab::Monitor), ("C盘清理", Tab::Cleaner)];
                    for (label, tab) in &tabs {
                        let selected = self.selected_tab == *tab;
                        let mut lbl = egui::RichText::new(*label).size(14.0);
                        if selected {
                            lbl = lbl.color(self.theme.accent_blue).strong();
                        } else {
                            lbl = lbl.color(self.theme.text_secondary);
                        }
                        if ui.selectable_label(selected, lbl).clicked() {
                            self.selected_tab = *tab;
                        }
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(Frame::none())
            .show(ctx, |ui| {
                ui.style_mut().visuals.window_fill = self.theme.bg_window;
                ui.style_mut().visuals.panel_fill = self.theme.bg_window;

                Frame::none()
                    .fill(self.theme.bg_card)
                    .rounding(self.theme.radius_md)
                    .stroke(Stroke::NONE)
                    .inner_margin(egui::Margin::same(8.0))
                    .show(ui, |ui| match self.selected_tab {
                        Tab::Monitor => self.render_monitor_panel(ui, ctx),
                        Tab::Cleaner => self.render_cleaner_panel(ui),
                    });
            });
    }

    fn render_monitor_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if matches!(self.monitor_state, MonitorState::Disconnected) {
            ui.label(egui::RichText::new("未连接 — 监控已停止").color(self.theme.accent_red));
            return;
        }

        let snapshot_data = self.latest_snapshot.as_ref().map(|s| {
            let processes: Vec<monitor::ProcessInfo> = s.processes.clone();
            let cpu_sum: f32 = processes.iter().map(|p| p.cpu).sum();
            (
                cpu_sum,
                s.summary.mem_used_mb,
                s.summary.mem_total_mb,
                s.summary.process_count,
                processes,
            )
        });

        let Some((cpu_sum, mem_used_mb, mem_total_mb, proc_count, all_processes)) = snapshot_data
        else {
            ui.label(egui::RichText::new("等待数据...").color(self.theme.text_secondary));
            return;
        };

        let mem_used = mem_used_mb / 1024.0;
        let mem_total = mem_total_mb / 1024.0;

        // ── 紧凑摘要行 ──
        ui.horizontal(|ui| {
            let cpu_color = if cpu_sum > 80.0 {
                self.theme.accent_red
            } else if cpu_sum > 50.0 {
                self.theme.accent_amber
            } else {
                self.theme.accent_blue
            };
            ui.label(
                egui::RichText::new(format!("CPU: {cpu_sum:.1}%"))
                    .color(cpu_color)
                    .size(13.0)
                    .strong(),
            );
            ui.separator();
            let mem_color = if mem_used / mem_total.max(1.0) > 0.85 {
                self.theme.accent_red
            } else if mem_used / mem_total.max(1.0) > 0.65 {
                self.theme.accent_amber
            } else {
                self.theme.accent_teal
            };
            ui.label(
                egui::RichText::new(format!("内存: {mem_used:.1}/{mem_total:.0}GB"))
                    .color(mem_color)
                    .size(13.0),
            );
            ui.separator();
            ui.label(
                egui::RichText::new(format!("进程: {proc_count}"))
                    .color(self.theme.text_secondary)
                    .size(13.0),
            );
        });

        // ── 搜索框 ──
        ui.add_space(6.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.process_search)
                .hint_text("🔍 搜索进程...")
                .char_limit(64)
                .desired_width(f32::INFINITY),
        );

        // ── Kill 反馈 ──
        if self.kill_feedback.is_some() {
            if let Some(feedback) = self.kill_feedback.take() {
                ui.label(egui::RichText::new(feedback).color(self.theme.accent_red));
                ctx.request_repaint();
            }
        }

        // ── 过滤 (CPU>10% or MEM>200MB) + 排序 ──
        let lower_query = self.process_search.to_lowercase();
        let mut display: Vec<&monitor::ProcessInfo> = all_processes
            .iter()
            .filter(|p| {
                if !lower_query.is_empty() {
                    p.name.to_lowercase().contains(&lower_query)
                } else {
                    p.cpu > 10.0 || p.mem_mb > 200.0
                }
            })
            .collect();

        let sort_cmp = |a: f32, b: f32| -> std::cmp::Ordering {
            match (a.is_nan(), b.is_nan()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal),
            }
        };

        match self.sort_state.field {
            SortField::Name => display.sort_by_key(|a| a.name.to_lowercase()),
            SortField::Cpu => display.sort_by(|a, b| sort_cmp(a.cpu, b.cpu)),
            SortField::Mem => display.sort_by(|a, b| sort_cmp(a.mem_mb as f32, b.mem_mb as f32)),
        }
        if self.sort_state.ascending {
            display.reverse();
        }

        // ── 进程表格 (Grid 自动对齐) ──
        let t = &self.theme;
        let sort_mark = |field: SortField, state: &SortState| -> &'static str {
            if state.field == field {
                if state.ascending { " ▲" } else { " ▼" }
            } else {
                ""
            }
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("proc_grid")
                .striped(true)
                .spacing([2.0, 0.0])
                .min_col_width(20.0)
                .show(ui, |ui| {
                    // 列头
                    let hdr = |text: &str| {
                        egui::RichText::new(text)
                            .color(t.text_secondary)
                            .size(11.0)
                            .strong()
                    };
                    if ui
                        .selectable_label(
                            self.sort_state.field == SortField::Name,
                            hdr(&format!(
                                "Name{}",
                                sort_mark(SortField::Name, &self.sort_state)
                            )),
                        )
                        .clicked()
                    {
                        self.sort_state = SortState {
                            field: SortField::Name,
                            ascending: !self.sort_state.ascending,
                        };
                    }
                    if ui
                        .selectable_label(
                            self.sort_state.field == SortField::Cpu,
                            hdr(&format!(
                                "CPU{}",
                                sort_mark(SortField::Cpu, &self.sort_state)
                            )),
                        )
                        .clicked()
                    {
                        self.sort_state = SortState {
                            field: SortField::Cpu,
                            ascending: !self.sort_state.ascending,
                        };
                    }
                    if ui
                        .selectable_label(
                            self.sort_state.field == SortField::Mem,
                            hdr(&format!(
                                "Mem{}",
                                sort_mark(SortField::Mem, &self.sort_state)
                            )),
                        )
                        .clicked()
                    {
                        self.sort_state = SortState {
                            field: SortField::Mem,
                            ascending: !self.sort_state.ascending,
                        };
                    }
                    ui.label(hdr("Mem%"));
                    ui.label(hdr(""));
                    ui.end_row();

                    // 空搜索提示
                    if display.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "没有进程匹配 \"{q}\"",
                                q = self.process_search
                            ))
                            .color(t.text_muted)
                            .size(11.0),
                        );
                        ui.end_row();
                    }

                    // 数据行
                    for p in &display {
                        let mem_pct = if mem_total_mb > 0.0 {
                            p.mem_mb / mem_total_mb * 100.0
                        } else {
                            0.0
                        };

                        let highlight = p.cpu > 10.0 || p.mem_mb > 200.0;
                        let c = || {
                            if highlight {
                                t.text_primary
                            } else {
                                t.text_secondary
                            }
                        };
                        let cpu_c = || {
                            if p.cpu > 80.0 {
                                t.accent_red
                            } else if p.cpu > 50.0 {
                                t.accent_amber
                            } else {
                                c()
                            }
                        };

                        let mem_label = if p.mem_mb > 1024.0 {
                            format!("{:.1}GB", p.mem_mb / 1024.0)
                        } else {
                            format!("{:.0}MB", p.mem_mb)
                        };

                        ui.label(egui::RichText::new(&p.name).color(c()).size(11.0));
                        ui.label(
                            egui::RichText::new(format!("{:.1}", p.cpu))
                                .color(cpu_c())
                                .size(11.0),
                        );
                        ui.label(egui::RichText::new(&mem_label).color(c()).size(11.0));
                        ui.label(
                            egui::RichText::new(format!("{:.0}%", mem_pct))
                                .color(t.text_muted)
                                .size(11.0),
                        );
                        if ui.button("×").clicked() {
                            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                            if let Some(tx) = &self.monitor_cmd_tx {
                                if tx
                                    .send(monitor::MonitorCommand::Kill {
                                        pid: p.pid,
                                        name: p.name.clone(),
                                        resp: resp_tx,
                                    })
                                    .is_err()
                                {
                                    tracing::warn!("Kill command dropped: monitor disconnected");
                                }
                            }
                            self.pending_kill = Some(resp_rx);
                            self.kill_feedback = None;
                        }
                        ui.end_row();
                    }
                });
        });
    }

    fn render_cleaner_panel(&mut self, ui: &mut egui::Ui) {
        let mut should_start_scan = false;
        let mut should_cancel_scan = false;
        let mut execute_delete = Vec::new();

        match &mut self.scan_state {
            ScanState::Idle => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("C盘安全清理")
                            .color(self.theme.text_primary)
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("扫描临时文件、浏览器缓存、回收站等可清理项")
                            .color(self.theme.text_secondary)
                            .size(12.0),
                    );
                    ui.add_space(16.0);
                    let btn = egui::Button::new(egui::RichText::new(" 开始扫描 ").size(16.0))
                        .fill(self.theme.accent_blue)
                        .min_size(egui::vec2(140.0, 40.0));
                    if ui.add(btn).clicked() {
                        should_start_scan = true;
                    }
                });
            }
            ScanState::Scanning {
                scanned,
                current,
                cancel_token,
                ..
            } => {
                let t = *scanned as f32;
                ui.add_space(8.0);
                let pb = egui::ProgressBar::new(0.0)
                    .desired_width(f32::INFINITY)
                    .desired_height(6.0)
                    .fill(self.theme.accent_blue)
                    .animate(true);
                ui.add(pb);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("已扫描 {t} 个文件"))
                            .color(self.theme.text_primary)
                            .size(13.0),
                    );
                    if ui.button("取消").clicked() {
                        should_cancel_scan = true;
                    }
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(current.as_str())
                        .color(self.theme.text_muted)
                        .size(11.0),
                );
                if should_cancel_scan {
                    cancel_token.cancel();
                }
            }
            ScanState::Done {
                items,
                checked,
                total_bytes,
            } => {
                let total_mb = *total_bytes as f64 / (1024.0 * 1024.0);
                let total_gb = total_mb / 1024.0;

                // 空结果
                if *total_bytes == 0 {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        ui.label(
                            egui::RichText::new("✓")
                                .color(self.theme.accent_teal)
                                .size(40.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("没有发现可清理文件")
                                .color(self.theme.text_primary)
                                .size(16.0),
                        );
                        ui.label(
                            egui::RichText::new("你的 C 盘状况良好")
                                .color(self.theme.text_secondary)
                                .size(12.0),
                        );
                        ui.add_space(12.0);
                        if ui.button("重新扫描").clicked() {
                            should_start_scan = true;
                        }
                    });
                    // Skip rest of Done rendering
                    return;
                }

                // ── 总计摘要 ──
                let summary_text = if total_gb >= 1.0 {
                    format!("总计可释放 {total_gb:.2}GB")
                } else {
                    format!("总计可释放 {total_mb:.0}MB")
                };
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(summary_text)
                            .color(self.theme.text_primary)
                            .size(18.0)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("重新扫描").clicked() {
                            should_start_scan = true;
                        }
                    });
                });

                // ── 按类别分组 ──
                let mut categories: std::collections::BTreeMap<&str, Vec<&cleaner::CleanItem>> =
                    std::collections::BTreeMap::new();
                for item in items.iter() {
                    categories.entry(&item.category).or_default().push(item);
                }

                let category_labels: std::collections::HashMap<&str, &str> = [
                    ("temp", "临时文件"),
                    ("cache", "浏览器缓存"),
                    ("prefetch", "Prefetch"),
                    ("recycle_bin", "回收站"),
                ]
                .into();

                // Category space breakdown
                let cat_totals: Vec<(&str, u64)> = categories
                    .iter()
                    .map(|(cat, items)| (*cat, items.iter().map(|i| i.size_bytes).sum::<u64>()))
                    .collect();

                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    for (cat, cat_bytes) in &cat_totals {
                        let color = match *cat {
                            "temp" => self.theme.accent_blue,
                            "cache" => self.theme.accent_purple,
                            "prefetch" => self.theme.accent_teal,
                            "recycle_bin" => self.theme.accent_amber,
                            _ => self.theme.text_muted,
                        };
                        let cat_mb = *cat_bytes as f64 / (1024.0 * 1024.0);
                        let label = category_labels.get(cat).copied().unwrap_or(cat);
                        let text = if cat_mb > 1024.0 {
                            format!("● {label} {:.1}GB", cat_mb / 1024.0)
                        } else {
                            format!("● {label} {cat_mb:.0}MB")
                        };
                        ui.label(egui::RichText::new(text).color(color).size(11.0));
                        ui.add_space(8.0);
                    }
                });

                ui.add_space(8.0);
                ui.separator();

                // ── 分类折叠列表 ──
                let checked_count = checked.len();
                let total_count = items.len();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (cat, cat_items) in &categories {
                        let cat_total: u64 = cat_items.iter().map(|i| i.size_bytes).sum();
                        let cat_mb = cat_total as f64 / (1024.0 * 1024.0);
                        let cat_label = category_labels.get(cat).copied().unwrap_or(cat);
                        let cat_summary = if cat_mb > 1024.0 {
                            format!("{:.1}GB", cat_mb / 1024.0)
                        } else {
                            format!("{cat_mb:.0}MB")
                        };

                        let all_checked_in_cat =
                            cat_items.iter().all(|i| checked.contains(&i.path));
                        let mut cat_checked = all_checked_in_cat;

                        let header_text = format!("{cat_label}  ({cat_summary})");

                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            ui.id().with(cat),
                            true,
                        )
                        .show_header(ui, |ui| {
                            if ui.checkbox(&mut cat_checked, "").changed() {
                                if cat_checked {
                                    for item in cat_items.iter() {
                                        checked.insert(item.path.clone());
                                    }
                                } else {
                                    for item in cat_items.iter() {
                                        checked.remove(&item.path);
                                    }
                                }
                            }
                            ui.label(
                                egui::RichText::new(&header_text)
                                    .color(self.theme.text_primary)
                                    .size(13.0)
                                    .strong(),
                            );
                        })
                        .body(|ui| {
                            for item in cat_items {
                                let is_checked = checked.contains(&item.path);
                                let mb = item.size_bytes as f64 / (1024.0 * 1024.0);
                                let size_str = if mb > 1024.0 {
                                    format!("{:.1}GB", mb / 1024.0)
                                } else {
                                    format!("{mb:.0}MB")
                                };

                                ui.horizontal(|ui| {
                                    let mut cs = is_checked;
                                    ui.checkbox(&mut cs, "");
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}  {}",
                                            size_str,
                                            item.path.display()
                                        ))
                                        .color(self.theme.text_primary)
                                        .size(11.0),
                                    );

                                    if cs && !is_checked {
                                        checked.insert(item.path.clone());
                                    } else if !cs && is_checked {
                                        checked.remove(&item.path);
                                    }
                                });
                            }
                        });
                    }
                });

                // ── 底部操作栏 ──
                ui.add_space(4.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("已选 {checked_count}/{total_count} 项"))
                            .color(self.theme.text_secondary),
                    );

                    let all_checked = checked_count == total_count;
                    if ui
                        .button(if all_checked {
                            "取消全选"
                        } else {
                            "全选"
                        })
                        .clicked()
                    {
                        if all_checked {
                            checked.clear();
                        } else {
                            for item in items.iter() {
                                checked.insert(item.path.clone());
                            }
                        }
                    }

                    let clean_btn =
                        egui::Button::new(egui::RichText::new("清理选中").color(Color32::WHITE))
                            .fill(self.theme.accent_red)
                            .min_size(egui::vec2(90.0, 28.0));
                    if ui.add_enabled(checked_count > 0, clean_btn).clicked() {
                        execute_delete = items
                            .iter()
                            .filter(|i| checked.contains(&i.path))
                            .map(|i| i.path.clone())
                            .collect();
                    }
                });
            }
            ScanState::Cancelled => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("扫描已取消")
                            .color(self.theme.text_secondary)
                            .size(16.0),
                    );
                    ui.add_space(8.0);
                    if ui.button("重新扫描").clicked() {
                        should_start_scan = true;
                    }
                });
            }
            ScanState::Error(msg) => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new(format!("扫描失败: {msg}"))
                            .color(self.theme.accent_red)
                            .size(14.0),
                    );
                    ui.add_space(8.0);
                    if ui.button("重试").clicked() {
                        should_start_scan = true;
                    }
                });
            }
            ScanState::Deleting { .. } => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("删除中...")
                            .color(self.theme.text_secondary)
                            .size(16.0),
                    );
                    let pb = egui::ProgressBar::new(0.5)
                        .desired_width(200.0)
                        .desired_height(6.0)
                        .fill(self.theme.accent_red)
                        .animate(true);
                    ui.add(pb);
                });
            }
        }

        if !execute_delete.is_empty() {
            let (done_tx, done_rx) = std::sync::mpsc::channel();
            self.scan_state = ScanState::Deleting { rx: done_rx };
            let _guard = self._rt.enter();
            tokio::task::spawn_blocking(move || {
                cleaner::delete_files(&execute_delete);
                let _ = done_tx.send(());
            });
        }
        if should_cancel_scan {
            // cancel was already done above via cancel_token
        }
        if should_start_scan {
            self.start_scan();
        }
    }

    fn start_scan(&mut self) {
        let _guard = self._rt.enter();
        let checked = if let ScanState::Done { checked, .. } = &self.scan_state {
            checked.clone()
        } else {
            HashSet::new()
        };
        let (tx, rx) = mpsc::channel();
        match cleaner::start_scan(tx) {
            Ok((cmd, cancel_token)) => {
                self.clean_cmd_tx = Some(cmd);
                self.clean_rx = Some(rx);
                self.scan_state = ScanState::Scanning {
                    cancel_token,
                    scanned: 0,
                    current: "Starting...".into(),
                    accumulated_items: Vec::new(),
                    accumulated_bytes: 0,
                    checked,
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
                    self.scan_state = ScanState::Error("扫描超时，请重试".into());
                    self.scan_start_time = None;
                    ctx.request_repaint();
                }
            }
        }

        // 检查删除是否完成
        if let ScanState::Deleting { rx } = &self.scan_state {
            if rx.try_recv().is_ok() {
                self.scan_state = ScanState::Idle;
                ctx.request_repaint();
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
            let _ = tx.send(monitor::MonitorCommand::Shutdown);
        }
        // monitor 线程通过 500ms recv_timeout 子间隔自动响应 Shutdown
        if let Some(tx) = &self.clean_cmd_tx {
            let _ = tx.send(cleaner::CleanCommand::Shutdown);
        }
        // 线程在收到 Shutdown 或 channel 断开后自行退出，不做 blocking join
    }
}
