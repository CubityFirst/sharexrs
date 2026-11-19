//! `ShareX.Tools/Tools/InspectWindow` (`InspectWindowWindow.axaml`,
//! `InspectWindowViewModel`, `InspectWindowPickerOverlay`): Pick window /
//! Pick control (the window hides, a translucent overlay on each screen
//! takes a click, `GetWindowAtPoint`), the window list, the selected
//! window's header, its top-most switch and opacity slider (top-level
//! windows), its properties as read-only Consolas boxes, Copy all and
//! Refresh. The windows are read and changed through a [`WindowSource`].

use egui::{FontId, Id, Rect, Ui, Vec2, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::system::inspect_window::{self as iw, Details, WindowSnapshot};
use sharex_ui::controls::{self, ComboBox, Slider, TextBox, ToggleSwitch};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use super::WindowList;
use super::native::WindowSource;
use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 940.0;
pub const HEIGHT: f32 = 650.0;
/// The left column (`ColumnDefinitions="300,*"`).
const LEFT_W: f32 = 300.0;

pub struct InspectWindowWindow {
    source: Box<dyn WindowSource>,
    list: WindowList,
    /// `SelectedListItem`.
    pub selected_item: Option<usize>,
    /// `_selectedWindow` and whether it's a top-level window.
    pub selected: Option<(isize, bool)>,
    pub details: Option<Details>,
    icon: Option<(image::RgbaImage, Option<egui::TextureHandle>)>,
    pub top_most: bool,
    pub opacity: f32,
    pub ignored: isize,
    opened: bool,
    scroll: f32,
}

impl InspectWindowWindow {
    pub fn new(source: Box<dyn WindowSource>) -> Self {
        Self {
            source,
            list: WindowList::default(),
            selected_item: None,
            selected: None,
            details: None,
            icon: None,
            top_most: false,
            opacity: 100.0,
            ignored: 0,
            opened: false,
            scroll: 0.0,
        }
    }

    /// Nothing read when it opens (`--render`).
    pub fn set_offline(&mut self) {
        self.opened = true;
    }

    /// `ReloadWindowList`.
    pub fn reload(&mut self) {
        self.list.set(self.source.visible_windows(self.ignored));
        self.selected_item = None;
    }

    /// `SelectWindow(handle, isTopLevelWindow)`.
    pub fn select_window(&mut self, handle: isize, top_level: bool) {
        if handle == 0 || handle == self.ignored {
            return;
        }
        self.selected = Some((handle, top_level));
        self.selected_item = None;
        self.update();
    }

    /// Shows `snapshot` as the selected window (`--render`: a synthetic one).
    pub fn show_snapshot(&mut self, snapshot: &WindowSnapshot, top_level: bool) {
        self.selected = Some((snapshot.handle, top_level));
        self.apply(snapshot, top_level);
    }

    /// `UpdateWindowInfo`.
    fn update(&mut self) {
        let Some((handle, top_level)) = self.selected else { return self.clear() };
        match self.source.snapshot(handle) {
            Some(snap) => {
                self.icon = self.source.icon(handle).map(|i| (i, None));
                self.apply(&snap, top_level);
            }
            None => self.clear(),
        }
    }

    fn apply(&mut self, snap: &WindowSnapshot, top_level: bool) {
        let d = iw::details(snap, top_level);
        if let Some(t) = d.top_most {
            self.top_most = t;
        }
        if let Some(o) = d.opacity_percent {
            self.opacity = o as f32;
        }
        self.details = Some(d);
        self.scroll = 0.0;
    }

    /// `ClearSelection`.
    fn clear(&mut self) {
        self.selected = None;
        self.details = None;
        self.icon = None;
        self.top_most = false;
        self.opacity = 100.0;
    }

    fn is_top_level(&self) -> bool {
        self.selected.is_some_and(|(_, t)| t) && self.details.is_some()
    }

    /// The header panel's height for `width`.
    fn header_height(&self, ui: &Ui, width: f32) -> f32 {
        let inner = width - 34.0;
        let hf = tools::heading_font();
        let title = self.details.as_ref().map_or(s::InspectWindowViewModel_No_target_selected(), |d| d.title.as_str());
        let lines = controls::wrap_lines(ui, title, &hf, inner - 24.0).len().clamp(1, 3);
        let title_h = (ui.fonts_mut(|f| f.row_height(&hf)) * lines as f32).ceil().max(16.0);
        let subtitle = self.details.as_ref().map_or(s::InspectWindowViewModel_Pick_target(), |d| d.subtitle.as_str());
        let sub_h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), inner, subtitle, font(13.0, 400), egui::Color32::WHITE, false);
        let badge = if self.details.is_some() { 5.0 + 3.0 + line_height(ui, &font(13.0, 400)) + 3.0 + 2.0 } else { 0.0 };
        34.0 + title_h + 8.0 + sub_h + badge
    }

    fn settings_height(ui: &Ui) -> f32 {
        let nf = font(13.0, 400);
        34.0 + tools::heading_height(ui) + 12.0 + ToggleSwitch::SIZE.y + 12.0 + line_height(ui, &nf) + tools::SLIDER_HEIGHT
    }

    /// The left column: the selection's header and its settings.
    fn left_column(&mut self, ui: &mut Ui, rect: Rect) {
        let p = palette(ui.ctx());
        let hh = self.header_height(ui, rect.width());
        let panel = Rect::from_min_size(rect.min, vec2(rect.width(), hh));
        tools::panel(ui, panel);
        let inner = panel.shrink(17.0);
        let hf = tools::heading_font();
        let title = self.details.as_ref().map_or(s::InspectWindowViewModel_No_target_selected().to_owned(), |d| d.title.clone());
        let mut lines = controls::wrap_lines(ui, &title, &hf, inner.width() - 24.0);
        lines.truncate(3);
        let row_h = ui.fonts_mut(|f| f.row_height(&hf));
        let title_h = (row_h * lines.len().max(1) as f32).ceil().max(16.0);
        let row0 = Rect::from_min_size(inner.min, vec2(inner.width(), title_h));
        let ir = Rect::from_min_size(pos2(row0.min.x, center_y(row0, 16.0)), Vec2::splat(16.0));
        match &mut self.icon {
            Some((img, tex)) => {
                let t = tex.get_or_insert_with(|| ui.ctx().load_texture("inspect-icon", crate::thumbnail::to_color_image(img), egui::TextureOptions::NEAREST));
                // `Stretch="None"`: the icon at its own size, centred.
                let size = vec2(img.width() as f32, img.height() as f32);
                ui.painter().image(t.id(), Rect::from_center_size(ir.center(), size), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
            }
            None => sharex_ui::icons::paint(ui.painter(), ir, icon::Window, p.text_secondary),
        }
        let ty = center_y(row0, title_h);
        for (i, line) in lines.iter().enumerate() {
            tools::text_at(ui, pos2(ir.max.x + 8.0, ty + (row_h * i as f32).round()), line, hf.clone(), p.text);
        }
        let nf = font(13.0, 400);
        let subtitle = self.details.as_ref().map_or(s::InspectWindowViewModel_Pick_target().to_owned(), |d| d.subtitle.clone());
        let sy = row0.max.y + 8.0;
        let sub_h = crate::video_tools::wrapped_text(ui, pos2(inner.min.x, sy), inner.width(), &subtitle, nf.clone(), p.text_secondary, true);
        if let Some(d) = &self.details {
            // The type badge (`Padding="7,3"`, left-aligned).
            let tw = text_width(ui, &d.kind, nf.clone());
            let badge = Rect::from_min_size(pos2(inner.min.x, sy + sub_h + 5.0), vec2(tw + 14.0 + 2.0, line_height(ui, &nf) + 6.0 + 2.0));
            sharex_ui::border::border(ui.painter(), badge, 4.0, 1.0, Some(sharex_ui::border::Brush::Solid(p.control_background)), Some(p.border));
            tools::text_at(ui, pos2(badge.min.x + 8.0, badge.min.y + 4.0), &d.kind, nf.clone(), p.text);
        }
        if !self.is_top_level() {
            return;
        }
        // Window settings.
        let sh = Self::settings_height(ui);
        let panel = Rect::from_min_size(pos2(rect.min.x, panel.max.y + 12.0), vec2(rect.width(), sh));
        tools::panel(ui, panel);
        let inner = panel.shrink(17.0);
        let hh = tools::heading_height(ui);
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), s::InspectWindowWindow_Window_settings());
        let ty = inner.min.y + hh + 12.0;
        let before = self.top_most;
        ToggleSwitch { enabled: true }.show(ui, pos2(inner.min.x, ty), Id::new("inspect-topmost"), &mut self.top_most);
        // The switch's `OnContent`/`OffContent` after it.
        let tr = Rect::from_min_size(pos2(inner.min.x + ToggleSwitch::SIZE.x, ty), vec2(inner.width(), ToggleSwitch::SIZE.y));
        controls::label(ui, tr, s::InspectWindowWindow_Always_on_top(), nf.clone(), p.text);
        if before != self.top_most
            && let Some((h, _)) = self.selected
        {
            // `OnIsTopMostChanged`: set, then read back.
            self.source.set_top_most(h, self.top_most);
            self.update();
        }
        let oy = ty + ToggleSwitch::SIZE.y + 12.0;
        let lh = line_height(ui, &nf);
        let row = Rect::from_min_size(pos2(inner.min.x, oy), vec2(inner.width(), lh));
        tools::row_label(ui, row, s::InspectWindowWindow_Opacity());
        let value = format!("{}%", tools::format_0(self.opacity));
        let vw = text_width(ui, &value, nf.clone());
        tools::text_at(ui, pos2(row.max.x - vw, row.min.y), &value, nf, p.text);
        // Fluent's 50px slider: its track centred, the control's thumb rect 13px above it.
        let sr = Rect::from_min_size(pos2(inner.min.x, row.max.y + 25.0 - 13.0), vec2(inner.width(), 32.0));
        let mut v = self.opacity;
        if (Slider { minimum: 10.0, maximum: 100.0, step: 10.0, enabled: true }).show(ui, sr, Id::new("inspect-opacity"), &mut v) {
            self.opacity = v;
            if let Some((h, _)) = self.selected {
                // `OnOpacityChanged`.
                self.source.set_opacity(h, iw::percent_to_opacity(v as f64));
            }
        }
    }

    /// The Window information panel.
    fn info_panel(&mut self, ui: &mut Ui, rect: Rect) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let mut out = Vec::new();
        tools::panel(ui, rect);
        let inner = rect.shrink(1.0);
        let head = Rect::from_min_size(pos2(inner.min.x + 12.0, inner.min.y + 12.0), vec2(inner.width() - 24.0, tools::BUTTON_HEIGHT));
        tools::heading(ui, head, s::InspectWindowWindow_Window_information());
        let refresh = s::InspectWindowWindow_Refresh();
        let rw = tools::icon_text_button_width(ui, refresh);
        let rr = Rect::from_min_size(pos2(head.max.x - rw, head.min.y), vec2(rw, tools::BUTTON_HEIGHT));
        let copy = s::InspectWindowWindow_Copy_all();
        let cw = tools::icon_text_button_width(ui, copy);
        let cr = Rect::from_min_size(pos2(rr.min.x - 8.0 - cw, head.min.y), vec2(cw, tools::BUTTON_HEIGHT));
        let has = self.details.is_some();
        if tools::icon_text_button(ui, cr, Id::new("inspect-copy"), icon::Copy, copy, has).clicked()
            && let Some(d) = &self.details
        {
            out.push(Effect::CopyText(iw::clipboard_text(&d.properties)));
        }
        if tools::icon_text_button(ui, rr, Id::new("inspect-refresh"), icon::Refresh, refresh, has).clicked() {
            self.update();
        }
        let body = Rect::from_min_max(pos2(inner.min.x + 12.0, head.max.y + 12.0), pos2(inner.max.x - 12.0, inner.max.y - 12.0));
        let Some(details) = self.details.clone() else {
            tools::empty_state(ui, body, icon::InspectWindow, 48.0, 10.0, &[(s::InspectWindowWindow_Pick_a_target_to_inspect_its_window_properties(), font(13.0, 400), p.text_secondary)]);
            return out;
        };
        // Each property: the label, 5px, the value box (`MinHeight="40"`,
        // `Padding="10,8"`), 10px.
        let nf = font(13.0, 400);
        let vf = FontId::new(14.0, sharex_ui::fonts::consolas());
        let lh = line_height(ui, &nf);
        let vh = ui.fonts_mut(|f| f.row_height(&vf));
        let heights: Vec<f32> = details.properties.iter().map(|pr| (pr.value.lines().count().max(1) as f32 * vh).ceil() + 18.0).map(|h| h.max(40.0)).collect();
        let content: f32 = heights.iter().map(|h| lh + 5.0 + h + 10.0).sum();
        let max_scroll = (content - body.height()).max(0.0);
        if ui.rect_contains_pointer(body) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
        child.set_clip_rect(body.intersect(ui.clip_rect()));
        let mut y = body.min.y - self.scroll;
        for (k, pr) in details.properties.iter().enumerate() {
            tools::row_label(&child, Rect::from_min_size(pos2(body.min.x, y), vec2(body.width(), lh)), &pr.name);
            let vr = Rect::from_min_size(pos2(body.min.x, y + lh + 5.0), vec2(body.width(), heights[k]));
            let mut value = pr.value.clone();
            TextBox { read_only: true, multiline: pr.multiline, padding: Thickness::hv(10.0, 8.0), font: Some(vf.clone()), ..TextBox::default() }.show(&mut child, vr, Id::new(("inspect-value", k)), &mut value);
            y = vr.max.y + 10.0;
        }
        if max_scroll > 0.0 {
            crate::dialogs::layout::scroll_bar(ui, body, content, self.scroll, 0.0);
        }
        out
    }
}

