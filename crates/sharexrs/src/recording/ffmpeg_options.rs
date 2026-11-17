//! The FFmpeg options window: `ShareX.ScreenCaptureLib/Presentation/
//! FFmpegOptions/FFmpegOptionsWindow.axaml(.cs)`, opened from Task settings
//! › Screen recorder › Screen recording options... (enabled when the
//! native recorder is off). It edits a copy of the task's `FFmpegOptions`,
//! which goes back to the task when it closes.
//!
//! Also FFmpeg's download: ShareX ships FFmpeg and its 22.4 doesn't offer
//! to download it, but its downloader is there (`FFmpegGitHubDownloader`:
//! the latest ShareX/FFmpeg release's `*win64.zip`, `ffmpeg.exe` extracted
//! from it). ShareXrs doesn't ship FFmpeg, so the "FFmpeg is missing"
//! message asks whether to download it with that.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use egui::{Color32, Id, Rect, Ui, pos2, vec2};
use sharex_screen_recording::devices::{self, CaptureSource, DirectShowDevices};
use sharex_screen_recording::{DesktopScreen, FFmpegCliManager, ScreenRecordingOptions, discovery};
use sharex_settings::ffmpeg_options::{
    FFmpegAMFQuality, FFmpegAMFUsage, FFmpegAudioCodec, FFmpegNVENCPreset, FFmpegNVENCTune, FFmpegPaletteGenStatsMode,
    FFmpegPaletteUseDither, FFmpegPreset, FFmpegQSVPreset, FFmpegVideoCodec,
};
use sharex_settings::types::Rectangle;
use sharex_settings::{FFmpegOptions, TaskSettingsCapture};
use sharex_strings::screen_capture_lib as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{self, Button, CheckBox, ComboBox, ComboItem, NumericUpDown, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{self, line_height, text_width};
use sharex_ui::icons;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::app_windows::TaskTarget;
use crate::window::EguiWindow;
use crate::App;

pub const WIDTH: f32 = 940.0;
pub const HEIGHT: f32 = 720.0;
const MIN_WIDTH: f32 = 760.0;
const MIN_HEIGHT: f32 = 620.0;

/// `RecorderDevicesUrl`.
const RECORDER_DEVICES_URL: &str = "https://github.com/ShareX/RecorderDevices/releases/tag/v0.12.10";

/// What the window asks the app for.
pub enum Effect {
    Browse,
    OpenUrl(&'static str),
}

/// `FFmpegOptionsWindow`'s state.
pub struct FFmpegOptionsWindow {
    /// `Options`: the recording options the command preview is built from.
    pub options: ScreenRecordingOptions,
    screens: Vec<DesktopScreen>,
    tab: usize,
    confirm_reset: bool,
    video_sources: Vec<CaptureSource>,
    audio_sources: Vec<CaptureSource>,
    status: String,
    refreshing: Option<Receiver<Result<DirectShowDevices, String>>>,
    refresh_path: PathBuf,
}

/// Whether this Windows is 10 or later (`Helpers.IsWindows10OrGreater`).
fn windows_10() -> bool {
    cfg!(windows)
}

impl FFmpegOptionsWindow {
    /// `ShowScreenRecordingOptions`: the task's FFmpeg options, frame rate,
    /// duration and cursor, recording `output.mp4` of the primary screen.
    pub fn new(capture: &TaskSettingsCapture) -> Self {
        let screens: Vec<DesktopScreen> = sharex_capture::screenshot::screens()
            .iter()
            .map(|s| DesktopScreen { bounds: Rectangle::new(s.bounds.x, s.bounds.y, s.bounds.w, s.bounds.h), primary: s.primary })
            .collect();
        let primary = screens.iter().find(|s| s.primary).map(|s| s.bounds).unwrap_or_default();
        let options = ScreenRecordingOptions {
            is_recording: true,
            ffmpeg: capture.FFmpegOptions.clone(),
            fps: capture.ScreenRecordFPS,
            duration: if capture.ScreenRecordFixedDuration { capture.ScreenRecordDuration } else { 0.0 },
            output_path: Some("output.mp4".into()),
            capture_area: primary,
            draw_cursor: capture.ScreenRecordShowCursor,
            ..Default::default()
        };
        let mut me = Self {
            options,
            screens,
            tab: 0,
            confirm_reset: false,
            video_sources: Vec::new(),
            audio_sources: Vec::new(),
            status: s::FFmpegOptionsWindow_Looking_for_devices().into(),
            refreshing: None,
            refresh_path: PathBuf::new(),
        };
        me.load_settings();
        me
    }

    /// `LoadSettingsAsync`: the confirmation hides and the devices are
    /// listed again.
    fn load_settings(&mut self) {
        self.confirm_reset = false;
        self.refresh_sources();
    }

    fn ffmpeg(&mut self) -> &mut FFmpegOptions {
        &mut self.options.ffmpeg
    }

    /// `RefreshSourcesAsync`: lists DirectShow devices on a thread when
    /// FFmpeg is there.
    pub fn refresh_sources(&mut self) {
        self.status = s::FFmpegOptionsWindow_Looking_for_devices().into();
        let path = discovery::ffmpeg_path(&self.options.ffmpeg);
        self.refresh_path = path.clone();
        let (tx, rx) = channel();
        if path.is_file() {
            std::thread::spawn(move || {
                let ffmpeg = FFmpegCliManager::new(&path);
                let _ = tx.send(Ok(ffmpeg.get_direct_show_devices()));
            });
        } else {
            let _ = tx.send(Err(String::new()));
        }
        self.refreshing = Some(rx);
        self.poll_refresh();
    }

    /// Takes the device list once it's there.
    fn poll_refresh(&mut self) -> bool {
        let Some(rx) = &self.refreshing else { return false };
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => Err("device enumeration failed".into()),
        };
        self.refreshing = None;
        let devices = result.as_ref().ok();
        let none = s::FFmpegOptionsWindow_None();
        self.video_sources = devices::video_sources(devices, windows_10(), [none, s::FFmpegOptionsWindow_Source_GDI(), s::FFmpegOptionsWindow_Source_DDA()]);
        self.audio_sources = devices::audio_sources(devices, none);
        devices::reconcile_sources(&mut self.options.ffmpeg, &self.video_sources, &self.audio_sources, false);
        let exists = self.refresh_path.is_file();
        self.status = if !exists {
            s::FFmpegOptionsWindow_FFmpeg_not_found().replace("{0}", &self.refresh_path.to_string_lossy())
        } else if result.is_err() {
            s::FFmpegOptionsWindow_Device_enumeration_failed().into()
        } else {
            let builtin = if windows_10() { 3 } else { 2 };
            let count = self.video_sources.len().saturating_sub(builtin) + self.audio_sources.len().saturating_sub(1);
            match count {
                0 => s::FFmpegOptionsWindow_No_devices_found().into(),
                1 => s::FFmpegOptionsWindow_One_device_found().replace("{0}", "1"),
                n => s::FFmpegOptionsWindow_Multiple_devices_found().replace("{0}", &n.to_string()),
            }
        };
        false
    }

    /// Waits for the device list (for `--render`).
    pub fn wait_for_devices(&mut self) {
        let start = Instant::now();
        while self.poll_refresh() && start.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Shows a page by name: `sources`, `video`, `audio`, `advanced`, with
    /// `-<codec>` to pick a video codec (`video-gif`), or `reset` (the
    /// reset confirmation on Sources).
    pub fn set_page(&mut self, page: &str) {
        let (name, codec) = page.split_once('-').unwrap_or((page, ""));
        self.tab = match name {
            "video" => 1,
            "audio" => 2,
            "advanced" => 3,
            _ => 0,
        };
        self.confirm_reset = name == "reset";
        if let Some(c) = FFmpegVideoCodec::ALL.iter().find(|c| c.name() == codec) {
            self.options.ffmpeg.VideoCodec = c.clone();
        }
    }

    /// `BrowseForFFmpegAsync`'s result.
    pub fn browsed(&mut self, path: &Path) {
        let f = self.ffmpeg();
        f.OverrideCLIPath = true;
        f.CLIPath = path.to_string_lossy().into_owned();
        self.refresh_sources();
    }

    /// `ResetOptionsAsync`: everything but the FFmpeg path back to defaults.
    fn reset(&mut self) {
        let f = &self.options.ffmpeg;
        let (o, p) = (f.OverrideCLIPath, f.CLIPath.clone());
        self.options.ffmpeg = FFmpegOptions { OverrideCLIPath: o, CLIPath: p, ..Default::default() };
        self.load_settings();
    }

    /// `UpdateCommandPreview`: the custom commands, or the command line the
    /// options give.
    pub fn command_preview(&self) -> String {
        let f = &self.options.ffmpeg;
        if f.UseCustomCommands { f.CustomCommands.clone() } else { self.options.get_ffmpeg_args(false, &self.screens).unwrap_or_default() }
    }

    /// Draws the window's client area.
    pub fn ui(&mut self, ui: &mut Ui) -> Vec<Effect> {
        let mut effects = Vec::new();
        if self.poll_refresh() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let header_bottom = self.header(ui, full);
        let body = Rect::from_min_max(pos2(full.min.x + 16.0, header_bottom + 16.0), pos2(full.max.x - 16.0, full.max.y - 16.0));
        let strip_bottom = self.tab_strip(ui, body);
        let content = Rect::from_min_max(pos2(body.min.x + 2.0, strip_bottom + 8.0 + 2.0), pos2(body.max.x - 2.0, body.max.y));
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(content));
        ui.set_clip_rect(Rect::from_min_max(pos2(content.min.x - 2.0, content.min.y - 2.0), pos2(content.max.x + 2.0, content.max.y)));
        match self.tab {
            0 => self.sources_tab(ui, content, &mut effects),
            1 => self.video_tab(ui, content),
            2 => self.audio_tab(ui, content),
            _ => self.advanced_tab(ui, content),
        }
        effects
    }

    /// The header: the 44px accent tile with the video icon, the title and
    /// description, Reset options (or its confirmation). Its bottom.
    fn header(&mut self, ui: &mut Ui, full: Rect) -> f32 {
        let p = palette(ui.ctx());
        let title_font = widgets::font(24.0, 600);
        let desc_font = widgets::font(13.0, 600);
        let reset_text = s::FFmpegOptionsWindow_Reset_options();
        let button_font = widgets::font(13.0, 400);
        let reset_w = 10.0 + 15.0 + 8.0 + text_width(ui, reset_text, button_font.clone()) + 10.0 + 2.0;
        let (cancel_text, confirm_text) = (s::FFmpegOptionsWindow_Cancel(), s::FFmpegOptionsWindow_Reset());
        let ask = s::FFmpegOptionsWindow_Reset_all_options();
        let cancel_w = Button::default().width(ui, cancel_text);
        let confirm_w = Button::primary(Thickness::hv(10.0, 6.0), 4.0).width(ui, confirm_text);
        let ask_w = text_width(ui, ask, widgets::font(13.0, 600));
        let right_w = if self.confirm_reset { ask_w + 8.0 + cancel_w + 8.0 + confirm_w } else { reset_w };
        let inner_left = full.min.x + 20.0;
        let text_left = inner_left + 44.0 + 14.0;
        let right_x = full.max.x - 20.0 - right_w;
        let text_w = (right_x - 14.0 - text_left).max(0.0);
        let title_h = line_height(ui, &title_font);
        let desc_h = controls::wrapped(ui, pos2(0.0, 0.0), text_w, s::FFmpegOptionsWindow_Description(), desc_font.clone(), Color32::TRANSPARENT, false);
        let row_h = (title_h + 3.0 + desc_h).max(44.0).max(40.0);
        let top = full.min.y + 18.0;
        let bottom = top + row_h + 18.0;
        let header = Rect::from_min_max(full.min, pos2(full.max.x, bottom + 1.0));
        ui.painter().rect_filled(header, 0.0, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_max(pos2(full.min.x, bottom), pos2(full.max.x, bottom + 1.0)), 0.0, p.border);
        let center = |avail: f32, size: f32| ((avail - size) / 2.0).round_ties_even();
        let tile = Rect::from_min_size(pos2(inner_left, top + center(row_h, 44.0)), vec2(44.0, 44.0));
        border::border(ui.painter(), tile, 10.0, 0.0, Some(Brush::accent(ui.ctx())), None);
        icons::paint(ui.painter(), Rect::from_center_size(tile.center(), vec2(22.0, 22.0)), icon::Video, p.accent_foreground);
        let text_top = top + center(row_h, title_h + 3.0 + desc_h);
        let g = widgets::galley(ui, s::FFmpegOptionsWindow_Screen_recording(), title_font, p.text, text_w);
        ui.painter().galley(pos2(text_left, text_top), g, p.text);
        controls::wrapped(ui, pos2(text_left, text_top + title_h + 3.0), text_w, s::FFmpegOptionsWindow_Description(), desc_font, p.text_secondary, true);
        let by = top + center(row_h, 40.0);
        if self.confirm_reset {
            let t = Rect::from_min_size(pos2(right_x, by), vec2(ask_w, 40.0));
            widgets::text_left(ui, t, t.min.x, ask, widgets::font(13.0, 600), p.text_secondary);
            let c = Rect::from_min_size(pos2(t.max.x + 8.0, by), vec2(cancel_w, 40.0));
            if Button::default().show(ui, c, Id::new("ffmpeg-reset-cancel"), cancel_text, true).clicked() {
                self.confirm_reset = false;
            }
            let r = Rect::from_min_size(pos2(c.max.x + 8.0, by), vec2(confirm_w, 40.0));
            if Button::primary(Thickness::hv(10.0, 6.0), 4.0).show(ui, r, Id::new("ffmpeg-reset-confirm"), confirm_text, true).clicked() {
                self.reset();
            }
        } else {
            let r = Rect::from_min_size(pos2(right_x, by), vec2(reset_w, 40.0));
            if icon_button(ui, r, "ffmpeg-reset", Some(icon::Reset), reset_text, true).on_hover_text(s::FFmpegOptionsWindow_Reset_encoding_options()).clicked() {
                self.confirm_reset = true;
            }
        }
        bottom + 1.0
    }

    /// `ListBox#OptionsTabStrip` in its toolbar-coloured border. Its bottom.
    fn tab_strip(&mut self, ui: &mut Ui, body: Rect) -> f32 {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let tabs = [
            (icon::Monitor, s::FFmpegOptionsWindow_Sources()),
            (icon::Video, s::FFmpegOptionsWindow_Video()),
            (icon::Audio, s::FFmpegOptionsWindow_Audio()),
            (icon::Terminal, s::FFmpegOptionsWindow_Advanced()),
        ];
        let font = widgets::font(13.0, 600);
        let lh = line_height(ui, &font);
        let item_h = 6.0 + lh.max(15.0) + 6.0;
        let widths: Vec<f32> = tabs.iter().map(|(_, t)| 11.0 + 15.0 + 6.0 + text_width(ui, t, font.clone()) + 11.0).collect();
        let strip_w = 4.0 + widths.iter().map(|w| w + 3.0).sum::<f32>() + 4.0;
        let strip = Rect::from_min_size(body.min, vec2(strip_w, 4.0 + item_h + 4.0));
        border::border(ui.painter(), strip, 6.0, 0.0, Some(Brush::Solid(p.background_toolbar)), None);
        let mut x = strip.min.x + 4.0;
        for (i, ((key, text), w)) in tabs.iter().zip(&widths).enumerate() {
            let r = Rect::from_min_size(pos2(x, strip.min.y + 4.0), vec2(*w, item_h));
            let resp = ui.interact(r, Id::new(("ffmpeg-tab", i)), egui::Sense::click());
            let selected = self.tab == i;
            if selected {
                border::border(ui.painter(), r, 4.0, 0.0, Some(Brush::accent(ui.ctx())), None);
            } else if resp.hovered() {
                border::border(ui.painter(), r, 4.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
            }
            let (icon_fg, text_fg) = if selected { (p.accent_foreground, p.accent_foreground) } else { (f.base_high, p.text_secondary) };
            let iy = r.min.y + ((r.height() - 15.0) / 2.0).round_ties_even();
            icons::paint(ui.painter(), Rect::from_min_size(pos2(r.min.x + 11.0, iy), vec2(15.0, 15.0)), key, icon_fg);
            widgets::text_left(ui, r, r.min.x + 11.0 + 15.0 + 6.0, text, font.clone(), text_fg);
            if resp.clicked() {
                self.tab = i;
            }
            x = r.max.x + 3.0;
        }
        strip.max.y
    }

    fn sources_tab(&mut self, ui: &mut Ui, content: Rect, effects: &mut Vec<Effect>) {
        let p = palette(ui.ctx());
        // The FFmpeg executable.
        let mut y = content.min.y;
        let card_h = 34.0 + title_h(ui) + 14.0 + 32.0 + 14.0 + 40.0;
        let card = Rect::from_min_size(pos2(content.min.x, y), vec2(content.width(), card_h));
        section_card(ui, card);
        let inner = card.shrink(17.0);
        let mut cy = inner.min.y;
        section_title(ui, inner.min.x, cy, inner.width(), s::FFmpegOptionsWindow_FFmpeg_executable());
        cy += title_h(ui) + 14.0;
        let mut custom = self.options.ffmpeg.OverrideCLIPath;
        let check = CheckBox { text_font: widgets::font(13.0, 400), text_color: Some(p.text_secondary), ..CheckBox::new(s::FFmpegOptionsWindow_Use_custom_path()) };
        let cr = Rect::from_min_size(pos2(inner.min.x, cy), vec2(check.width(ui), 32.0));
        if check.show(ui, cr, Id::new("ffmpeg-custom-path"), &mut custom).changed() {
            self.options.ffmpeg.OverrideCLIPath = custom;
        }
        cy += 32.0 + 14.0;
        let browse = Rect::from_min_size(pos2(inner.max.x - 40.0, cy), vec2(40.0, 40.0));
        let path = Rect::from_min_max(pos2(inner.min.x, cy), pos2(browse.min.x - 8.0, cy + 40.0));
        let enabled = self.options.ffmpeg.OverrideCLIPath;
        let tb = TextBox { enabled, placeholder: s::FFmpegOptionsWindow_Path_to_ffmpeg(), ..Default::default() };
        let mut text = self.options.ffmpeg.CLIPath.clone();
        if tb.show(ui, path, Id::new("ffmpeg-path"), &mut text).changed() {
            self.options.ffmpeg.CLIPath = text;
        }
        if icon_button(ui, browse, "ffmpeg-browse", Some(icon::FolderOpen), "", enabled).on_hover_text(s::FFmpegOptionsWindow_Browse_for_ffmpeg()).clicked() {
            effects.push(Effect::Browse);
        }
        y = card.max.y + 12.0;
        // Capture sources.
        let font_hint = widgets::font(12.0, 400);
        let get_text = s::FFmpegOptionsWindow_Get_recorder_devices();
        let get_w = Button::default().width(ui, get_text);
        let label_h = line_height(ui, &widgets::font(13.0, 600));
        let info_text_w = content.width() - 34.0 - 26.0 - 14.0 - 10.0 - 10.0 - get_w;
        let status_h = controls::wrapped(ui, pos2(0.0, 0.0), info_text_w, &self.status, font_hint.clone(), Color32::TRANSPARENT, false);
        let info_h = 26.0 + status_h.max(40.0);
        let card_h = 34.0 + 40.0 + 14.0 + label_h + 5.0 + 40.0 + 14.0 + info_h;
        let card = Rect::from_min_size(pos2(content.min.x, y), vec2(content.width(), card_h));
        section_card(ui, card);
        let inner = card.shrink(17.0);
        let refresh_text = s::FFmpegOptionsWindow_Refresh_devices();
        let refresh_w = 10.0 + 15.0 + 8.0 + text_width(ui, refresh_text, widgets::font(13.0, 400)) + 10.0 + 2.0;
        let row = Rect::from_min_size(inner.min, vec2(inner.width(), 40.0));
        section_title(ui, row.min.x, row.min.y + ((40.0 - title_h(ui)) / 2.0).round_ties_even(), row.width() - refresh_w - 12.0, s::FFmpegOptionsWindow_Capture_sources());
        let rr = Rect::from_min_size(pos2(row.max.x - refresh_w, row.min.y), vec2(refresh_w, 40.0));
        if icon_button(ui, rr, "ffmpeg-refresh", Some(icon::Refresh), refresh_text, self.refreshing.is_none()).clicked() {
            self.refresh_sources();
        }
        let mut cy = row.max.y + 14.0;
        let col_w = (inner.width() - 12.0) / 2.0;
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        for (i, (label, video)) in [(s::FFmpegOptionsWindow_Video_source(), true), (s::FFmpegOptionsWindow_Audio_source(), false)].into_iter().enumerate() {
            let x = inner.min.x + i as f32 * (col_w + 12.0);
            field_label(ui, x, cy, label);
            let r = Rect::from_min_size(pos2(x, cy + label_h + 5.0), vec2(col_w, 40.0));
            let (list, value) = if video { (&self.video_sources, &self.options.ffmpeg.VideoSource) } else { (&self.audio_sources, &self.options.ffmpeg.AudioSource) };
            let items: Vec<ComboItem> = list.iter().map(|s| ComboItem::text(&s.title)).collect();
            let selected = list.iter().position(|s| s.value.eq_ignore_ascii_case(value));
            if let Some(n) = combo.show(ui, r, Id::new(("ffmpeg-source", i)), &items, selected) {
                let v = list[n].value.clone();
                if video {
                    self.options.ffmpeg.VideoSource = v;
                } else {
                    self.options.ffmpeg.AudioSource = v;
                }
            }
        }
        cy += label_h + 5.0 + 40.0 + 14.0;
        let info = Rect::from_min_size(pos2(inner.min.x, cy), vec2(inner.width(), info_h));
        info_card(ui, info, None);
        let ii = info.shrink(13.0);
        let icon_size = 14.0;
        let iy = ii.min.y + ((ii.height() - icon_size) / 2.0).round_ties_even();
        icons::paint(ui.painter(), Rect::from_min_size(pos2(ii.min.x, iy), vec2(icon_size, icon_size)), icon::Info, p.accent_icon);
        let gx = ii.max.x - get_w;
        let tx = ii.min.x + icon_size + 10.0;
        let tw = gx - 10.0 - tx;
        let ty = ii.min.y + ((ii.height() - status_h) / 2.0).round_ties_even();
        controls::wrapped(ui, pos2(tx, ty), tw, &self.status, font_hint, p.text_secondary, true);
        let gr = Rect::from_min_size(pos2(gx, ii.min.y + ((ii.height() - 40.0) / 2.0).round_ties_even()), vec2(get_w, 40.0));
        if Button::default().show(ui, gr, Id::new("ffmpeg-recorder-devices"), get_text, true).clicked() {
            effects.push(Effect::OpenUrl(RECORDER_DEVICES_URL));
        }
    }

    fn video_tab(&mut self, ui: &mut Ui, content: Rect) {
        let left = Rect::from_min_size(content.min, vec2(300.0, 0.0));
        let right_x = left.max.x + 12.0;
        let f = &self.options.ffmpeg;
        let codec = f.VideoCodec.clone();
        let (summary, hint) = video_codec_description(&codec);
        // The left card's height.
        let iw = 300.0 - 34.0 - 26.0;
        let sum_h = controls::wrapped(ui, pos2(0.0, 0.0), iw, summary, widgets::font(13.0, 600), Color32::TRANSPARENT, false);
        let hint_h = if hint.is_empty() { line_height(ui, &widgets::font(12.0, 400)) } else { controls::wrapped(ui, pos2(0.0, 0.0), iw, hint, widgets::font(12.0, 400), Color32::TRANSPARENT, false) };
        let left_h = 34.0 + title_h(ui) + 12.0 + 40.0 + 12.0 + 26.0 + sum_h + 4.0 + hint_h;
        let right_h = self.video_settings(ui, None, content.max.x - right_x);
        // The grid stretches to the viewport (its 2px margin in).
        let h = left_h.max(right_h).max(content.height() - 2.0);
        let left = Rect::from_min_size(content.min, vec2(300.0, h));
        section_card(ui, left);
        let inner = left.shrink(17.0);
        section_title(ui, inner.min.x, inner.min.y, inner.width(), s::FFmpegOptionsWindow_Video_encoder());
        let cy = inner.min.y + title_h(ui) + 12.0;
        let items: Vec<ComboItem> = FFmpegVideoCodec::ALL.iter().map(|c| ComboItem::text(c.description())).collect();
        let sel = FFmpegVideoCodec::ALL.iter().position(|c| *c == codec);
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let r = Rect::from_min_size(pos2(inner.min.x, cy), vec2(inner.width(), 40.0));
        if let Some(i) = combo.show(ui, r, Id::new("ffmpeg-video-codec"), &items, sel) {
            self.options.ffmpeg.VideoCodec = FFmpegVideoCodec::ALL[i].clone();
        }
        let info = Rect::from_min_size(pos2(inner.min.x, r.max.y + 12.0), vec2(inner.width(), 26.0 + sum_h + 4.0 + hint_h));
        info_card(ui, info, None);
        let p = palette(ui.ctx());
        let ii = info.shrink(13.0);
        controls::wrapped(ui, ii.min, ii.width(), summary, widgets::font(13.0, 600), p.text_secondary, true);
        controls::wrapped(ui, pos2(ii.min.x, ii.min.y + sum_h + 4.0), ii.width(), hint, widgets::font(12.0, 400), p.text_secondary, true);
        let right = Rect::from_min_size(pos2(right_x, content.min.y), vec2(content.max.x - right_x, h));
        section_card(ui, right);
        self.video_settings(ui, Some(right.shrink(17.0)), right.width());
    }

    /// The video encoder's settings (`UpdateUI`'s visible panel): laid out
    /// in `inner` when given; the card's height either way.
    fn video_settings(&mut self, ui: &mut Ui, inner: Option<Rect>, card_w: f32) -> f32 {
        let paint = inner.is_some();
        let inner = inner.unwrap_or(Rect::from_min_size(pos2(0.0, 0.0), vec2(card_w - 34.0, 0.0)));
        let p = palette(ui.ctx());
        let mut s = Stack::new(ui, inner, paint);
        s.title(s::FFmpegOptionsWindow_Encoder_settings(), 14.0);
        let f = &mut self.options.ffmpeg;
        let combo_w = inner.width().max(220.0);
        match f.VideoCodec.clone() {
            FFmpegVideoCodec::libx264 | FFmpegVideoCodec::libx265 => {
                let presets = [
                    s::FFmpegOptionsWindow_Preset_ultra_fast(),
                    s::FFmpegOptionsWindow_Preset_super_fast(),
                    s::FFmpegOptionsWindow_Preset_very_fast(),
                    s::FFmpegOptionsWindow_Preset_faster(),
                    s::FFmpegOptionsWindow_Preset_fast(),
                    s::FFmpegOptionsWindow_Preset_medium(),
                    s::FFmpegOptionsWindow_Preset_slow(),
                    s::FFmpegOptionsWindow_Preset_slower(),
                    s::FFmpegOptionsWindow_Preset_very_slow(),
                    s::FFmpegOptionsWindow_Preset_placebo(),
                ];
                if let Some(i) = s.combo("x264-preset", s::FFmpegOptionsWindow_Preset(), &presets, enum_index(FFmpegPreset::ALL, &f.x264_Preset), combo_w, 12.0) {
                    f.x264_Preset = FFmpegPreset::ALL[i].clone();
                }
                let slow = FFmpegPreset::ALL.iter().position(|v| *v == f.x264_Preset).is_some_and(|i| i > 4);
                if slow {
                    s.warning(s::FFmpegOptionsWindow_Slow_presets_warning(), 12.0);
                }
                let mut use_bitrate = f.x264_Use_Bitrate;
                if s.check("x264-bitrate-check", s::FFmpegOptionsWindow_Use_target_bitrate(), &mut use_bitrate, 12.0) {
                    f.x264_Use_Bitrate = use_bitrate;
                }
                if f.x264_Use_Bitrate {
                    s.number("x264-bitrate", s::FFmpegOptionsWindow_Target_bitrate(), &mut f.x264_Bitrate, 100, 100_000, 100, None, 0.0);
                } else {
                    s.number("x264-crf", s::FFmpegOptionsWindow_Constant_rate_factor(), &mut f.x264_CRF, 0, 51, 1, Some(s::FFmpegOptionsWindow_CRF_hint()), 0.0);
                }
                s.y -= 12.0;
                s.y += 14.0;
            }
            FFmpegVideoCodec::libvpx | FFmpegVideoCodec::libvpx_vp9 => {
                s.number("vpx-bitrate", s::FFmpegOptionsWindow_Target_bitrate(), &mut f.VPx_Bitrate, 100, 100_000, 100, None, 14.0);
            }
            FFmpegVideoCodec::libxvid => {
                s.number_spaced("xvid", s::FFmpegOptionsWindow_Variable_bitrate_quality(), &mut f.XviD_QScale, 1, 31, Some(s::FFmpegOptionsWindow_Variable_bitrate_hint()), 14.0);
            }
            FFmpegVideoCodec::h264_nvenc | FFmpegVideoCodec::hevc_nvenc => {
                let presets = [
                    s::FFmpegOptionsWindow_NVENC_fastest_lowest_quality(),
                    s::FFmpegOptionsWindow_NVENC_faster_lower_quality(),
                    s::FFmpegOptionsWindow_NVENC_fast_low_quality(),
                    s::FFmpegOptionsWindow_NVENC_medium_medium_quality(),
                    s::FFmpegOptionsWindow_NVENC_slow_good_quality(),
                    s::FFmpegOptionsWindow_NVENC_slower_better_quality(),
                    s::FFmpegOptionsWindow_NVENC_slowest_best_quality(),
                ];
                if let Some(i) = s.combo("nvenc-preset", s::FFmpegOptionsWindow_Preset(), &presets, enum_index(FFmpegNVENCPreset::ALL, &f.NVENC_Preset), combo_w, 12.0) {
                    f.NVENC_Preset = FFmpegNVENCPreset::ALL[i].clone();
                }
                let tunes = [
                    s::FFmpegOptionsWindow_NVENC_high_quality(),
                    s::FFmpegOptionsWindow_NVENC_low_latency(),
                    s::FFmpegOptionsWindow_NVENC_ultra_low_latency(),
                    s::FFmpegOptionsWindow_NVENC_lossless(),
                ];
                if let Some(i) = s.combo("nvenc-tune", s::FFmpegOptionsWindow_Tune(), &tunes, enum_index(FFmpegNVENCTune::ALL, &f.NVENC_Tune), combo_w, 12.0) {
                    f.NVENC_Tune = FFmpegNVENCTune::ALL[i].clone();
                }
                s.number("nvenc-bitrate", s::FFmpegOptionsWindow_Target_bitrate(), &mut f.NVENC_Bitrate, 100, 100_000, 100, None, 14.0);
            }
            FFmpegVideoCodec::gif => {
                let modes = [s::FFmpegOptionsWindow_Palette_full(), s::FFmpegOptionsWindow_Palette_diff(), s::FFmpegOptionsWindow_Palette_single()];
                if let Some(i) = s.combo("gif-stats", s::FFmpegOptionsWindow_Palette_mode(), &modes, enum_index(FFmpegPaletteGenStatsMode::ALL, &f.GIFStatsMode), combo_w, 12.0) {
                    f.GIFStatsMode = FFmpegPaletteGenStatsMode::ALL[i].clone();
                }
                let dithers: Vec<String> = FFmpegPaletteUseDither::ALL
                    .iter()
                    .map(|d| if *d == FFmpegPaletteUseDither::none { s::FFmpegOptionsWindow_None().to_owned() } else { d.name().to_owned() })
                    .collect();
                let dither_refs: Vec<&str> = dithers.iter().map(String::as_str).collect();
                let bayer = f.GIFDither == FFmpegPaletteUseDither::bayer;
                if let Some(i) = s.combo("gif-dither", s::FFmpegOptionsWindow_Dithering_mode(), &dither_refs, enum_index(FFmpegPaletteUseDither::ALL, &f.GIFDither), combo_w, if bayer { 12.0 } else { 14.0 }) {
                    f.GIFDither = FFmpegPaletteUseDither::ALL[i].clone();
                }
                if bayer {
                    s.number("gif-bayer", s::FFmpegOptionsWindow_Bayer_scale(), &mut f.GIFBayerScale, 0, 5, 1, Some(s::FFmpegOptionsWindow_Bayer_scale_hint()), 14.0);
                }
            }
            FFmpegVideoCodec::h264_amf | FFmpegVideoCodec::hevc_amf => {
                let usages = [
                    s::FFmpegOptionsWindow_AMF_generic_transcoding(),
                    s::FFmpegOptionsWindow_AMF_ultra_low_latency_transcoding(),
                    s::FFmpegOptionsWindow_AMF_low_latency_transcoding(),
                    s::FFmpegOptionsWindow_AMF_webcam(),
                    s::FFmpegOptionsWindow_AMF_high_quality_transcoding(),
                    s::FFmpegOptionsWindow_AMF_low_latency_high_quality_transcoding(),
                ];
                if let Some(i) = s.combo("amf-usage", s::FFmpegOptionsWindow_Usage(), &usages, enum_index(FFmpegAMFUsage::ALL, &f.AMF_Usage), combo_w, 12.0) {
                    f.AMF_Usage = FFmpegAMFUsage::ALL[i].clone();
                }
                let qualities = [s::FFmpegOptionsWindow_AMF_prefer_speed(), s::FFmpegOptionsWindow_AMF_balanced(), s::FFmpegOptionsWindow_AMF_prefer_quality()];
                if let Some(i) = s.combo("amf-quality", s::FFmpegOptionsWindow_Quality(), &qualities, enum_index(FFmpegAMFQuality::ALL, &f.AMF_Quality), combo_w, 12.0) {
                    f.AMF_Quality = FFmpegAMFQuality::ALL[i].clone();
                }
                s.number("amf-bitrate", s::FFmpegOptionsWindow_Target_bitrate(), &mut f.AMF_Bitrate, 100, 100_000, 100, None, 14.0);
            }
            FFmpegVideoCodec::h264_qsv | FFmpegVideoCodec::hevc_qsv => {
                let presets = [
                    s::FFmpegOptionsWindow_Preset_very_fast(),
                    s::FFmpegOptionsWindow_Preset_faster(),
                    s::FFmpegOptionsWindow_Preset_fast(),
                    s::FFmpegOptionsWindow_Preset_medium(),
                    s::FFmpegOptionsWindow_Preset_slow(),
                    s::FFmpegOptionsWindow_Preset_slower(),
                    s::FFmpegOptionsWindow_Preset_very_slow(),
                ];
                if let Some(i) = s.combo("qsv-preset", s::FFmpegOptionsWindow_Preset(), &presets, enum_index(FFmpegQSVPreset::ALL, &f.QSV_Preset), combo_w, 12.0) {
                    f.QSV_Preset = FFmpegQSVPreset::ALL[i].clone();
                }
                s.number("qsv-bitrate", s::FFmpegOptionsWindow_Target_bitrate(), &mut f.QSV_Bitrate, 100, 100_000, 100, None, 14.0);
            }
            _ => {
                // `NoVideoSettingsPanel`.
                s.info(icon::CircleCheck, p.accent_icon, s::FFmpegOptionsWindow_No_additional_settings(), 14.0);
            }
        }
        s.y - inner.min.y - 14.0 + 34.0
    }

    fn audio_tab(&mut self, ui: &mut Ui, content: Rect) {
        let animated = self.options.ffmpeg.is_animated_image();
        let warn_w = 300.0 - 34.0 - 26.0 - 14.0 - 8.0;
        let warn_h = controls::wrapped(ui, pos2(0.0, 0.0), warn_w, s::FFmpegOptionsWindow_Animated_formats_no_audio(), widgets::font(12.0, 400), Color32::TRANSPARENT, false);
        let left_h = 34.0 + title_h(ui) + 12.0 + 40.0 + if animated { 12.0 + 26.0 + warn_h.max(14.0) } else { 0.0 };
        let right_x = content.min.x + 300.0 + 12.0;
        let right_h = self.audio_settings(ui, None, content.max.x - right_x);
        let h = left_h.max(right_h).max(content.height() - 2.0);
        let left = Rect::from_min_size(content.min, vec2(300.0, h));
        section_card(ui, left);
        let inner = left.shrink(17.0);
        section_title(ui, inner.min.x, inner.min.y, inner.width(), s::FFmpegOptionsWindow_Audio_encoder());
        let cy = inner.min.y + title_h(ui) + 12.0;
        let items: Vec<ComboItem> = FFmpegAudioCodec::ALL.iter().map(|c| ComboItem::text(c.description())).collect();
        let sel = FFmpegAudioCodec::ALL.iter().position(|c| *c == self.options.ffmpeg.AudioCodec);
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: !animated };
        let r = Rect::from_min_size(pos2(inner.min.x, cy), vec2(inner.width(), 40.0));
        if let Some(i) = combo.show(ui, r, Id::new("ffmpeg-audio-codec"), &items, sel) {
            self.options.ffmpeg.AudioCodec = FFmpegAudioCodec::ALL[i].clone();
        }
        if animated {
            let info = Rect::from_min_size(pos2(inner.min.x, r.max.y + 12.0), vec2(inner.width(), 26.0 + warn_h.max(14.0)));
            warning_card(ui, info, s::FFmpegOptionsWindow_Animated_formats_no_audio());
        }
        let right = Rect::from_min_size(pos2(right_x, content.min.y), vec2(content.max.x - right_x, h));
        section_card(ui, right);
        self.audio_settings(ui, Some(right.shrink(17.0)), right.width());
    }

    fn audio_settings(&mut self, ui: &mut Ui, inner: Option<Rect>, card_w: f32) -> f32 {
        let paint = inner.is_some();
        let inner = inner.unwrap_or(Rect::from_min_size(pos2(0.0, 0.0), vec2(card_w - 34.0, 0.0)));
        let enabled = !self.options.ffmpeg.is_animated_image();
        let mut s = Stack::new(ui, inner, paint);
        s.enabled = enabled;
        s.title(s::FFmpegOptionsWindow_Encoder_settings(), 14.0);
        let f = &mut self.options.ffmpeg;
        let w = inner.width().max(220.0);
        let pick = |values: &[i32], value: i32, default: i32| values.iter().position(|v| *v == value).or_else(|| values.iter().position(|v| *v == default));
        match f.AudioCodec {
            FFmpegAudioCodec::libvoaacenc => {
                let values: Vec<i32> = (2..=10).map(|x| x * 32).collect();
                let labels: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                if let Some(i) = s.combo("aac", s::FFmpegOptionsWindow_Bitrate(), &refs, pick(&values, f.AAC_Bitrate, 128), w, 14.0) {
                    f.AAC_Bitrate = values[i];
                }
            }
            FFmpegAudioCodec::libopus => {
                let values: Vec<i32> = (1..=16).map(|x| x * 32).collect();
                let labels: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                if let Some(i) = s.combo("opus", s::FFmpegOptionsWindow_Bitrate(), &refs, pick(&values, f.Opus_Bitrate, 128), w, 14.0) {
                    f.Opus_Bitrate = values[i];
                }
            }
            FFmpegAudioCodec::libvorbis => {
                let values: Vec<i32> = (0..=10).collect();
                let labels: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                if let Some(i) = s.combo("vorbis", s::FFmpegOptionsWindow_Quality(), &refs, pick(&values, f.Vorbis_QScale, 3), w, 14.0) {
                    f.Vorbis_QScale = values[i];
                }
            }
            FFmpegAudioCodec::libmp3lame => {
                let values: Vec<i32> = (0..=9).rev().collect();
                let labels: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                if let Some(i) = s.combo("mp3", s::FFmpegOptionsWindow_Quality(), &refs, pick(&values, f.MP3_QScale, 4), w, 0.0) {
                    f.MP3_QScale = values[i];
                }
                s.hint(s::FFmpegOptionsWindow_Lower_values_higher_quality(), 14.0);
            }
            _ => {}
        }
        s.y - inner.min.y - 14.0 + 34.0
    }

    fn advanced_tab(&mut self, ui: &mut Ui, content: Rect) {
        let p = palette(ui.ctx());
        // Additional arguments.
        let card = Rect::from_min_size(content.min, vec2(content.width(), 34.0 + title_h(ui) + 10.0 + 40.0));
        section_card(ui, card);
        let inner = card.shrink(17.0);
        section_title(ui, inner.min.x, inner.min.y, inner.width(), s::FFmpegOptionsWindow_Additional_arguments());
        let r = Rect::from_min_size(pos2(inner.min.x, inner.min.y + title_h(ui) + 10.0), vec2(inner.width(), 40.0));
        let tb = TextBox { placeholder: s::FFmpegOptionsWindow_Arguments_example(), ..Default::default() };
        let mut args = self.options.ffmpeg.UserArgs.clone();
        if tb.show(ui, r, Id::new("ffmpeg-user-args"), &mut args).changed() {
            self.options.ffmpeg.UserArgs = args;
        }
        // The FFmpeg command.
        let top = card.max.y + 12.0;
        let hint_font = widgets::font(12.0, 400);
        let placeholders = s::FFmpegOptionsWindow_Custom_command_placeholders();
        let ph = controls::wrapped(ui, pos2(0.0, 0.0), content.width() - 34.0 - 26.0, placeholders, hint_font.clone(), Color32::TRANSPARENT, false);
        // `MinHeight="260"` for the command box; the card is as tall as it.
        let card = Rect::from_min_size(pos2(content.min.x, top), vec2(content.width(), 34.0 + 32.0 + 10.0 + 26.0 + ph + 10.0 + 260.0));
        section_card(ui, card);
        let inner = card.shrink(17.0);
        let check = CheckBox { text_font: widgets::font(13.0, 400), text_color: Some(p.text_secondary), ..CheckBox::new(s::FFmpegOptionsWindow_Use_custom_commands()) };
        let cw = check.width(ui);
        let row = Rect::from_min_size(inner.min, vec2(inner.width(), 32.0));
        section_title(ui, row.min.x, row.min.y + ((32.0 - title_h(ui)) / 2.0).round_ties_even(), row.width() - cw - 12.0, s::FFmpegOptionsWindow_FFmpeg_command());
        let mut custom = self.options.ffmpeg.UseCustomCommands;
        if check.show(ui, Rect::from_min_size(pos2(row.max.x - cw, row.min.y), vec2(cw, 32.0)), Id::new("ffmpeg-custom-commands"), &mut custom).changed() {
            self.options.ffmpeg.UseCustomCommands = custom;
            if custom && self.options.ffmpeg.CustomCommands.trim().is_empty() {
                self.options.ffmpeg.CustomCommands = self.options.get_ffmpeg_args(true, &self.screens).unwrap_or_default();
            }
        }
        let info = Rect::from_min_size(pos2(inner.min.x, row.max.y + 10.0), vec2(inner.width(), 26.0 + ph));
        info_card(ui, info, None);
        controls::wrapped(ui, info.shrink(13.0).min, inner.width() - 26.0, placeholders, hint_font, p.text_secondary, true);
        let boxr = Rect::from_min_max(pos2(inner.min.x, info.max.y + 10.0), pos2(inner.max.x, info.max.y + 10.0 + 260.0));
        let custom = self.options.ffmpeg.UseCustomCommands;
        let mono = egui::FontId::new(13.0, sharex_ui::fonts::consolas());
        // `Padding="8,6"`; Consolas' line box puts its first line 2px lower
        // and 1px further left than the shared box's metrics do.
        let tb = TextBox { multiline: true, wrap: true, read_only: !custom, font: Some(mono), padding: Thickness::new(7.0, 8.0, 8.0, 6.0), ..Default::default() };
        let mut text = self.command_preview();
        if tb.show(ui, boxr, Id::new("ffmpeg-command"), &mut text).changed() && custom {
            self.options.ffmpeg.CustomCommands = text;
        }
    }
}

/// `GetVideoCodecDescription`.
fn video_codec_description(codec: &FFmpegVideoCodec) -> (&'static str, &'static str) {
    use FFmpegVideoCodec as C;
    match codec {
        C::libx264 => (s::FFmpegOptionsWindow_Codec_H264_software(), s::FFmpegOptionsWindow_Codec_H264_software_hint()),
        C::libx265 => (s::FFmpegOptionsWindow_Codec_H265_software(), s::FFmpegOptionsWindow_Codec_H265_software_hint()),
        C::libvpx => (s::FFmpegOptionsWindow_Codec_VP8_software(), s::FFmpegOptionsWindow_Codec_VP8_software_hint()),
        C::libvpx_vp9 => (s::FFmpegOptionsWindow_Codec_VP9_software(), s::FFmpegOptionsWindow_Codec_VP9_software_hint()),
        C::libxvid => (s::FFmpegOptionsWindow_Codec_MPEG4_Xvid(), s::FFmpegOptionsWindow_Codec_MPEG4_Xvid_hint()),
        C::h264_nvenc => (s::FFmpegOptionsWindow_Codec_H264_NVIDIA(), s::FFmpegOptionsWindow_Codec_H264_NVIDIA_hint()),
        C::hevc_nvenc => (s::FFmpegOptionsWindow_Codec_HEVC_NVIDIA(), s::FFmpegOptionsWindow_Codec_HEVC_NVIDIA_hint()),
        C::h264_amf => (s::FFmpegOptionsWindow_Codec_H264_AMD(), s::FFmpegOptionsWindow_Codec_H264_AMD_hint()),
        C::hevc_amf => (s::FFmpegOptionsWindow_Codec_HEVC_AMD(), s::FFmpegOptionsWindow_Codec_HEVC_AMD_hint()),
        C::h264_qsv => (s::FFmpegOptionsWindow_Codec_H264_Intel_Quick_Sync(), s::FFmpegOptionsWindow_Codec_H264_Intel_Quick_Sync_hint()),
        C::hevc_qsv => (s::FFmpegOptionsWindow_Codec_HEVC_Intel_Quick_Sync(), s::FFmpegOptionsWindow_Codec_HEVC_Intel_Quick_Sync_hint()),
        C::gif => (s::FFmpegOptionsWindow_Codec_animated_GIF(), s::FFmpegOptionsWindow_Codec_animated_GIF_hint()),
        C::libwebp => (s::FFmpegOptionsWindow_Codec_animated_WebP(), s::FFmpegOptionsWindow_Codec_animated_WebP_hint()),
        C::apng => (s::FFmpegOptionsWindow_Codec_animated_PNG(), s::FFmpegOptionsWindow_Codec_animated_PNG_hint()),
        C::Unknown(_) => (s::FFmpegOptionsWindow_Codec_video_encoding(), ""),
    }
}

fn enum_index<T: PartialEq>(all: &[T], value: &T) -> Option<usize> {
    all.iter().position(|v| v == value)
}

/// `TextBlock.section-title`: 16, SemiBold.
fn title_h(ui: &Ui) -> f32 {
    line_height(ui, &widgets::font(16.0, 600))
}

fn section_title(ui: &Ui, x: f32, y: f32, w: f32, text: &str) {
    let p = palette(ui.ctx());
    let g = widgets::galley(ui, text, widgets::font(16.0, 600), p.text, w);
    ui.painter().galley(pos2(x, y), g, p.text);
}

/// `TextBlock.field-label`: SemiBold 13, secondary, 5px above its control.
fn field_label(ui: &Ui, x: f32, y: f32, text: &str) {
    let p = palette(ui.ctx());
    let g = widgets::galley(ui, text, widgets::font(13.0, 600), p.text_secondary, f32::INFINITY);
    ui.painter().galley(pos2(x, y), g, p.text_secondary);
}

/// `Border.section-card`: the panel colour, 1px border, 8px corners,
/// `Padding="16"`.
fn section_card(ui: &Ui, rect: Rect) {
    let p = palette(ui.ctx());
    border::border(ui.painter(), rect, 8.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
}

/// `Border.info-card`: the toolbar colour, 1px border (or `border`), 6px
/// corners, `Padding="12"`.
fn info_card(ui: &Ui, rect: Rect, border_color: Option<Color32>) {
    let p = palette(ui.ctx());
    border::border(ui.painter(), rect, 6.0, 1.0, Some(Brush::Solid(p.background_toolbar)), Some(border_color.unwrap_or(p.border)));
}

/// `Brushes.Goldenrod`.
const GOLDENROD: Color32 = Color32::from_rgb(0xDA, 0xA5, 0x20);

/// An info card with a goldenrod border and warning icon.
fn warning_card(ui: &Ui, rect: Rect, text: &str) {
    let p = palette(ui.ctx());
    info_card(ui, rect, Some(GOLDENROD));
    let inner = rect.shrink(13.0);
    icons::paint(ui.painter(), Rect::from_min_size(inner.min, vec2(14.0, 14.0)), icon::Warning, GOLDENROD);
    controls::wrapped(ui, pos2(inner.min.x + 14.0 + 8.0, inner.min.y), inner.width() - 22.0, text, widgets::font(12.0, 400), p.text_secondary, true);
}

/// A `Button` with an icon (15px) and a `TextBlock` 8 apart (or just the
/// 16px icon), centred.
fn icon_button(ui: &Ui, rect: Rect, id: &str, key: Option<&str>, text: &str, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let r = Button::default().show(ui, rect, Id::new(id), "", enabled);
    let inner = rect.shrink(1.0);
    let fade = |c: Color32| if enabled { c } else { border::group_content(c, p.control_background, 0.35, p.background_panel) };
    let font = widgets::font(13.0, 400);
    if text.is_empty() {
        if let Some(k) = key {
            let at = pos2(inner.min.x + ((inner.width() - 16.0) / 2.0).round_ties_even(), inner.min.y + ((inner.height() - 16.0) / 2.0).round_ties_even());
            icons::paint(ui.painter(), Rect::from_min_size(at, vec2(16.0, 16.0)), k, fade(p.text));
        }
        return r;
    }
    let w = 15.0 + 8.0 + text_width(ui, text, font.clone());
    let left = inner.min.x + ((inner.width() - w) / 2.0).round_ties_even().max(10.0);
    if let Some(k) = key {
        let iy = inner.min.y + ((inner.height() - 15.0) / 2.0).round_ties_even();
        icons::paint(ui.painter(), Rect::from_min_size(pos2(left, iy), vec2(15.0, 15.0)), k, fade(p.text));
    }
    widgets::text_left(ui, inner, left + 23.0, text, font, fade(p.text_secondary));
    r
}

/// A `StackPanel` of a card's fields, laid out top to bottom (and only
/// measured when not painting).
struct Stack<'u> {
    ui: &'u mut Ui,
    x: f32,
    w: f32,
    y: f32,
    paint: bool,
    enabled: bool,
}

impl<'u> Stack<'u> {
    fn new(ui: &'u mut Ui, inner: Rect, paint: bool) -> Self {
        Self { ui, x: inner.min.x, w: inner.width(), y: inner.min.y, paint, enabled: true }
    }

    fn title(&mut self, text: &str, after: f32) {
        if self.paint {
            section_title(self.ui, self.x, self.y, self.w, text);
        }
        self.y += title_h(self.ui) + after;
    }

    fn label(&mut self, text: &str) {
        if self.paint {
            field_label(self.ui, self.x, self.y, text);
        }
        self.y += line_height(self.ui, &widgets::font(13.0, 600)) + 5.0;
    }

    fn combo(&mut self, id: &str, label: &str, items: &[&str], selected: Option<usize>, width: f32, after: f32) -> Option<usize> {
        self.label(label);
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(width.min(self.w.max(220.0)), 40.0));
        self.y += 40.0 + after;
        if !self.paint {
            return None;
        }
        let items: Vec<ComboItem> = items.iter().map(|t| ComboItem::text(*t)).collect();
        ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: self.enabled }.show(self.ui, r, Id::new(("ffmpeg", id)), &items, selected)
    }

    #[allow(clippy::too_many_arguments)]
    fn number(&mut self, id: &str, label: &str, value: &mut i32, min: i64, max: i64, increment: i64, hint: Option<&str>, after: f32) {
        self.label(label);
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(150.0, 40.0));
        self.y += 40.0;
        if self.paint {
            let mut v = *value as i64;
            let spinner = controls::DecimalUpDown { minimum: min as f64, maximum: max as f64, increment: increment as f64, decimals: 0, enabled: self.enabled };
            let mut f = v as f64;
            if spinner.show(self.ui, r, Id::new(("ffmpeg", id)), &mut f) {
                v = f.round() as i64;
                *value = v as i32;
            }
        }
        if let Some(h) = hint {
            self.y += 6.0;
            self.hint(h, 0.0);
        }
        self.y += after;
    }

    /// A field with `Spacing="8"` (Xvid's panel): label, number, hint.
    #[allow(clippy::too_many_arguments)]
    fn number_spaced(&mut self, id: &str, label: &str, value: &mut i32, min: i64, max: i64, hint: Option<&str>, after: f32) {
        if self.paint {
            field_label(self.ui, self.x, self.y, label);
        }
        self.y += line_height(self.ui, &widgets::font(13.0, 600)) + 5.0 + 8.0;
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(150.0, 40.0));
        if self.paint {
            let mut v = *value as i64;
            if (NumericUpDown { minimum: min, maximum: max, enabled: self.enabled }).show(self.ui, r, Id::new(("ffmpeg", id)), &mut v) {
                *value = v as i32;
            }
        }
        self.y += 40.0;
        if let Some(h) = hint {
            self.y += 8.0;
            self.hint(h, 0.0);
        }
        self.y += after;
    }

    fn check(&mut self, id: &str, text: &str, value: &mut bool, after: f32) -> bool {
        let p = palette(self.ui.ctx());
        let c = CheckBox { text_font: widgets::font(13.0, 400), text_color: Some(p.text_secondary), ..CheckBox::new(text) };
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(c.width(self.ui), 32.0));
        self.y += 32.0 + after;
        self.paint && c.show(self.ui, r, Id::new(("ffmpeg", id)), value).changed()
    }

    fn hint(&mut self, text: &str, after: f32) {
        let p = palette(self.ui.ctx());
        let h = controls::wrapped(self.ui, pos2(self.x, self.y), self.w, text, widgets::font(12.0, 400), p.text_secondary, self.paint);
        self.y += h + after;
    }

    fn warning(&mut self, text: &str, after: f32) {
        let h = controls::wrapped(self.ui, pos2(0.0, 0.0), self.w - 26.0 - 22.0, text, widgets::font(12.0, 400), Color32::TRANSPARENT, false);
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(self.w, 26.0 + h.max(14.0)));
        if self.paint {
            warning_card(self.ui, r, text);
        }
        self.y += r.height() + after;
    }

    fn info(&mut self, key: &str, color: Color32, text: &str, after: f32) {
        let p = palette(self.ui.ctx());
        let h = controls::wrapped(self.ui, pos2(0.0, 0.0), self.w - 26.0 - 22.0, text, widgets::font(12.0, 400), Color32::TRANSPARENT, false);
        let r = Rect::from_min_size(pos2(self.x, self.y), vec2(self.w, 26.0 + h.max(14.0)));
        if self.paint {
            info_card(self.ui, r, None);
            let inner = r.shrink(13.0);
            icons::paint(self.ui.painter(), Rect::from_min_size(inner.min, vec2(14.0, 14.0)), key, color);
            controls::wrapped(self.ui, pos2(inner.min.x + 22.0, inner.min.y), inner.width() - 22.0, text, widgets::font(12.0, 400), p.text_secondary, true);
        }
        self.y += r.height() + after;
    }
}

