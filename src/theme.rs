use egui::{Color32, Rounding, Stroke};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Theme {
    pub bg_window: Color32,
    pub bg_card: Color32,
    pub bg_card_hover: Color32,
    pub bg_surface: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub accent_blue: Color32,
    pub accent_teal: Color32,
    pub accent_amber: Color32,
    pub accent_red: Color32,
    pub accent_purple: Color32,
    pub radius_sm: Rounding,
    pub radius_md: Rounding,
    pub radius_lg: Rounding,
    pub border_subtle: Stroke,
    pub border_focus: Stroke,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg_window: Color32::TRANSPARENT,
            bg_card: Color32::from_rgba_unmultiplied(31, 36, 43, 235),
            bg_card_hover: Color32::from_rgba_unmultiplied(37, 42, 49, 245),
            bg_surface: Color32::from_rgb(20, 23, 28),
            text_primary: Color32::from_rgb(232, 234, 237),
            text_secondary: Color32::from_rgb(154, 160, 166),
            text_muted: Color32::from_rgb(95, 99, 104),
            accent_blue: Color32::from_rgb(138, 180, 248),
            accent_teal: Color32::from_rgb(129, 201, 149),
            accent_amber: Color32::from_rgb(253, 214, 99),
            accent_red: Color32::from_rgb(242, 139, 130),
            accent_purple: Color32::from_rgb(197, 138, 249),
            radius_sm: Rounding::same(6.0),
            radius_md: Rounding::same(12.0),
            radius_lg: Rounding::same(16.0),
            border_subtle: Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 15)),
            border_focus: Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 31)),
        }
    }
}
