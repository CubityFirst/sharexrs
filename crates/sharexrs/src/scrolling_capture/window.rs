//! `ScrollingCaptureWindow` (`ShareX.ScreenCaptureLib/Presentation/
//! ScrollingCapture/ScrollingCaptureWindow.axaml(.cs)`): the docked toolbar
//! (Capture..., Upload / Save, Copy, Options..., Help), the result preview
//! (a `ScrollViewer` panned by dragging), the empty state, the status row,
//! and the options overlay. The capture itself (selecting the window, the
//! loop, the region outline) is the app's (`super`), which reports back
//! through [`ScrollingCaptureWindow::event`].

use egui::{Color32, Id, Rect, Ui, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_capture::scrolling::{ScrollMethod, ScrollingCaptureOptions, ScrollingCaptureStatus};
use sharex_settings::TaskSettings;
use sharex_strings::screen_capture_lib as s;
use sharex_ui::border;
use sharex_ui::controls::{self, Button, CheckBox, ComboBox, ComboItem, DecimalUpDown, content_font};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};

/// `Brushes.DodgerBlue`, `IndianRed`, `Goldenrod`, `MediumSeaGreen`.
pub const DODGER_BLUE: Color32 = Color32::from_rgb(0x1E, 0x90, 0xFF);
pub const INDIAN_RED: Color32 = Color32::from_rgb(0xCD, 0x5C, 0x5C);
pub const GOLDENROD: Color32 = Color32::from_rgb(0xDA, 0xA5, 0x20);
pub const MEDIUM_SEA_GREEN: Color32 = Color32::from_rgb(0x3C, 0xB3, 0x71);

/// `Links.DocsScrollingScreenshot`.
pub const HELP_URL: &str = "https://getsharex.com/docs/scrolling-screenshot";

/// The window's `Margin="12,0,12,12"` and `RowSpacing="8"`.
const MARGIN: f32 = 12.0;
const ROW_SPACING: f32 = 8.0;
/// `Button.compact`: `Height="34"`, `Padding="10,4"`.
const BUTTON_HEIGHT: f32 = 34.0;
/// `Border.docked-top-toolbar`: `Padding="8"`, no top border.
const TOOLBAR_HEIGHT: f32 = 8.0 + BUTTON_HEIGHT + 8.0 + 1.0;

/// What the app tells the window (`ScrollingCaptureService`'s results).
pub enum Event {
    /// `SelectWindowAsync` returned false (cancelled).
    SelectionCancelled,
    /// The window is selected: `CaptureSelectedWindowAsync` starts.
    CaptureStarted,
    /// `StartCaptureAsync` returned (the result kept, if any).
    CaptureFinished(ScrollingCaptureStatus, Option<RgbaImage>),
}

/// The status row's icon and text.
#[derive(Clone)]
struct Status {
    icon: &'static str,
    /// `None`: `ShareX.Brush.Text.Secondary`.
    color: Option<Color32>,
    text: String,
}

/// The options overlay's inputs (`LoadOptions` fills them).
#[derive(Clone)]
struct OptionsForm {
    start_delay: i64,
    scroll_delay: i64,
    method: usize,
    amount: i64,
    auto_scroll_top: bool,
    auto_ignore_bottom_edge: bool,
    show_region: bool,
    auto_upload: bool,
}

pub struct ScrollingCaptureWindow {
    /// `_service.Options`: the task's options, edited in place.
    pub options: ScrollingCaptureOptions,
    /// The task the result is uploaded with (`UploadManager.RunImageTask`).
    pub task: TaskSettings,
    pub capturing: bool,
    /// `_captureOperation`.
    pub capture_operation: bool,
    close_requested: bool,
    status: Status,
    result_size: String,
    result: Option<RgbaImage>,
    picture: Option<Tiled>,
    /// `UploadButton`/`CopyButton`/`CaptureButton`/`OptionsButton` enabled.
    capture_enabled: bool,
    result_buttons: bool,
    form: Option<OptionsForm>,
    /// `PreviewScrollViewer.Offset`.
    offset: Vec2,
    /// `_panStart`, `_panStartOffset`.
    pan: Option<(egui::Pos2, Vec2)>,
}