/// The open FFmpeg options window and the task it edits.
pub struct FFmpegOptionsHost {
    pub window: EguiWindow,
    pub state: FFmpegOptionsWindow,
    pub target: TaskTarget,
}

impl App {
    /// Task settings › Screen recorder › Screen recording options...
    /// (`ShowScreenRecordingOptions`): one at a time.
    pub(crate) fn show_ffmpeg_options(&mut self, event_loop: &ActiveEventLoop, target: TaskTarget) {
        if let Some(h) = &self.ffmpeg_options {
            h.window.window.focus_window();
            return;
        }
        let capture = match target {
            TaskTarget::Default => self.settings.settings.DefaultTaskSettings.CaptureSettings.clone(),
            TaskTarget::Hotkey(i) => self.settings.hotkeys.Hotkeys.get(i).and_then(|h| h.TaskSettings.CaptureSettings.clone()),
        }
        .unwrap_or_default();
        let theme = self.current_theme;
        let gpu = match &self.gpu {
            Some(g) => g,
            None => match sharex_ui::gpu::Gpu::new() {
                Ok(g) => self.gpu.insert(g),
                Err(e) => {
                    eprintln!("{e}");
                    return;
                }
            },
        };
        let attrs = winit::window::WindowAttributes::default()
            .with_title(crate::rs_title(s::FFmpegOptionsWindow_Title()))
            .with_window_icon(winit::window::Icon::from_rgba(crate::app_icon::rgba64(), 64, 64).ok())
            .with_theme(Some(match theme {
                egui::Theme::Dark => winit::window::Theme::Dark,
                egui::Theme::Light => winit::window::Theme::Light,
            }))
            .with_visible(false)
            .with_inner_size(LogicalSize::new(WIDTH, HEIGHT))
            .with_min_inner_size(LogicalSize::new(MIN_WIDTH, MIN_HEIGHT));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("couldn't open the FFmpeg options: {e}");
                return;
            }
        };
        let w = match EguiWindow::new(window, gpu, None) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("couldn't open the FFmpeg options: {e}");
                return;
            }
        };
        crate::set_theme(&w, theme);
        // `WindowStartupLocation="CenterOwner"`.
        let owner = self.task_window.as_ref().map(|t| (t.window.window.outer_position().ok(), t.window.window.outer_size()));
        if let Some((Some(pos), size)) = owner {
            let own = w.window.outer_size();
            w.window.set_outer_position(winit::dpi::PhysicalPosition::new(
                pos.x + (size.width as i32 - own.width as i32) / 2,
                pos.y + (size.height as i32 - own.height as i32) / 2,
            ));
        } else {
            crate::center(&w.window, event_loop);
        }
        w.window.set_visible(true);
        w.window.focus_window();
        w.window.request_redraw();
        self.ffmpeg_options = Some(FFmpegOptionsHost { window: w, state: FFmpegOptionsWindow::new(&capture), target });
    }

    /// `capture.FFmpegOptions = window.Options.FFmpeg` once it closes.
    fn close_ffmpeg_options(&mut self) {
        let Some(h) = self.ffmpeg_options.take() else { return };
        let task = match h.target {
            TaskTarget::Default => Some(&mut self.settings.settings.DefaultTaskSettings),
            TaskTarget::Hotkey(i) => self.settings.hotkeys.Hotkeys.get_mut(i).map(|h| &mut h.TaskSettings),
        };
        if let Some(t) = task {
            t.CaptureSettings.get_or_insert_default().FFmpegOptions = h.state.options.ffmpeg;
        }
        if let Some(t) = &self.task_window {
            t.window.window.request_redraw();
        }
    }

    pub(crate) fn ffmpeg_options_window_event(&mut self, _event_loop: &ActiveEventLoop, id: WindowId, event: &WindowEvent) -> bool {
        let Some(h) = self.ffmpeg_options.as_mut().filter(|h| h.window.window.id() == id) else { return false };
        match event {
            WindowEvent::CloseRequested => self.close_ffmpeg_options(),
            WindowEvent::RedrawRequested => {
                let state = &mut h.state;
                let effects = h.window.paint(|ui| state.ui(ui));
                for e in effects {
                    match e {
                        Effect::Browse => {
                            // `BrowseForFFmpegAsync`: from the current FFmpeg's folder,
                            // else Program Files.
                            let current = discovery::ffmpeg_path(&h.state.options.ffmpeg);
                            let start = current.parent().filter(|d| d.is_dir()).map(Path::to_path_buf).or_else(|| std::env::var_os("ProgramFiles").map(PathBuf::from));
                            let mut dialog = rfd::FileDialog::new()
                                .set_title(s::FFmpegOptionsWindow_Browse_for_ffmpeg())
                                .add_filter(s::FFmpegOptionsWindow_FFmpeg_executable(), &["exe"]);
                            if let Some(d) = start {
                                dialog = dialog.set_directory(d);
                            }
                            if let Some(file) = dialog.pick_file() {
                                h.state.browsed(&file);
                            }
                            h.window.window.request_redraw();
                        }
                        Effect::OpenUrl(url) => crate::app_windows::open_path(Path::new(url)),
                    }
                }
            }
            other => h.window.on_event(other),
        }
        true
    }

    pub(crate) fn ffmpeg_options_wake_at(&mut self, now: Instant) -> Option<Instant> {
        let h = self.ffmpeg_options.as_mut()?;
        if h.window.repaint_at.is_some_and(|at| at <= now) {
            h.window.window.request_redraw();
            h.window.repaint_at = None;
        }
        h.window.repaint_at
    }

    /// The "FFmpeg is missing" message was answered Yes:
    /// `FFmpegGitHubDownloader.DownloadFFmpeg` on a thread, `ffmpeg.exe`
    /// extracted to where FFmpeg was looked for.
    pub(crate) fn download_ffmpeg(&mut self, target: PathBuf) {
        if self.ffmpeg_download.is_some() {
            return;
        }
        eprintln!("Downloading FFmpeg to {}", target.display());
        let proxy = self.proxy.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let result = download_ffmpeg_to(&target);
            let _ = tx.send(result);
            let _ = proxy.send_event(crate::UserEvent::Recording(super::Event::FFmpegDownloaded));
        });
        self.ffmpeg_download = Some(Download { rx });
    }
}

