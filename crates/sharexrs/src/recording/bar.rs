//! The recorder bar shown before recording: `ShareX/Presentation/
//! ScreenRecording/ScreenRecorderBarWindow.axaml(.cs)` and
//! `ScreenRecorderBarSettings`, as ShareX builds it for the FFmpeg recorder
//! (`native: false`: no audio, camera or bitrate settings, titled "Screen
//! recorder"), which is the recorder ShareXrs runs.
//!
//! A grip, Record, Start delay (on/off, and 1-5 s from its arrow), Cursor,
//! Mouse highlighter (on/off, and its options from its arrow), Options and
//! Cancel; Options and the highlighter's arrow open a settings panel under
//! the bar. Its edits go to a copy of the capture settings, which the app
//! commits when the bar closes, recording or not.

use egui::{Color32, Id, Pos2, Rect, Ui, pos2, vec2};
use sharex_settings::TaskSettingsCapture;
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{CheckBox, DecimalUpDown, NumericUpDown};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, MenuEntry, Placement, Toggle};
use sharex_ui::theme::{fluent, palette};
use sharex_ui::widgets::{self, line_height, text_width};
use sharex_ui::icons;

/// `Recorder.Record.Background` / `Hover` / `Pressed`.
const RECORD: Color32 = Color32::from_rgb(0xB8, 0x32, 0x3F);
const RECORD_HOVER: Color32 = Color32::from_rgb(0xCA, 0x3E, 0x4B);
const RECORD_PRESSED: Color32 = Color32::from_rgb(0xA0, 0x29, 0x35);

/// `Button.recorder-setup` / `ToggleButton.recorder-setup`: 36 tall,
/// `Padding="10,6"`, inside a `Margin="3,2"`.
const SETUP_HEIGHT: f32 = 36.0;
const ROW_HEIGHT: f32 = SETUP_HEIGHT + 4.0;
const ICON: f32 = 16.0;

/// What the bar asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarAction {
    /// `RequestRecord`.
    Record,
    /// `Cancel`.
    Cancel,
    /// The grip was pressed: the bar moves with the pointer.
    BeginDrag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    Options,
    Highlighter,
}

/// The bar's state (`ScreenRecorderBarWindow`).
pub struct Bar {
    /// `_draft.Capture`.
    pub capture: TaskSettingsCapture,
    gif: bool,
    panel: Option<Panel>,
    options_tab: usize,
    error: Option<String>,
    delay_menu: Option<Flyout<u32>>,
    /// Where the start delay's split control was drawn (the menu's anchor).
    delay_anchor: Rect,
}

/// The parts of one toolbar control.
struct Measure {
    w: f32,
}

impl Bar {
    pub fn new(capture: TaskSettingsCapture, gif: bool) -> Self {
        Self { capture, gif, panel: None, options_tab: 0, error: None, delay_menu: None, delay_anchor: Rect::NOTHING }
    }

