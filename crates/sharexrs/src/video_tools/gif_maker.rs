//! `ShareX.Tools/Tools/AnimatedGifMaker` (`AnimatedGifMakerWindow.axaml`,
//! `AnimatedGifMakerViewModel`): the image list with moves, the frame
//! delay, loop and repeat count, the animated preview (`CreatePreviewFrames`
//! after 100 ms, played by a `DispatcherTimer` at the delay), Create GIF...
//! (save dialog, `AnimatedGifMakerService.Create` off the UI thread), the
//! output panel and the message.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_strings::tools as s;
use sharex_ui::controls::{CheckBox, DecimalUpDown, NumericUpDown};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools::{self as tools, FileList, Job, ListAction, Picture};

pub const WIDTH: f32 = 1120.0;
pub const HEIGHT: f32 = 650.0;
pub const MIN_WIDTH: f32 = 900.0;
pub const MIN_HEIGHT: f32 = 560.0;

type Frames = Result<Vec<Vec<u8>>, String>;

pub struct AnimatedGifMakerWindow {
    pub list: FileList,
    /// `Delay` (a `decimal`).
    pub delay: f64,
    pub looping: bool,
    pub repeat_count: i64,
    pub output_file_path: String,
    pub message: String,
    busy: bool,
    create: Option<mpsc::Receiver<Result<String, String>>>,
    preview: Job<Frames>,
    frames: Vec<Picture>,
    frame_index: usize,
    completed_repeats: i64,
    /// The preview timer's next tick, while it runs.
    next_tick: Option<Instant>,
}

impl Default for AnimatedGifMakerWindow {
    fn default() -> Self {
        Self {
            list: FileList::semibold(),
            delay: 500.0,
            looping: true,
            repeat_count: 0,
            output_file_path: String::new(),
            message: String::new(),
            busy: false,
            create: None,
            preview: Job::new(100),
            frames: Vec::new(),
            frame_index: 0,
            completed_repeats: 0,
            next_tick: None,
        }
    }
}

impl AnimatedGifMakerWindow {
    /// Previews made at once (`--render`).
    pub fn set_sync(&mut self) {
        self.preview.sync = true;
    }

    /// `AddFiles`.
    pub fn add_files(&mut self, files: Vec<String>) {
        if files.is_empty() {
            return;
        }
        for f in files.into_iter().filter(|f| tools::file_exists(f)) {
            self.list.items.push(f);
        }
        if self.list.primary.is_none() && !self.list.items.is_empty() {
            self.list.select(Some(0));
        }
        self.collection_changed();
    }

    fn has_output(&self) -> bool {
        tools::file_exists(&self.output_file_path)
    }

    fn can_create(&self) -> bool {
        !self.busy && self.list.items.len() > 1 && (10.0..=655350.0).contains(&self.delay) && (0..=u16::MAX as i64).contains(&self.repeat_count)
    }

    /// `PreviewInfoText`.
    fn preview_info_text(&self) -> String {
        s::AnimatedGifMakerViewModel_Preview_info().replace("{0}", &self.list.items.len().to_string()).replace("{1}", &(self.delay as i64).to_string())
    }

    /// `NotifyCollectionChanged`: the output and message cleared, the
    /// preview made again.
    fn collection_changed(&mut self) {
        self.output_file_path.clear();
        self.message.clear();
        self.queue_preview();
    }

    /// `NotifyOptionsChanged` + `RestartPreview`.
    fn options_changed(&mut self) {
        self.output_file_path.clear();
        self.message.clear();
        self.restart_preview();
    }

    /// `RefreshPreviewAsync`: fewer than two images clear it at once.
    fn queue_preview(&mut self) {
        if self.list.items.len() < 2 {
            self.preview.cancel();
            self.replace_frames(Vec::new());
            return;
        }
        self.preview.queue();
    }

    fn replace_frames(&mut self, frames: Vec<Picture>) {
        self.frames = frames;
        self.restart_preview();
    }

    /// `RestartPreview`: the first frame, the timer at the delay.
    fn restart_preview(&mut self) {
        self.frame_index = 0;
        self.completed_repeats = 0;
        self.next_tick = (self.frames.len() > 1).then(|| Instant::now() + self.interval());
    }

    fn interval(&self) -> Duration {
        Duration::from_secs_f64(self.delay.clamp(10.0, 655350.0) / 1000.0)
    }