impl App {
    /// The download ended: says how (`DownloaderWindow`'s end isn't
    /// ported; a message box tells).
    pub(crate) fn ffmpeg_downloaded(&mut self, event_loop: &ActiveEventLoop) {
        let Some(result) = self.ffmpeg_download.as_ref().and_then(Download::finished) else { return };
        self.ffmpeg_download = None;
        let message = match result {
            Ok(path) => {
                eprintln!("FFmpeg downloaded: {}", path.display());
                crate::message_box::MessageBox::ok("ShareX - FFmpeg", path.display().to_string(), crate::message_box::MessageBoxIcon::Information)
            }
            Err(e) => {
                eprintln!("FFmpeg download failed: {e}");
                crate::message_box::MessageBox::ok("ShareX - FFmpeg", e, crate::message_box::MessageBoxIcon::Error)
            }
        };
        if let Err(e) = self.open_dialog(event_loop, message, crate::DialogPurpose::Information) {
            eprintln!("couldn't open the message box: {e}");
        }
    }
}

/// FFmpeg being downloaded.
pub struct Download {
    rx: Receiver<Result<PathBuf, String>>,
}

impl Download {
    /// The result, once there.
    pub fn finished(&self) -> Option<Result<PathBuf, String>> {
        self.rx.try_recv().ok()
    }
}

