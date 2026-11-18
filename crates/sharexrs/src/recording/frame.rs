//! The recording frame: `ShareX.ScreenCaptureLib/Presentation/
//! ScreenRecording/ScreenRecordWindow.axaml(.cs)` and `RecordingRegionBorder`.
//!
//! One transparent, topmost tool window holds the 1px dashed border around
//! the recorded area and the toolbar (timer, Start/Stop, Pause/Resume,
//! Restart, Abort, the inline abort confirmation) 3px below it, or above it
//! when it doesn't fit below. As ShareX, the window is cut to the border
//! ring and the toolbar (`SetWindowRgn`), so the area inside stays
//! clickable and nothing of the window is over what's recorded.

use std::time::{Duration, Instant};

use egui::{Color32, Pos2, Rect, Ui, pos2, vec2};
use sharex_capture::Rect as PxRect;
use sharex_screen_recording::session::{ScreenRecordState, ScreenRecordingStatus};
use sharex_strings::screen_capture_lib as s;
use sharex_ui::border::{self, Brush, Part};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{self, line_height, text_width};
use sharex_ui::{controls, icons};

/// `BorderPixels`.
pub const BORDER: i32 = 1;
/// `ToolbarGapPixels`.
pub const GAP: i32 = 3;
const TIMER_WIDTH: f32 = 112.0;
const ACTION_BUTTON_WIDTH: f32 = 86.0;
const COMPACT_ACTION_BUTTON_WIDTH: f32 = 40.0;
const ACTION_BUTTON_COUNT: f32 = 4.0;
const TOOLBAR_WIDTH: f32 = 461.0;
pub const TOOLBAR_HEIGHT: f32 = 42.0;

/// `Brushes.Goldenrod` and `Brushes.Red`.
pub const GOLDENROD: Color32 = Color32::from_rgb(0xDA, 0xA5, 0x20);
pub const RED: Color32 = Color32::from_rgb(0xFF, 0x00, 0x00);
/// `RecordingRegionBorder.BackdropPen`: #DC141414.
const BACKDROP: [u8; 4] = [20, 20, 20, 220];

/// `System.Diagnostics.Stopwatch`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stopwatch {
    elapsed: Duration,
    started: Option<Instant>,
}

impl Stopwatch {
    pub fn start(&mut self) {
        self.started.get_or_insert_with(Instant::now);
    }

    pub fn stop(&mut self) {
        if let Some(t) = self.started.take() {
            self.elapsed += t.elapsed();
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed + self.started.map_or(Duration::ZERO, |t| t.elapsed())
    }

    pub fn is_running(&self) -> bool {
        self.started.is_some()
    }
}

/// `TimeSpan.ToString("mm\\:ss\\:ff")`: minutes (hours dropped), seconds
/// and hundredths, each truncated.
pub fn format_timer(d: Duration) -> String {
    let cs = d.as_millis() / 10;
    format!("{:02}:{:02}:{:02}", (cs / 6000) % 60, (cs / 100) % 60, cs % 100)
}

/// Where the parts of the window are, in physical pixels from its top-left
/// (`ConfigureGeometry`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub frame_left: i32,
    pub frame_top: i32,
    pub frame_width: i32,
    pub frame_height: i32,
    pub toolbar_left: i32,
    pub toolbar_top: i32,
    pub toolbar_width: i32,
    pub toolbar_height: i32,
    pub width: i32,
    pub height: i32,
}

impl Geometry {
    /// `ConfigureGeometry`: the toolbar centred under the frame (above it
    /// when it fits only there on `work_area`, the working area of the
    /// screen with the recording's top-left).
    pub fn new(capture: PxRect, toolbar_width: f32, scaling: f64, work_area: Option<PxRect>) -> Self {
        let scaling = scaling.max(0.5);
        let frame_width = capture.w + BORDER * 2;
        let frame_height = capture.h + BORDER * 2;
        let toolbar_w = (toolbar_width as f64 * scaling).ceil() as i32;
        let toolbar_h = (TOOLBAR_HEIGHT as f64 * scaling).ceil() as i32;
        let content_width = frame_width.max(toolbar_w);
        let frame_left = (content_width - frame_width) / 2;
        let toolbar_left = (content_width - toolbar_w) / 2;
        let mut above = false;
        if let Some(work) = work_area {
            let frame_top = capture.y - BORDER;
            let frame_bottom = frame_top + frame_height;
            let fits_below = frame_bottom + GAP + toolbar_h <= work.bottom();
            let fits_above = frame_top - GAP - toolbar_h >= work.y;
            above = !fits_below && fits_above;
        }
        let frame_top = if above { toolbar_h + GAP } else { 0 };
        let toolbar_top = if above { 0 } else { frame_height + GAP };
        // `Height = frameHeight + gap + ToolbarHeight` (DIPs).
        let height = frame_height + GAP + (TOOLBAR_HEIGHT as f64 * scaling).round() as i32;
        Self {
            frame_left,
            frame_top,
            frame_width,
            frame_height,
            toolbar_left,
            toolbar_top,
            toolbar_width: toolbar_w,
            toolbar_height: toolbar_h,
            width: content_width,
            height,
        }
    }

