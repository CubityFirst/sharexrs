//! `ShareX.Tools/Tools/VideoTrimmer` (`VideoTrimmerWindow.axaml(.cs)`,
//! `VideoTrimmerViewModel`, `VideoTrimmerTimeline`, `VideoTrimmerService`)
//! and `TaskHelpers.OpenVideoTrimmer`: the video played in the preview,
//! its 12 filmstrip thumbnails (`NativeVideoThumbnails`), the selection on
//! the timeline and in the time boxes, playback stopping at the end of the
//! selection, Lossless (stream copy) or Re-encode (H.264/AAC MP4), and
//! Save trimmed video... with FFmpeg's progress and Cancel. Playback is
//! [`super::media::MediaPlayer`]; the thumbnails are FFmpeg's too.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use egui::{Color32, Id, Rect, Sense, Ui, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::video::trimmer::{format_time, try_parse_time};
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{RadioButton, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use super::media::{MediaInfo, MediaPlayer};
use super::timeline::{self, CellCache, CellPlacement, TimelineInput, TimelineKey, TimelineState};
use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools::{self as tools, Picture};
use crate::message_box::{MessageBox, MessageBoxIcon};

pub const WIDTH: f32 = 900.0;
pub const HEIGHT: f32 = 720.0;
pub const MIN_WIDTH: f32 = 760.0;
pub const MIN_HEIGHT: f32 = 620.0;

/// `NativeVideoThumbnails.CreateAsync`: 12 frames at `duration * i / 12`,
/// each scaled to fit 160x90 (even sizes), decoded by FFmpeg.
pub fn create_thumbnails(ffmpeg: &std::path::Path, path: &str, info: &MediaInfo) -> Vec<(f64, image::RgbaImage)> {
    use std::io::Read;
    let scale = (160.0 / info.width as f64).min(90.0 / info.height as f64);
    let tw = (((info.width as f64 * scale) as u32) & !1).max(2);
    let th = (((info.height as f64 * scale) as u32) & !1).max(2);
    let mut out = Vec::new();
    for i in 0..12 {
        let target = info.duration * i as f64 / 12.0;
        let mut c = std::process::Command::new(ffmpeg);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000);
        }
        c.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-ss", &format!("{target:.6}"), "-i", path, "-map", "0:v:0", "-frames:v", "1"]);
        c.args(["-vf", &format!("scale={tw}:{th}:flags=bilinear"), "-pix_fmt", "rgba", "-f", "rawvideo", "-"]);
        let Ok(mut child) = c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn() else { break };
        let mut buf = vec![0u8; (tw * th * 4) as usize];
        let ok = child.stdout.take().is_some_and(|mut o| o.read_exact(&mut buf).is_ok());
        let _ = child.wait();
        if ok && let Some(img) = image::RgbaImage::from_raw(tw, th, buf) {
            out.push((target, img));
        }
    }
    out
}

struct Export {
    rx: mpsc::Receiver<Result<(), String>>,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
    output: String,
}

pub struct VideoTrimmerWindow {
    pub player: MediaPlayer,
    pub input_file_path: String,
    pub output_file_path: String,
    pub status_text: String,
    pub duration: f64,
    pub position: f64,
    pub start: f64,
    pub end: f64,
    pub precise: bool,
    loading: bool,
    thumbnails: Vec<(f64, Picture)>,
    thumbnails_rx: Option<mpsc::Receiver<Vec<(f64, image::RgbaImage)>>>,
    thumbnails_generation: u64,
    cells: CellCache,
    export: Option<Export>,
    progress: f64,
    ffmpeg: Option<std::path::PathBuf>,
    task: TaskSettings,
    /// Shown exporting (`--render`) without FFmpeg running.
    fake_export: bool,
    /// A load's error message box is open.
    error_open: bool,
}

impl VideoTrimmerWindow {
    /// `new VideoTrimmerWindow(ffmpegPath, inputFilePath, playNotificationSound, resolveFFmpegPath)`.
    pub fn new(task: TaskSettings, ffmpeg: Option<std::path::PathBuf>, input: Option<String>) -> Self {
        let mut w = Self {
            player: MediaPlayer::new(ffmpeg.clone()),
            input_file_path: String::new(),
            output_file_path: String::new(),
            status_text: s::VideoTrimmer_ChooseVideo().into(),
            duration: 0.0,
            position: 0.0,
            start: 0.0,
            end: 0.0,
            precise: false,
            loading: false,
            thumbnails: Vec::new(),
            thumbnails_rx: None,
            thumbnails_generation: 0,
            cells: CellCache::default(),
            export: None,
            progress: 0.0,
            ffmpeg,
            task,
            fake_export: false,
            error_open: false,
        };
        if let Some(f) = input.filter(|f| !f.is_empty()) {
            w.load_input(&f);
        }
        w
    }