impl ScrollingCaptureWindow {
    pub fn new(options: ScrollingCaptureOptions, task: TaskSettings) -> Self {
        Self {
            options,
            task,
            capturing: false,
            capture_operation: false,
            close_requested: false,
            status: Status { icon: icon::CircleDashed, color: None, text: s::ScrollingCaptureWindow_Ready().to_string() },
            result_size: String::new(),
            result: None,
            picture: None,
            capture_enabled: true,
            result_buttons: false,
            form: None,
            offset: Vec2::ZERO,
            pan: None,
        }
    }

    /// `StartStopAsync`: stops a capture, or selects a window to capture.
    pub fn start_stop(&mut self) -> Vec<Effect> {
        if self.capturing {
            self.status.text = s::ScrollingCaptureWindow_Stopping_capture().to_string();
            return vec![Effect::ScrollingStop];
        }
        if !self.capture_operation {
            // `SelectWindowAsync`: the overlay closes, the window minimizes,
            // 250 ms, the region capture.
            self.capture_operation = true;
            self.form = None;
            return vec![Effect::ScrollingSelect];
        }
        Vec::new()
    }

    /// What the app reports.
    pub fn event(&mut self, event: Event) -> Vec<Effect> {
        match event {
            Event::SelectionCancelled => {
                self.status.text = s::ScrollingCaptureWindow_Selection_cancelled().to_string();
                self.capture_operation = false;
                vec![Effect::RestoreAndActivate]
            }
            Event::CaptureStarted => {
                // `CaptureSelectedWindowAsync`.
                self.capturing = true;
                self.set_capture_controls_enabled(false);
                self.result_size.clear();
                self.reset_preview();
                self.status = Status { icon: icon::Loading, color: Some(DODGER_BLUE), text: s::ScrollingCaptureWindow_Capturing().to_string() };
                Vec::new()
            }
            Event::CaptureFinished(status, result) => {
                self.capturing = false;
                self.set_status(status);
                let mut effects = vec![Effect::ScrollingSound];
                self.result = result;
                self.set_capture_controls_enabled(true);
                if let Some(r) = self.result.clone() {
                    self.load_image(r);
                }
                effects.push(Effect::RestoreAndActivate);
                if self.options.auto_upload {
                    effects.extend(self.upload_result());
                }
                self.capture_operation = false;
                if self.close_requested {
                    effects.push(Effect::Close);
                }
                effects
            }
        }
    }

    /// `OnActivated`: activating the window stops a capture.
    pub fn activated(&mut self) -> Vec<Effect> {
        if self.capturing {
            self.status.text = s::ScrollingCaptureWindow_Stopping_capture().to_string();
            return vec![Effect::ScrollingStop];
        }
        Vec::new()
    }

    fn set_capture_controls_enabled(&mut self, enabled: bool) {
        self.capture_enabled = enabled;
        self.result_buttons = enabled && self.result.is_some();
    }

    /// `SetStatus`.
    fn set_status(&mut self, status: ScrollingCaptureStatus) {
        self.status = match status {
            ScrollingCaptureStatus::Failed => Status { icon: icon::CircleX, color: Some(INDIAN_RED), text: s::ScrollingCaptureWindow_Capture_failed().to_string() },
            ScrollingCaptureStatus::PartiallySuccessful => {
                Status { icon: icon::Warning, color: Some(GOLDENROD), text: s::ScrollingCaptureWindow_Capture_partially_successful().to_string() }
            }
            ScrollingCaptureStatus::Successful => {
                Status { icon: icon::CircleCheck, color: Some(MEDIUM_SEA_GREEN), text: s::ScrollingCaptureWindow_Capture_successful().to_string() }
            }
        };
    }

    /// `LoadImage`.
    fn load_image(&mut self, image: RgbaImage) {
        self.result_size = format!("{}x{}", image.width(), image.height());
        self.picture = Some(Tiled::new(image));
        self.offset = Vec2::ZERO;
        self.result_buttons = true;
    }