    /// The window's title (`Strings.TaskSettingsWindow_ScreenRecorder` for
    /// the FFmpeg recorder).
    pub fn title() -> &'static str {
        s::TaskSettingsWindow_ScreenRecorder()
    }

    /// `ShowError`.
    pub fn show_error(&mut self, message: &str) {
        self.error = Some(message.to_owned());
    }

    /// Escape: closes the panel if one is open, else cancels.
    pub fn escape(&mut self) -> Option<BarAction> {
        if self.delay_menu.take().is_some() {
            return None;
        }
        if self.panel.take().is_some() { None } else { Some(BarAction::Cancel) }
    }

    /// `Deactivated`: `ClosePanel`.
    pub fn deactivated(&mut self) {
        self.panel = None;
        self.delay_menu = None;
    }

    /// The rectangles the window shows (the bar, and the start delay menu
    /// when open), relative to the bar's top-left, for the window region.
    pub fn popup_rects(&self) -> Vec<Rect> {
        self.delay_menu.as_ref().map(|m| m.rects().to_vec()).unwrap_or_default()
    }

    fn labels(&self) -> bool {
        self.capture.ScreenRecordShowButtonLabels
    }

    /// `ButtonContent`'s width: the icon, then (with labels) the label and,
    /// for a toggle, "On"/"Off", 6px apart.
    fn content_width(&self, ui: &Ui, label: &str, toggle: Option<bool>) -> f32 {
        if !self.labels() {
            return ICON;
        }
        let bold = toggle.is_some();
        let mut w = ICON + 6.0 + text_width(ui, label, widgets::font(13.0, if bold { 600 } else { 400 }));
        if let Some(on) = toggle {
            w += 6.0 + text_width(ui, if on { s::ScreenRecorderBar_On() } else { s::ScreenRecorderBar_Off() }, widgets::font(11.0, 600));
        }
        w
    }

    /// A setup button's width: `Padding` 10 each side and the 1px border.
    fn setup_width(&self, ui: &Ui, label: &str, toggle: Option<bool>) -> Measure {
        Measure { w: 10.0 + self.content_width(ui, label, toggle) + 10.0 + 2.0 }
    }

    /// The width of the toolbar row and of each control in it.
    fn row_widths(&self, ui: &Ui) -> (f32, [f32; 6]) {
        let record = self.setup_width(ui, s::ScreenRecorderBar_Record(), None).w;
        let delay = self.setup_width(ui, s::ScreenRecorderBar_StartDelay(), Some(self.capture.ScreenRecordStartDelayEnabled)).w;
        let cursor = self.setup_width(ui, s::ScreenRecorderBar_Cursor(), Some(self.capture.ScreenRecordShowCursor)).w;
        let highlighter = self.setup_width(ui, s::MouseHighlighter(), Some(self.capture.ScreenRecordMouseHighlighter)).w;
        let options = self.setup_width(ui, s::ScreenRecorderBar_Options(), None).w;
        let arrow = 30.0;
        let widths = [record, delay, cursor, highlighter, options, arrow];
        let total = 24.0 + (3.0 + record + 3.0) + (3.0 + delay + arrow + 3.0) + (3.0 + cursor + 3.0) + (3.0 + highlighter + arrow + 3.0)
            + (3.0 + options + 3.0) + (3.0 + 28.0 + 3.0);
        (total, widths)
    }

    /// The bar's size (`SizeToContent`): the border (1) and padding (2)
    /// round the toolbar row, the error and the settings panel, 6 apart.
    pub fn size(&self, ui: &Ui) -> egui::Vec2 {
        let (row, _) = self.row_widths(ui);
        let mut w = row;
        let mut h = ROW_HEIGHT;
        if let Some(e) = &self.error {
            let max = 550.0f32.min(w.max(1.0));
            h += 6.0 + wrapped_height(ui, e, max);
        }
        if let Some(panel) = self.panel {
            let (pw, ph) = self.panel_size(ui, panel);
            w = w.max(pw);
            h += 6.0 + ph;
        }
        vec2(w + 6.0, h + 6.0)
    }

    /// Draws the bar with its top-left at `origin` and gives what it asks.
    pub fn ui(&mut self, ui: &mut Ui, origin: Pos2) -> Option<BarAction> {
        let p = palette(ui.ctx());
        let size = self.size(ui);
        let outer = Rect::from_min_size(origin, size);
        border::border(ui.painter(), outer, 8.0, 1.0, Some(Brush::Solid(p.background_toolbar)), Some(p.border));
        let content = outer.shrink(3.0);
        let (_, [record_w, delay_w, cursor_w, highlighter_w, options_w, arrow_w]) = self.row_widths(ui);
        let row = Rect::from_min_size(content.min, vec2(content.width(), ROW_HEIGHT));
        let mut action = None;
        let mut x = row.min.x;
        // The grip (24x36, centred).
        let grip = Rect::from_min_size(pos2(x, row.min.y + 2.0), vec2(24.0, SETUP_HEIGHT));
        icons::paint(ui.painter(), Rect::from_center_size(grip.center(), vec2(ICON, ICON)), icon::Grip, p.text);
        let gr = ui.interact(grip, Id::new("bar-grip"), egui::Sense::click_and_drag());
        if gr.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
        }
        if gr.contains_pointer() && ui.input(|i| i.pointer.primary_pressed()) {
            action = Some(BarAction::BeginDrag);
        }
        x = grip.max.x;
        let setup = |x: f32, w: f32| Rect::from_min_size(pos2(x + 3.0, row.min.y + 2.0), vec2(w, SETUP_HEIGHT));
        // Record.
        let r = setup(x, record_w);
        if self.button(ui, r, "record", icon::Circle, s::ScreenRecorderBar_Record(), Kind::Record, true).clicked() {
            action = Some(BarAction::Record);
        }
        x = r.max.x + 3.0;
        // Start delay.
        let t = setup(x, delay_w);
        let a = Rect::from_min_size(pos2(t.max.x, t.min.y), vec2(arrow_w, SETUP_HEIGHT));
        let enabled = self.capture.ScreenRecordStartDelayEnabled;
        if self.toggle(ui, t, "delay", icon::Timer, s::ScreenRecorderBar_StartDelay(), enabled, [5.0, 0.0, 0.0, 5.0]).clicked() {
            self.capture.ScreenRecordStartDelayEnabled = !enabled;
        }
        if self.button(ui, a, "delay-arrow", icon::ChevronDown, "", Kind::Arrow, true).clicked() {
            self.panel = None;
            self.open_delay_menu(Rect::from_min_max(t.min, a.max));
        }
        self.delay_anchor = Rect::from_min_max(t.min, a.max);
        x = a.max.x + 3.0;
        // Cursor.
        let c = setup(x, cursor_w);
        let on = self.capture.ScreenRecordShowCursor;
        if self.toggle(ui, c, "cursor", icon::Pointer, s::ScreenRecorderBar_Cursor(), on, [4.0; 4]).clicked() {
            self.capture.ScreenRecordShowCursor = !on;
        }
        x = c.max.x + 3.0;
        // Mouse highlighter.
        let t = setup(x, highlighter_w);
        let a = Rect::from_min_size(pos2(t.max.x, t.min.y), vec2(arrow_w, SETUP_HEIGHT));
        let on = self.capture.ScreenRecordMouseHighlighter;
        if self.toggle(ui, t, "highlighter", icon::PointerClick, s::MouseHighlighter(), on, [5.0, 0.0, 0.0, 5.0]).clicked() {
            self.capture.ScreenRecordMouseHighlighter = !on;
        }
        if self.button(ui, a, "highlighter-arrow", icon::ChevronDown, "", Kind::Arrow, true).clicked() {
            self.toggle_panel(Panel::Highlighter);
        }
        // Options and Cancel are at the right of the row (`Auto` columns
        // after the `*` sources).
        let cancel = Rect::from_min_size(pos2(row.max.x - 3.0 - 28.0, row.center().y - 14.0), vec2(28.0, 28.0));
        let o = Rect::from_min_size(pos2(cancel.min.x - 3.0 - 3.0 - options_w, row.min.y + 2.0), vec2(options_w, SETUP_HEIGHT));
        if self.button(ui, o, "options", icon::Settings, s::ScreenRecorderBar_Options(), Kind::Setup, true).clicked() {
            self.toggle_panel(Panel::Options);
        }
        if self.button(ui, cancel, "cancel", icon::Close, "", Kind::Close, true).on_hover_text(s::TaskSettingsWindow_Cancel()).clicked() {
            action = Some(BarAction::Cancel);
        }
        let mut y = row.max.y;
        if let Some(e) = self.error.clone() {
            y += 6.0;
            let max = 550.0f32.min(content.width());
            let h = wrapped_height(ui, &e, max);
            sharex_ui::controls::wrapped(ui, pos2(content.min.x, y), max, &e, widgets::font(13.0, 600), p.text_secondary, true);
            y += h;
        }
        if let Some(panel) = self.panel {
            y += 6.0;
            let (_, ph) = self.panel_size(ui, panel);
            let rect = Rect::from_min_size(pos2(content.min.x, y), vec2(content.width(), ph));
            self.panel_ui(ui, rect, panel);
        }
        if let Some(menu) = self.delay_menu.as_mut() {
            let out = menu.show(ui, ui.max_rect());
            if let Some(delay) = out.command {
                self.capture.ScreenRecordStartDelay = delay as f32;
            }
            if out.closed {
                self.delay_menu = None;
            }
        }
        let _ = (delay_w, cursor_w);
        action
    }

    /// `ShowPanel` (a second click on the same arrow closes it).
    fn toggle_panel(&mut self, panel: Panel) {
        self.delay_menu = None;
        self.panel = if self.panel == Some(panel) { None } else { Some(panel) };
        if panel == Panel::Highlighter && self.panel.is_some() {
            eprintln!("The mouse highlighter's options in the recorder bar aren't ported yet (Task settings › Mouse highlighter has them).");
        }
    }

    /// Opens the Options panel on its Recording (or Bar) tab.
    pub fn open_options(&mut self, bar_tab: bool) {
        self.panel = Some(Panel::Options);
        self.options_tab = usize::from(bar_tab);
    }

    /// `ShowStartDelayMenu`: 1 to 5 seconds, the current one checked.
    fn open_delay_menu(&mut self, anchor: Rect) {
        let entries = (1..=5u32)
            .map(|d| {
                let mut e = MenuEntry::item(start_delay_label(d as f32), "", d);
                e.toggle = Toggle::Radio;
                e.checked = self.capture.ScreenRecordStartDelay == d as f32;
                e
            })
            .collect();
        let mut menu = Flyout::menu(Id::new("start-delay-menu"), entries, anchor, Placement::BottomEdgeAlignedLeft);
        // A plain `ContextMenu`, not the compact menus.
        menu.style = sharex_ui::menu::MenuStyle::Standard;
        self.delay_menu = Some(menu);
    }

    fn panel_title(panel: Panel) -> &'static str {
        match panel {
            Panel::Options => s::ScreenRecorderBar_Options(),
            Panel::Highlighter => s::MouseHighlighter(),
        }
    }

    /// The settings panel's size: `Padding="12"`, a title row (28), 8, the
    /// content.
    fn panel_size(&self, ui: &Ui, panel: Panel) -> (f32, f32) {
        let (cw, ch) = match panel {
            Panel::Options => self.options_size(ui),
            Panel::Highlighter => (260.0, line_height(ui, &widgets::font(13.0, 400))),
        };
        (cw + 26.0, 13.0 + 28.0 + 8.0 + ch + 13.0)
    }

    fn panel_ui(&mut self, ui: &mut Ui, rect: Rect, panel: Panel) {
        let p = palette(ui.ctx());
        border::border(ui.painter(), rect, 6.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
        let inner = rect.shrink(13.0);
        let title = Rect::from_min_size(inner.min, vec2(inner.width(), 28.0));
        widgets::text_left(ui, title, title.min.x, Self::panel_title(panel), widgets::font(13.0, 600), p.text_secondary);
        let close = Rect::from_min_size(pos2(title.max.x - 28.0, title.min.y), vec2(28.0, 28.0));
        if self.button(ui, close, "panel-close", icon::Close, "", Kind::Close, true).on_hover_text(s::ActionsToolbarWindow_Close()).clicked() {
            self.panel = None;
            return;
        }
        let body = Rect::from_min_max(pos2(inner.min.x, title.max.y + 8.0), inner.max);
        match panel {
            Panel::Options => self.options_ui(ui, body),
            Panel::Highlighter => {
                widgets::text_left(ui, body, body.min.x, s::TaskSettingsWindow_MouseHighlighterOptions(), widgets::font(13.0, 400), p.text_secondary);
            }
        }
    }

    /// `BuildOptionsPanel`: a `TabControl` (Fluent: 48px headers in Inter
    /// 14, a 3px accent pipe 2px above their bottom, 12px content inset)
    /// with the Recording tab (FPS or GIF FPS, fixed duration and its
    /// seconds) and the Bar tab (show before recording, timer, button
    /// labels): `Margin="0,8"`, `Spacing="10"`.
    fn options_size(&self, ui: &Ui) -> (f32, f32) {
        let body = if self.options_tab == 0 { 40.0 + 10.0 + 32.0 + 10.0 + 40.0 } else { 3.0 * 32.0 + 2.0 * 10.0 };
        let labels = [
            if self.gif { s::TaskSettingsWindow_GIFFPS() } else { s::TaskSettingsWindow_ScreenRecordingFPS() },
            s::TaskSettingsWindow_DurationSeconds(),
        ];
        let label_w = labels.iter().map(|l| text_width(ui, l, widgets::font(13.0, 600))).fold(0.0, f32::max);
        let checks = [
            s::TaskSettingsWindow_UseFixedDuration(),
            s::ScreenRecorderBar_ShowBeforeRecording(),
            s::TaskSettingsWindow_ShowRecordingTimer(),
            s::TaskSettingsWindow_ShowRecordingButtonLabels(),
        ];
        let check_w = checks.iter().map(|c| CheckBox::new(c).width(ui)).fold(0.0, f32::max);
        let tab_font = sharex_ui::controls::content_font(14.0, 400);
        let tab_w = text_width(ui, s::TaskSettingsWindow_Recording(), tab_font.clone()) + text_width(ui, s::ScreenRecorderBar_Bar(), tab_font) + 4.0 * 12.0;
        let content = (label_w + 12.0 + 150.0).max(check_w) + 2.0 * TAB_INSET;
        (content.max(tab_w), TAB_HEADER + 10.0 + body + 8.0)
    }

    fn options_ui(&mut self, ui: &mut Ui, rect: Rect) {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        // The tab strip.
        let mut x = rect.min.x;
        let font = sharex_ui::controls::content_font(14.0, 400);
        for (i, title) in [s::TaskSettingsWindow_Recording(), s::ScreenRecorderBar_Bar()].into_iter().enumerate() {
            let tw = text_width(ui, title, font.clone());
            let r = Rect::from_min_size(pos2(x, rect.min.y), vec2(tw + 24.0, TAB_HEADER));
            let resp = ui.interact(r, Id::new(("bar-tab", i)), egui::Sense::click());
            let selected = self.options_tab == i;
            let color = if selected || resp.hovered() { f.base_high } else { f.base_medium };
            widgets::text_left(ui, r, r.min.x + 12.0, title, font.clone(), color);
            if selected {
                let pipe = Rect::from_min_size(pos2(r.min.x + 12.0, r.max.y - 2.0 - 3.0), vec2(tw, 3.0));
                ui.painter().rect_filled(pipe, 1.5, p.accent_icon);
            }
            if resp.clicked() {
                self.options_tab = i;
            }
            x = r.max.x;
        }
        let left = rect.min.x + TAB_INSET;
        let right = rect.max.x - TAB_INSET;
        let mut y = rect.min.y + TAB_HEADER + 10.0;
        let c = &mut self.capture;
        let row = |y: f32, h: f32| Rect::from_min_max(pos2(left, y), pos2(right, y + h));
        let number_rect = |r: Rect| Rect::from_min_size(pos2(r.max.x - 150.0, r.min.y), vec2(150.0, 40.0));
        if self.options_tab == 0 {
            let r = row(y, 40.0);
            let (label, value, max) = if self.gif {
                (s::TaskSettingsWindow_GIFFPS(), &mut c.GIFFPS, 30)
            } else {
                (s::TaskSettingsWindow_ScreenRecordingFPS(), &mut c.ScreenRecordFPS, 120)
            };
            widgets::text_left(ui, r, r.min.x, label, widgets::font(13.0, 600), p.text_secondary);
            let mut v = *value as i64;
            if (NumericUpDown { minimum: 1, maximum: max, enabled: true }).show(ui, number_rect(r), Id::new("bar-fps"), &mut v) {
                *value = v as i32;
            }
            y += 40.0 + 10.0;
            let mut fixed = c.ScreenRecordFixedDuration;
            if CheckBox::new(s::TaskSettingsWindow_UseFixedDuration()).show(ui, row(y, 32.0), Id::new("bar-fixed"), &mut fixed).changed() {
                c.ScreenRecordFixedDuration = fixed;
            }
            y += 32.0 + 10.0;
            let r = row(y, 40.0);
            widgets::text_left(ui, r, r.min.x, s::TaskSettingsWindow_DurationSeconds(), widgets::font(13.0, 600), p.text_secondary);
            let mut d = c.ScreenRecordDuration as f64;
            // A decimal shows as many places as it has ("3", "2.5").
            let decimals = usize::from(d.fract().abs() > 1e-6);
            let spinner = DecimalUpDown { minimum: 0.1, maximum: 86400.0, increment: 0.1, decimals, enabled: c.ScreenRecordFixedDuration };
            if spinner.show(ui, number_rect(r), Id::new("bar-duration"), &mut d) {
                c.ScreenRecordDuration = d as f32;
            }
        } else {
            for (i, (text, value)) in [
                (s::ScreenRecorderBar_ShowBeforeRecording(), &mut c.ScreenRecordShowBar),
                (s::TaskSettingsWindow_ShowRecordingTimer(), &mut c.ScreenRecordShowTimer),
                (s::TaskSettingsWindow_ShowRecordingButtonLabels(), &mut c.ScreenRecordShowButtonLabels),
            ]
            .into_iter()
            .enumerate()
            {
                CheckBox::new(text).show(ui, row(y, 32.0), Id::new(("bar-check", i)), value);
                y += 32.0 + 10.0;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn button(&self, ui: &Ui, rect: Rect, id: &str, key: &str, label: &str, kind: Kind, enabled: bool) -> egui::Response {
        let p = palette(ui.ctx());
        let r = ui.interact(rect, Id::new(("bar", id)), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
        let over = enabled && (r.hovered() || r.is_pointer_button_down_on());
        let pressed = enabled && r.is_pointer_button_down_on();
        let (fill, stroke, fg) = match kind {
            Kind::Record => (if pressed { RECORD_PRESSED } else if over { RECORD_HOVER } else { RECORD }, Some(if pressed { RECORD_PRESSED } else if over { RECORD_HOVER } else { RECORD }), Color32::WHITE),
            Kind::Close => (if pressed { p.control_background } else if over { p.control_background_hover } else { Color32::TRANSPARENT }, None, p.text),
            Kind::Setup | Kind::Arrow => (if over && !pressed { p.control_background_hover } else { p.control_background }, Some(p.border), p.text),
        };
        let radius = match kind {
            Kind::Arrow => [0.0, 5.0, 5.0, 0.0],
            _ => [4.0; 4],
        };
        paint_shell(ui, rect, radius, fill, stroke);
        let content_w = if label.is_empty() { ICON } else { self.content_width(ui, label, None) };
        let inner = rect.shrink(1.0);
        let left = inner.min.x + ((inner.width() - content_w) / 2.0).round_ties_even();
        let icon_rect = Rect::from_min_size(pos2(left, inner.min.y + ((inner.height() - ICON) / 2.0).round_ties_even()), vec2(ICON, ICON));
        icons::paint(ui.painter(), icon_rect, key, fg);
        if !label.is_empty() && self.labels() {
            widgets::text_left(ui, inner, left + ICON + 6.0, label, widgets::font(13.0, 400), fg);
        }
        r
    }

    /// `ToggleControl`: Fluent's `ToggleButton` (base-low fill unchecked),
    /// the accent fill, border and foreground when checked; an "off" icon
    /// for some when off.
    #[allow(clippy::too_many_arguments)]
    fn toggle(&self, ui: &Ui, rect: Rect, id: &str, key: &'static str, label: &str, on: bool, radius: [f32; 4]) -> egui::Response {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let r = ui.interact(rect, Id::new(("bar-toggle", id)), egui::Sense::click());
        let over = r.hovered() || r.is_pointer_button_down_on();
        let pressed = r.is_pointer_button_down_on();
        let (fill, stroke, fg) = if on {
            (if over { p.accent_fill_light[0] } else { p.accent_start }, Some(p.accent_border), p.accent_foreground)
        } else {
            let fill = if pressed {
                f.base_medium_low
            } else if over {
                f.base_high.gamma_multiply(0.1)
            } else {
                f.base_low
            };
            (fill, None, if over { f.base_high } else { p.text_secondary })
        };
        paint_shell(ui, rect, radius, fill, stroke);
        let key = if on { key } else { off_icon(key) };
        let content_w = self.content_width(ui, label, Some(on));
        let inner = rect.shrink(1.0);
        let left = inner.min.x + ((inner.width() - content_w) / 2.0).round_ties_even();
        let icon_fg = if on { p.accent_foreground } else { f.base_high };
        let icon_rect = Rect::from_min_size(pos2(left, inner.min.y + ((inner.height() - ICON) / 2.0).round_ties_even()), vec2(ICON, ICON));
        icons::paint(ui.painter(), icon_rect, key, icon_fg);
        if self.labels() {
            let x = left + ICON + 6.0;
            widgets::text_left(ui, inner, x, label, widgets::font(13.0, 600), fg);
            let x = x + text_width(ui, label, widgets::font(13.0, 600)) + 6.0;
            let state = if on { s::ScreenRecorderBar_On() } else { s::ScreenRecorderBar_Off() };
            widgets::text_left(ui, inner, x, state, widgets::font(11.0, 600), fg.gamma_multiply(0.8));
        }
        r.on_hover_text(format!("{label}\n{}", if id == "cursor" { label.to_owned() } else if on { start_delay_label(self.capture.ScreenRecordStartDelay) } else { s::ScreenRecorderBar_Off().to_owned() }))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Setup,
    Record,
    Arrow,
    Close,
}

/// Fluent's `TabItem` height (`TabItemMinHeight`).
const TAB_HEADER: f32 = 48.0;
/// The `TabControl`'s content inset.
const TAB_INSET: f32 = 12.0;

fn wrapped_height(ui: &Ui, text: &str, width: f32) -> f32 {
    sharex_ui::controls::wrapped(ui, Pos2::ZERO, width, text, widgets::font(13.0, 600), Color32::TRANSPARENT, false)
}

/// A shell with per-corner radii: the uniform-radius border where the
/// corners agree, else the fill clipped per quarter.
fn paint_shell(ui: &Ui, rect: Rect, radius: [f32; 4], fill: Color32, stroke: Option<Color32>) {
    if radius.iter().all(|r| *r == radius[0]) {
        border::border(ui.painter(), rect, radius[0], 1.0, Some(Brush::Solid(fill)), stroke);
        return;
    }
    let mid = rect.center();
    let quads = [
        Rect::from_min_max(rect.min, mid),
        Rect::from_min_max(pos2(mid.x, rect.min.y), pos2(rect.max.x, mid.y)),
        Rect::from_min_max(mid, rect.max),
        Rect::from_min_max(pos2(rect.min.x, mid.y), pos2(mid.x, rect.max.y)),
    ];
    for (quad, r) in quads.into_iter().zip(radius) {
        border::border(&ui.painter().with_clip_rect(quad), rect, r, 1.0, Some(Brush::Solid(fill)), stroke);
    }
}

/// `OffIcon`.
fn off_icon(key: &'static str) -> &'static str {
    match key {
        k if k == icon::Volume => icon::Mute,
        k if k == icon::Microphone => icon::MicrophoneOff,
        k if k == icon::Webcam => icon::WebcamOff,
        k if k == icon::Timer => icon::TimerOff,
        k => k,
    }
}

/// `StartDelayLabel`.
pub fn start_delay_label(delay: f32) -> String {
    match delay {
        1.0 => s::ScreenRecorderBar_StartDelay_1Second().into(),
        2.0 => s::ScreenRecorderBar_StartDelay_2Seconds().into(),
        3.0 => s::ScreenRecorderBar_StartDelay_3Seconds().into(),
        4.0 => s::ScreenRecorderBar_StartDelay_4Seconds().into(),
        5.0 => s::ScreenRecorderBar_StartDelay_5Seconds().into(),
        d => {
            // `{delay:0.##}`.
            let t = format!("{d:.2}");
            let t = t.trim_end_matches('0').trim_end_matches('.');
            format!("{t} {}", s::AutoCaptureWindow_Seconds())
        }
    }
}

/// `ScreenRecorderBarSettings.ApplyCapture`: the bar's settings onto
/// `target`.
pub fn apply_capture(draft: &TaskSettingsCapture, target: &mut TaskSettingsCapture) {
    target.ScreenRecordShowBar = draft.ScreenRecordShowBar;
    target.ScreenRecordSystemAudio = draft.ScreenRecordSystemAudio;
    target.ScreenRecordSystemAudioDeviceId = draft.ScreenRecordSystemAudioDeviceId.clone();
    target.ScreenRecordSystemAudioGain = draft.ScreenRecordSystemAudioGain;
    target.ScreenRecordMicrophone = draft.ScreenRecordMicrophone;
    target.ScreenRecordMicrophoneDeviceId = draft.ScreenRecordMicrophoneDeviceId.clone();
    target.ScreenRecordMicrophoneGain = draft.ScreenRecordMicrophoneGain;
    target.ScreenRecordCamera = draft.ScreenRecordCamera;
    target.ScreenRecordCameraDeviceId = draft.ScreenRecordCameraDeviceId.clone();
    target.ScreenRecordCameraResolution = draft.ScreenRecordCameraResolution.clone();
    target.ScreenRecordCameraFPS = draft.ScreenRecordCameraFPS;
    target.ScreenRecordCameraPosition = draft.ScreenRecordCameraPosition.clone();
    target.ScreenRecordCameraShape = draft.ScreenRecordCameraShape.clone();
    target.ScreenRecordCameraWidthPercent = draft.ScreenRecordCameraWidthPercent;
    target.ScreenRecordCameraMargin = draft.ScreenRecordCameraMargin;
    target.ScreenRecordRequireHardwareEncoder = draft.ScreenRecordRequireHardwareEncoder;
    target.ScreenRecordVideoBitrate = draft.ScreenRecordVideoBitrate;
    target.ScreenRecordFPS = draft.ScreenRecordFPS;
    target.GIFFPS = draft.GIFFPS;
    target.ScreenRecordShowCursor = draft.ScreenRecordShowCursor;
    target.ScreenRecordMouseHighlighter = draft.ScreenRecordMouseHighlighter;
    target.ScreenRecordShowTimer = draft.ScreenRecordShowTimer;
    target.ScreenRecordShowButtonLabels = draft.ScreenRecordShowButtonLabels;
    target.ScreenRecordStartDelayEnabled = draft.ScreenRecordStartDelayEnabled;
    target.ScreenRecordStartDelay = draft.ScreenRecordStartDelay;
    target.ScreenRecordFixedDuration = draft.ScreenRecordFixedDuration;
    target.ScreenRecordDuration = draft.ScreenRecordDuration;
}

/// `PositionNearRegion`: centred over the region, 8px above it (inside
/// the screen's top edge when there's no room), then kept on the work area
/// (`ClampToScreen`). `size` in pixels; the bar's top-left.
pub fn position_near_region(region: sharex_capture::Rect, work: sharex_capture::Rect, size: (i32, i32), scaling: f64) -> (i32, i32) {
    let gap = (8.0 * scaling).ceil() as i32;
    let mut top = region.y - size.1 - gap;
    if top < work.y {
        top = region.y.max(work.y) + gap;
    }
    let mut left = region.x.max(work.x);
    let mut right = region.right().min(work.right());
    if right <= left {
        left = work.x;
        right = work.right();
    }
    clamp_to_work((left + (right - left - size.0) / 2, top), work, size)
}

/// `ClampToScreen`.
pub fn clamp_to_work(at: (i32, i32), work: sharex_capture::Rect, size: (i32, i32)) -> (i32, i32) {
    (at.0.clamp(work.x, work.x.max(work.right() - size.0)), at.1.clamp(work.y, work.y.max(work.bottom() - size.1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sharex_capture::Rect as PxRect;

    #[test]
    fn the_bar_sits_above_the_region_or_inside_the_top_edge() {
        let work = PxRect::new(0, 0, 1920, 1040);
        assert_eq!(position_near_region(PxRect::new(400, 300, 400, 200), work, (300, 46), 1.0), (450, 246));
        // At the top: inside it, 8px down.
        assert_eq!(position_near_region(PxRect::new(0, 0, 400, 200), work, (300, 46), 1.0), (50, 8));
        // Kept on the work area.
        assert_eq!(position_near_region(PxRect::new(1900, 500, 20, 20), work, (300, 46), 1.0), (1620, 446));
    }

    #[test]
    fn start_delay_labels() {
        assert_eq!(start_delay_label(3.0), s::ScreenRecorderBar_StartDelay_3Seconds());
        assert!(start_delay_label(2.5).starts_with("2.5 "));
    }
}