    fn has_video(&self) -> bool {
        self.duration > 0.0
    }

    fn is_exporting(&self) -> bool {
        self.export.is_some() || self.fake_export
    }

    fn can_edit(&self) -> bool {
        self.has_video() && !self.is_exporting()
    }

    fn can_browse(&self) -> bool {
        !self.is_exporting()
    }

    fn can_trim(&self) -> bool {
        self.can_edit() && (self.start >= 0.001 || self.end <= self.duration - 0.001)
    }

    fn minimum_selection(&self) -> f64 {
        0.001f64.min(self.duration)
    }

    /// `LoadInputAsync`.
    pub fn load_input(&mut self, file: &str) {
        if !self.can_browse() {
            return;
        }
        self.player.unload();
        self.duration = 0.0;
        self.position = 0.0;
        self.start = 0.0;
        self.end = 0.0;
        self.clear_thumbnails();
        self.input_file_path = file.to_string();
        self.output_file_path.clear();
        self.loading = true;
        self.status_text = s::VideoTrimmer_Loading().into();
        if !std::path::Path::new(file).is_file() {
            self.loading = false;
            self.status_text = s::VideoTrimmer_PlaybackError().replace("{0}", s::VideoTrimmer_InvalidVideo());
            return;
        }
        self.player.load(file);
    }

    fn clear_thumbnails(&mut self) {
        self.thumbnails.clear();
        self.thumbnails_rx = None;
        self.thumbnails_generation += 1;
        self.cells.clear();
    }

    /// `Position = value` (and the seek it makes).
    fn set_position(&mut self, value: f64) {
        let v = if value.is_finite() { value.clamp(0.0, self.duration) } else { 0.0 };
        self.position = v;
        if self.has_video() {
            self.player.seek(v);
        }
    }

    /// `Start = value` (`OnStartChanged`: clamped, then the playhead there).
    fn set_start(&mut self, value: f64) {
        let v = if value.is_finite() { value.clamp(0.0, (self.end - self.minimum_selection()).max(0.0)) } else { 0.0 };
        if v == self.start {
            return;
        }
        self.start = v;
        if self.has_video() {
            self.set_position(v);
        }
    }

    /// `End = value`.
    fn set_end(&mut self, value: f64) {
        let v = if value.is_finite() { value.clamp(self.duration.min(self.start + self.minimum_selection()), self.duration) } else { self.duration };
        if v == self.end {
            return;
        }
        self.end = v;
        if self.has_video() {
            self.set_position(v);
        }
    }

    /// `TogglePlay`.
    fn toggle_play(&mut self) {
        if !self.can_edit() {
            return;
        }
        if self.player.is_playing() {
            self.player.pause();
        } else {
            if self.position < self.start || self.position >= self.end - 0.001 {
                self.set_position(self.start);
            }
            self.player.play();
        }
    }

    /// `Reset`.
    fn reset(&mut self) {
        if self.can_edit() {
            self.set_start(0.0);
            self.set_end(self.duration);
            self.set_position(0.0);
        }
    }

    /// `ExportAsync`: FFmpeg resolved first (`CheckFFmpeg`), then the save
    /// dialog with the mode's extension.
    fn export(&mut self, ffmpeg: Option<std::path::PathBuf>) {
        if !self.can_trim() {
            return;
        }
        let Some(ffmpeg) = ffmpeg else { return };
        let ext = if self.precise { ".mp4".to_string() } else { sharex_tools::dotnet::extension(&self.input_file_path).to_string() };
        let name = format!("{}-trimmed{ext}", sharex_tools::dotnet::file_name_without_extension(&self.input_file_path));
        let folder = tools::directory_name(&self.input_file_path);
        let pattern = ext.trim_start_matches('.').to_string();
        let Some(output) = super::save_file(s::VideoTrimmer_Export(), &name, &folder, s::VideoTrimmer_VideoFiles(), &[pattern.as_str()]) else { return };
        self.start_export(ffmpeg, output);
    }