/// `FFmpegUpdateChecker("ShareX", "FFmpeg")`'s release list.
const FFMPEG_RELEASES: &str = "https://api.github.com/repos/ShareX/FFmpeg/releases/latest";

/// `FFmpegUpdateChecker.UpdateReleaseInfo`: a `v`-tagged release's first
/// asset whose name ends with `win64.zip` (`winarm64.zip` on ARM), its
/// browser download URL.
pub fn release_download_url(json: &str) -> Option<String> {
    let release: serde_json::Value = serde_json::from_str(json).ok()?;
    let tag = release.get("tag_name")?.as_str()?;
    if tag.len() <= 1 || !tag.starts_with('v') {
        return None;
    }
    let ends_with = if cfg!(target_arch = "aarch64") { "winarm64.zip" } else { "win64.zip" };
    release.get("assets")?.as_array()?.iter().find_map(|a| {
        let name = a.get("name")?.as_str()?;
        name.to_ascii_lowercase().ends_with(ends_with).then(|| a.get("browser_download_url")?.as_str().map(str::to_owned))?
    })
}

/// `FFmpegGitHubDownloader.ExtractFFmpeg`: the archive's `ffmpeg.exe`
/// (any folder, at most 200 MB uncompressed) written to `target`.
pub fn extract_ffmpeg(archive: &[u8], target: &Path) -> Result<(), String> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().rsplit(['/', '\\']).next().unwrap_or_default().to_owned();
        if !name.eq_ignore_ascii_case("ffmpeg.exe") || entry.is_dir() {
            continue;
        }
        if entry.size() > 200_000_000 {
            return Err("ffmpeg.exe is larger than 200 MB".into());
        }
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry.by_ref().take(200_000_001).read_to_end(&mut data).map_err(|e| e.to_string())?;
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(target, data).map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err("the archive has no ffmpeg.exe".into())
}