    /// `OnPreviewTimerTick`.
    fn tick(&mut self) {
        if self.frame_index + 1 < self.frames.len() {
            self.frame_index += 1;
            return;
        }
        if !self.looping || self.repeat_count > 0 && self.completed_repeats >= self.repeat_count {
            self.next_tick = None;
            return;
        }
        self.completed_repeats += 1;
        self.frame_index = 0;
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let files = self.list.items.clone();
        if let Some(result) = self.preview.poll(ctx, move || Box::new(move || sharex_tools::video::gif_maker::create_preview_frames(&files))) {
            match result {
                Ok(bytes) => {
                    let frames = bytes.iter().filter_map(|b| Picture::decode(b)).collect();
                    self.replace_frames(frames);
                }
                Err(e) => {
                    eprintln!("AnimatedGifMakerViewModel: Failed to create animated GIF preview. {e}");
                    self.replace_frames(Vec::new());
                    self.message = s::AnimatedGifMakerViewModel_Preview_failed().replace("{0}", &e);
                }
            }
        }
        if let Some(rx) = &self.create
            && let Ok(result) = rx.try_recv()
        {
            self.create = None;
            self.busy = false;
            match result {
                Ok(path) => {
                    self.output_file_path = path;
                    self.message = s::AnimatedGifMakerViewModel_Created().into();
                }
                Err(e) => {
                    eprintln!("AnimatedGifMakerViewModel: Failed to create animated GIF. {e}");
                    self.message = s::AnimatedGifMakerViewModel_Create_failed().replace("{0}", &e);
                }
            }
        }
        if self.create.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if let Some(at) = self.next_tick {
            let now = Instant::now();
            if now >= at {
                self.tick();
                if self.next_tick.is_some() {
                    self.next_tick = Some(now + self.interval());
                }
            }
            if let Some(at) = self.next_tick {
                ctx.request_repaint_after(at.saturating_duration_since(now));
            }
        }
    }

    /// `CreateAsync`: the save dialog (`<first>_animated.gif` in the first
    /// image's folder), then the GIF written on a thread.
    fn create(&mut self) {
        if !self.can_create() {
            return;
        }
        let first = &self.list.items[0];
        let name = format!("{}_animated.gif", sharex_tools::dotnet::file_name_without_extension(first));
        let Some(path) = super::save_file(s::AnimatedGifMakerWindow_Save_GIF_dialog(), &name, &tools::directory_name(first), s::AnimatedGifMakerWindow_GIF_images(), &["gif"]) else {
            return;
        };
        self.start_create(sharex_tools::dotnet::change_extension(&path, ".gif"));
    }

    /// Writes the GIF to `path` (`AnimatedGifMakerService.Create` on a thread).
    pub fn start_create(&mut self, path: String) {
        let files = self.list.items.clone();
        let (delay, looping, repeat) = (self.delay as i32, self.looping, self.repeat_count as i32);
        self.busy = true;
        self.message = s::AnimatedGifMakerViewModel_Creating().into();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let r = sharex_tools::video::gif_maker::create(&path, &files, delay, looping, repeat).map(|_| path);
            let _ = tx.send(r);
        });
        self.create = Some(rx);
    }

    /// Waits for a create started by [`Self::start_create`] (`--render`).
    pub fn finish_create(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.create.take() {
            let r = rx.recv().unwrap_or_else(|_| Err("stopped".into()));
            let (tx, rx) = mpsc::channel();
            let _ = tx.send(r);
            self.create = Some(rx);
            self.poll(ctx);
        }
    }
}