    /// `TrimAsync` on a thread.
    pub fn start_export(&mut self, ffmpeg: std::path::PathBuf, output: String) {
        self.player.pause();
        self.thumbnails_rx = None;
        self.progress = 0.0;
        self.status_text = s::VideoTrimmer_Exporting().into();
        let progress = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (p, c) = (progress.clone(), cancel.clone());
        let (input, out, start, end, duration, precise) = (self.input_file_path.clone(), output.clone(), self.start, self.end, self.duration, self.precise);
        std::thread::spawn(move || {
            let r = sharex_tools::video::trimmer::trim_cancellable(
                &ffmpeg,
                &input,
                &out,
                start,
                end,
                duration,
                precise,
                |v| p.store(v.to_bits(), Ordering::Relaxed),
                Some(&c),
            );
            let _ = tx.send(r);
        });
        self.export = Some(Export { rx, progress, cancel, output });
    }

    /// Waits for an export ([`Self::start_export`], `--render`).
    pub fn finish_export(&mut self) {
        if let Some(e) = &self.export {
            let r = e.rx.recv().unwrap_or_else(|_| Err("stopped".into()));
            let (tx, rx) = mpsc::channel();
            let _ = tx.send(r);
            if let Some(e) = &mut self.export {
                e.rx = rx;
            }
            self.poll(&egui::Context::default());
        }
    }

    /// Shows the exporting state at `progress` percent (`--render`).
    pub fn set_exporting(&mut self, progress: f64) {
        self.fake_export = true;
        self.progress = progress;
        self.status_text = s::VideoTrimmer_Exporting().into();
    }

    fn error_box(text: &str) -> Effect {
        Effect::MessageBox(Box::new(MessageBox::ok(crate::rs_title(s::VideoTrimmer_Title()), text, MessageBoxIcon::Error)))
    }

    pub fn poll(&mut self, ctx: &egui::Context) -> Vec<Effect> {
        let mut effects = Vec::new();
        match self.player.poll(ctx) {
            Some(Ok(info)) => {
                self.duration = info.duration;
                self.end = info.duration;
                self.position = 0.0;
                if let Some(ffmpeg) = self.ffmpeg.clone() {
                    let (tx, rx) = mpsc::channel();
                    let path = self.input_file_path.clone();
                    let work = move || {
                        let _ = tx.send(create_thumbnails(&ffmpeg, &path, &info));
                    };
                    if self.player.sync {
                        work();
                    } else {
                        std::thread::spawn(work);
                    }
                    self.thumbnails_rx = Some(rx);
                } else {
                    self.loading = false;
                    self.status_text.clear();
                }
            }
            Some(Err(e)) => {
                // The modal error box: `IsLoading` until it's dismissed.
                self.error_open = true;
                self.duration = 0.0;
                self.clear_thumbnails();
                self.status_text = s::VideoTrimmer_PlaybackError().replace("{0}", &e);
                effects.push(Self::error_box(&self.status_text));
            }
            None => {}
        }
        if let Some(rx) = &self.thumbnails_rx
            && let Ok(thumbs) = rx.try_recv()
        {
            self.thumbnails = thumbs.into_iter().map(|(p, img)| (p, Picture::new(img))).collect();
            self.thumbnails_generation += 1;
            self.thumbnails_rx = None;
            self.loading = false;
            self.status_text.clear();
        }
        if self.thumbnails_rx.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
        // `UpdatePlaybackPosition`: playback stops at the end of the selection.
        if self.has_video() && self.player.is_playing() {
            let p = self.player.position();
            if p >= self.end {
                self.player.pause();
                self.position = self.end;
                self.player.seek(self.end);
            } else {
                self.position = p;
            }
        } else if self.has_video() && !self.player.is_playing() && self.player.position() != self.position && self.export.is_none() {
            // The player paused itself (at the end of the video).
            self.position = self.player.position().min(self.duration);
        }
        if let Some(e) = &self.export {
            self.progress = f64::from_bits(e.progress.load(Ordering::Relaxed));
            if let Ok(r) = e.rx.try_recv() {
                let e = self.export.take().expect("export");
                match r {
                    Ok(()) => {
                        self.progress = 100.0;
                        self.output_file_path = e.output.clone();
                        self.status_text = s::VideoTrimmer_Saved().replace("{0}", &e.output);
                        super::action_sound(&self.task);
                    }
                    Err(err) if err == sharex_tools::video::trimmer::CANCELLED || e.cancel.load(Ordering::Relaxed) => {
                        self.progress = 0.0;
                        self.status_text = s::VideoTrimmer_Cancelled().into();
                    }
                    Err(err) => {
                        self.progress = 0.0;
                        self.status_text = err.clone();
                        effects.push(Self::error_box(&err));
                    }
                }
            } else {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }
        effects
    }

    fn time_box(ui: &mut Ui, rect: Rect, id: Id, shown: &str, enabled: bool) -> Option<String> {
        let buf_id = id.with("buffer");
        let focused = ui.ctx().memory(|m| m.has_focus(id));
        let mut text = if focused { ui.ctx().data(|d| d.get_temp::<String>(buf_id)).unwrap_or_else(|| shown.to_string()) } else { shown.to_string() };
        let r = TextBox { centered: true, enabled, ..TextBox::default() }.show(ui, rect, id, &mut text);
        if r.has_focus() {
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, text.clone()));
        }
        if r.lost_focus() {
            ui.ctx().data_mut(|d| d.remove::<String>(buf_id));
            return Some(text);
        }
        None
    }
}