impl Dialog for InspectWindowWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::InspectWindowWindow_ShareX_Inspect_Window()), WIDTH, HEIGHT, 800.0, 540.0)
    }

    fn opened(&mut self, handle: Option<isize>) {
        // `OnOpened` → `SetIgnoredWindowHandle`.
        self.ignored = handle.unwrap_or(0);
        if !self.opened {
            self.opened = true;
            self.reload();
        }
    }

    fn window_picked(&mut self, handle: isize, top_level: bool) {
        self.select_window(handle, top_level);
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let mut out = Vec::new();
        let c = tools::background(ui);
        let top = Rect::from_min_size(c.min, vec2(c.width(), 26.0 + tools::BUTTON_HEIGHT));
        tools::panel(ui, top);
        let row = top.shrink(13.0);
        let pick_w = s::InspectWindowWindow_Pick_window();
        let ww = tools::icon_text_button_width(ui, pick_w);
        let wr = Rect::from_min_size(row.min, vec2(ww, tools::BUTTON_HEIGHT));
        let pick_c = s::InspectWindowWindow_Pick_control();
        let cw = tools::icon_text_button_width(ui, pick_c);
        let cr = Rect::from_min_size(pos2(wr.max.x + 8.0, row.min.y), vec2(cw, tools::BUTTON_HEIGHT));
        if tools::icon_text_button(ui, wr, Id::new("inspect-pick-window"), icon::Region, pick_w, true).clicked() {
            out.push(Effect::PickWindow(true));
        }
        if tools::icon_text_button(ui, cr, Id::new("inspect-pick-control"), icon::Pointer, pick_c, true).clicked() {
            out.push(Effect::PickWindow(false));
        }
        let area = Rect::from_min_max(pos2(cr.max.x + 8.0, row.min.y), row.max);
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let placeholder = s::InspectWindowWindow_Select_window();
        let ctx = ui.ctx().clone();
        let items = self.list.items(&ctx, 20.0);
        let w = match self.selected_item {
            Some(_) => combo.auto_width(ui, &items, self.selected_item),
            None => combo.placeholder_width(ui, placeholder),
        }
        .min(area.width());
        let id = Id::new("inspect-windows");
        let was_open = ComboBox::is_open(ui.ctx(), id);
        if let Some(i) = combo.show_with_placeholder(ui, Rect::from_min_size(area.min, vec2(w, area.height())), id, &items, self.selected_item, placeholder) {
            // `OnSelectedListItemChanged` → `SelectWindow(handle, true)`.
            let handle = self.list.windows[i].entry.handle;
            self.select_window(handle, true);
            self.selected_item = Some(i);
        }
        if !was_open && ComboBox::is_open(ui.ctx(), id) {
            // `OnWindowListDropDownOpened`.
            self.reload();
        }
        let below = Rect::from_min_max(pos2(c.min.x, top.max.y + 12.0), c.max);
        let left = Rect::from_min_size(below.min, vec2(LEFT_W, below.height()));
        self.left_column(ui, left);
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, below.min.y), below.max);
        out.extend(self.info_panel(ui, right));
        out
    }
}