    /// `ResetPreview`.
    fn reset_preview(&mut self) {
        self.picture = None;
        self.result_buttons = false;
    }

    /// `UploadResult`: a copy through the task's after capture tasks.
    fn upload_result(&self) -> Vec<Effect> {
        self.result.clone().map(|r| vec![Effect::RunImageTask(r, Box::new(self.task.clone()))]).unwrap_or_default()
    }

    /// `LoadOptions`.
    fn load_options(&mut self) {
        let o = &self.options;
        self.form = Some(OptionsForm {
            start_delay: o.start_delay as i64,
            scroll_delay: o.scroll_delay as i64,
            method: o.scroll_method.index(),
            amount: o.scroll_amount as i64,
            auto_scroll_top: o.auto_scroll_top,
            auto_ignore_bottom_edge: o.auto_ignore_bottom_edge,
            show_region: o.show_region,
            auto_upload: o.auto_upload,
        });
    }

    /// `OnSaveOptionsClick`.
    fn save_options(&mut self) {
        if let Some(f) = self.form.take() {
            let o = &mut self.options;
            o.start_delay = f.start_delay as i32;
            o.auto_scroll_top = f.auto_scroll_top;
            o.scroll_delay = f.scroll_delay as i32;
            o.scroll_method = ScrollMethod::ALL.get(f.method).copied().unwrap_or_default();
            o.scroll_amount = f.amount as i32;
            o.auto_upload = f.auto_upload;
            o.show_region = f.show_region;
            o.auto_ignore_bottom_edge = f.auto_ignore_bottom_edge;
        }
    }

    /// `--render`: the options overlay open.
    pub fn open_options(&mut self) {
        self.load_options();
    }

    /// `--render`: as after a capture.
    pub fn show_result(&mut self, status: ScrollingCaptureStatus, image: RgbaImage) {
        self.set_status(status);
        self.result = Some(image.clone());
        self.load_image(image);
    }

    /// `--render`: as after a cancelled selection.
    pub fn show_cancelled(&mut self) {
        self.status.text = s::ScrollingCaptureWindow_Selection_cancelled().to_string();
    }
}

/// The result as textures of at most 2048 px a side (a scrolling capture
/// can be taller than the GPU's largest texture), an image pixel to a
/// device-independent pixel (Avalonia's 96 DPI `Bitmap`), filtered when the
/// display scales it.
struct Tiled {
    image: RgbaImage,
    tiles: Vec<(sharex_capture::Rect, egui::TextureHandle)>,
}

impl Tiled {
    const SIDE: u32 = 2048;

    fn new(image: RgbaImage) -> Self {
        Self { image, tiles: Vec::new() }
    }

