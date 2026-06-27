use eframe::egui;

pub struct PonyCleanApp {
    #[allow(dead_code)]
    pub(crate) rt: tokio::runtime::Runtime,
}

impl PonyCleanApp {
    pub fn new(rt: tokio::runtime::Runtime) -> Self {
        Self { rt }
    }
}

impl eframe::App for PonyCleanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

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
