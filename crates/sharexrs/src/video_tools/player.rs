//! `ShareX.Tools/Tools/VideoPlayer` (`VideoPlayerWindow.axaml(.cs)`,
//! `VideoPlayerViewModel`): the video filling the window (black behind it),
//! the empty/loading/error state, and the playback controls in a
//! translucent bar 12 pixels above the bottom (ShareX's `Popup`): the seek
//! slider, Open, previous/next frame, play/pause, the time, mute, volume
//! and fullscreen. The bar hides 3 seconds after the last pointer move
//! while playing. Keys: Space, Left/Right (5 s), Up/Down (volume), M,
//! Ctrl+O, F11 / Alt+Enter, Escape in fullscreen. Playback is
//! [`super::media::MediaPlayer`].

use std::time::{Duration, Instant};

use egui::{Color32, Id, Rect, Sense, Ui, pos2, vec2};
use sharex_strings::tools as s;
use sharex_ui::controls::Slider;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use super::media::MediaPlayer;
use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 960.0;
pub const HEIGHT: f32 = 600.0;
pub const MIN_WIDTH: f32 = 540.0;
pub const MIN_HEIGHT: f32 = 320.0;

/// `VideoPlayerViewModel.FormatTime`: `m:ss`-style, hours when there are any.
pub fn format_time(seconds: f64) -> String {
    let ticks = (seconds.max(0.0) * 10_000_000.0) as i64;
    let total = ticks / 10_000_000;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h >= 1 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

pub struct VideoPlayerWindow {
    pub player: MediaPlayer,
    pub input_file_path: String,
    pub duration: f64,
    pub position: f64,
    pub volume: f64,
    pub muted: bool,
    pub status_text: String,
    pub loading: bool,
    fullscreen: bool,
    seeking: bool,
    resume_after_seek: bool,
    last_interaction: Instant,
    last_pointer: Option<egui::Pos2>,
    /// Shows the bar whatever the timer says (`--render`).
    pub force_controls: Option<bool>,
    title_sent: String,
}

impl VideoPlayerWindow {
    /// `new VideoPlayerWindow(inputFilePath)`.
    pub fn new(ffmpeg: Option<std::path::PathBuf>, input: Option<String>) -> Self {
        let mut w = Self {
            player: MediaPlayer::new(ffmpeg),
            input_file_path: String::new(),
            duration: 0.0,
            position: 0.0,
            volume: 1.0,
            muted: false,
            status_text: s::VideoTrimmer_ChooseVideo().into(),
            loading: false,
            fullscreen: false,
            seeking: false,
            resume_after_seek: false,
            last_interaction: Instant::now(),
            last_pointer: None,
            force_controls: None,
            title_sent: String::new(),
        };
        if let Some(f) = input.filter(|f| !f.is_empty()) {
            w.load_input(&f);
        }
        w
    }

    fn has_video(&self) -> bool {
        self.duration > 0.0
    }

    fn sound_off(&self) -> bool {
        self.muted || self.volume == 0.0
    }

    /// `WindowTitle`.
    fn window_title(&self) -> String {
        let title = crate::rs_title(s::VideoPlayer_Title());
        if self.input_file_path.is_empty() { title } else { format!("{} - {title}", sharex_tools::dotnet::file_name(&self.input_file_path)) }
    }

    /// `LoadInputAsync`.
    pub fn load_input(&mut self, path: &str) {
        self.seeking = false;
        self.resume_after_seek = false;
        self.player.unload();
        self.duration = 0.0;
        self.position = 0.0;
        self.input_file_path = path.to_string();
        if !std::path::Path::new(path).is_file() {
            self.failed(s::VideoTrimmer_InvalidVideo());
            return;
        }
        self.status_text = s::VideoPlayer_Loading().into();
        self.loading = true;
        self.player.load(path);
    }

    /// `HandlePlaybackFailure`.
    fn failed(&mut self, message: &str) {
        self.seeking = false;
        self.resume_after_seek = false;
        self.player.unload();
        self.duration = 0.0;
        self.position = 0.0;
        self.loading = false;
        self.status_text = s::VideoTrimmer_PlaybackError().replace("{0}", message);
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        match self.player.poll(ctx) {
            Some(Ok(info)) => {
                self.loading = false;
                self.duration = info.duration;
                self.status_text.clear();
                self.player.set_volume(self.volume);
                self.player.set_muted(self.muted);
                // `_player.Play()` once loaded.
                self.player.play();
                self.show_controls();
            }
            Some(Err(e)) => self.failed(&e),
            None => {}
        }
        if !self.seeking && self.has_video() {
            self.position = self.player.position();
        }
    }

    fn toggle_play(&mut self) {
        if !self.has_video() {
            return;
        }
        if self.player.is_playing() {
            self.player.pause();
        } else {
            self.player.play();
        }
    }

    /// `ToggleMute`.
    fn toggle_mute(&mut self) {
        if self.sound_off() {
            if self.volume == 0.0 {
                self.set_volume(1.0);
            }
            self.muted = false;
        } else {
            self.muted = true;
        }
        self.player.set_muted(self.muted);
    }

    fn set_volume(&mut self, v: f64) {
        self.volume = v.clamp(0.0, 1.0);
        self.player.set_volume(self.volume);
    }

    /// `SeekRelative` / `Position = …`.
    fn set_position(&mut self, value: f64) {
        if !self.has_video() {
            return;
        }
        self.position = value.clamp(0.0, self.duration);
        if self.seeking {
            return;
        }
        let resume = self.player.is_playing();
        self.player.seek(self.position);
        if resume {
            self.player.play();
        }
    }

    fn show_controls(&mut self) {
        self.last_interaction = Instant::now();
    }

    /// `UpdateControls`: the bar's `IsOpen`.
    fn controls_open(&self, ui: &Ui, over_controls: bool) -> bool {
        if let Some(f) = self.force_controls {
            return f;
        }
        let focused = ui.input(|i| i.viewport().focused.unwrap_or(true));
        focused && (!self.player.is_playing() || self.seeking || over_controls || self.last_interaction.elapsed() < Duration::from_millis(3000))
    }

    fn keys(&mut self, ui: &Ui) -> Vec<Effect> {
        let mut effects = Vec::new();
        let events = ui.input(|i| i.events.clone());
        for e in events {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = e else { continue };
            let handled = if key == egui::Key::O && modifiers.command_only() {
                effects.extend(self.browse());
                true
            } else if key == egui::Key::F11 || key == egui::Key::Enter && modifiers.alt || key == egui::Key::Escape && self.fullscreen {
                effects.push(self.toggle_fullscreen());
                true
            } else if modifiers.is_none() {
                match key {
                    egui::Key::Space => self.toggle_play(),
                    egui::Key::ArrowLeft => self.set_position(self.position - 5.0),
                    egui::Key::ArrowRight => self.set_position(self.position + 5.0),
                    egui::Key::ArrowUp => self.set_volume(self.volume + 0.05),
                    egui::Key::ArrowDown => self.set_volume(self.volume - 0.05),
                    egui::Key::M => self.toggle_mute(),
                    _ => continue,
                }
                true
            } else {
                false
            };
            if handled {
                self.show_controls();
            }
        }
        effects
    }

    fn toggle_fullscreen(&mut self) -> Effect {
        self.fullscreen = !self.fullscreen;
        self.show_controls();
        Effect::SetFullscreen(self.fullscreen)
    }

    /// `BrowseAsync`.
    fn browse(&mut self) -> Vec<Effect> {
        let video = s::VideoTrimmer_VideoFiles();
        let all = s::VideoTrimmer_AllFiles();
        if let Some(f) = rfd::FileDialog::new().set_title(s::VideoTrimmer_Open()).add_filter(video, &super::VIDEO_PATTERNS).add_filter(all, &["*"]).pick_file() {
            self.load_input(&f.to_string_lossy());
        }
        self.show_controls();
        Vec::new()
    }

    /// A `Button.player-button`: 32x32, transparent, `#30FFFFFF` under the
    /// pointer, `#55FFFFFF` pressed, the icon white (35% disabled).
    fn player_button(ui: &Ui, rect: Rect, id: Id, key: &str, enabled: bool) -> egui::Response {
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let r = ui.interact(rect, id, sense);
        if enabled {
            let fill = if r.is_pointer_button_down_on() {
                Some(Color32::from_rgba_unmultiplied(255, 255, 255, 0x55))
            } else if r.hovered() {
                Some(Color32::from_rgba_unmultiplied(255, 255, 255, 0x30))
            } else {
                None
            };
            if let Some(c) = fill {
                sharex_ui::border::border(ui.painter(), rect, 4.0, 0.0, Some(sharex_ui::border::Brush::Solid(c)), None);
            }
        }
        let size = 13.0;
        let b = Rect::from_min_size(pos2(rect.min.x + ((rect.width() - size) / 2.0).round_ties_even(), center_y(rect, size)), vec2(size, size));
        let color = if enabled { Color32::WHITE } else { Color32::WHITE.gamma_multiply(0.35) };
        sharex_ui::icons::paint(ui.painter(), b, key, color);
        r
    }
}

impl Dialog for VideoPlayerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(self.window_title(), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        self.poll(ui.ctx());
        let mut effects = self.keys(ui);
        let p = palette(ui.ctx());
        let viewport = ui.max_rect();
        let has_video = self.has_video();
        ui.painter().rect_filled(viewport, 0.0, if has_video { Color32::BLACK } else { p.background_main });
        let title = self.window_title();
        if title != self.title_sent {
            self.title_sent = title.clone();
            effects.push(Effect::SetTitle(title));
        }
        // A pointer move over the window counts as interaction.
        let pointer = ui.input(|i| i.pointer.hover_pos());
        if pointer.is_some() && pointer != self.last_pointer {
            self.last_interaction = Instant::now();
        }
        self.last_pointer = pointer;

        // The video, `Width = min(available, height × aspect)`, centred.
        if has_video {
            let aspect = self.player.aspect() as f32;
            let w = viewport.width().min(viewport.height() * aspect);
            let h = w / aspect.max(0.001);
            let video = Rect::from_center_size(viewport.center(), vec2(w, h));
            if let Some((tex, _)) = self.player.texture(ui.ctx()) {
                ui.painter().image(tex, video, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            let r = ui.interact(video, Id::new("player-video"), Sense::click());
            if r.clicked() {
                self.toggle_play();
                self.show_controls();
            }
        } else {
            // The empty state: `Margin="32,32,32,100"`, `MaxWidth="640"`,
            // `Spacing="16"`: the 40px film icon, the status, the bar.
            let area = Rect::from_min_max(pos2(viewport.min.x + 32.0, viewport.min.y + 32.0), pos2(viewport.max.x - 32.0, viewport.max.y - 100.0));
            let nf = font(13.0, 400);
            let max_w = area.width().min(640.0);
            let lines = sharex_ui::controls::wrap_lines(ui, &self.status_text, &nf, max_w);
            let lh = line_height(ui, &nf);
            let text_h = lh * lines.len().max(1) as f32;
            let text_w = lines.iter().map(|l| text_width(ui, l, nf.clone())).fold(0.0, f32::max);
            let bar = if self.loading { 16.0 + 4.0 } else { 0.0 };
            let widest = text_w.max(40.0).max(if self.loading { 160.0 } else { 0.0 });
            let total = 40.0 + 16.0 + text_h + bar;
            let x0 = area.min.x + ((area.width() - widest) / 2.0).round_ties_even();
            let mut y = area.min.y + ((area.height() - total) / 2.0).round_ties_even();
            let ir = Rect::from_min_size(pos2(x0 + ((widest - 40.0) / 2.0).round_ties_even(), y), vec2(40.0, 40.0));
            // The `IconPresenter` keeps Fluent's foreground (base high).
            sharex_ui::icons::paint(ui.painter(), ir, icon::Film, sharex_ui::theme::fluent(ui.ctx()).base_high);
            y += 56.0;
            for l in &lines {
                // `TextAlignment="Center"`.
                let lw = text_width(ui, l, nf.clone());
                tools::text_at(ui, pos2(x0 + (widest - lw) / 2.0, y), l, nf.clone(), p.text);
                y += lh;
            }
            if self.loading {
                y += 16.0;
                let b = Rect::from_min_size(pos2(x0 + ((widest - 160.0) / 2.0).round_ties_even(), y), vec2(160.0, 4.0));
                sharex_ui::controls::indeterminate_progress(ui.painter(), b, ui.input(|i| i.time));
                ui.ctx().request_repaint();
            }
        }

        // The playback controls: `Width = min(500, viewport - 24)`, bottom
        // 12px above the viewport's, centred; `Padding="8,4"`, 6px corners.
        let cw = (viewport.width() - 24.0).clamp(0.0, 500.0);
        let ch = 4.0 + 30.0 + 2.0 + 36.0 + 4.0;
        let controls = Rect::from_min_size(pos2(viewport.center().x - cw / 2.0, viewport.max.y - 12.0 - ch), vec2(cw, ch));
        let over = ui.rect_contains_pointer(controls);
        if !self.controls_open(ui, over) {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
            return effects;
        }
        if self.player.is_playing() {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
        sharex_ui::border::border(ui.painter(), controls, 6.0, 0.0, Some(sharex_ui::border::Brush::Solid(Color32::from_rgba_unmultiplied(0, 0, 0, 0x80))), None);
        let inner = Rect::from_min_max(pos2(controls.min.x + 8.0, controls.min.y + 4.0), pos2(controls.max.x - 8.0, controls.max.y - 4.0));
        // Row 0: the seek slider (`Height="30"`, `Padding="5,0"`).
        let seek = Rect::from_min_size(inner.min, vec2(inner.width(), 30.0));
        let mut v = self.position as f32;
        let slider = Slider { minimum: 0.0, maximum: self.duration.max(0.0) as f32, step: 0.0, enabled: has_video };
        let seek_id = Id::new("player-seek");
        let track = Rect::from_min_max(pos2(seek.min.x + 5.0, seek.min.y - 1.0), pos2(seek.max.x - 5.0, seek.max.y + 1.0));
        let changed = slider.show(ui, track, seek_id, &mut v);
        let dragging = ui.ctx().is_being_dragged(seek_id) || ui.input(|i| i.pointer.primary_down()) && ui.ctx().memory(|m| m.focused() == Some(seek_id));
        if has_video && ui.ctx().is_being_dragged(seek_id) && !self.seeking {
            // `BeginSeek`.
            self.seeking = true;
            self.resume_after_seek = self.player.is_playing();
            self.player.pause();
        }
        if changed {
            self.set_position(v as f64);
        }
        if self.seeking && !dragging {
            // `EndSeek`.
            self.seeking = false;
            self.player.seek(self.position);
            if self.resume_after_seek {
                self.player.play();
            }
            self.resume_after_seek = false;
            self.show_controls();
        }
        // Row 1: the buttons (`ColumnSpacing="2"`).
        let row = Rect::from_min_size(pos2(inner.min.x, seek.max.y + 2.0), vec2(inner.width(), 36.0));
        let b = |x: f32| Rect::from_min_size(pos2(x, center_y(row, 32.0)), vec2(32.0, 32.0));
        let mut x = row.min.x;
        if Self::player_button(ui, b(x), Id::new("player-open"), icon::FolderOpen, true).clicked() {
            effects.extend(self.browse());
        }
        x += 34.0;
        if Self::player_button(ui, b(x), Id::new("player-back"), icon::StepBack, has_video).clicked() {
            self.player.step_frame(false);
            self.show_controls();
        }
        x += 34.0;
        let play_icon = if self.player.is_playing() { icon::Pause } else { icon::Play };
        if Self::player_button(ui, b(x), Id::new("player-play"), play_icon, has_video).clicked() {
            self.toggle_play();
            self.show_controls();
        }
        x += 34.0;
        if Self::player_button(ui, b(x), Id::new("player-forward"), icon::StepForward, has_video).clicked() {
            self.player.step_frame(true);
            self.show_controls();
        }
        x += 34.0;
        let right = row.max.x;
        let full = Rect::from_min_size(pos2(right - 32.0, center_y(row, 32.0)), vec2(32.0, 32.0));
        let volume = Rect::from_min_size(pos2(full.min.x - 2.0 - 60.0, row.min.y), vec2(60.0, 36.0));
        let mute = Rect::from_min_size(pos2(volume.min.x - 2.0 - 32.0, center_y(row, 32.0)), vec2(32.0, 32.0));
        // The time, centred in the `*` column with `Margin="6,0"`.
        let nf = font(13.0, 400);
        let time = format!("{} / {}", format_time(self.position), format_time(self.duration));
        let col = Rect::from_min_max(pos2(x + 6.0, row.min.y), pos2(mute.min.x - 2.0 - 6.0, row.max.y));
        let tw = text_width(ui, &time, nf.clone());
        tools::text_at(ui, pos2(col.min.x + ((col.width() - tw) / 2.0).round_ties_even(), center_y(row, line_height(ui, &nf))), &time, nf, Color32::WHITE);
        let mute_icon = if self.sound_off() { icon::Mute } else { icon::Volume };
        if Self::player_button(ui, mute, Id::new("player-mute"), mute_icon, true).clicked() {
            self.toggle_mute();
            self.show_controls();
        }
        let mut vol = self.volume as f32;
        let vtrack = Rect::from_min_max(pos2(volume.min.x, volume.min.y + 2.0), pos2(volume.max.x, volume.max.y - 2.0));
        if (Slider { minimum: 0.0, maximum: 1.0, step: 0.0, enabled: true }).show(ui, vtrack, Id::new("player-volume"), &mut vol) {
            self.set_volume(vol as f64);
            self.show_controls();
        }
        let full_icon = if self.fullscreen { icon::Shrink } else { icon::Expand };
        if Self::player_button(ui, full, Id::new("player-fullscreen"), full_icon, true).clicked() {
            effects.push(self.toggle_fullscreen());
        }
        effects
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(f) = files.first() {
            self.load_input(&f.to_string_lossy());
        }
        Vec::new()
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        self.player.unload();
        Vec::new()
    }

    fn activated(&mut self) -> Vec<Effect> {
        self.show_controls();
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_minutes_until_an_hour() {
        assert_eq!(format_time(0.0), "00:00");
        assert_eq!(format_time(65.9), "01:05");
        assert_eq!(format_time(3725.0), "1:02:05");
        assert_eq!(format_time(-3.0), "00:00");
    }

    #[test]
    fn a_missing_file_is_a_playback_error() {
        let mut w = VideoPlayerWindow::new(None, None);
        assert_eq!(w.status_text, s::VideoTrimmer_ChooseVideo());
        w.load_input("no such file.mp4");
        assert!(!w.has_video());
        assert_eq!(w.status_text, s::VideoTrimmer_PlaybackError().replace("{0}", s::VideoTrimmer_InvalidVideo()));
        assert!(w.window_title().starts_with("no such file.mp4 - "));
    }

    #[test]
    fn mute_restores_a_zero_volume() {
        let mut w = VideoPlayerWindow::new(None, None);
        w.set_volume(0.0);
        assert!(w.sound_off());
        w.toggle_mute();
        assert!(!w.sound_off() && w.volume == 1.0);
        w.toggle_mute();
        assert!(w.muted);
    }
}