    fn size(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    fn tiles(&mut self, ctx: &egui::Context) -> Vec<(sharex_capture::Rect, egui::TextureId)> {
        if self.tiles.is_empty() {
            let (w, h) = self.image.dimensions();
            for y in (0..h).step_by(Self::SIDE as usize) {
                for x in (0..w).step_by(Self::SIDE as usize) {
                    let (tw, th) = (Self::SIDE.min(w - x), Self::SIDE.min(h - y));
                    let part = image::imageops::crop_imm(&self.image, x, y, tw, th).to_image();
                    let ci = egui::ColorImage::from_rgba_unmultiplied([tw as usize, th as usize], part.as_raw());
                    let tex = ctx.load_texture(format!("scrolling-result-{x}-{y}"), ci, egui::TextureOptions::LINEAR);
                    self.tiles.push((sharex_capture::Rect::new(x as i32, y as i32, tw as i32, th as i32), tex));
                }
            }
        }
        self.tiles.iter().map(|(r, t)| (*r, t.id())).collect()
    }
}

/// A `NumericUpDown` with `FormatString="0"` and its `Increment`.
fn int_up_down(ui: &mut Ui, rect: Rect, id: Id, value: &mut i64, range: (i64, i64), increment: i64) {
    let mut v = *value as f64;
    let spinner = DecimalUpDown { minimum: range.0 as f64, maximum: range.1 as f64, increment: increment as f64, decimals: 0, enabled: true };
    if spinner.show(ui, rect, id, &mut v) {
        *value = v.round() as i64;
    }
}

/// A `TextBlock` in a `Button` (Segoe UI Normal).
fn button_font() -> egui::FontId {
    font(13.0, 400)
}

/// A toolbar `Button.compact` with an `IconPresenter` and a `TextBlock`
/// (`StackPanel Spacing="7"`): its width.
fn toolbar_button_width(ui: &Ui, text: &str) -> f32 {
    1.0 + 10.0 + 13.0 + 7.0 + text_width(ui, text, button_font()) + 10.0 + 1.0
}

fn toolbar_button(ui: &Ui, rect: Rect, id: Id, key: &str, text: &str, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let r = Button::compact(Thickness::hv(10.0, 4.0)).show(ui, rect, id, "", enabled);
    let opacity = if enabled { 1.0 } else { 0.35 };
    let body = button_font();
    let tw = text_width(ui, text, body.clone());
    let content_w = 13.0 + 7.0 + tw;
    let inner = Rect::from_min_max(pos2(rect.min.x + 11.0, rect.min.y + 5.0), pos2(rect.max.x - 11.0, rect.max.y - 5.0));
    let x = inner.min.x + ((inner.width() - content_w) / 2.0).round_ties_even().max(0.0);
    let lh = line_height(ui, &body);
    let top = center_y(inner, lh);
    let color = |c: Color32| if enabled { c } else { border::group_content(c, p.control_background, opacity, p.background_panel) };
    icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(inner, 13.0)), Vec2::splat(13.0)), key, color(p.text));
    let c = color(p.text);
    ui.painter().galley(pos2(x + 20.0, top), galley(ui, text, body, c, f32::INFINITY), c);
    r
}

