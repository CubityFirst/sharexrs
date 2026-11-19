//! `ShareX.Tools/Tools/MonitorTest` (`MonitorTestWindow.axaml`,
//! `MonitorTestViewModel`, `Controls/MonitorTestPatternControl`): a
//! borderless, topmost window over the screen under the pointer, the
//! pattern drawn by `sharex_tools::monitor_test` (moving bars advance with
//! time), and the 720px controls panel: the five modes and each mode's
//! options. A click on the pattern or Space toggles the panel, a right
//! click or Escape closes, Left/Right change the mode, the wheel changes
//! the motion speed.

use std::time::Instant;

use egui::{Color32, Id, Rect, Ui, Vec2, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::monitor_test::{self as mt, GradientDirection, Mode, MonitorTest, MotionDirection, Pattern};
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{self, Button, ComboBox, ComboItem, Slider};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

/// The controls panel's `Width="720"`.
const PANEL_W: f32 = 720.0;
/// The option rows' label column (`ColumnDefinitions="96,*,52"`).
const LABEL_W: f32 = 96.0;

pub const MODES: [Mode; 5] = [Mode::Grayscale, Mode::Rgb, Mode::Gradient, Mode::Pattern, Mode::Motion];
pub const DIRECTIONS: [GradientDirection; 4] = [GradientDirection::Horizontal, GradientDirection::Vertical, GradientDirection::ForwardDiagonal, GradientDirection::BackwardDiagonal];
pub const PATTERNS: [Pattern; 3] = [Pattern::HorizontalLines, Pattern::VerticalLines, Pattern::Checkerboard];
pub const MOTIONS: [MotionDirection; 2] = [MotionDirection::VerticalBars, MotionDirection::HorizontalBars];

/// `LocalizedDisplayConverter`'s texts.
pub fn direction_text(d: GradientDirection) -> &'static str {
    match d {
        GradientDirection::Horizontal => s::LocalizedDisplayConverter_Horizontal(),
        GradientDirection::Vertical => s::LocalizedDisplayConverter_Vertical(),
        GradientDirection::ForwardDiagonal => s::LocalizedDisplayConverter_Forward_diagonal(),
        GradientDirection::BackwardDiagonal => s::LocalizedDisplayConverter_Backward_diagonal(),
    }
}

pub fn pattern_text(p: Pattern) -> &'static str {
    match p {
        Pattern::HorizontalLines => s::LocalizedDisplayConverter_Horizontal_lines(),
        Pattern::VerticalLines => s::LocalizedDisplayConverter_Vertical_lines(),
        Pattern::Checkerboard => s::LocalizedDisplayConverter_Checkerboard(),
    }
}

pub fn motion_text(m: MotionDirection) -> &'static str {
    match m {
        MotionDirection::VerticalBars => s::LocalizedDisplayConverter_Vertical_bars(),
        MotionDirection::HorizontalBars => s::LocalizedDisplayConverter_Horizontal_bars(),
    }
}

pub fn direction_items() -> Vec<ComboItem> {
    DIRECTIONS.iter().map(|d| ComboItem::normal(direction_text(*d))).collect()
}

pub fn pattern_items() -> Vec<ComboItem> {
    PATTERNS.iter().map(|p| ComboItem::normal(pattern_text(*p))).collect()
}

pub fn motion_items() -> Vec<ComboItem> {
    MOTIONS.iter().map(|m| ComboItem::normal(motion_text(*m))).collect()
}

pub struct MonitorTestWindow {
    pub test: MonitorTest,
    pub controls_visible: bool,
    /// The screen it covers (desktop pixels) and its scaling.
    pub bounds: sharex_capture::Rect,
    pub scale: f64,
    /// How far the bars have moved (px).
    pub motion_offset: f64,
    last_frame: Option<Instant>,
    /// The pattern as a texture, made again when the options or size change.
    texture: Option<(MonitorTest, (u32, u32), i64, egui::TextureHandle)>,
    /// The open colour flyout (0 RGB, 1 gradient start, 2 gradient end).
    color_open: Option<usize>,
    /// Motion doesn't advance (`--render`).
    pub frozen: bool,
}

impl MonitorTestWindow {
    /// `OnOpened`: sized to the screen under the pointer (else the primary).
    pub fn new() -> Self {
        let screens = sharex_capture::screenshot::screens();
        let cursor = sharex_capture::platform::cursor_position();
        let screen = cursor
            .and_then(|(x, y)| screens.iter().find(|s| s.bounds.contains(x, y)))
            .or_else(|| screens.iter().find(|s| s.primary))
            .or(screens.first());
        let (bounds, scale) = screen.map_or((sharex_capture::Rect::new(0, 0, 1024, 768), 1.0), |s| (s.bounds, s.scale));
        Self::on(bounds, scale)
    }

