//! `AutoCaptureWindow` (`ShareX/Presentation/AutoCapture/AutoCaptureWindow
//! .axaml(.cs)`): Custom region / Fullscreen, the region text and Select
//! region..., the repeat time, Automatically minimize to tray, Wait until
//! tasks complete, Start/Stop, and the status bar with its progress. The
//! timers are [`super::timer::AutoCaptureTimer`]; the app ticks them and
//! takes the screenshots.

use std::time::Instant;

use egui::{Color32, Id, Rect, Ui, Vec2, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_settings::types::Rectangle;
use sharex_strings::sharex as s;
use sharex_ui::border;
use sharex_ui::controls::{self, Button, CheckBox, DecimalUpDown, RadioButton, content_font};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use super::timer::{self, AutoCaptureTimer};
use crate::dialogs::{Cx, Dialog, Effect, Spec};

/// `Brushes.DodgerBlue`, `Brushes.Gray`.
const DODGER_BLUE: Color32 = Color32::from_rgb(0x1E, 0x90, 0xFF);
const GRAY: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
/// The radio buttons' row and the repeat time's row, as ShareX lays them
/// out (measured: Fluent's `RadioButton` with 14px content, the
/// `NumericUpDown` with `FormatString="0.0"`).
const RADIO_ROW: f32 = 33.0;
const REPEAT_ROW: f32 = 41.0;

pub struct AutoCaptureWindow {
    /// `TaskSettings`: what the screenshots run with.
    pub task: TaskSettings,
    pub timer: AutoCaptureTimer,
    /// `_customRegion`.
    custom_region: Rectangle,
    /// `FullscreenRadio.IsChecked` (the window opens on Custom region).
    fullscreen: bool,
    /// `StatusProgress.Value`, `StatusText.Text`, `StatusIcon.Foreground`.
    progress: i32,
    status_text: String,
    status_color: Option<Color32>,
}

impl AutoCaptureWindow {
    pub fn new(task: TaskSettings, settings: &sharex_settings::ApplicationConfig) -> Self {
        Self {
            task,
            timer: AutoCaptureTimer::default(),
            custom_region: settings.AutoCaptureRegion,
            fullscreen: false,
            progress: 0,
            status_text: s::AutoCaptureWindow_Ready().to_string(),
            status_color: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.timer.running
    }

    /// `Execute`: Start or Stop.
    pub fn execute(&mut self, settings: &sharex_settings::ApplicationConfig) -> Vec<Effect> {
        if self.timer.running { self.stop() } else { self.start(settings) }
    }

    fn start(&mut self, settings: &sharex_settings::ApplicationConfig) -> Vec<Effect> {
        self.timer.start(Instant::now(), settings.AutoCaptureRepeatTime, settings.AutoCaptureWaitUpload);
        if settings.AutoCaptureMinimizeToTray {
            return vec![Effect::AutoCaptureTray(true)];
        }
        Vec::new()
    }

    fn stop(&mut self) -> Vec<Effect> {
        self.timer.stop();
        self.progress = 0;
        self.status_text = s::AutoCaptureWindow_Ready().to_string();
        self.status_color = Some(GRAY);
        Vec::new()
    }

    /// `UpdateStatus`.
    pub fn update_status(&mut self, now: Instant) {
        if !self.timer.running {
            return;
        }
        let (p, text) = self.timer.status(now);
        self.progress = p;
        self.status_text = text;
        self.status_color = Some(DODGER_BLUE);
    }

    /// `--render`: Fullscreen chosen.
    pub fn choose_fullscreen(&mut self, settings: &mut sharex_settings::ApplicationConfig) {
        self.custom_region = settings.AutoCaptureRegion;
        settings.AutoCaptureRegion = screen_bounds();
        self.fullscreen = true;
    }
}

/// `CaptureHelpers.GetScreenBounds()`.
fn screen_bounds() -> Rectangle {
    let screens = sharex_capture::screenshot::screens();
    let b = sharex_capture::screenshot::screen_bounds(&screens);
    Rectangle::new(b.x, b.y, b.w, b.h)
}

/// A `Button` with an `IconPresenter` and a `TextBlock` (`StackPanel
/// Spacing`), its content centred: draws it; true when clicked.
#[allow(clippy::too_many_arguments)]
fn icon_text_button(ui: &Ui, rect: Rect, id: Id, key: &str, text: &str, spacing: f32, enabled: bool) -> bool {
    let p = palette(ui.ctx());
    let r = Button::default().show(ui, rect, id, "", enabled);
    let body = font(13.0, 400);
    let tw = text_width(ui, text, body.clone());
    let inner = Rect::from_min_max(pos2(rect.min.x + 11.0, rect.min.y + 7.0), pos2(rect.max.x - 11.0, rect.max.y - 7.0));
    let content_w = 13.0 + spacing + tw;
    let x = inner.min.x + ((inner.width() - content_w) / 2.0).round_ties_even().max(0.0);
    let color = if enabled { p.text } else { border::group_content(p.text, p.control_background, 0.35, p.background_panel) };
    icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(inner, 13.0)), Vec2::splat(13.0)), key, color);
    let lh = line_height(ui, &body);
    ui.painter().galley(pos2(x + 13.0 + spacing, center_y(inner, lh)), galley(ui, text, body, color, f32::INFINITY), color);
    r.clicked()
}