impl Dialog for ScrollingCaptureWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ScrollingCaptureWindow_Title()), 900.0, 650.0, 620.0, 440.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("scrolling-capture")
    }

    fn close_requested(&mut self) -> Vec<Effect> {
        // `OnClosing`: a capture stops first, then the window closes.
        if self.capturing {
            self.close_requested = true;
            return vec![Effect::ScrollingStop];
        }
        vec![Effect::Close]
    }

    fn activated(&mut self) -> Vec<Effect> {
        ScrollingCaptureWindow::activated(self)
    }

    fn as_scrolling(&mut self) -> Option<&mut ScrollingCaptureWindow> {
        Some(self)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let mut effects = Vec::new();
        let overlay = self.form.is_some();
        let live = !overlay;
        let content = Rect::from_min_max(pos2(full.min.x + MARGIN, full.min.y), pos2(full.max.x - MARGIN, full.max.y - MARGIN));

        // The toolbar.
        let bar = Rect::from_min_size(content.min, vec2(content.width(), TOOLBAR_HEIGHT));
        crate::history_ui::common::docked_toolbar(ui, bar);
        let row = Rect::from_min_max(pos2(bar.min.x + 1.0 + 8.0, bar.min.y + 8.0), pos2(bar.max.x - 1.0 - 8.0, bar.min.y + 8.0 + BUTTON_HEIGHT));
        let mut x = row.min.x;
        let buttons = [
            (icon::ScanLine, s::ScrollingCaptureWindow_Capture(), self.capture_enabled, 0),
            (icon::Upload, s::ScrollingCaptureWindow_Upload_Save(), self.result_buttons, 1),
            (icon::Copy, s::ScrollingCaptureWindow_Copy(), self.result_buttons, 2),
            (icon::Settings, s::ScrollingCaptureWindow_Options(), self.capture_enabled, 3),
        ];
        for (key, text, enabled, k) in buttons {
            let w = toolbar_button_width(ui, text);
            let r = Rect::from_min_size(pos2(x, row.min.y), vec2(w, BUTTON_HEIGHT));
            if toolbar_button(ui, r, Id::new(("scrolling-tb", k)), key, text, enabled).clicked() && live {
                match k {
                    0 => effects.extend(self.start_stop()),
                    1 => effects.extend(self.upload_result()),
                    2 => {
                        if let Some(r) = &self.result {
                            effects.push(Effect::CopyImage(r.clone()));
                        }
                    }
                    _ => self.load_options(),
                }
            }
            x += w + 6.0;
        }
        let help = Rect::from_min_size(pos2(row.max.x - 34.0, row.min.y), Vec2::splat(34.0));
        if crate::image_tools::icon_button(ui, help, Id::new("scrolling-help"), icon::Help, true).clicked() && live {
            effects.push(Effect::OpenUrl(HELP_URL.to_string()));
        }

        // The status row.
        let nf = font(13.0, 400);
        let lh = line_height(ui, &nf).max(17.0);
        let status = Rect::from_min_max(pos2(content.min.x, content.max.y - lh), content.max);
        let ir = Rect::from_min_size(pos2(status.min.x, center_y(status, 17.0)), Vec2::splat(17.0));
        icons::paint(ui.painter(), ir, self.status.icon, self.status.color.unwrap_or(p.text_secondary));
        let size_w = if self.result_size.is_empty() { 0.0 } else { text_width(ui, &self.result_size, nf.clone()) };
        let text_rect = Rect::from_min_max(pos2(ir.max.x + 8.0, status.min.y), pos2(status.max.x - size_w - 8.0, status.max.y));
        controls::label(ui, text_rect, &self.status.text, nf.clone(), p.text_secondary);
        if !self.result_size.is_empty() {
            controls::label(ui, Rect::from_min_max(pos2(status.max.x - size_w, status.min.y), status.max), &self.result_size, nf.clone(), p.text_secondary);
        }

        // The preview panel (`Padding="1"`).
        let panel = Rect::from_min_max(pos2(content.min.x, bar.max.y + ROW_SPACING), pos2(content.max.x, status.min.y - ROW_SPACING));
        crate::image_tools::panel(ui, panel);
        let view = panel.shrink(2.0);
        match self.picture.as_mut() {
            Some(pic) => {
                let (w, h) = pic.size();
                // `Image Stretch="None"` on a 96 DPI `Bitmap`: an image pixel is a
                // device-independent pixel, 1.25 screen pixels at 125% (SCR-02).
                let size = vec2(w as f32, h as f32);
                let max = vec2((size.x - view.width()).max(0.0), (size.y - view.height()).max(0.0));
                let resp = ui.interact(view, Id::new("scrolling-preview"), egui::Sense::drag());
                if live {
                    if resp.drag_started() {
                        self.pan = resp.interact_pointer_pos().map(|p| (p, self.offset));
                    }
                    if resp.dragged()
                        && let (Some((start, from)), Some(now)) = (self.pan, resp.interact_pointer_pos())
                    {
                        self.offset = from - (now - start);
                    }
                    if resp.drag_stopped() {
                        self.pan = None;
                    }
                    if self.pan.is_some() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                    }
                    if ui.rect_contains_pointer(view) {
                        let d = ui.input(|i| i.smooth_scroll_delta);
                        self.offset -= d;
                    }
                }
                self.offset = vec2(self.offset.x.clamp(0.0, max.x), self.offset.y.clamp(0.0, max.y));
                // `Stretch="None"`, centred when smaller than the viewport.
                let x0 = if max.x > 0.0 { view.min.x - self.offset.x } else { view.min.x + ((view.width() - size.x) / 2.0).round_ties_even() };
                let y0 = if max.y > 0.0 { view.min.y - self.offset.y } else { view.min.y + ((view.height() - size.y) / 2.0).round_ties_even() };
                let painter = ui.painter().with_clip_rect(view);
                // On whole screen pixels (Avalonia's layout rounding), so at
                // 100% the filtered texture is drawn pixel for pixel.
                let ppp = ui.ctx().pixels_per_point();
                let origin = pos2((x0 * ppp).round() / ppp, (y0 * ppp).round() / ppp);
                for (tile, tex) in pic.tiles(ui.ctx()) {
                    let r = Rect::from_min_size(origin + vec2(tile.x as f32, tile.y as f32), vec2(tile.w as f32, tile.h as f32));
                    if r.intersects(view) {
                        painter.image(tex, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                }
                crate::dialogs::layout::scroll_bar(ui, view, size.y, self.offset.y, 0.0);
            }
            None => {
                crate::image_tools::empty_state(
                    ui,
                    view,
                    icon::ScrollingCapture,
                    48.0,
                    8.0,
                    &[(s::ScrollingCaptureWindow_Select_scrollable_window(), nf.clone(), p.text_secondary)],
                );
            }
        }

        if overlay {
            effects.extend(self.options_overlay(ui, full));
        }
        effects
    }
}