/// `InspectWindowPickerOverlay`: a translucent, topmost window over one
/// screen with the instruction at its top; a left click picks what's
/// under the pointer, a right click or Escape cancels.
pub struct PickerOverlay {
    /// The Inspect window it picks for (none in a render).
    pub owner: Option<winit::window::WindowId>,
    pub top_level: bool,
    /// The screen it covers (desktop pixels) and its scaling.
    pub bounds: sharex_capture::Rect,
    pub scale: f64,
}

impl Dialog for PickerOverlay {
    fn spec(&self) -> Spec {
        let mut spec = Spec::sized("", (self.bounds.w as f64 / self.scale) as f32, (self.bounds.h as f64 / self.scale) as f32, 0.0, 0.0);
        spec.resizable = false;
        spec.topmost = true;
        spec.skip_taskbar = true;
        spec.borderless = true;
        spec
    }

    fn position(&self, _work_area: sharex_capture::Rect, _size: (u32, u32)) -> Option<(i32, i32)> {
        Some((self.bounds.x, self.bounds.y))
    }

    fn picker_owner(&self) -> Option<winit::window::WindowId> {
        self.owner
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        // `Background="#19000000"` on the window and again on its grid.
        let shade = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 0x19);
        ui.painter().rect_filled(full, 0.0, shade);
        ui.painter().rect_filled(full, 0.0, shade);
        let text = if self.top_level { s::InspectWindowPickerOverlay_Click_window() } else { s::InspectWindowPickerOverlay_Click_control() };
        let nf = font(13.0, 400);
        let tw = text_width(ui, text, nf.clone());
        let lh = line_height(ui, &nf);
        let content_h = lh.max(20.0);
        let w = 2.0 + 36.0 + 20.0 + 10.0 + tw;
        let h = 2.0 + 24.0 + content_h;
        let panel = Rect::from_min_size(pos2(full.min.x + ((full.width() - w) / 2.0).round_ties_even(), full.min.y + 16.0), vec2(w, h));
        tools::panel(ui, panel);
        let inner = Rect::from_min_max(pos2(panel.min.x + 19.0, panel.min.y + 13.0), pos2(panel.max.x - 19.0, panel.max.y - 13.0));
        let ir = Rect::from_min_size(pos2(inner.min.x, center_y(inner, 20.0)), Vec2::splat(20.0));
        sharex_ui::icons::paint(ui.painter(), ir, icon::Crosshair, p.text);
        tools::text_at(ui, pos2(ir.max.x + 10.0, center_y(inner, lh)), text, nf, p.text);
        let r = ui.interact(full, Id::new("picker-overlay"), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::Crosshair);
        let cancel = r.secondary_clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape));
        if r.clicked() {
            return vec![Effect::WindowPicked { owner: self.owner, picked: true, top_level: self.top_level }];
        }
        if cancel {
            return vec![Effect::WindowPicked { owner: self.owner, picked: false, top_level: self.top_level }];
        }
        Vec::new()
    }

    fn close_requested(&mut self) -> Vec<Effect> {
        vec![Effect::WindowPicked { owner: self.owner, picked: false, top_level: self.top_level }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_tools::native::ListedWindow;
    use sharex_tools::system::inspect_window::{Rect as WRect, WindowEntry};
    use std::sync::{Arc, Mutex};

    /// One pretend window (handle 42); top-most and opacity changes are recorded.
    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<(bool, u8)>>);

    impl WindowSource for Fake {
        fn visible_windows(&self, ignored: isize) -> Vec<ListedWindow> {
            let e = WindowEntry { handle: 42, title: "Pattern".into(), class_name: "C".into(), process_name: "sharexrs".into(), visible: true, cloaked: false, rectangle: WRect { x: 0, y: 0, width: 5, height: 5 } };
            iw::visible_windows(vec![e], ignored).into_iter().map(|entry| ListedWindow { entry, icon: None }).collect()
        }
        fn snapshot(&self, handle: isize) -> Option<WindowSnapshot> {
            let (top, opacity) = *self.0.lock().unwrap();
            (handle == 42).then(|| WindowSnapshot { handle, title: "Pattern".into(), class_name: "C".into(), process_name: "sharexrs".into(), style: Some(0x14CF_0000), ex_style: Some(0x100), top_most: top, opacity, ..Default::default() })
        }
        fn icon(&self, _: isize) -> Option<image::RgbaImage> {
            None
        }
        fn window_at_point(&self, _: i32, _: i32, _: bool) -> isize {
            42
        }
        fn set_top_most(&self, _: isize, t: bool) {
            self.0.lock().unwrap().0 = t;
        }
        fn set_opacity(&self, _: isize, o: u8) {
            self.0.lock().unwrap().1 = o;
        }
    }

    #[test]
    fn selects_inspects_and_changes_a_window() {
        let fake = Fake(Arc::new(Mutex::new((false, 255))));
        let mut w = InspectWindowWindow::new(Box::new(fake.clone()));
        w.opened(Some(7));
        assert_eq!(w.list.windows.len(), 1);
        w.select_window(42, true);
        let d = w.details.clone().unwrap();
        assert_eq!(d.title, "Pattern");
        assert_eq!(d.subtitle, "sharexrs  |  C");
        assert_eq!(d.properties[0].value, "0x0000002A");
        assert!(w.is_top_level());
        w.source.set_top_most(42, true);
        w.update();
        assert!(w.top_most);
        // Its own window is ignored.
        w.select_window(7, true);
        assert_eq!(w.selected, Some((42, true)));
        w.select_window(0, false);
        assert_eq!(w.selected, Some((42, true)));
    }
}