    /// The window's top-left for a recording at `capture` (`Position`).
    pub fn position(&self, capture: PxRect) -> (i32, i32) {
        (capture.x - self.frame_left - BORDER, capture.y - self.frame_top - BORDER)
    }

    /// `RecordingRegion` for the window at `position`.
    pub fn recording_region(&self, position: (i32, i32), size: (i32, i32)) -> PxRect {
        PxRect::new(position.0 + self.frame_left + BORDER, position.1 + self.frame_top + BORDER, size.0, size.1)
    }
}

/// What a click asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameAction {
    StartStop,
    PauseResume,
    Restart,
    /// `RequestAbortRecording` (asks first with `AskConfirmationOnAbort`).
    Abort,
    CancelAbort,
    ConfirmAbort,
    /// The timer was pressed while it can move (waiting or paused).
    BeginDrag,
}

/// `ScreenRecordWindow`'s state, without the window.
pub struct Frame {
    pub capture_size: (i32, i32),
    pub status: ScreenRecordingStatus,
    pub show_timer: bool,
    pub show_labels: bool,
    pub ask_confirmation_on_abort: bool,
    /// `Duration` (a fixed duration counts down).
    pub duration: f32,
    pub confirming_abort: bool,
    pub accent: Color32,
    pub timer: Stopwatch,
    pub countdown: Duration,
    pub is_countdown: bool,
    start_text: &'static str,
    start_icon: &'static str,
    pause_text: &'static str,
    pause_icon: &'static str,
    restart_enabled: bool,
    /// The tray icon: its picture (key, colour) and text, as `SetTrayIcon`
    /// and `SetTrayText` last set them; `None` is a progress percentage.
    pub tray: TrayState,
    /// The tray menu's state (`SetTrayMenuEnabled`, headers).
    pub tray_menu_enabled: bool,
    pub hidden: bool,
    /// The timer's text, fixed (for `--render`).
    pub timer_text_override: Option<String>,
}