impl ScrollingCaptureWindow {
    /// `OptionsOverlay`: the modal background and the 500px options panel.
    fn options_overlay(&mut self, ui: &mut Ui, full: Rect) -> Vec<Effect> {
        let p = palette(ui.ctx());
        ui.painter().rect_filled(full, 0.0, p.overlay_modal);
        // Blocks the window under it.
        let _ = ui.interact(full, Id::new("scrolling-overlay-block"), egui::Sense::click_and_drag());
        let Some(mut f) = self.form.clone() else { return Vec::new() };
        let nf = font(13.0, 400);
        let heading_font = font(15.0, 600);
        let check_font = content_font(13.0, 600);
        let label_h = line_height(ui, &nf);
        let heading_h = line_height(ui, &heading_font).max(20.0);
        const CONTROL: f32 = 40.0;
        let page_down = ScrollMethod::ALL.get(f.method) == Some(&ScrollMethod::PageDown);
        // Grid rows: Auto x 4 with `RowSpacing="9"` (a hidden row keeps its spacing).
        let grid_h = CONTROL * 3.0 + if page_down { 0.0 } else { CONTROL } + 9.0 * 3.0;
        let checks_h = 32.0 * 4.0 + 8.0 * 3.0;
        let buttons_h = 40.0;
        let content_h = heading_h + 14.0 + grid_h + 14.0 + checks_h + 14.0 + buttons_h;
        let panel_h = (content_h + 36.0 + 2.0).min(620.0);
        let panel = Rect::from_min_size(
            pos2(full.min.x + ((full.width() - 500.0) / 2.0).round_ties_even(), full.min.y + ((full.height() - panel_h) / 2.0).round_ties_even()),
            vec2(500.0, panel_h),
        );
        crate::image_tools::panel(ui, panel);
        let c = panel.shrink(1.0 + 18.0);
        let mut y = c.min.y;

        // Heading.
        let ir = Rect::from_min_size(pos2(c.min.x, y + ((heading_h - 20.0) / 2.0).floor()), Vec2::splat(20.0));
        icons::paint(ui.painter(), ir, icon::Settings, p.accent_icon);
        let hr = Rect::from_min_max(pos2(ir.max.x + 10.0, y), pos2(c.max.x, y + heading_h));
        crate::image_tools::text_at(ui, pos2(hr.min.x, center_y(hr, line_height(ui, &heading_font))), s::ScrollingCaptureWindow_Options_heading(), heading_font, p.text);
        y += heading_h + 14.0;

        // The 170,*,Auto grid.
        let hints = [s::ScrollingCaptureWindow_Milliseconds(), s::ScrollingCaptureWindow_Times()];
        let hint_w = hints[..if page_down { 1 } else { 2 }].iter().map(|h| text_width(ui, h, nf.clone())).fold(0.0, f32::max);
        let col1 = c.min.x + 170.0 + 8.0;
        let col2 = c.max.x - hint_w;
        let col1_end = col2 - 8.0;
        let label = |ui: &Ui, y: f32, text: &str| {
            controls::label(ui, Rect::from_min_size(pos2(c.min.x, y), vec2(170.0, CONTROL)), text, nf.clone(), p.text);
            let _ = label_h;
        };
        let hint = |ui: &Ui, y: f32, text: &str| {
            controls::label(ui, Rect::from_min_max(pos2(col2, y), pos2(c.max.x, y + CONTROL)), text, nf.clone(), p.text_secondary);
        };
        label(ui, y, s::ScrollingCaptureWindow_Start_delay());
        int_up_down(ui, Rect::from_min_max(pos2(col1, y), pos2(col1_end, y + CONTROL)), Id::new("sc-start-delay"), &mut f.start_delay, (100, 5000), 100);
        hint(ui, y, s::ScrollingCaptureWindow_Milliseconds());
        y += CONTROL + 9.0;
        label(ui, y, s::ScrollingCaptureWindow_Scroll_delay());
        int_up_down(ui, Rect::from_min_max(pos2(col1, y), pos2(col1_end, y + CONTROL)), Id::new("sc-scroll-delay"), &mut f.scroll_delay, (100, 5000), 100);
        hint(ui, y, s::ScrollingCaptureWindow_Milliseconds());
        y += CONTROL + 9.0;
        label(ui, y, s::ScrollingCaptureWindow_Scroll_method());
        let items: Vec<ComboItem> = ScrollMethod::ALL.iter().map(|m| ComboItem::text(m.description())).collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let cw = combo.auto_width(ui, &items, Some(f.method));
        let combo_rect = Rect::from_min_size(pos2(col1, y), vec2(cw, CONTROL));
        y += CONTROL + 9.0;
        if !page_down {
            label(ui, y, s::ScrollingCaptureWindow_Scroll_amount());
            int_up_down(ui, Rect::from_min_max(pos2(col1, y), pos2(col1_end, y + CONTROL)), Id::new("sc-amount"), &mut f.amount, (1, 6), 1);
            hint(ui, y, s::ScrollingCaptureWindow_Times());
            y += CONTROL;
        } else {
            y -= 9.0;
        }
        y += 14.0;

        // The check boxes.
        let checks: [(&str, &mut bool); 4] = [
            (s::ScrollingCaptureWindow_Automatically_scroll_to_top(), &mut f.auto_scroll_top),
            (s::ScrollingCaptureWindow_Auto_ignore_bottom_edge(), &mut f.auto_ignore_bottom_edge),
            (s::ScrollingCaptureWindow_Show_capture_region(), &mut f.show_region),
            (s::ScrollingCaptureWindow_Automatically_upload_save(), &mut f.auto_upload),
        ];
        for (k, (text, value)) in checks.into_iter().enumerate() {
            let cb = CheckBox { text_font: check_font.clone(), ..CheckBox::new(text) };
            let w = cb.width(ui);
            cb.show(ui, Rect::from_min_size(pos2(c.min.x, y), vec2(w, 32.0)), Id::new(("sc-check", k)), value);
            y += 32.0 + 8.0;
        }
        y += 14.0 - 8.0;

        // Cancel, OK (right-aligned, `Spacing="8"`).
        let b = Button::default();
        let ok_w = b.width(ui, s::ScrollingCaptureWindow_OK());
        let cancel_w = b.width(ui, s::ScrollingCaptureWindow_Cancel());
        let ok = Rect::from_min_size(pos2(c.max.x - ok_w, y), vec2(ok_w, 40.0));
        let cancel = Rect::from_min_size(pos2(ok.min.x - 8.0 - cancel_w, y), vec2(cancel_w, 40.0));
        let cancelled = b.show(ui, cancel, Id::new("sc-cancel"), s::ScrollingCaptureWindow_Cancel(), true).clicked();
        let saved = b.show(ui, ok, Id::new("sc-ok"), s::ScrollingCaptureWindow_OK(), true).clicked();

        // The combo box last, so its dropdown is above the rest.
        if let Some(i) = combo.show(ui, combo_rect, Id::new("sc-method"), &items, Some(f.method)) {
            f.method = i;
        }

        self.form = Some(f);
        if saved {
            self.save_options();
            return vec![Effect::ScrollingOptionsSaved(Box::new(self.options.clone()))];
        }
        if cancelled {
            self.form = None;
        }
        Vec::new()
    }
}
