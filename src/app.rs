use eframe::egui;

pub struct PonyCleanApp {
    // 持有 tokio runtime 使其在窗口生命周期内不析构
    // TASK-002 接入后将移除此前缀
    _rt: tokio::runtime::Runtime,
}

impl PonyCleanApp {
    pub fn new(rt: tokio::runtime::Runtime) -> Self {
        Self { _rt: rt }
    }
}

impl eframe::App for PonyCleanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                ui.heading("PonyClean");
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}