/// What the frame's own tray icon shows.
#[derive(Clone, Debug, PartialEq)]
pub struct TrayState {
    pub icon: TrayIcon,
    pub text: String,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrayIcon {
    /// `TrayIconImage.CreateIconBytes(icon, color)`; `None` is the
    /// taskbar's theme colour.
    Glyph(&'static str, Option<Color32>),
    /// `Helpers.GetProgressIconBytes(progress, #8C0024)`.
    Progress(i32),
}

/// `Color.Gold`.
pub const GOLD: Color32 = Color32::from_rgb(0xFF, 0xD7, 0x00);

impl Frame {
    /// `new ScreenRecordWindow(region)` with the recording's options, then
    /// `ChangeState(Waiting)`.
    pub fn new(capture_size: (i32, i32), show_timer: bool, show_labels: bool, ask_confirmation_on_abort: bool, duration: f32) -> Self {
        let mut f = Self {
            capture_size,
            status: ScreenRecordingStatus::Waiting,
            show_timer,
            show_labels,
            ask_confirmation_on_abort,
            duration,
            confirming_abort: false,
            accent: GOLDENROD,
            timer: Stopwatch::default(),
            countdown: Duration::ZERO,
            is_countdown: false,
            start_text: s::ScreenRecordWindow_Start(),
            start_icon: icon::CirclePlay,
            pause_text: s::ScreenRecordWindow_Pause(),
            pause_icon: icon::Pause,
            restart_enabled: false,
            tray: TrayState { icon: TrayIcon::Glyph(icon::VideoOff, Some(GOLD)), text: "ShareX".into(), visible: false },
            tray_menu_enabled: false,
            hidden: false,
            timer_text_override: None,
        };
        f.change_state(ScreenRecordState::Waiting, ScreenRecordingStatus::Waiting);
        f
    }

    /// `CurrentToolbarWidth`.
    fn current_toolbar_width(&self) -> f32 {
        TOOLBAR_WIDTH - TIMER_WIDTH + if self.show_timer { TIMER_WIDTH } else { 0.0 }
            - if self.show_labels { 0.0 } else { (ACTION_BUTTON_WIDTH - COMPACT_ACTION_BUTTON_WIDTH) * ACTION_BUTTON_COUNT }
    }

    /// `DisplayedToolbarWidth`.
    pub fn toolbar_width(&self) -> f32 {
        if self.confirming_abort { TOOLBAR_WIDTH } else { self.current_toolbar_width() }
    }

    fn set_tray_text(&mut self, text: String) {
        self.tray.text = text.chars().take(63).collect();
    }

    /// `ChangeState`: the worker's state, with the status it left.
    pub fn change_state(&mut self, state: ScreenRecordState, status: ScreenRecordingStatus) {
        self.status = status;
        match state {
            ScreenRecordState::Waiting => {
                self.set_tray_text(format!("ShareX - {}", s::ScreenRecordForm_StartRecording_Waiting___()));
                self.tray.icon = TrayIcon::Glyph(icon::VideoOff, Some(GOLD));
                self.tray_menu_enabled = false;
                self.tray.visible = true;
            }
            ScreenRecordState::BeforeStart => {
                self.tray_menu_enabled = true;
                self.update_ui();
            }
            ScreenRecordState::AfterStart => self.update_ui(),
            ScreenRecordState::AfterRecordingStart => {
                self.start_recording_timer();
                self.update_ui();
            }
            ScreenRecordState::RecordingEnd => {
                self.stop_recording_timer();
                self.update_ui();
            }
            ScreenRecordState::Encoding => {
                self.hidden = true;
                self.tray_menu_enabled = false;
                self.set_tray_text(format!("ShareX - {}", s::ScreenRecordForm_StartRecording_Encoding___()));
                self.tray.icon = TrayIcon::Glyph(icon::FileVideo, None);
            }
        }
    }

    /// `ChangeStateProgress`.
    pub fn change_state_progress(&mut self, progress: i32) {
        self.set_tray_text(format!("ShareX - {} {progress}%", s::ScreenRecordForm_StartRecording_Encoding___()));
        if !self.tray.visible {
            return;
        }
        self.tray.icon = if progress >= 0 { TrayIcon::Progress(progress) } else { TrayIcon::Glyph(icon::FileVideo, None) };
    }

    /// `StartCountdown`.
    pub fn start_countdown(&mut self, milliseconds: u32) {
        self.is_countdown = true;
        self.countdown = Duration::from_millis(milliseconds as u64);
        self.timer.start();
    }

    /// `StartRecordingTimer`.
    fn start_recording_timer(&mut self) {
        if self.is_countdown {
            self.timer.reset();
            self.is_countdown = false;
        }
        if self.duration > 0.0 {
            self.is_countdown = true;
            self.countdown = Duration::from_secs_f32(self.duration);
        }
        self.accent = RED;
        self.timer.start();
    }

    /// `StopRecordingTimer`.
    fn stop_recording_timer(&mut self) {
        self.timer.stop();
    }

    /// `PauseResumeRecording` pausing in process (the native recorder):
    /// `StopRecordingTimer`, `UpdateUI`.
    pub fn paused_in_process(&mut self) {
        self.status = ScreenRecordingStatus::Paused;
        self.stop_recording_timer();
        self.update_ui();
    }

    /// `PauseResumeRecording` resuming in process: `Timer.Start`,
    /// `UpdateUI` (which leaves the accent goldenrod, as ShareX's does).
    pub fn resumed_in_process(&mut self) {
        self.status = ScreenRecordingStatus::Recording;
        self.timer.start();
        self.update_ui();
    }

    /// The UI side of `RestartRecording` (the status is the control's).
    pub fn restarted(&mut self, status: ScreenRecordingStatus) {
        self.status = status;
        self.timer.reset();
        self.is_countdown = false;
        self.stop_recording_timer();
        self.update_ui();
    }

    /// `ShowAbortConfirmation`.
    pub fn show_abort_confirmation(&mut self, show: bool) {
        self.confirming_abort = show;
    }

    /// The timer's text (`UpdateTimer`).
    pub fn timer_text(&self) -> String {
        if let Some(t) = &self.timer_text_override {
            return t.clone();
        }
        let value = if self.is_countdown { self.countdown.saturating_sub(self.timer.elapsed()) } else { self.timer.elapsed() };
        format_timer(value)
    }

    /// The refresh timer (50 ms) runs with the stopwatch.
    pub fn ticking(&self) -> bool {
        self.timer.is_running()
    }

    /// `UpdateUI`.
    pub fn update_ui(&mut self) {
        self.show_abort_confirmation(false);
        match self.status {
            ScreenRecordingStatus::Working => {
                self.set_tray_text(format!("ShareX - {}", s::ScreenRecordForm_StartRecording_Click_tray_icon_to_stop_recording_()));
                self.tray.icon = TrayIcon::Glyph(icon::Video, Some(RED));
                self.start_text = s::ScreenRecordForm_Stop();
                self.start_icon = icon::Stop;
                self.restart_enabled = false;
                self.accent = GOLDENROD;
            }
            ScreenRecordingStatus::Waiting | ScreenRecordingStatus::Paused => {
                let paused = self.status == ScreenRecordingStatus::Paused;
                self.set_tray_text(format!(
                    "ShareX - {}",
                    if paused {
                        s::ScreenRecordForm_StartRecording_Click_tray_icon_to_stop_recording_()
                    } else {
                        s::ScreenRecordForm_StartRecording_Click_tray_icon_to_start_recording_()
                    }
                ));
                self.tray.icon = TrayIcon::Glyph(icon::VideoOff, Some(GOLD));
                self.start_text = if paused { s::ScreenRecordForm_Stop() } else { s::ScreenRecordForm_Start() };
                self.start_icon = if paused { icon::Stop } else { icon::CirclePlay };
                self.pause_text = s::Resume();
                self.pause_icon = icon::Play;
                self.restart_enabled = paused;
                self.accent = GOLDENROD;
            }
            ScreenRecordingStatus::Recording => {
                self.tray.icon = TrayIcon::Glyph(icon::Video, Some(RED));
                self.start_text = s::ScreenRecordForm_Stop();
                self.start_icon = icon::Stop;
                self.pause_text = s::Pause();
                self.pause_icon = icon::Pause;
                self.restart_enabled = true;
            }
            _ => {}
        }
    }

    /// The tray menu's entries: (header, icon, enabled), in ShareX's order
    /// (`_trayStartItem`, pause, restart, abort).
    pub fn tray_menu(&self) -> [(&'static str, &'static str, bool); 4] {
        let enabled = self.tray_menu_enabled;
        let restart = enabled && matches!(self.status, ScreenRecordingStatus::Recording | ScreenRecordingStatus::Paused);
        let waiting_or_paused = matches!(self.status, ScreenRecordingStatus::Waiting | ScreenRecordingStatus::Paused);
        let stop = matches!(self.status, ScreenRecordingStatus::Working | ScreenRecordingStatus::Recording | ScreenRecordingStatus::Paused);
        // `_trayStartItem.Header` follows `StartText` once the UI updated;
        // it starts as `ScreenRecordForm_Start`.
        let start = if self.start_text == s::ScreenRecordWindow_Start() { s::ScreenRecordForm_Start() } else { self.start_text };
        let pause = if self.pause_text == s::ScreenRecordWindow_Pause() { s::Pause() } else { self.pause_text };
        [
            (start, if stop { icon::Stop } else { icon::CirclePlay }, enabled),
            (pause, if waiting_or_paused { icon::Play } else { icon::Pause }, enabled),
            (s::ScreenRecordWindow_Restart(), icon::Reset, restart),
            (s::ScreenRecordWindow_Abort(), icon::Close, enabled),
        ]
    }

    /// Whether the timer can be dragged (`OnTimerPointerPressed`).
    pub fn can_drag(&self) -> bool {
        matches!(self.status, ScreenRecordingStatus::Waiting | ScreenRecordingStatus::Paused)
    }

    /// Draws the border and the toolbar in `ui` (the whole window, at
    /// `geometry`), and gives what was clicked.
    pub fn ui(&mut self, ui: &mut Ui, g: &Geometry) -> Option<FrameAction> {
        let ppp = ui.ctx().pixels_per_point();
        paint_border(ui, g, self.accent);
        let toolbar = Rect::from_min_size(
            pos2(g.toolbar_left as f32 / ppp, g.toolbar_top as f32 / ppp),
            vec2(self.toolbar_width(), TOOLBAR_HEIGHT),
        );
        self.toolbar_ui(ui, toolbar)
    }

    /// The toolbar alone at `toolbar` (for `--render`).
    pub fn toolbar_only(&mut self, ui: &mut Ui, toolbar: Rect) -> Option<FrameAction> {
        self.toolbar_ui(ui, toolbar)
    }

    fn toolbar_ui(&mut self, ui: &mut Ui, toolbar: Rect) -> Option<FrameAction> {
        let p = palette(ui.ctx());
        // `Border#Toolbar`: CornerRadius 8, 1px border, the toolbar colour.
        border::border(ui.painter(), toolbar, 8.0, 1.0, Some(Brush::Solid(p.background_toolbar)), Some(p.border));
        let inner = toolbar.shrink(1.0);
        let mut action = None;
        if self.confirming_abort {
            // `Grid#AbortConfirmation`: "*,Auto,Auto", ColumnSpacing 1.
            let abort = Rect::from_min_max(pos2(inner.max.x - ACTION_BUTTON_WIDTH, inner.min.y), inner.max);
            let cancel = Rect::from_min_size(pos2(abort.min.x - 1.0 - ACTION_BUTTON_WIDTH, inner.min.y), vec2(ACTION_BUTTON_WIDTH, inner.height()));
            let text = Rect::from_min_max(pos2(inner.min.x + 10.0, inner.min.y), pos2(cancel.min.x - 1.0 - 10.0, inner.max.y));
            widgets::text_left(ui, text, text.min.x, s::ScreenRecordWindow_Abort_this_recording(), widgets::font(13.0, 600), p.text_secondary);
            if action_button(ui, cancel, "abort-cancel", Corners::NONE, Content::Text(s::ScreenRecordWindow_Cancel()), true).clicked() {
                action = Some(FrameAction::CancelAbort);
            }
            if action_button(ui, abort, "abort-confirm", Corners::RIGHT, Content::Text(s::ScreenRecordWindow_Abort()), true).clicked() {
                action = Some(FrameAction::ConfirmAbort);
            }
            return action;
        }
        let mut x = inner.min.x;
        if self.show_timer {
            let timer = Rect::from_min_size(inner.min, vec2(TIMER_WIDTH, inner.height()));
            fill_corners(ui, timer, Corners::LEFT, p.background_panel);
            // A StackPanel (Spacing 9) of the 8px status dot and the timer
            // text, centred.
            let font = widgets::mono(14.0);
            let text = self.timer_text();
            let tw = text_width(ui, &text, font.clone());
            let w = 8.0 + 9.0 + tw;
            let left = timer.min.x + ((timer.width() - w) / 2.0).round_ties_even();
            let th = line_height(ui, &font);
            let panel_top = timer.min.y + ((timer.height() - th.max(8.0)) / 2.0).round_ties_even();
            let dot = Rect::from_min_size(pos2(left, panel_top + ((th - 8.0) / 2.0).round_ties_even()), vec2(8.0, 8.0));
            border::ellipse(ui.painter(), dot, self.accent);
            let g = widgets::galley(ui, &text, font, p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2(left + 17.0, panel_top), g, p.text_secondary);
            let r = ui.interact(timer, ui.id().with("timer"), egui::Sense::click_and_drag());
            if self.can_drag() {
                if r.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                }
                if r.is_pointer_button_down_on() && r.drag_started() || r.contains_pointer() && ui.input(|i| i.pointer.primary_pressed()) {
                    action = Some(FrameAction::BeginDrag);
                }
            }
            x = timer.max.x;
        }
        let bw = if self.show_labels { ACTION_BUTTON_WIDTH } else { COMPACT_ACTION_BUTTON_WIDTH };
        let buttons: [(&str, Corners, &str, &str, bool, FrameAction); 4] = [
            ("start", Corners::LEFT, self.start_icon, self.start_text, true, FrameAction::StartStop),
            ("pause", Corners::NONE, self.pause_icon, self.pause_text, true, FrameAction::PauseResume),
            ("restart", Corners::NONE, icon::Reset, s::ScreenRecordWindow_Restart(), self.restart_enabled, FrameAction::Restart),
            ("abort", Corners::RIGHT, icon::Close, s::ScreenRecordWindow_Abort(), true, FrameAction::Abort),
        ];
        for (id, corners, key, text, enabled, a) in buttons {
            let rect = Rect::from_min_size(pos2(x, inner.min.y), vec2(bw, inner.height()));
            let content = if self.show_labels { Content::IconText(key, text) } else { Content::Icon(key) };
            let r = action_button(ui, rect, id, corners, content, enabled);
            let r = if self.show_labels { r } else { r.on_hover_text(text) };
            if r.clicked() {
                action = Some(a);
            }
            x += bw + 1.0;
        }
        action
    }
}

/// A rounded rectangle's corner radii (top-left, top-right, bottom-right,
/// bottom-left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corners([f32; 4]);

impl Corners {
    pub const NONE: Self = Self([0.0; 4]);
    /// `left-rounded`: `7,0,0,7`.
    pub const LEFT: Self = Self([7.0, 0.0, 0.0, 7.0]);
    /// `right-rounded`: `0,7,7,0`.
    pub const RIGHT: Self = Self([0.0, 7.0, 7.0, 0.0]);
}

/// Fills `rect` with `corners` rounded, antialiased as Skia's GPU backend
/// does (half a pixel minus the signed distance to the edge).
fn fill_corners(ui: &Ui, rect: Rect, corners: Corners, color: Color32) {
    if corners == Corners::NONE {
        ui.painter().rect_filled(rect, 0.0, color);
        return;
    }
    // Each rounded corner is a quarter of a rounded rectangle of that
    // radius; the rest is square.
    let [tl, tr, br, bl] = corners.0;
    let painter = ui.painter();
    let mid = rect.center();
    let quads = [
        (Rect::from_min_max(rect.min, mid), tl, Rect::from_min_size(rect.min, rect.size())),
        (Rect::from_min_max(pos2(mid.x, rect.min.y), pos2(rect.max.x, mid.y)), tr, rect),
        (Rect::from_min_max(mid, rect.max), br, rect),
        (Rect::from_min_max(pos2(rect.min.x, mid.y), pos2(mid.x, rect.max.y)), bl, rect),
    ];
    for (quad, radius, full) in quads {
        if radius <= 0.0 {
            painter.rect_filled(quad, 0.0, color);
        } else {
            border::paint(&painter.with_clip_rect(quad), &[Part::Border { rect: full, radius, thickness: 0.0, background: Some(Brush::Solid(color)), border: None }], 1.0);
        }
    }
}

enum Content<'a> {
    Icon(&'a str),
    IconText(&'a str, &'a str),
    /// A string `Content` (the abort confirmation's buttons).
    Text(&'a str),
}

/// `Button.recorder-action`: no border, the control colour (lighter while
/// the pointer is over it), the icon 15px in the accent icon colour and the
/// label 7px after it, centred; disabled at 35% opacity.
fn action_button(ui: &Ui, rect: Rect, id: &str, corners: Corners, content: Content, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let id = ui.id().with(("recorder-action", id));
    let r = ui.interact(rect, id, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let over = enabled && (r.hovered() || r.is_pointer_button_down_on());
    let pressed = enabled && r.is_pointer_button_down_on();
    let fill = if over && !pressed { p.control_background_hover } else { p.control_background };
    let opacity = if enabled { 1.0 } else { 0.35 };
    let shown_fill = if enabled { fill } else { mix(p.background_toolbar, fill, opacity) };
    fill_corners(ui, rect, corners, shown_fill);
    let fade = |c: Color32| if enabled { c } else { border::group_content(c, fill, opacity, p.background_toolbar) };
    // `Padding="8,4"`, content centred.
    let inner = Rect::from_min_max(pos2(rect.min.x + 8.0, rect.min.y + 4.0), pos2(rect.max.x - 8.0, rect.max.y - 4.0));
    let center = |avail: f32, size: f32| ((avail - size) / 2.0).round_ties_even();
    match content {
        Content::Icon(key) => {
            let at = pos2(inner.min.x + center(inner.width(), 15.0), inner.min.y + center(inner.height(), 15.0));
            icons::paint(ui.painter(), Rect::from_min_size(at, vec2(15.0, 15.0)), key, fade(p.accent_icon));
        }
        Content::IconText(key, text) => {
            let font = widgets::font(13.0, 400);
            let tw = text_width(ui, text, font.clone());
            let lh = line_height(ui, &font);
            let w = 15.0 + 7.0 + tw;
            let h = lh.max(15.0);
            let left = inner.min.x + center(inner.width(), w).max(0.0);
            let top = inner.min.y + center(inner.height(), h);
            let icon_at = pos2(left, top + center(h, 15.0));
            icons::paint(ui.painter(), Rect::from_min_size(icon_at, vec2(15.0, 15.0)), key, fade(p.accent_icon));
            let g = widgets::galley(ui, text, font, p.text_secondary, (inner.max.x - left - 22.0).max(0.0));
            ui.painter().galley(pos2(left + 22.0, top + center(h, lh)), g, fade(p.text_secondary));
        }
        Content::Text(text) => {
            let font = controls::content_font(13.0, 400);
            let tw = text_width(ui, text, font.clone());
            let x = inner.min.x + center(inner.width(), tw).max(0.0);
            widgets::text_left(ui, inner, x, text, font, fade(p.text_secondary));
        }
    }
    r
}

fn mix(under: Color32, over: Color32, a: f32) -> Color32 {
    let m = |u: u8, o: u8| (u as f32 * (1.0 - a) + o as f32 * a).round() as u8;
    Color32::from_rgb(m(under.r(), over.r()), m(under.g(), over.g()), m(under.b(), over.b()))
}

/// The border ring's pixels (`RecordingRegionBorder.Render`): a 1px
/// rectangle through the centres of the frame's edge pixels, stroked with
/// the dark backdrop in 5px dashes and the accent in the 5px between
/// (`DashStyle([5, 5], 5)`), clockwise from the top-left corner. Each pixel
/// is premultiplied RGBA; `(x, y)` from the frame's top-left.
pub fn border_pixels(width: i32, height: i32, accent: Color32) -> Vec<(i32, i32, [u8; 4])> {
    let (w, h) = (width as f32, height as f32);
    if width <= 1 || height <= 1 {
        return Vec::new();
    }
    let (l1, l2) = (w - 1.0, h - 1.0);
    let on = |d: f32, offset: f32| (d + offset).rem_euclid(10.0) < 5.0;
    // Whether the sample at (x, y) is inside the dashes with this offset.
    let inside = |x: f32, y: f32, offset: f32| -> bool {
        let mut hit = false;
        let in_x = (0.5..=w - 0.5).contains(&x);
        let in_y = (0.5..=h - 0.5).contains(&y);
        if y < 1.0 && in_x {
            hit |= on(x - 0.5, offset);
        }
        if x > w - 1.0 && in_y {
            hit |= on(l1 + y - 0.5, offset);
        }
        if y > h - 1.0 && in_x {
            hit |= on(l1 + l2 + (w - 0.5 - x), offset);
        }
        if x < 1.0 && in_y {
            hit |= on(2.0 * l1 + l2 + (h - 0.5 - y), offset);
        }
        // The quarter outside each corner stays empty: ShareX's corner
        // pixels are three quarters covered.
        hit
    };
    const N: usize = 4;
    let coverage = |px: i32, py: i32, offset: f32| -> f32 {
        let mut n = 0;
        for i in 0..N {
            for j in 0..N {
                let x = px as f32 + (i as f32 + 0.5) / N as f32;
                let y = py as f32 + (j as f32 + 0.5) / N as f32;
                if inside(x, y, offset) {
                    n += 1;
                }
            }
        }
        n as f32 / (N * N) as f32
    };
    let mut ring = Vec::with_capacity(2 * (width + height) as usize);
    for x in 0..width {
        ring.push((x, 0));
        if height > 1 {
            ring.push((x, height - 1));
        }
    }
    for y in 1..height - 1 {
        ring.push((0, y));
        if width > 1 {
            ring.push((width - 1, y));
        }
    }
    let back = [BACKDROP[0] as f32, BACKDROP[1] as f32, BACKDROP[2] as f32, BACKDROP[3] as f32 / 255.0];
    let acc = [accent.r() as f32, accent.g() as f32, accent.b() as f32, accent.a() as f32 / 255.0];
    ring.into_iter()
        .map(|(x, y)| {
            let mut px = [0.0f32; 4];
            for (color, offset) in [(back, 0.0), (acc, 5.0)] {
                let a = color[3] * coverage(x, y, offset);
                for i in 0..3 {
                    px[i] = color[i] * a + px[i] * (1.0 - a);
                }
                px[3] = a + px[3] * (1.0 - a);
            }
            let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
            (x, y, [q(px[0]), q(px[1]), q(px[2]), q(px[3] * 255.0)])
        })
        .collect()
}

/// Paints the border at `g`'s frame, as four 1px strips.
pub fn paint_border(ui: &Ui, g: &Geometry, accent: Color32) {
    let ctx = ui.ctx();
    let key = egui::Id::new(("recording-border", g.frame_width, g.frame_height, accent));
    let strips: Vec<(egui::TextureHandle, PxRect)> = match ctx.data(|d| d.get_temp::<std::sync::Arc<Vec<(egui::TextureHandle, PxRect)>>>(key)) {
        Some(t) => t.as_ref().clone(),
        None => {
            let (w, h) = (g.frame_width, g.frame_height);
            let pixels = border_pixels(w, h, accent);
            let rects = [PxRect::new(0, 0, w, 1), PxRect::new(0, h - 1, w, 1), PxRect::new(0, 1, 1, (h - 2).max(0)), PxRect::new(w - 1, 1, 1, (h - 2).max(0))];
            let mut out = Vec::new();
            for (i, r) in rects.iter().enumerate() {
                if r.w <= 0 || r.h <= 0 {
                    continue;
                }
                let mut img = egui::ColorImage::new([r.w as usize, r.h as usize], vec![Color32::TRANSPARENT; (r.w * r.h) as usize]);
                for &(x, y, c) in &pixels {
                    if r.contains(x, y) {
                        img.pixels[((y - r.y) * r.w + (x - r.x)) as usize] = Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]);
                    }
                }
                let tex = ctx.load_texture(format!("recording-border-{i}"), img, egui::TextureOptions::NEAREST);
                out.push((tex, *r));
            }
            ctx.data_mut(|d| d.insert_temp(key, std::sync::Arc::new(out.clone())));
            out
        }
    };
    let ppp = ctx.pixels_per_point();
    for (tex, r) in strips {
        let min = Pos2::new((g.frame_left + r.x) as f32 / ppp, (g.frame_top + r.y) as f32 / ppp);
        let rect = Rect::from_min_size(min, vec2(r.w as f32 / ppp, r.h as f32 / ppp));
        ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timer_reads_minutes_seconds_hundredths() {
        assert_eq!(format_timer(Duration::from_millis(0)), "00:00:00");
        assert_eq!(format_timer(Duration::from_millis(61_239)), "01:01:23");
        assert_eq!(format_timer(Duration::from_secs(3600 + 5)), "00:05:00");
    }

    #[test]
    fn the_toolbar_goes_under_the_frame_or_above_it() {
        let work = PxRect::new(0, 0, 1920, 1040);
        let g = Geometry::new(PxRect::new(100, 100, 400, 300), 277.0, 1.0, Some(work));
        assert_eq!((g.frame_left, g.frame_top, g.toolbar_left, g.toolbar_top), (0, 0, 62, 305));
        assert_eq!((g.width, g.height), (402, 302 + 3 + 42));
        assert_eq!(g.position(PxRect::new(100, 100, 400, 300)), (99, 99));
        // No room below: above.
        let g = Geometry::new(PxRect::new(100, 900, 200, 120), 461.0, 1.0, Some(work));
        assert_eq!((g.frame_left, g.frame_top, g.toolbar_left, g.toolbar_top), (129, 45, 0, 0));
        assert_eq!(g.recording_region(g.position(PxRect::new(100, 900, 200, 120)), (200, 120)), PxRect::new(100, 900, 200, 120));
    }

    #[test]
    fn the_border_alternates_backdrop_and_accent_dashes() {
        let px = border_pixels(40, 20, GOLDENROD);
        let at = |x: i32, y: i32| px.iter().find(|p| p.0 == x && p.1 == y).unwrap().2;
        // Mid-dash pixels on the top edge: the backdrop (d 1..4), then the
        // accent (d 6..9).
        assert_eq!(at(2, 0), [20, 20, 20, 220].map(|v: u8| if v == 220 { 220 } else { (v as f32 * 220.0 / 255.0).round() as u8 }));
        assert_eq!(at(8, 0), [0xDA, 0xA5, 0x20, 255]);
        // Inside pixels aren't part of it.
        assert!(!px.iter().any(|p| p.0 == 5 && p.1 == 5));
    }

    #[test]
    fn statuses_label_the_buttons_as_sharex() {
        let mut f = Frame::new((100, 100), true, false, false, 0.0);
        assert_eq!((f.start_text, f.pause_text, f.restart_enabled), (s::ScreenRecordWindow_Start(), s::ScreenRecordWindow_Pause(), false));
        f.change_state(ScreenRecordState::BeforeStart, ScreenRecordingStatus::Waiting);
        assert_eq!((f.start_text, f.pause_text), (s::ScreenRecordForm_Start(), s::Resume()));
        f.change_state(ScreenRecordState::AfterStart, ScreenRecordingStatus::Working);
        assert_eq!(f.start_text, s::ScreenRecordForm_Stop());
        f.change_state(ScreenRecordState::AfterRecordingStart, ScreenRecordingStatus::Recording);
        assert_eq!((f.accent, f.pause_text, f.restart_enabled), (RED, s::Pause(), true));
        assert!(f.ticking());
        f.change_state(ScreenRecordState::RecordingEnd, ScreenRecordingStatus::Paused);
        assert_eq!((f.accent, f.start_text, f.pause_text), (GOLDENROD, s::ScreenRecordForm_Stop(), s::Resume()));
        assert!(!f.ticking());
    }

    #[test]
    fn in_process_pause_stops_and_restarts_the_timer() {
        let mut f = Frame::new((100, 100), true, false, false, 0.0);
        f.change_state(ScreenRecordState::AfterRecordingStart, ScreenRecordingStatus::Recording);
        f.paused_in_process();
        assert_eq!((f.status, f.pause_text, f.restart_enabled, f.accent), (ScreenRecordingStatus::Paused, s::Resume(), true, GOLDENROD));
        assert!(!f.ticking());
        f.resumed_in_process();
        assert_eq!((f.status, f.pause_text), (ScreenRecordingStatus::Recording, s::Pause()));
        assert!(f.ticking());
    }
}
