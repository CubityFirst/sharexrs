//! ShareXrs's icon (`assets/icon.svg`: a rounded blue square with an "S"),
//! drawn with egui so it needs no SVG text rendering.

use egui::{Color32, CornerRadius, Rect, Ui};

const BLUE: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB);

pub fn paint(ui: &Ui, rect: Rect) {
    let side = rect.width().min(rect.height());
    let radius = (side * 14.0 / 64.0).round() as u8;
    ui.painter().rect_filled(rect, CornerRadius::same(radius), BLUE);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "S",
        sharex_ui::widgets::font(side * 0.5, 600),
        Color32::WHITE,
    );
}

/// The icon as 64x64 RGBA pixels, for the window and taskbar.
pub fn rgba64() -> Vec<u8> {
    const PNG: &[u8] = include_bytes!("../../../assets/ShareXrs.iconset/icon_32x32@2x.png");
    image::load_from_memory(PNG).map(|i| i.to_rgba8().into_raw()).unwrap_or_default()
}