    pub fn on(bounds: sharex_capture::Rect, scale: f64) -> Self {
        Self { test: MonitorTest::default(), controls_visible: true, bounds, scale, motion_offset: 0.0, last_frame: None, texture: None, color_open: None, frozen: false }
    }

    /// `SelectRelativeMode`.
    pub fn select_relative_mode(&mut self, offset: i32) {
        let i = MODES.iter().position(|m| *m == self.test.mode).unwrap_or(0) as i32;
        self.test.mode = MODES[((i + offset).rem_euclid(5)) as usize];
    }

    /// `RgbText` and the gradient texts: `#RRGGBB`.
    fn rgb(&self) -> [u8; 4] {
        [mt::to_byte(self.test.red), mt::to_byte(self.test.green), mt::to_byte(self.test.blue), 255]
    }

    /// The pattern, drawn into a texture at the window's pixel size.
    fn draw_pattern(&mut self, ui: &Ui, rect: Rect) {
        let ppp = ui.ctx().pixels_per_point();
        let size = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
        let offset_key = if self.test.mode == Mode::Motion { (self.motion_offset * 1000.0) as i64 } else { 0 };
        let stale = self.texture.as_ref().is_none_or(|(t, s, o, _)| *t != self.test || *s != size || *o != offset_key);
        if stale {
            let bmp = self.test.render(size.0, size.1, self.motion_offset);
            let image = egui::ColorImage::from_rgba_premultiplied([size.0 as usize, size.1 as usize], bmp.data());
            let tex = ui.ctx().load_texture("monitor-test-pattern", image, egui::TextureOptions::NEAREST);
            self.texture = Some((self.test.clone(), size, offset_key, tex));
        }
        if let Some((_, _, _, tex)) = &self.texture {
            ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    }

    /// A mode button: the icon and text (`Spacing="7"`) centred, the accent
    /// fill when `.active`.
    fn mode_button(ui: &Ui, rect: Rect, id: Id, key: &str, text: &str, active: bool) -> bool {
        let p = palette(ui.ctx());
        let r = Button::default().show(ui, rect, id, "", true);
        let fg = if active {
            // `.active`: the accent brush behind the content, its foreground.
            border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::accent(ui.ctx())), Some(p.border));
            p.accent_foreground
        } else {
            p.text
        };
        let f = font(13.0, 400);
        let w = 13.0 + 7.0 + text_width(ui, text, f.clone());
        let x = rect.min.x + ((rect.width() - w) / 2.0).round_ties_even();
        sharex_ui::icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(rect, 13.0)), Vec2::splat(13.0)), key, fg);
        tools::text_at(ui, pos2(x + 13.0 + 7.0, center_y(rect, line_height(ui, &f))), text, f, fg);
        r.clicked()
    }

    /// `Button.color-button`: the 24px swatch and the hex text; a click
    /// opens the colour flyout (the shared colour popup).
    fn color_button(&mut self, ui: &Ui, rect: Rect, k: usize, color: [u8; 4]) -> Option<[u8; 4]> {
        let p = palette(ui.ctx());
        let id = Id::new(("monitor-color", k));
        let r = Button::default().show(ui, rect, id, "", true);
        let inner = Rect::from_min_max(pos2(rect.min.x + 11.0, rect.min.y + 7.0), pos2(rect.max.x - 11.0, rect.max.y - 7.0));
        let sw = Rect::from_min_size(pos2(inner.min.x, center_y(inner, 24.0)), Vec2::splat(24.0));
        let c = Color32::from_rgb(color[0], color[1], color[2]);
        border::border(ui.painter(), sw, 4.0, 1.0, Some(Brush::Solid(c)), Some(p.border));
        let f = font(13.0, 400);
        tools::text_at(ui, pos2(sw.max.x + 10.0, center_y(inner, line_height(ui, &f))), &mt::hex(color), f, p.text);
        let just = r.clicked() && self.color_open != Some(k);
        if r.clicked() {
            self.color_open = if self.color_open == Some(k) { None } else { Some(k) };
        }
        if self.color_open == Some(k) {
            let mut value = c;
            let (changed, keep) = controls::color_popup(ui, id, rect, &mut value, just);
            if !keep {
                self.color_open = None;
            }
            if changed {
                return Some([value.r(), value.g(), value.b(), 255]);
            }
        }
        None
    }

    /// A slider row (`96,*,<value>`): returns the new value.
    #[allow(clippy::too_many_arguments)]
    fn slider_row(ui: &Ui, row: Rect, id: &str, label: &str, value: f64, range: (f32, f32), text: &str, value_w: f32) -> Option<f64> {
        let p = palette(ui.ctx());
        tools::row_label(ui, Rect::from_min_size(row.min, vec2(LABEL_W, row.height())), label);
        let sx = row.min.x + LABEL_W + 12.0;
        let sr = Rect::from_min_max(pos2(sx, row.min.y + (row.height() / 2.0).floor() - 13.0), pos2(row.max.x - value_w - 12.0, row.min.y + (row.height() / 2.0).floor() + 19.0));
        let mut v = value as f32;
        let changed = Slider { minimum: range.0, maximum: range.1, step: 0.0, enabled: true }.show(ui, sr, Id::new(("monitor-slider", id)), &mut v);
        // `TextBlock.option-value`: Normal, right-aligned in its column.
        let f = font(13.0, 400);
        let tw = text_width(ui, text, f.clone());
        tools::text_at(ui, pos2(row.max.x - tw, center_y(row, line_height(ui, &f))), text, f, p.text);
        changed.then_some(v as f64)
    }

    fn options_height(&self) -> f32 {
        match self.test.mode {
            Mode::Grayscale => tools::SLIDER_HEIGHT,
            Mode::Rgb => 40.0 + 3.0 * (8.0 + tools::SLIDER_HEIGHT),
            Mode::Gradient => 40.0 + 8.0 + 40.0,
            Mode::Pattern | Mode::Motion => 40.0 + 8.0 + tools::SLIDER_HEIGHT,
        }
    }

    /// The controls panel; true when Close was clicked.
    fn controls_panel(&mut self, ui: &mut Ui, full: Rect) -> bool {
        let p = palette(ui.ctx());
        let hf = tools::heading_font();
        let nf = font(13.0, 400);
        let row0 = (line_height(ui, &hf) + 2.0 + line_height(ui, &nf)).max(40.0);
        let h = 34.0 + row0 + 12.0 + 40.0 + 12.0 + 1.0 + 12.0 + self.options_height();
        let panel = Rect::from_min_size(pos2(full.min.x + ((full.width() - PANEL_W) / 2.0).round_ties_even(), full.min.y + 16.0), vec2(PANEL_W, h));
        tools::panel(ui, panel);
        // The panel takes the pointer from the pattern under it.
        let _ = ui.interact(panel, Id::new("monitor-panel"), egui::Sense::click());
        let inner = panel.shrink(17.0);
        let r0 = Rect::from_min_size(inner.min, vec2(inner.width(), row0));
        let close = Rect::from_min_size(pos2(r0.max.x - tools::ICON_BUTTON_WIDTH, center_y(r0, 40.0)), vec2(tools::ICON_BUTTON_WIDTH, 40.0));
        let closed = tools::icon_button(ui, close, Id::new("monitor-close"), icon::Close, true).clicked();
        let ty = center_y(r0, line_height(ui, &hf) + 2.0 + line_height(ui, &nf));
        tools::text_at(ui, pos2(r0.min.x, ty), s::MonitorTestWindow_Monitor_test(), hf.clone(), p.text);
        tools::text_at(ui, pos2(r0.min.x, ty + line_height(ui, &hf) + 2.0), s::MonitorTestWindow_Choose_a_test_pattern_and_adjust_its_properties(), nf.clone(), p.text_secondary);
        // The modes: five star columns, `ColumnSpacing="8"`.
        let r1 = Rect::from_min_size(pos2(inner.min.x, r0.max.y + 12.0), vec2(inner.width(), 40.0));
        let col = (r1.width() - 32.0) / 5.0;
        let modes = [
            (icon::Contrast, s::MonitorTestWindow_Grayscale()),
            (icon::Palette, s::MonitorTestWindow_RGB()),
            (icon::Blend, s::MonitorTestWindow_Gradient()),
            (icon::Grid, s::MonitorTestWindow_Pattern()),
            (icon::MoveHorizontal, s::MonitorTestWindow_Motion()),
        ];
        let start = |k: usize| (r1.min.x + k as f32 * (col + 8.0)).floor();
        for (k, (key, text)) in modes.into_iter().enumerate() {
            let x1 = if k == 4 { r1.max.x } else { start(k + 1) - 8.0 };
            let br = Rect::from_min_max(pos2(start(k), r1.min.y), pos2(x1, r1.max.y));
            if Self::mode_button(ui, br, Id::new(("monitor-mode", k)), key, text, self.test.mode == MODES[k]) {
                self.test.mode = MODES[k];
            }
        }
        // The options under a 1px top border.
        let line = Rect::from_min_size(pos2(inner.min.x, r1.max.y + 12.0), vec2(inner.width(), 1.0));
        ui.painter().rect_filled(line, 0.0, p.border);
        let o = Rect::from_min_size(pos2(inner.min.x, line.max.y + 12.0), vec2(inner.width(), self.options_height()));
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let fmt0 = |v: f64| tools::format_0(v as f32);
        match self.test.mode {
            Mode::Grayscale => {
                let row = Rect::from_min_size(o.min, vec2(o.width(), tools::SLIDER_HEIGHT));
                if let Some(v) = Self::slider_row(ui, row, "gray", s::MonitorTestWindow_Brightness(), self.test.gray, (0.0, 255.0), &fmt0(self.test.gray), 52.0) {
                    self.test.gray = v;
                }
            }
            Mode::Rgb => {
                let row = Rect::from_min_size(o.min, vec2(o.width(), 40.0));
                tools::row_label(ui, Rect::from_min_size(row.min, vec2(LABEL_W, 40.0)), s::MonitorTestWindow_Color());
                let br = Rect::from_min_max(pos2(row.min.x + LABEL_W + 12.0, row.min.y), row.max);
                if let Some(c) = self.color_button(ui, br, 0, self.rgb()) {
                    (self.test.red, self.test.green, self.test.blue) = (c[0] as f64, c[1] as f64, c[2] as f64);
                }
                let channels = [(s::MonitorTestWindow_Red(), self.test.red), (s::MonitorTestWindow_Green(), self.test.green), (s::MonitorTestWindow_Blue(), self.test.blue)];
                for (k, (label, value)) in channels.into_iter().enumerate() {
                    let row = Rect::from_min_size(pos2(o.min.x, o.min.y + 48.0 + k as f32 * (tools::SLIDER_HEIGHT + 8.0)), vec2(o.width(), tools::SLIDER_HEIGHT));
                    if let Some(v) = Self::slider_row(ui, row, label, label, value, (0.0, 255.0), &fmt0(value), 52.0) {
                        match k {
                            0 => self.test.red = v,
                            1 => self.test.green = v,
                            _ => self.test.blue = v,
                        }
                    }
                }
            }
            Mode::Gradient => {
                let row = Rect::from_min_size(o.min, vec2(o.width(), 40.0));
                tools::row_label(ui, Rect::from_min_size(row.min, vec2(LABEL_W, 40.0)), s::MonitorTestWindow_Direction());
                let items = direction_items();
                let sel = DIRECTIONS.iter().position(|d| *d == self.test.gradient_direction);
                let cw = combo.auto_width(ui, &items, sel);
                if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(row.min.x + LABEL_W + 12.0, row.min.y), vec2(cw, 40.0)), Id::new("monitor-direction"), &items, sel) {
                    self.test.gradient_direction = DIRECTIONS[i];
                }
                let row = Rect::from_min_size(pos2(o.min.x, o.min.y + 48.0), vec2(o.width(), 40.0));
                tools::row_label(ui, Rect::from_min_size(row.min, vec2(LABEL_W, 40.0)), s::MonitorTestWindow_Colors());
                let area = Rect::from_min_max(pos2(row.min.x + LABEL_W + 12.0, row.min.y), row.max);
                let half = ((area.width() - 8.0) / 2.0).floor();
                let left = Rect::from_min_size(area.min, vec2(half, 40.0));
                let right = Rect::from_min_max(pos2(area.max.x - half, area.min.y), area.max);
                if let Some(c) = self.color_button(ui, left, 1, self.test.gradient_start) {
                    self.test.gradient_start = c;
                }
                if let Some(c) = self.color_button(ui, right, 2, self.test.gradient_end) {
                    self.test.gradient_end = c;
                }
            }
            Mode::Pattern => {
                let row = Rect::from_min_size(o.min, vec2(o.width(), 40.0));
                tools::row_label(ui, Rect::from_min_size(row.min, vec2(LABEL_W, 40.0)), s::MonitorTestWindow_Pattern());
                let items = pattern_items();
                let sel = PATTERNS.iter().position(|x| *x == self.test.pattern);
                let cw = combo.auto_width(ui, &items, sel);
                if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(row.min.x + LABEL_W + 12.0, row.min.y), vec2(cw, 40.0)), Id::new("monitor-pattern"), &items, sel) {
                    self.test.pattern = PATTERNS[i];
                }
                let row = Rect::from_min_size(pos2(o.min.x, o.min.y + 48.0), vec2(o.width(), tools::SLIDER_HEIGHT));
                if let Some(v) = Self::slider_row(ui, row, "size", s::MonitorTestWindow_Cell_size(), self.test.pattern_size, (1.0, 100.0), &fmt0(self.test.pattern_size), 52.0) {
                    self.test.pattern_size = v;
                }
            }
            Mode::Motion => {
                let row = Rect::from_min_size(o.min, vec2(o.width(), 40.0));
                tools::row_label(ui, Rect::from_min_size(row.min, vec2(LABEL_W, 40.0)), s::MonitorTestWindow_Direction());
                let items = motion_items();
                let sel = MOTIONS.iter().position(|x| *x == self.test.motion_direction);
                let cw = combo.auto_width(ui, &items, sel);
                if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(row.min.x + LABEL_W + 12.0, row.min.y), vec2(cw, 40.0)), Id::new("monitor-motion"), &items, sel) {
                    self.test.motion_direction = MOTIONS[i];
                }
                let row = Rect::from_min_size(pos2(o.min.x, o.min.y + 48.0), vec2(o.width(), tools::SLIDER_HEIGHT));
                let text = format!("{} px/s", fmt0(self.test.motion_speed));
                if let Some(v) = Self::slider_row(ui, row, "speed", s::MonitorTestWindow_Speed(), self.test.motion_speed, (100.0, 2000.0), &text, 76.0) {
                    self.test.motion_speed = v;
                }
            }
        }
        closed
    }
}

