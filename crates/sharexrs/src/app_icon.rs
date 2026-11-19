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

/// The windows' icon now (`ShareX.WindowIcon`, from `ShareXResources.IconBytes`):
/// the white one with `UseWhiteShareXIcon`.
pub fn window_icon() -> Option<winit::window::Icon> {
    winit::window::Icon::from_rgba(crate::tray::current_icon(64).into_raw(), 64, 64).ok()
}

/// The title bar's logo (`MainWindow.TitleBarIcon`, `ShareXResources.IconBytes`):
/// the white icon with `UseWhiteShareXIcon`, else [`paint`]'s.
pub fn paint_current(ui: &Ui, rect: Rect) {
    if !crate::tray::is_white_icon() {
        paint(ui, rect);
        return;
    }
    let ppp = ui.ctx().pixels_per_point();
    let side = (rect.width().min(rect.height()) * ppp).round().max(1.0) as u32;
    let id = egui::Id::new(("white-app-icon", side));
    let tex = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(id)).unwrap_or_else(|| {
        let img = crate::tray::current_icon(side);
        let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
        let t = ui.ctx().load_texture("white-app-icon", ci, egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(id, t.clone()));
        t
    });
    ui.painter().image(tex.id(), rect, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
}

/// The icon as 64x64 RGBA pixels, for the window and taskbar.
pub fn rgba64() -> Vec<u8> {
    const PNG: &[u8] = include_bytes!("../../../assets/ShareXrs.iconset/icon_32x32@2x.png");
    image::load_from_memory(PNG).map(|i| i.to_rgba8().into_raw()).unwrap_or_default()
}