fn icon_text_button_width(ui: &Ui, text: &str, spacing: f32) -> f32 {
    1.0 + 10.0 + 13.0 + spacing + text_width(ui, text, font(13.0, 400)) + 10.0 + 1.0
}

impl Dialog for AutoCaptureWindow {
    fn spec(&self) -> Spec {
        Spec { resizable: false, maximizable: false, ..Spec::sized(crate::rs_title(s::AutoCaptureWindow_Title()), 460.0, 430.0, 420.0, 390.0) }
    }

    fn single(&self) -> Option<&'static str> {
        Some("auto-capture")
    }

    fn as_auto_capture(&mut self) -> Option<&mut AutoCaptureWindow> {
        Some(self)
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        // `OnClosed`: `Stop`, the tray icon goes.
        self.stop();
        vec![Effect::AutoCaptureTray(false)]
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let mut effects = Vec::new();
        let nf = font(13.0, 400);
        let lh = line_height(ui, &nf);
        let settings = &mut *cx.settings;

        // The status bar (`Padding="12,8"`, a 1px top border).
        let bar_h = 1.0 + 8.0 + lh.max(16.0) + 6.0 + 4.0 + 8.0;
        let bar = Rect::from_min_max(pos2(full.min.x, full.max.y - bar_h), full.max);
        ui.painter().rect_filled(bar, 0.0, p.background_panel);
        ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width(), 1.0)), 0.0, p.border);
        let row = Rect::from_min_size(pos2(bar.min.x + 12.0, bar.min.y + 1.0 + 8.0), vec2(bar.width() - 24.0, lh.max(16.0)));
        let ir = Rect::from_min_size(pos2(row.min.x, center_y(row, 16.0)), Vec2::splat(16.0));
        icons::paint(ui.painter(), ir, icon::Timer, self.status_color.unwrap_or(p.text_secondary));
        let text_rect = Rect::from_min_max(pos2(ir.max.x + 7.0, row.min.y), row.max);
        let status = crate::image_tools::ellipsize(ui, &self.status_text, &nf, text_rect.width());
        ui.painter().galley(pos2(text_rect.min.x, row.min.y), galley(ui, &status, nf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
        // `ProgressBar` `Height="4"`: Fluent's track and the accent indicator.
        let track = Rect::from_min_size(pos2(row.min.x, row.max.y + 6.0), vec2(row.width(), 4.0));
        let f = fluent(ui.ctx());
        border::border(ui.painter(), track, 2.0, 0.0, Some(border::Brush::Solid(f.base_low)), None);
        let done = (track.width() * (self.progress as f32 / 100.0).clamp(0.0, 1.0)).round();
        if done > 0.0 {
            border::border(ui.painter(), Rect::from_min_size(track.min, vec2(done, 4.0)), 2.0, 0.0, Some(border::Brush::Solid(p.accent_start)), None);
        }

        // `Margin="14"`, `RowSpacing="12"`.
        let c = Rect::from_min_max(pos2(full.min.x + 14.0, full.min.y + 14.0), pos2(full.max.x - 14.0, bar.min.y - 14.0));

        // The region panel (`Padding="14"`).
        let panel0 = Rect::from_min_size(c.min, vec2(c.width(), 1.0 + 14.0 + RADIO_ROW + 10.0 + 40.0 + 14.0 + 1.0));
        crate::image_tools::panel(ui, panel0);
        let inner = panel0.shrink(15.0);
        let radio_font = content_font(14.0, 400);
        let half = ((inner.width() - 8.0) / 2.0).floor();
        let radios = [(s::AutoCaptureWindow_CustomRegion(), false), (s::AutoCaptureWindow_Fullscreen(), true)];
        for (k, (text, full_screen)) in radios.into_iter().enumerate() {
            let x = inner.min.x + k as f32 * (half + 8.0);
            let rb = RadioButton { text, text_font: radio_font.clone(), text_color: Some(fluent(ui.ctx()).base_high), padding_left: 8.0, enabled: true };
            let checked = self.fullscreen == full_screen;
            if rb.show(ui, Rect::from_min_size(pos2(x, inner.min.y), vec2(half, RADIO_ROW)), Id::new(("auto-region", k)), checked).clicked() && !checked {
                if full_screen {
                    // `OnFullscreenChanged`.
                    self.custom_region = settings.AutoCaptureRegion;
                    settings.AutoCaptureRegion = screen_bounds();
                } else {
                    // `OnCustomRegionChanged`.
                    settings.AutoCaptureRegion = self.custom_region;
                }
                self.fullscreen = full_screen;
            }
        }
        let region_row = Rect::from_min_size(pos2(inner.min.x, inner.min.y + RADIO_ROW + 10.0), vec2(inner.width(), 40.0));
        let bw = icon_text_button_width(ui, s::AutoCaptureWindow_SelectRegion(), 7.0);
        let button = Rect::from_min_size(pos2(region_row.max.x - bw, region_row.min.y), vec2(bw, 40.0));
        let text_box = Rect::from_min_max(region_row.min, pos2(button.min.x - 8.0, region_row.max.y));
        border::border(ui.painter(), text_box, 4.0, 1.0, Some(border::Brush::Solid(p.control_background)), Some(p.border));
        let region_text = timer::region_text(settings.AutoCaptureRegion);
        let tr = Rect::from_min_max(pos2(text_box.min.x + 1.0 + 9.0, text_box.min.y), pos2(text_box.max.x - 1.0 - 9.0, text_box.max.y));
        let shown = crate::image_tools::ellipsize(ui, &region_text, &nf, tr.width());
        controls::label(ui, tr, &shown, nf.clone(), p.text);
        if icon_text_button(ui, button, Id::new("auto-select-region"), icon::Region, s::AutoCaptureWindow_SelectRegion(), 7.0, !self.fullscreen) {
            effects.push(Effect::AutoCaptureSelectRegion);
        }

        // The repeat panel.
        let panel1 = Rect::from_min_size(pos2(c.min.x, panel0.max.y + 12.0), vec2(c.width(), 1.0 + 14.0 + REPEAT_ROW + 12.0 + 32.0 + 8.0 + 32.0 + 14.0 + 1.0));
        crate::image_tools::panel(ui, panel1);
        let inner = panel1.shrink(15.0);
        let label = s::AutoCaptureWindow_RepeatTime();
        let lw = text_width(ui, label, nf.clone());
        let row = Rect::from_min_size(inner.min, vec2(inner.width(), REPEAT_ROW));
        controls::label(ui, Rect::from_min_size(row.min, vec2(lw, REPEAT_ROW)), label, nf.clone(), p.text_secondary);
        // ShareX's box comes out 121 wide one pixel to the right of its
        // column (measured); the text after it follows.
        let nud = Rect::from_min_size(pos2(row.min.x + lw + 8.0 + 1.0, row.min.y), vec2(121.0, REPEAT_ROW));
        let mut v = settings.AutoCaptureRepeatTime;
        // `Minimum="1" Maximum="86400" Increment="0.1" FormatString="0.0"`.
        if (DecimalUpDown { minimum: 1.0, maximum: 86400.0, increment: 0.1, decimals: 1, enabled: true }).show(ui, nud, Id::new("auto-repeat"), &mut v) {
            settings.AutoCaptureRepeatTime = v;
        }
        controls::label(ui, Rect::from_min_size(pos2(nud.max.x + 8.0, row.min.y), vec2(200.0, REPEAT_ROW)), s::AutoCaptureWindow_Seconds(), nf.clone(), p.text_secondary);
        let mut y = row.max.y + 12.0;
        let checks: [(&str, &mut bool); 2] = [
            (s::AutoCaptureWindow_AutoMinimize(), &mut settings.AutoCaptureMinimizeToTray),
            (s::AutoCaptureWindow_WaitTasks(), &mut settings.AutoCaptureWaitUpload),
        ];
        for (k, (text, value)) in checks.into_iter().enumerate() {
            let cb = CheckBox::new(text);
            let w = cb.width(ui);
            cb.show(ui, Rect::from_min_size(pos2(inner.min.x, y), vec2(w, 32.0)), Id::new(("auto-check", k)), value);
            y += 32.0 + 8.0;
        }

        // Start / Stop (`Width="240" Height="42"`, centred).
        let exec = Rect::from_min_size(pos2(c.min.x + ((c.width() - 240.0) / 2.0).round_ties_even(), panel1.max.y + 12.0), vec2(240.0, 42.0));
        let (key, text) = if self.timer.running { (icon::Stop, s::AutoCaptureWindow_Stop()) } else { (icon::Play, s::AutoCaptureWindow_Start()) };
        let enabled = !timer::is_empty(settings.AutoCaptureRegion);
        if icon_text_button(ui, exec, Id::new("auto-execute"), key, text, 8.0, enabled) {
            effects.extend(self.execute(settings));
        }
        effects
    }
}