impl Dialog for VideoTrimmerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::VideoTrimmer_Title()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let mut effects = self.poll(ui.ctx());
        let p = palette(ui.ctx());
        let c = tools::background(ui);
        let nf = font(13.0, 400);
        let can_edit = self.can_edit();
        let has_video = self.has_video();

        // Row 0: the video file.
        let row0 = Rect::from_min_size(c.min, vec2(c.width(), 26.0 + tools::BUTTON_HEIGHT));
        tools::panel(ui, row0);
        let q = row0.shrink(13.0);
        let heading = s::VideoThumbnailerWindow_Video();
        let hw = text_width(ui, heading, tools::heading_font());
        tools::heading(ui, Rect::from_min_size(q.min, vec2(hw, q.height())), heading);
        let open = s::VideoTrimmer_Open();
        let ow = tools::icon_text_button_width(ui, open);
        let orect = Rect::from_min_size(pos2(q.max.x - ow, q.min.y), vec2(ow, tools::BUTTON_HEIGHT));
        let display = if self.input_file_path.is_empty() { s::VideoTrimmer_ChooseVideo() } else { &self.input_file_path };
        let path_rect = Rect::from_min_max(pos2(q.min.x + hw + 12.0, q.min.y), pos2(orect.min.x - 12.0, q.max.y));
        tools::path_tip(ui, path_rect, Id::new("video-trimmer-path-tip"), &self.input_file_path);
        tools::path_text(ui, path_rect, display);
        if tools::icon_text_button(ui, orect, Id::new("video-trimmer-open"), icon::FolderOpen, open, self.can_browse()).clicked()
            && let Some(f) = rfd::FileDialog::new()
                .set_title(open)
                .add_filter(s::VideoTrimmer_VideoFiles(), &super::VIDEO_PATTERNS)
                .add_filter(s::VideoTrimmer_AllFiles(), &["*"])
                .pick_file()
        {
            self.load_input(&f.to_string_lossy());
        }

        // Row 2: trim mode and Save.
        let row2 = Rect::from_min_max(pos2(c.min.x, c.max.y - 26.0 - tools::BUTTON_HEIGHT), c.max);
        let row1 = Rect::from_min_max(pos2(c.min.x, row0.max.y + 12.0), pos2(c.max.x, row2.min.y - 12.0));
        tools::panel(ui, row2);
        let q = row2.shrink(13.0);
        let label = s::VideoTrimmer_TrimMode();
        let lw = text_width(ui, label, nf.clone());
        tools::row_label(ui, Rect::from_min_size(q.min, vec2(lw, q.height())), label);
        let mut x = q.min.x + lw + 16.0;
        for (k, (text, checked)) in [(s::VideoTrimmer_Lossless(), !self.precise), (s::VideoTrimmer_Precise(), self.precise)].into_iter().enumerate() {
            let rb = RadioButton { text, text_font: nf.clone(), text_color: Some(p.text), padding_left: 8.0, enabled: self.can_browse() };
            let w = 20.0 + 8.0 + text_width(ui, text, nf.clone());
            let hint = if k == 0 { s::VideoTrimmer_CopyHint() } else { s::VideoTrimmer_PreciseHint() };
            tools::tip(ui, Rect::from_min_size(pos2(x, q.min.y), vec2(w, q.height())), Id::new(("video-trimmer-mode-tip", k)), hint, self.can_browse());
            if rb.show(ui, Rect::from_min_size(pos2(x, q.min.y), vec2(w, q.height())), Id::new(("video-trimmer-mode", k)), checked).clicked() {
                self.precise = k == 1;
            }
            x += w + 16.0;
        }
        let exporting = self.is_exporting();
        let primary = if exporting { s::VideoTrimmer_Cancel() } else { s::VideoTrimmer_Export() };
        let folder = Rect::from_min_size(pos2(q.max.x - 40.0, q.min.y), vec2(40.0, tools::BUTTON_HEIGHT));
        let pw = tools::icon_text_button_width(ui, primary);
        let prect = Rect::from_min_size(pos2(folder.min.x - 8.0 - pw, q.min.y), vec2(pw, tools::BUTTON_HEIGHT));
        let picon = if exporting { icon::Close } else { icon::Scissors };
        let mut export_clicked = false;
        if tools::icon_text_button(ui, prect, Id::new("video-trimmer-save"), picon, primary, exporting || self.can_trim()).clicked() {
            if let Some(e) = &self.export {
                // `Cancel`.
                e.cancel.store(true, Ordering::Relaxed);
            } else {
                export_clicked = true;
            }
        }
        tools::tip(ui, folder, Id::new("video-trimmer-show-output-tip"), s::VideoTrimmer_ShowOutput(), !self.output_file_path.is_empty());
        if tools::icon_button(ui, folder, Id::new("video-trimmer-show-output"), icon::FolderOpen, !self.output_file_path.is_empty()).clicked() {
            effects.push(Effect::OpenFolderWithFile(self.output_file_path.clone().into()));
        }

        // Row 1: preview, transport, timeline, progress, times, status.
        tools::panel(ui, row1);
        let q = row1.shrink(13.0);
        let status_h = if self.status_text.is_empty() {
            0.0
        } else {
            super::wrapped_text(ui, pos2(0.0, 0.0), q.width(), &self.status_text, nf.clone(), p.text, false)
        };
        // The progress row is empty unless working (`IsWorking`), the status
        // row unless there's a status; the `RowSpacing` around them stays.
        let bar_h = if self.loading || self.is_exporting() { 4.0 } else { 0.0 };
        let fixed = 4.0 + 40.0 + timeline::HEIGHT + bar_h + 40.0 + status_h + 5.0 * 8.0;
        let preview = Rect::from_min_size(q.min, vec2(q.width(), (q.height() - fixed).max(0.0)));
        border::border(ui.painter(), preview, 4.0, 0.0, Some(Brush::Solid(if has_video { Color32::BLACK } else { p.background_main })), None);
        if has_video {
            let aspect = self.player.aspect() as f32;
            let w = preview.width().min(preview.height() * aspect);
            let video = Rect::from_center_size(preview.center(), vec2(w, w / aspect.max(0.001)));
            if let Some((tex, _)) = self.player.texture(ui.ctx()) {
                ui.painter().image(tex, video, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            if ui.interact(video, Id::new("video-trimmer-video"), Sense::click()).clicked() {
                self.toggle_play();
            }
        } else {
            let text = s::VideoTrimmer_ChooseVideo();
            let (lh, tw) = (line_height(ui, &nf), text_width(ui, text, nf.clone()));
            let widest = tw.max(36.0);
            let x0 = preview.min.x + ((preview.width() - widest) / 2.0).round_ties_even();
            let y0 = preview.min.y + ((preview.height() - (36.0 + 12.0 + lh)) / 2.0).round_ties_even();
            let ir = Rect::from_min_size(pos2(x0 + ((widest - 36.0) / 2.0).round_ties_even(), y0), vec2(36.0, 36.0));
            // The `IconPresenter` keeps Fluent's foreground (base high).
            sharex_ui::icons::paint(ui.painter(), ir, icon::Film, sharex_ui::theme::fluent(ui.ctx()).base_high);
            tools::text_at(ui, pos2(x0 + ((widest - tw) / 2.0).round_ties_even(), y0 + 48.0), text, nf.clone(), p.text);
        }
        border::clip_corners(ui.painter(), preview, 4.0, p.background_panel);
        let mut y = preview.max.y + 8.0 + 4.0;

        // The transport and the times (`Spacing="6"`).
        let texts = [format_time(self.position), "/".to_string(), format_time(self.duration)];
        let widths: Vec<f32> = texts.iter().map(|t| text_width(ui, t, nf.clone())).collect();
        let times_w = widths.iter().sum::<f32>() + 12.0;
        // Three 40px buttons 8 apart, 16 to the times.
        let total = 40.0 * 3.0 + 2.0 * 8.0 + 16.0 + times_w;
        let mut x = q.min.x + ((q.width() - total) / 2.0).round_ties_even();
        let play_icon = if self.player.is_playing() { icon::Pause } else { icon::Play };
        let play_text = if self.player.is_playing() { s::AnimatedGifTrimmer_Pause() } else { s::AnimatedGifTrimmer_Play() };
        let transport_tips = [s::AnimatedGifTrimmer_PreviousFrame(), play_text, s::AnimatedGifTrimmer_NextFrame()];
        for (k, key) in [icon::StepBack, play_icon, icon::StepForward].into_iter().enumerate() {
            tools::tip(ui, Rect::from_min_size(pos2(x, y), vec2(40.0, 40.0)), Id::new(("video-trimmer-transport-tip", k)), transport_tips[k], can_edit);
            if tools::icon_button(ui, Rect::from_min_size(pos2(x, y), vec2(40.0, 40.0)), Id::new(("video-trimmer-transport", k)), key, can_edit).clicked() {
                match k {
                    0 => self.player.step_frame(false),
                    1 => self.toggle_play(),
                    _ => self.player.step_frame(true),
                }
            }
            x += 48.0;
        }
        x += 8.0;
        let ty = center_y(Rect::from_min_size(pos2(x, y), vec2(1.0, 40.0)), line_height(ui, &nf));
        for (i, t) in texts.iter().enumerate() {
            tools::text_at(ui, pos2(x, ty), t, nf.clone(), p.text);
            x += widths[i] + 6.0;
        }
        y += 40.0 + 8.0;

        // The timeline.
        let tl_id = Id::new("video-trimmer-timeline");
        let state = TimelineState {
            duration: self.duration,
            start: self.start,
            end: self.end,
            position: self.position,
            thumbnails: &mut self.thumbnails,
            placement: CellPlacement::Position,
            has_media: has_video,
            enabled: can_edit,
        };
        match timeline::show(ui, Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), timeline::HEIGHT)), tl_id, state, &mut self.cells, self.thumbnails_generation) {
            Some(TimelineInput::Start(v)) => self.set_start(v),
            Some(TimelineInput::End(v)) => self.set_end(v),
            Some(TimelineInput::Seek(v)) => self.set_position(v),
            None => {}
        }
        if can_edit {
            for k in timeline::keys(ui, tl_id) {
                match k {
                    TimelineKey::Left { shift } => self.set_position(self.position - if shift { 1.0 } else { 0.1 }),
                    TimelineKey::Right { shift } => self.set_position(self.position + if shift { 1.0 } else { 0.1 }),
                    TimelineKey::Home => self.set_position(self.start),
                    TimelineKey::End => self.set_position(self.end),
                    TimelineKey::I => self.set_start(self.position),
                    TimelineKey::O => self.set_end(self.position),
                }
            }
        }
        y += timeline::HEIGHT + 8.0;
        let bar = Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 4.0));
        if self.loading {
            sharex_ui::controls::indeterminate_progress(ui.painter(), bar, ui.input(|i| i.time));
            ui.ctx().request_repaint();
        } else if exporting {
            sharex_ui::controls::determinate_progress(ui.painter(), bar, self.progress / 100.0);
        }
        y += bar_h + 8.0;

        // Start, End, Duration, Reset.
        let dur_label = s::VideoTrimmer_Duration();
        let dur_text = format_time(self.end - self.start);
        let dur_w = text_width(ui, dur_label, nf.clone()) + 6.0 + text_width(ui, &dur_text, nf.clone());
        let star = ((q.width() - dur_w - 40.0 - 3.0 * 16.0) / 2.0).max(0.0);
        let mut x = q.min.x;
        let shown = [format_time(self.start), format_time(self.end)];
        for (k, (label, key)) in [(s::VideoTrimmer_Start(), icon::ArrowLeftToLine), (s::VideoTrimmer_End(), icon::ArrowRightToLine)].into_iter().enumerate() {
            let lw = text_width(ui, label, nf.clone());
            tools::row_label(ui, Rect::from_min_size(pos2(x, y), vec2(lw, 40.0)), label);
            let br = Rect::from_min_size(pos2(x + star - 40.0, y), vec2(40.0, 40.0));
            let tb = Rect::from_min_max(pos2(x + lw + 8.0, y), pos2(br.min.x - 8.0, y + 40.0));
            if let Some(text) = Self::time_box(ui, tb, Id::new(("video-trimmer-time", k)), &shown[k], can_edit)
                && let Some(v) = try_parse_time(&text)
            {
                if k == 0 {
                    self.set_start(v);
                } else {
                    self.set_end(v);
                }
            }
            tools::tip(ui, br, Id::new(("video-trimmer-set-tip", k)), if k == 0 { s::VideoTrimmer_SetStart() } else { s::VideoTrimmer_SetEnd() }, can_edit);
            if tools::icon_button(ui, br, Id::new(("video-trimmer-set", k)), key, can_edit).clicked() {
                if k == 0 {
                    self.set_start(self.position);
                } else {
                    self.set_end(self.position);
                }
            }
            x += star + 16.0;
        }
        let row = Rect::from_min_size(pos2(x, y), vec2(dur_w, 40.0));
        let lw = text_width(ui, dur_label, nf.clone());
        tools::row_label(ui, Rect::from_min_size(row.min, vec2(lw, 40.0)), dur_label);
        tools::text_at(ui, pos2(x + lw + 6.0, center_y(row, line_height(ui, &nf))), &dur_text, nf.clone(), p.text);
        tools::tip(ui, Rect::from_min_size(pos2(q.max.x - 40.0, y), vec2(40.0, 40.0)), Id::new("video-trimmer-reset-tip"), s::VideoTrimmer_Reset(), can_edit);
        if tools::icon_button(ui, Rect::from_min_size(pos2(q.max.x - 40.0, y), vec2(40.0, 40.0)), Id::new("video-trimmer-reset"), icon::Reset, can_edit).clicked() {
            self.reset();
        }
        y += 40.0 + 8.0;
        // The status (`HasStatus`).
        if status_h > 0.0 {
            super::wrapped_text(ui, pos2(q.min.x, y), q.width(), &self.status_text, nf.clone(), p.text, true);
        }
        if export_clicked {
            effects.push(Effect::TrimmerExport);
        }
        effects
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        if self.can_browse()
            && let Some(f) = files.first()
        {
            self.load_input(&f.to_string_lossy());
        }
        Vec::new()
    }

    fn answered(&mut self, _yes: bool) {
        if std::mem::take(&mut self.error_open) {
            self.loading = false;
        }
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(e) = &self.export {
            e.cancel.store(true, Ordering::Relaxed);
        }
        self.player.unload();
        Vec::new()
    }

    fn trimmer_export(&mut self, ffmpeg: Option<std::path::PathBuf>) {
        self.export(ffmpeg);
    }

    fn task(&self) -> Option<&TaskSettings> {
        Some(&self.task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded(duration: f64) -> VideoTrimmerWindow {
        let mut w = VideoTrimmerWindow::new(TaskSettings::default(), None, None);
        w.duration = duration;
        w.end = duration;
        w
    }

    #[test]
    fn the_selection_is_clamped() {
        let mut w = loaded(8.0);
        assert!(!w.can_trim());
        w.set_start(9.0);
        assert!((w.start - 7.999).abs() < 1e-9);
        assert_eq!(w.position, w.start);
        w.set_end(1.0);
        assert!((w.end - 8.0).abs() < 1e-9, "the end stays after the start");
        w.set_start(2.0);
        w.set_end(5.0);
        assert_eq!((w.start, w.end, w.position), (2.0, 5.0, 5.0));
        assert!(w.can_trim());
        w.reset();
        assert_eq!((w.start, w.end, w.position), (0.0, 8.0, 0.0));
    }

    #[test]
    fn a_missing_file_says_so() {
        let mut w = VideoTrimmerWindow::new(TaskSettings::default(), None, None);
        assert_eq!(w.status_text, s::VideoTrimmer_ChooseVideo());
        w.load_input("no such video.mp4");
        assert!(!w.has_video() && !w.loading);
        assert_eq!(w.status_text, s::VideoTrimmer_PlaybackError().replace("{0}", s::VideoTrimmer_InvalidVideo()));
    }
}