fn download_ffmpeg_to(target: &Path) -> Result<PathBuf, String> {
    use sharex_uploaders::http;
    let get = |url: &str| -> Result<Vec<u8>, String> {
        let request = http::HttpRequest { method: sharex_uploaders::HttpMethod::GET, url: url.to_owned(), headers: Vec::new(), content: None };
        let response = http::send(request, http::RequestOptions::default()).map_err(|e| e.to_string())?;
        if !response.is_success() {
            return Err(format!("{} {}", response.status_code, response.status_description));
        }
        Ok(response.response_data.or_else(|| response.response_text.map(String::into_bytes)).unwrap_or_default())
    };
    let json = String::from_utf8(get(FFMPEG_RELEASES)?).map_err(|e| e.to_string())?;
    let url = release_download_url(&json).ok_or("no FFmpeg download in the latest release")?;
    let archive = get(&url)?;
    extract_ffmpeg(&archive, target)?;
    Ok(target.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_gives_the_win64_zip() {
        let json = r#"{"tag_name":"v7.1","assets":[{"name":"ffmpeg-7.1-win32.zip","browser_download_url":"https://x/32"},{"name":"ffmpeg-7.1-win64.zip","browser_download_url":"https://x/64"}]}"#;
        if !cfg!(target_arch = "aarch64") {
            assert_eq!(release_download_url(json).as_deref(), Some("https://x/64"));
        }
        assert_eq!(release_download_url(r#"{"tag_name":"7.1","assets":[]}"#), None);
    }

    #[test]
    fn ffmpeg_exe_comes_out_of_any_folder() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o: zip::write::SimpleFileOptions = Default::default();
            w.start_file("ffmpeg-7.1/bin/ffprobe.exe", o).unwrap();
            w.write_all(b"probe").unwrap();
            w.start_file("ffmpeg-7.1/bin/FFmpeg.exe", o).unwrap();
            w.write_all(b"MZ ffmpeg").unwrap();
            w.finish().unwrap();
        }
        let dir = std::env::temp_dir().join(format!("sharexrs-ffmpeg-extract-{}", std::process::id()));
        let target = dir.join("ffmpeg.exe");
        extract_ffmpeg(&buf, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"MZ ffmpeg");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_preview_is_the_command_line() {
        let capture = TaskSettingsCapture::default();
        let w = FFmpegOptionsWindow::new(&capture);
        let preview = w.command_preview();
        assert!(preview.contains("-c:v libx264"), "{preview}");
        assert!(preview.ends_with("\"output.mp4\""), "{preview}");
    }
}