impl Dialog for MonitorTestWindow {
    fn spec(&self) -> Spec {
        let mut spec = Spec::sized(crate::rs_title(s::MonitorTestWindow_ShareX_Monitor_Test()), (self.bounds.w as f64 / self.scale) as f32, (self.bounds.h as f64 / self.scale) as f32, 0.0, 0.0);
        spec.resizable = false;
        spec.topmost = true;
        spec.skip_taskbar = true;
        spec.borderless = true;
        spec
    }

    fn position(&self, _work_area: sharex_capture::Rect, _size: (u32, u32)) -> Option<(i32, i32)> {
        Some((self.bounds.x, self.bounds.y))
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, Color32::BLACK);
        if self.test.mode == Mode::Motion && !self.frozen {
            let now = Instant::now();
            if let Some(last) = self.last_frame {
                self.motion_offset = mt::advance_motion(self.motion_offset, (now - last).as_secs_f64(), self.test.motion_speed);
            }
            self.last_frame = Some(now);
            ui.ctx().request_repaint();
        } else {
            self.last_frame = None;
        }
        self.draw_pattern(ui, full);
        // The pattern surface: clicks and the wheel.
        let surface = ui.interact(full, Id::new("monitor-surface"), egui::Sense::click());
        let mut out = Vec::new();
        if self.controls_visible && self.controls_panel(ui, full) {
            out.push(Effect::Close);
        }
        if surface.clicked() {
            self.controls_visible = !self.controls_visible;
        }
        if surface.secondary_clicked() {
            out.push(Effect::Close);
        }
        if self.test.mode == Mode::Motion && surface.hovered() {
            // `OnSurfacePointerWheelChanged`: ±50 per wheel event.
            let steps: Vec<f32> = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::MouseWheel { delta, .. } = e { Some(delta.y) } else { None }).collect());
            for dy in steps.into_iter().filter(|d| *d != 0.0) {
                self.test.motion_speed = (self.test.motion_speed + dy.signum() as f64 * 50.0).clamp(100.0, 2000.0);
            }
        }
        let (esc, space, left, right) = ui.input(|i| (i.key_pressed(egui::Key::Escape), i.key_pressed(egui::Key::Space), i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
        if esc {
            out.push(Effect::Close);
        }
        if space {
            self.controls_visible = !self.controls_visible;
        }
        if left {
            self.select_relative_mode(-1);
        }
        if right {
            self.select_relative_mode(1);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_cycle_both_ways() {
        let mut w = MonitorTestWindow::on(sharex_capture::Rect::new(0, 0, 64, 48), 1.0);
        w.select_relative_mode(-1);
        assert_eq!(w.test.mode, Mode::Motion);
        w.select_relative_mode(1);
        w.select_relative_mode(1);
        assert_eq!(w.test.mode, Mode::Rgb);
        assert_eq!(mt::hex(w.rgb()), "#FF0000");
    }
}