impl Dialog for AnimatedGifMakerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(s::AnimatedGifMakerWindow_Animated_GIF_maker(), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        self.poll(ui.ctx());
        let p = palette(ui.ctx());
        let c = tools::background(ui);
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - tools::BUTTON_HEIGHT), c.max);
        let main = Rect::from_min_max(c.min, pos2(c.max.x, bottom.min.y - 12.0));
        let left = Rect::from_min_size(main.min, vec2(360.0, main.height()));
        let middle = Rect::from_min_size(pos2(left.max.x + 12.0, main.min.y), vec2(320.0, main.height()));
        let right = Rect::from_min_max(pos2(middle.max.x + 12.0, main.min.y), main.max);
        let mut effects = Vec::new();

        // The image list.
        let count = tools::count_text(s::ImageThumbnailerViewModel_One_image(), s::ImageThumbnailerViewModel_Image_count(), self.list.items.len());
        let (action, _) = tools::list_panel(
            ui,
            left,
            "gif-maker",
            &mut self.list,
            s::ImageThumbnailerWindow_Add(),
            &count,
            tools::semibold_font(),
            s::ImageThumbnailerWindow_Drop_images_here_or_use_the_Add_button(),
            tools::semibold_font(),
            true,
            true,
        );
        match action {
            ListAction::Add => {
                let files = tools::pick_images(s::AnimatedGifMakerWindow_Add_images_dialog());
                self.add_files(files);
            }
            ListAction::Remove => {
                self.list.remove_selected();
                // `SelectedImage = Images.FirstOrDefault()`.
                self.list.select((!self.list.items.is_empty()).then_some(0));
                self.collection_changed();
            }
            ListAction::Up => {
                if self.list.move_selected(-1) {
                    self.collection_changed();
                }
            }
            ListAction::Down => {
                if self.list.move_selected(1) {
                    self.collection_changed();
                }
            }
            ListAction::None => {}
        }

        // The animation panel (`Padding="16"`).
        let hh = tools::heading_height(ui);
        let nf = tools::normal_font();
        let sf = tools::semibold_font();
        let inner_w = middle.width() - 34.0;
        let note = s::AnimatedGifMakerWindow_Zero_repeats_forever();
        let note_h = sharex_ui::controls::wrapped(ui, pos2(0.0, 0.0), inner_w, note, sf.clone(), p.text_secondary, false);
        let panel_h = 34.0 + tools::grid_height(&[hh, 40.0, 32.0, 40.0, note_h], 10.0);
        let panel = Rect::from_min_size(middle.min, vec2(middle.width(), panel_h));
        tools::panel(ui, panel);
        let q = panel.shrink(17.0);
        tools::heading(ui, Rect::from_min_size(q.min, vec2(q.width(), hh)), s::AnimatedGifMakerWindow_Animation());
        let mut y = q.min.y + hh + 10.0;
        let before = (self.delay, self.looping, self.repeat_count);
        // `ColumnDefinitions="110,*,Auto"`: the unit after the box.
        let unit = s::NetworkMonitorViewModel_Milliseconds().replace("{0:0}", "").trim().to_string();
        let uw = text_width(ui, &unit, nf.clone());
        let row = Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 40.0));
        tools::row_label(ui, Rect::from_min_size(row.min, vec2(110.0, 40.0)), s::AnimatedGifMakerWindow_Frame_delay());
        let box_rect = Rect::from_min_max(pos2(row.min.x + 120.0, row.min.y), pos2(row.max.x - uw - 10.0, row.max.y));
        DecimalUpDown { minimum: 10.0, maximum: 60000.0, increment: 100.0, decimals: 0, enabled: true }.show(ui, box_rect, Id::new("gif-maker-delay"), &mut self.delay);
        tools::row_label(ui, Rect::from_min_max(pos2(row.max.x - uw, row.min.y), row.max), &unit);
        y += 50.0;
        let cb = CheckBox::new(s::AnimatedGifMakerWindow_Loop_animation());
        cb.show(ui, Rect::from_min_size(pos2(q.min.x, y), vec2(cb.width(ui), 32.0)), Id::new("gif-maker-loop"), &mut self.looping);
        y += 42.0;
        let control = tools::label_row(ui, Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 40.0)), 110.0, s::AnimatedGifMakerWindow_Repeat_count());
        NumericUpDown { minimum: 0, maximum: 65535, enabled: self.looping }.show(ui, control, Id::new("gif-maker-repeat"), &mut self.repeat_count);
        y += 50.0;
        sharex_ui::controls::wrapped(ui, pos2(q.min.x, y), inner_w, note, sf.clone(), p.text_secondary, true);
        if before != (self.delay, self.looping, self.repeat_count) {
            self.options_changed();
        }
        let mut y = panel.max.y + 12.0;

        // The output panel (`HasOutput`).
        if self.has_output() {
            let row_h = line_height(ui, &nf).max(tools::BUTTON_HEIGHT);
            let panel = Rect::from_min_size(pos2(middle.min.x, y), vec2(middle.width(), 26.0 + hh + 8.0 + row_h));
            tools::panel(ui, panel);
            let q = panel.shrink(13.0);
            tools::heading(ui, Rect::from_min_size(q.min, vec2(q.width(), hh)), s::AnimatedGifMakerWindow_Output());
            let row = Rect::from_min_size(pos2(q.min.x, q.min.y + hh + 8.0), vec2(q.width(), row_h));
            let br = Rect::from_min_size(pos2(row.max.x - tools::ICON_BUTTON_WIDTH, center_y(row, tools::BUTTON_HEIGHT)), vec2(tools::ICON_BUTTON_WIDTH, tools::BUTTON_HEIGHT));
            tools::path_text(ui, Rect::from_min_max(row.min, pos2(br.min.x - 8.0, row.max.y)), &self.output_file_path);
            if tools::icon_button(ui, br, Id::new("gif-maker-open-output"), icon::FolderOpen, true).clicked() {
                effects.push(Effect::OpenFolderWithFile(self.output_file_path.clone().into()));
            }
            y = panel.max.y + 12.0;
        }
        // The message panel (`HasMessage`).
        if !self.message.trim().is_empty() {
            tools::message_panel(ui, pos2(middle.min.x, y), middle.width(), &self.message, true);
        }

        // The preview.
        tools::panel(ui, right);
        let inner = right.shrink(1.0);
        let lh = hh.max(line_height(ui, &sf));
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), 10.0 + lh + 10.0 + 1.0));
        let radius = egui::CornerRadius { nw: 3, ne: 3, ..Default::default() };
        ui.painter().rect_filled(header, radius, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_size(pos2(header.min.x, header.max.y - 1.0), vec2(header.width(), 1.0)), 0.0, p.border);
        let hrow = Rect::from_min_max(pos2(header.min.x + 12.0, header.min.y + 10.0), pos2(header.max.x - 12.0, header.max.y - 11.0));
        tools::heading(ui, hrow, s::AnimatedGifMakerWindow_Preview());
        let info = self.preview_info_text();
        let iw = text_width(ui, &info, sf.clone());
        tools::text_at(ui, pos2(hrow.max.x - iw, center_y(hrow, line_height(ui, &sf))), &info, sf.clone(), p.text_secondary);
        let body = Rect::from_min_max(pos2(inner.min.x, header.max.y), inner.max);
        let index = self.frame_index.min(self.frames.len().saturating_sub(1));
        if let Some(pic) = self.frames.get_mut(index) {
            super::draw_image(ui, body.shrink(12.0), pic);
        } else {
            tools::empty_state(ui, body, icon::Film, 54.0, 8.0, &[(s::AnimatedGifMakerWindow_Add_two_images_to_preview(), sf.clone(), p.text_secondary)]);
        }
        if self.preview.busy {
            let t = self.preview.busy_since.elapsed().as_secs_f64();
            sharex_ui::controls::indeterminate_progress(ui.painter(), Rect::from_min_size(body.min, vec2(body.width(), 4.0)), t);
            ui.ctx().request_repaint();
        }

        // Create GIF...
        let text = s::AnimatedGifMakerWindow_Create_GIF();
        let bw = tools::icon_text_button_width(ui, text);
        let br = Rect::from_min_size(pos2(bottom.max.x - bw, bottom.min.y), vec2(bw, tools::BUTTON_HEIGHT));
        if tools::icon_text_button(ui, br, Id::new("gif-maker-create"), icon::Film, text, self.can_create()).clicked() {
            self.create();
        }
        effects
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.add_files(files.into_iter().map(|f| f.to_string_lossy().into_owned()).collect());
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_timer_stops_after_the_repeats() {
        let mut w = AnimatedGifMakerWindow::default();
        w.frames = (0..3).map(|_| Picture::new(image::RgbaImage::new(1, 1))).collect();
        w.repeat_count = 1;
        w.restart_preview();
        let mut shown = vec![w.frame_index];
        while w.next_tick.is_some() {
            w.tick();
            shown.push(w.frame_index);
        }
        // 0 1 2, one repeat 0 1 2, then the timer stops on the last frame.
        assert_eq!(shown, [0, 1, 2, 0, 1, 2, 2]);
        w.looping = false;
        w.restart_preview();
        w.tick();
        w.tick();
        w.tick();
        assert!(w.next_tick.is_none());
    }

    #[test]
    fn create_needs_two_images_and_a_valid_delay() {
        let mut w = AnimatedGifMakerWindow::default();
        assert!(!w.can_create());
        w.list.items = vec!["a.png".into(), "b.png".into()];
        assert!(w.can_create());
        w.delay = 5.0;
        assert!(!w.can_create());
        assert_eq!(s::AnimatedGifMakerViewModel_Preview_info().contains("{1}"), true);
    }
}
