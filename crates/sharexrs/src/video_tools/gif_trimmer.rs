//! `ShareX.Tools/Tools/AnimatedGifTrimmer` (`AnimatedGifTrimmerWindow.axaml`,
//! `AnimatedGifTrimmerViewModel`, `AnimatedGifTrimmerCropOverlay`): the GIF
//! opened off the UI thread with its preview and 12 timeline thumbnails,
//! frame stepping and playback at each frame's delay, the trim selection
//! on the timeline and in the time boxes, the crop box drawn on the preview
//! and the output size, Lossless or Re-encode, and Save trimmed GIF...
//! with its progress and Cancel. The selection's rules are
//! `sharex_tools::video::gif_trimmer::TrimSelection`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use egui::{Color32, CursorIcon, Id, Pos2, Rect, Sense, Ui, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::video::gif_trimmer::{AnimatedGifTrimmerDocument, CropRect, TrimSelection};
use sharex_tools::video::trimmer::format_time;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{DecimalUpDown, RadioButton, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use super::timeline::{self, CellCache, CellPlacement, TimelineInput, TimelineKey, TimelineState};
use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools::{self as tools, Picture};
use crate::message_box::{MessageBox, MessageBoxIcon};

pub const WIDTH: f32 = 900.0;
pub const HEIGHT: f32 = 720.0;
pub const MIN_WIDTH: f32 = 760.0;
pub const MIN_HEIGHT: f32 = 620.0;

type Doc = Arc<AnimatedGifTrimmerDocument>;

enum LoadMsg {
    Opened(Doc),
    Preview(image::RgbaImage),
    Thumbnail(f64, image::RgbaImage),
    Done,
    Failed(String),
}

struct Export {
    rx: mpsc::Receiver<Result<bool, String>>,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
    output: String,
}

/// `AnimatedGifTrimmerCropOverlay.Edges`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
struct Edges(u8);

impl Edges {
    const NONE: Edges = Edges(0);
    const LEFT: u8 = 1;
    const TOP: u8 = 2;
    const RIGHT: u8 = 4;
    const BOTTOM: u8 = 8;
    const MOVE: Edges = Edges(16);
    const NEW: Edges = Edges(32);

    fn has(self, f: u8) -> bool {
        self.0 & f != 0
    }
}

pub struct AnimatedGifTrimmerWindow {
    pub input_file_path: String,
    pub output_file_path: String,
    pub status_text: String,
    doc: Option<Doc>,
    pub sel: Option<TrimSelection>,
    preview: Option<Picture>,
    preview_rx: Option<mpsc::Receiver<(u64, usize, image::RgbaImage)>>,
    preview_version: u64,
    displayed_version: u64,
    thumbnails: Vec<(f64, Picture)>,
    thumbnails_generation: u64,
    cells: CellCache,
    loading: bool,
    load: Option<mpsc::Receiver<LoadMsg>>,
    export: Option<Export>,
    progress: f64,
    playing: bool,
    next_frame: Option<Instant>,
    /// The crop drag: its edges, where it started (GIF pixels) and the crop then.
    crop_drag: Option<(Edges, (f64, f64), CropRect)>,
    task: TaskSettings,
    /// Loads and renders on this thread (`--render`).
    sync: bool,
    /// Shown exporting (`--render`) without writing.
    fake_export: bool,
}

impl AnimatedGifTrimmerWindow {
    /// `new AnimatedGifTrimmerWindow(inputFilePath, playNotificationSound)`.
    pub fn new(task: TaskSettings, input: Option<String>) -> Self {
        let mut w = Self {
            input_file_path: String::new(),
            output_file_path: String::new(),
            status_text: s::AnimatedGifTrimmer_ChooseGif().into(),
            doc: None,
            sel: None,
            preview: None,
            preview_rx: None,
            preview_version: 0,
            displayed_version: 0,
            thumbnails: Vec::new(),
            thumbnails_generation: 0,
            cells: CellCache::default(),
            loading: false,
            load: None,
            export: None,
            progress: 0.0,
            playing: false,
            next_frame: None,
            crop_drag: None,
            task,
            sync: false,
            fake_export: false,
        };
        // `Opened`: the command line's file.
        if let Some(f) = input.filter(|f| !f.is_empty()) {
            w.load_input(&f);
        }
        w
    }

    /// Loads at once, on this thread (`--render`).
    pub fn set_sync(&mut self) {
        self.sync = true;
    }

    fn is_exporting(&self) -> bool {
        self.export.is_some() || self.fake_export
    }

    fn can_browse(&self) -> bool {
        !self.is_exporting()
    }

    fn can_edit(&self) -> bool {
        self.doc.is_some() && !self.loading && !self.is_exporting()
    }

    fn can_play(&self) -> bool {
        self.can_edit() && self.sel.as_ref().is_some_and(|s| s.can_play())
    }

    fn can_trim(&self) -> bool {
        self.can_edit() && self.sel.as_ref().is_some_and(|s| s.can_trim())
    }

    fn is_transformed(&self) -> bool {
        self.sel.as_ref().is_some_and(|s| s.is_transformed())
    }

    /// `LoadInputAsync`: the document, its first frame and the 12
    /// thumbnails made off the UI thread.
    pub fn load_input(&mut self, file: &str) {
        if !self.can_browse() {
            return;
        }
        self.stop_playback();
        self.clear_document();
        self.input_file_path = file.to_string();
        self.output_file_path.clear();
        self.loading = true;
        self.status_text = s::AnimatedGifTrimmer_Loading().into();
        let (tx, rx) = mpsc::channel();
        let path = file.to_string();
        let work = move || {
            let doc = match AnimatedGifTrimmerDocument::open_file(&path) {
                Ok(d) => Arc::new(d),
                Err(e) => {
                    let _ = tx.send(LoadMsg::Failed(e));
                    return;
                }
            };
            let _ = tx.send(LoadMsg::Opened(doc.clone()));
            let _ = tx.send(LoadMsg::Preview(to_rgba(&doc.render_frame_bitmap(0, u32::MAX))));
            for i in 0..12 {
                let index = doc.frame_at(doc.duration() * i as f64 / 12.0);
                if tx.send(LoadMsg::Thumbnail(doc.frame_start(index), to_rgba(&doc.render_frame_bitmap(index, 640)))).is_err() {
                    return;
                }
            }
            let _ = tx.send(LoadMsg::Done);
        };
        if self.sync {
            work();
        } else {
            std::thread::spawn(work);
        }
        self.load = Some(rx);
    }

    fn clear_document(&mut self) {
        self.preview = None;
        self.thumbnails.clear();
        self.thumbnails_generation += 1;
        self.cells.clear();
        self.doc = None;
        self.sel = None;
        self.preview_rx = None;
        self.load = None;
    }

    fn stop_playback(&mut self) {
        self.playing = false;
        self.next_frame = None;
    }

    /// `NotifySelectionChanged`'s status.
    fn selection_changed(&mut self) {
        if let (Some(doc), Some(sel)) = (&self.doc, &self.sel)
            && !self.loading
            && !self.is_exporting()
        {
            self.status_text = sel.status_text(doc);
        }
    }

    /// `SetPositionIndex` → `RefreshPreviewAsync`.
    fn set_position(&mut self, index: usize) {
        let Some(sel) = &mut self.sel else { return };
        if sel.position_index == index {
            return;
        }
        sel.position_index = index;
        self.refresh_preview();
    }

    fn refresh_preview(&mut self) {
        let (Some(doc), Some(sel)) = (&self.doc, &self.sel) else { return };
        self.preview_version += 1;
        let (doc, index, version) = (doc.clone(), sel.position_index, self.preview_version);
        let (tx, rx) = mpsc::channel();
        let work = move || {
            let _ = tx.send((version, index, to_rgba(&doc.render_frame_bitmap(index, u32::MAX))));
        };
        if self.sync {
            work();
        } else {
            std::thread::spawn(work);
        }
        self.preview_rx = Some(rx);
    }

    /// Applies a change to the selection, then what the view model notifies.
    fn edit(&mut self, f: impl FnOnce(&mut TrimSelection, &AnimatedGifTrimmerDocument)) {
        let (Some(doc), Some(sel)) = (self.doc.clone(), &mut self.sel) else { return };
        let before = sel.clone();
        f(sel, &doc);
        let after = sel.clone();
        if (before.start_index, before.end_index) != (after.start_index, after.end_index) {
            self.stop_playback();
        }
        if before.position_index != after.position_index {
            self.refresh_preview();
        }
        self.selection_changed();
    }

    fn step(&mut self, frames: i32) {
        if self.can_edit() {
            self.stop_playback();
            self.edit(|s, _| s.step_position(frames));
        }
    }

    /// `SetStartHere`.
    fn set_start_here(&mut self) {
        if self.can_edit() {
            self.edit(|s, _| set_start_index(s, s.position_index.min(s.end_index - 1)));
        }
    }

    /// `SetEndHere`.
    fn set_end_here(&mut self) {
        if self.can_edit() {
            self.edit(|s, _| set_end_index(s, (s.position_index + 1).max(s.start_index + 1)));
        }
    }

    /// `Reset`: the whole GIF, at its first frame.
    fn reset(&mut self) {
        if self.can_edit() {
            self.edit(|s, _| {
                set_start_index(s, 0);
                set_end_index(s, s.frame_count);
                s.position_index = 0;
            });
        }
    }

    /// `TogglePlayAsync`.
    fn toggle_play(&mut self) {
        if !self.can_play() {
            return;
        }
        if self.playing {
            self.stop_playback();
            return;
        }
        let sel = self.sel.as_ref().expect("selection");
        if sel.position_index < sel.start_index || sel.position_index + 1 >= sel.end_index {
            let start = sel.start_index;
            self.set_position(start);
        }
        self.playing = true;
        self.schedule_next_frame();
    }

    fn schedule_next_frame(&mut self) {
        let (Some(doc), Some(sel)) = (&self.doc, &self.sel) else { return };
        let i = sel.position_index;
        let wait = (doc.frame_end(i) - doc.frame_start(i)).max(0.0);
        self.next_frame = Some(Instant::now() + Duration::from_secs_f64(wait));
    }

    /// `OnPlayTimerTick`.
    fn play_tick(&mut self) {
        self.next_frame = None;
        let Some(sel) = &self.sel else { return };
        if !self.playing {
            return;
        }
        if sel.position_index + 1 >= sel.end_index {
            self.stop_playback();
            return;
        }
        let next = sel.position_index + 1;
        self.set_position(next);
        if self.playing {
            self.schedule_next_frame();
        }
    }

    /// `ExportAsync`.
    fn export(&mut self) -> Vec<Effect> {
        if !self.can_trim() {
            return Vec::new();
        }
        let (Some(doc), Some(sel)) = (self.doc.clone(), self.sel.clone()) else { return Vec::new() };
        if !sel.reencode && !doc.can_copy_selection(sel.start_index, sel.end_index) {
            return vec![self.error_box(s::AnimatedGifTrimmer_UseReencode())];
        }
        let folder = tools::directory_name(&self.input_file_path);
        let Some(output) = super::save_file(s::AnimatedGifTrimmer_Export(), &TrimSelection::output_name(&self.input_file_path), &folder, s::AnimatedGifTrimmer_GifFiles(), &["gif"])
        else {
            return Vec::new();
        };
        self.start_export(output);
        Vec::new()
    }

    /// Saves the selection to `output` on a thread.
    pub fn start_export(&mut self, output: String) {
        let (Some(doc), Some(sel)) = (self.doc.clone(), self.sel.clone()) else { return };
        self.stop_playback();
        self.progress = 0.0;
        self.status_text = s::AnimatedGifTrimmer_Exporting().into();
        let progress = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let (p, c, input, out) = (progress.clone(), cancel.clone(), self.input_file_path.clone(), output.clone());
        let work = move || {
            let mut report = |v: f64| p.store(v.clamp(0.0, 100.0).to_bits(), Ordering::Relaxed);
            let _ = tx.send(sel.export_cancellable(&doc, &input, &out, Some(&mut report), &c));
        };
        if self.sync {
            work();
        } else {
            std::thread::spawn(work);
        }
        self.export = Some(Export { rx, progress, cancel, output });
    }

    /// Shows the exporting state at `progress` percent (`--render exporting`).
    pub fn set_exporting(&mut self, progress: f64) {
        self.fake_export = true;
        self.progress = progress;
        self.status_text = s::AnimatedGifTrimmer_Exporting().into();
    }

    /// The manifest's states (`--render gif-trimmer`).
    pub fn render_state(&mut self, state: &str, frame: Option<usize>, count: f64, text: Option<&str>) -> Result<(), String> {
        let ctx = egui::Context::default();
        match state {
            "" | "empty" | "loaded" | "invalid" | "timeline-drag" => {}
            "trimmed" => self.edit(|s, _| {
                set_start_index(s, 1.min(s.frame_count - 1));
                let end = 5.min(s.frame_count).max(s.start_index + 1);
                set_end_index(s, end);
            }),
            "crop" => self.edit(|s, _| {
                let (w, h) = (s.width, s.height);
                s.set_crop(CropRect { x: w / 4, y: h / 4, width: w / 2, height: h / 2 });
            }),
            "resize-reencode" => self.edit(|s, _| s.set_output_width((s.width / 2) as f64)),
            "exporting" => {
                self.edit(|s, _| set_start_index(s, 1));
                self.set_exporting(count);
            }
            "saved" => {
                self.edit(|s, _| set_start_index(s, 1));
                self.start_export(text.ok_or("--text <output.gif> is required")?.to_string());
                self.poll(&ctx);
            }
            other => return Err(format!("no gif-trimmer state {other}")),
        }
        if let Some(i) = frame {
            self.edit(|s, _| s.position_index = i.min(s.frame_count.saturating_sub(1)));
        }
        self.poll(&ctx);
        Ok(())
    }

    fn error_box(&self, text: &str) -> Effect {
        Effect::MessageBox(Box::new(MessageBox::ok(s::AnimatedGifTrimmer_Title(), text, MessageBoxIcon::Error)))
    }

    /// Takes in what the threads finished.
    pub fn poll(&mut self, ctx: &egui::Context) -> Vec<Effect> {
        let mut effects = Vec::new();
        while let Some(rx) = &self.load {
            let Ok(msg) = rx.try_recv() else { break };
            match msg {
                LoadMsg::Opened(doc) => {
                    self.sel = Some(TrimSelection::new(&doc));
                    self.doc = Some(doc);
                    self.selection_changed();
                }
                LoadMsg::Preview(img) => self.preview = Some(Picture::new(img)),
                LoadMsg::Thumbnail(pos, img) => {
                    self.thumbnails.push((pos, Picture::new(img)));
                    self.thumbnails_generation += 1;
                }
                LoadMsg::Done => {
                    self.status_text.clear();
                    self.loading = false;
                    self.load = None;
                }
                LoadMsg::Failed(e) => {
                    self.clear_document();
                    self.loading = false;
                    self.status_text = e.clone();
                    effects.push(self.error_box(&e));
                }
            }
        }
        if let Some(rx) = &self.preview_rx
            && let Ok((version, _index, img)) = rx.try_recv()
            && version >= self.displayed_version
        {
            self.preview = Some(Picture::new(img));
            self.displayed_version = version;
            self.preview_rx = None;
        }
        if let Some(e) = &self.export {
            self.progress = self.progress.max(f64::from_bits(e.progress.load(Ordering::Relaxed)));
            if let Ok(result) = e.rx.try_recv() {
                let e = self.export.take().expect("export");
                let cancelled = e.cancel.load(Ordering::Relaxed);
                match result {
                    _ if cancelled => self.status_text = s::VideoTrimmer_Cancelled().into(),
                    Ok(true) => {
                        self.output_file_path = e.output.clone();
                        if let Some(sel) = &mut self.sel {
                            sel.reset_crop();
                        }
                        self.progress = 100.0;
                        self.status_text = s::VideoTrimmer_Saved().replace("{0}", &e.output);
                        super::action_sound(&self.task);
                    }
                    Ok(false) => {
                        self.progress = 0.0;
                        self.status_text = s::AnimatedGifTrimmer_UseReencode().into();
                        effects.push(self.error_box(s::AnimatedGifTrimmer_UseReencode()));
                    }
                    Err(err) => {
                        self.status_text = err.clone();
                        effects.push(self.error_box(&err));
                    }
                }
            }
        }
        if let Some(at) = self.next_frame {
            let now = Instant::now();
            if now >= at {
                self.play_tick();
            }
        }
        if let Some(at) = self.next_frame {
            ctx.request_repaint_after(at.saturating_duration_since(Instant::now()));
        }
        if self.load.is_some() || self.preview_rx.is_some() || self.export.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        effects
    }

    /// The preview area: the image, the crop box over it and the empty state.
    fn preview_area(&mut self, ui: &Ui, area: Rect) {
        let p = palette(ui.ctx());
        border::border(ui.painter(), area, 4.0, 0.0, Some(Brush::Solid(p.background_main)), None);
        // `StretchDirection="DownOnly"` keeps the image inside the area.
        if let Some(pic) = &mut self.preview {
            super::draw_image(ui, area, pic);
        }
        if self.doc.is_some() {
            self.crop_overlay(ui, area);
        } else {
            // The `StackPanel` (`Spacing="12"`): the icon and the text in the
            // window's foreground.
            let nf = font(13.0, 400);
            let text = s::AnimatedGifTrimmer_ChooseGif();
            let (lh, tw) = (line_height(ui, &nf), text_width(ui, text, nf.clone()));
            let widest = tw.max(36.0);
            let x0 = area.min.x + ((area.width() - widest) / 2.0).round_ties_even();
            let y0 = area.min.y + ((area.height() - (36.0 + 12.0 + lh)) / 2.0).round_ties_even();
            let ir = Rect::from_min_size(pos2(x0 + ((widest - 36.0) / 2.0).round_ties_even(), y0), vec2(36.0, 36.0));
            sharex_ui::icons::paint(ui.painter(), ir, icon::Film, p.text);
            tools::text_at(ui, pos2(x0 + ((widest - tw) / 2.0).round_ties_even(), y0 + 48.0), text, nf, p.text);
        }
        border::clip_corners(ui.painter(), area, 4.0, p.background_panel);
    }

    /// `ImageBounds`: the GIF scaled down to fit, centred (not rounded).
    fn image_bounds(&self, area: Rect) -> (Rect, f64) {
        let Some(sel) = &self.sel else { return (Rect::NOTHING, 1.0) };
        let (sw, sh) = (sel.width as f64, sel.height as f64);
        let scale = 1f64.min((area.width() as f64 / sw).min(area.height() as f64 / sh));
        let (w, h) = (sw * scale, sh * scale);
        let x = area.min.x as f64 + (area.width() as f64 - w) / 2.0;
        let y = area.min.y as f64 + (area.height() as f64 - h) / 2.0;
        (Rect::from_min_size(pos2(x as f32, y as f32), vec2(w as f32, h as f32)), scale)
    }

    fn to_view(&self, area: Rect, c: CropRect) -> Rect {
        let (image, scale) = self.image_bounds(area);
        let s = scale as f32;
        Rect::from_min_size(pos2(image.min.x + c.x as f32 * s, image.min.y + c.y as f32 * s), vec2(c.width as f32 * s, c.height as f32 * s))
    }

    fn to_source(&self, area: Rect, p: Pos2) -> (f64, f64) {
        let (image, scale) = self.image_bounds(area);
        let sel = self.sel.as_ref().expect("selection");
        (((p.x - image.min.x) as f64 / scale).clamp(0.0, sel.width as f64), ((p.y - image.min.y) as f64 / scale).clamp(0.0, sel.height as f64))
    }

    /// `EdgesAt`.
    fn edges_at(&self, area: Rect, point: Pos2) -> Edges {
        let Some(sel) = &self.sel else { return Edges::NONE };
        if !self.can_edit() {
            return Edges::NONE;
        }
        const GRIP: f32 = 8.0;
        let crop = self.to_view(area, sel.crop);
        if !self.image_bounds(area).0.expand(GRIP).contains(point) {
            return Edges::NONE;
        }
        let inside_y = point.y >= crop.min.y - GRIP && point.y <= crop.max.y + GRIP;
        let inside_x = point.x >= crop.min.x - GRIP && point.x <= crop.max.x + GRIP;
        let mut e = 0u8;
        if inside_y && (point.x - crop.min.x).abs() <= GRIP {
            e |= Edges::LEFT;
        } else if inside_y && (point.x - crop.max.x).abs() <= GRIP {
            e |= Edges::RIGHT;
        }
        if inside_x && (point.y - crop.min.y).abs() <= GRIP {
            e |= Edges::TOP;
        } else if inside_x && (point.y - crop.max.y).abs() <= GRIP {
            e |= Edges::BOTTOM;
        }
        if e != 0 {
            return Edges(e);
        }
        if crop.contains(point) { Edges::MOVE } else { Edges::NEW }
    }

    /// `AnimatedGifTrimmerCropOverlay`: the shade outside the crop, its
    /// frame and handles, and the drags that draw, move and resize it.
    fn crop_overlay(&mut self, ui: &Ui, area: Rect) {
        let p = palette(ui.ctx());
        let id = Id::new("gif-trimmer-crop");
        let (image, _) = self.image_bounds(area);
        let r = ui.interact(image.expand(8.0).intersect(area), id, Sense::click_and_drag());
        let pointer = ui.input(|i| i.pointer.interact_pos());
        if let Some(pos) = r.hover_pos().or(pointer.filter(|_| self.crop_drag.is_some())) {
            let edges = self.crop_drag.map(|d| d.0).unwrap_or_else(|| self.edges_at(area, pos));
            let cursor = match edges.0 {
                3 | 12 => Some(CursorIcon::ResizeNwSe),
                6 | 9 => Some(CursorIcon::ResizeNeSw),
                1 | 4 => Some(CursorIcon::ResizeHorizontal),
                2 | 8 => Some(CursorIcon::ResizeVertical),
                16 => Some(CursorIcon::Move),
                32 => Some(CursorIcon::Crosshair),
                _ => None,
            };
            if let Some(c) = cursor {
                ui.ctx().set_cursor_icon(c);
            }
        }
        if r.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) && self.crop_drag.is_none()
            && let Some(pos) = pointer
        {
            let edges = self.edges_at(area, pos);
            if edges != Edges::NONE
                && let Some(sel) = &self.sel
            {
                self.crop_drag = Some((edges, self.to_source(area, pos), sel.crop));
            }
        }
        if let Some((edges, start, mut crop)) = self.crop_drag {
            if let Some(pos) = pointer
                && let Some(sel) = &self.sel
            {
                let (cx, cy) = self.to_source(area, pos);
                let (x, y) = (cx.round_ties_even() as i32, cy.round_ties_even() as i32);
                if edges == Edges::NEW {
                    let (sx, sy) = (start.0.round_ties_even() as i32, start.1.round_ties_even() as i32);
                    crop = CropRect { x: sx.min(x), y: sy.min(y), width: (sx.max(x) - sx.min(x)), height: (sy.max(y) - sy.min(y)) };
                } else if edges == Edges::MOVE {
                    let dx = ((cx - start.0).round_ties_even() as i32).clamp(-crop.x, sel.width - (crop.x + crop.width));
                    let dy = ((cy - start.1).round_ties_even() as i32).clamp(-crop.y, sel.height - (crop.y + crop.height));
                    crop.x += dx;
                    crop.y += dy;
                } else {
                    let (mut l, mut t, mut rr, mut b) = (crop.x, crop.y, crop.x + crop.width, crop.y + crop.height);
                    if edges.has(Edges::LEFT) {
                        l = x.min(rr - 1);
                    }
                    if edges.has(Edges::RIGHT) {
                        rr = x.max(l + 1);
                    }
                    if edges.has(Edges::TOP) {
                        t = y.min(b - 1);
                    }
                    if edges.has(Edges::BOTTOM) {
                        b = y.max(t + 1);
                    }
                    crop = CropRect { x: l, y: t, width: rr - l, height: b - t };
                }
                if crop.width >= 1 && crop.height >= 1 {
                    self.edit(|s, _| s.set_crop(crop));
                }
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                self.crop_drag = None;
            }
        }

        let Some(sel) = &self.sel else { return };
        let painter = ui.painter().with_clip_rect(area.intersect(ui.clip_rect()));
        let crop = self.to_view(area, sel.crop);
        let shade = Color32::from_rgba_unmultiplied(0, 0, 0, 150);
        painter.rect_filled(Rect::from_min_max(image.min, pos2(image.max.x, crop.min.y)), 0.0, shade);
        painter.rect_filled(Rect::from_min_max(pos2(image.min.x, crop.max.y), image.max), 0.0, shade);
        painter.rect_filled(Rect::from_min_max(pos2(image.min.x, crop.min.y), pos2(crop.min.x, crop.max.y)), 0.0, shade);
        painter.rect_filled(Rect::from_min_max(pos2(crop.max.x, crop.min.y), pos2(image.max.x, crop.max.y)), 0.0, shade);
        let accent = p.accent_start;
        let halo = timeline::halo_color(accent);
        let frame = crop.shrink(1.0);
        border::stroke(&painter, frame, 0.0, 3.0, Brush::Solid(halo));
        border::stroke(&painter, frame, 0.0, 1.5, Brush::accent(ui.ctx()));
        if !self.can_edit() {
            return;
        }
        for y in [frame.min.y, frame.center().y, frame.max.y] {
            for x in [frame.min.x, frame.center().x, frame.max.x] {
                if x == frame.center().x && y == frame.center().y {
                    continue;
                }
                let h = Rect::from_center_size(pos2(x, y), vec2(8.0, 8.0));
                border::paint(
                    &painter,
                    &[
                        border::Part::Border { rect: h, radius: 0.0, thickness: 0.0, background: Some(Brush::accent(ui.ctx())), border: None },
                        border::Part::Stroke { rect: h, radius: 0.0, thickness: 1.0, brush: Brush::Solid(halo) },
                    ],
                    1.0,
                );
            }
        }
    }

    /// A time box (`TextBox` bound one way, applied on Enter or focus loss).
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

/// `SetStartIndex`: the playhead goes to the new start.
fn set_start_index(s: &mut TrimSelection, index: usize) {
    if index != s.start_index {
        s.start_index = index;
        s.position_index = index;
    }
}

/// `SetEndIndex`: the playhead goes to the last kept frame.
fn set_end_index(s: &mut TrimSelection, index: usize) {
    if index != s.end_index {
        s.end_index = index;
        s.position_index = (index - 1).min(s.frame_count - 1);
    }
}

fn to_rgba(b: &sharex_tools::imaging::Bitmap) -> image::RgbaImage {
    image::RgbaImage::from_raw(b.width(), b.height(), b.to_straight()).unwrap_or_default()
}

impl Dialog for AnimatedGifTrimmerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::AnimatedGifTrimmer_Title()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let mut effects = self.poll(ui.ctx());
        let p = palette(ui.ctx());
        let c = tools::background(ui);
        let nf = font(13.0, 400);
        let can_edit = self.can_edit();

        // Row 0: the GIF.
        let row0 = Rect::from_min_size(c.min, vec2(c.width(), 26.0 + tools::BUTTON_HEIGHT));
        tools::panel(ui, row0);
        let q = row0.shrink(13.0);
        let heading = s::AnimatedGifTrimmer_Gif();
        let hw = text_width(ui, heading, tools::heading_font());
        tools::heading(ui, Rect::from_min_size(q.min, vec2(hw, q.height())), heading);
        let open = s::AnimatedGifTrimmer_Open();
        let ow = tools::icon_text_button_width(ui, open);
        let orect = Rect::from_min_size(pos2(q.max.x - ow, q.min.y), vec2(ow, tools::BUTTON_HEIGHT));
        let display = if self.input_file_path.is_empty() { s::AnimatedGifTrimmer_ChooseGif() } else { &self.input_file_path };
        tools::path_text(ui, Rect::from_min_max(pos2(q.min.x + hw + 12.0, q.min.y), pos2(orect.min.x - 12.0, q.max.y)), display);
        if tools::icon_text_button(ui, orect, Id::new("gif-trimmer-open"), icon::FolderOpen, open, self.can_browse()).clicked()
            && let Some(f) = rfd::FileDialog::new()
                .set_title(open)
                .add_filter(s::AnimatedGifTrimmer_GifFiles(), &["gif"])
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
        let reencode = self.sel.as_ref().is_some_and(|s| s.reencode);
        let lossless_enabled = self.can_browse() && !self.is_transformed();
        for (k, (text, checked, enabled)) in
            [(s::AnimatedGifTrimmer_Lossless(), !reencode, lossless_enabled), (s::AnimatedGifTrimmer_Reencode(), reencode, self.can_browse())].into_iter().enumerate()
        {
            let rb = RadioButton { text, text_font: nf.clone(), text_color: Some(p.text), padding_left: 8.0, enabled };
            let w = 20.0 + 8.0 + text_width(ui, text, nf.clone());
            if rb.show(ui, Rect::from_min_size(pos2(x, q.min.y), vec2(w, q.height())), Id::new(("gif-trimmer-mode", k)), checked).clicked() {
                let want = k == 1;
                if let Some(sel) = &mut self.sel {
                    sel.reencode = want;
                }
                self.selection_changed();
            }
            x += w + 16.0;
        }
        let exporting = self.is_exporting();
        let primary = if exporting { s::VideoTrimmer_Cancel() } else { s::AnimatedGifTrimmer_Export() };
        let folder = Rect::from_min_size(pos2(q.max.x - 40.0, q.min.y), vec2(40.0, tools::BUTTON_HEIGHT));
        let pw = tools::icon_text_button_width(ui, primary);
        let prect = Rect::from_min_size(pos2(folder.min.x - 8.0 - pw, q.min.y), vec2(pw, tools::BUTTON_HEIGHT));
        let picon = if exporting { icon::Close } else { icon::Scissors };
        if tools::icon_text_button(ui, prect, Id::new("gif-trimmer-save"), picon, primary, exporting || self.can_trim()).clicked() {
            if let Some(e) = &self.export {
                e.cancel.store(true, Ordering::Relaxed);
            } else {
                effects.extend(self.export());
            }
        }
        if tools::icon_button(ui, folder, Id::new("gif-trimmer-show-output"), icon::FolderOpen, !self.output_file_path.is_empty()).clicked() {
            effects.push(Effect::OpenFolderWithFile(self.output_file_path.clone().into()));
        }

        // Row 1: preview, transport, timeline, progress, times, size.
        tools::panel(ui, row1);
        let q = row1.shrink(13.0);
        let fixed = 4.0 + 40.0 + timeline::HEIGHT + 4.0 + 40.0 + 40.0 + 5.0 * 8.0;
        let preview = Rect::from_min_size(q.min, vec2(q.width(), (q.height() - fixed).max(0.0)));
        self.preview_area(ui, preview);
        let mut y = preview.max.y + 8.0 + 4.0;
        // The transport: three 40px buttons, then the times.
        let duration = self.doc.as_ref().map_or(0.0, |d| d.duration());
        let position = match (&self.doc, &self.sel) {
            (Some(d), Some(s)) => d.frame_start(s.position_index),
            _ => 0.0,
        };
        let texts = [format_time(position), "/".to_string(), format_time(duration), self.sel.as_ref().map(|s| s.frame_text()).unwrap_or_default()];
        let widths: Vec<f32> = texts.iter().map(|t| text_width(ui, t, nf.clone())).collect();
        let shown: Vec<usize> = (0..4).filter(|&i| !texts[i].is_empty()).collect();
        let times_w = shown.iter().map(|&i| widths[i]).sum::<f32>() + 8.0 * shown.len().saturating_sub(1) as f32;
        let total = 40.0 * 3.0 + 16.0 + times_w;
        let mut x = q.min.x + ((q.width() - total) / 2.0).round_ties_even();
        let can_play = self.can_play();
        let play_icon = if self.playing { icon::Pause } else { icon::Play };
        for (k, (key, enabled)) in [(icon::StepBack, can_edit), (play_icon, can_play), (icon::StepForward, can_edit)].into_iter().enumerate() {
            if tools::icon_button(ui, Rect::from_min_size(pos2(x, y), vec2(40.0, 40.0)), Id::new(("gif-trimmer-transport", k)), key, enabled).clicked() {
                match k {
                    0 => self.step(-1),
                    1 => self.toggle_play(),
                    _ => self.step(1),
                }
            }
            x += 48.0;
        }
        x += 16.0 - 8.0;
        let row = Rect::from_min_size(pos2(x, y), vec2(times_w, 40.0));
        let ty = center_y(row, line_height(ui, &nf));
        for &i in &shown {
            tools::text_at(ui, pos2(x, ty), &texts[i], nf.clone(), p.text);
            x += widths[i] + 8.0;
        }
        y += 40.0 + 8.0;

        // The timeline.
        let tl_rect = Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), timeline::HEIGHT));
        let tl_id = Id::new("gif-trimmer-timeline");
        let (start, end) = match (&self.doc, &self.sel) {
            (Some(d), Some(s)) => (d.frame_start(s.start_index), if s.end_index >= d.frame_count() { d.duration() } else { d.frame_start(s.end_index) }),
            _ => (0.0, 0.0),
        };
        let state = TimelineState {
            duration,
            start,
            end,
            position,
            thumbnails: &mut self.thumbnails,
            placement: CellPlacement::Index,
            has_media: self.doc.is_some(),
            enabled: can_edit,
        };
        let input = timeline::show(ui, tl_rect, tl_id, state, &mut self.cells, self.thumbnails_generation);
        match input {
            Some(TimelineInput::Start(v)) => self.edit(|s, d| s.set_start(d, v)),
            Some(TimelineInput::End(v)) => self.edit(|s, d| s.set_end(d, v)),
            Some(TimelineInput::Seek(v)) => {
                self.stop_playback();
                self.edit(|s, d| s.seek(d, v));
            }
            None => {}
        }
        if can_edit {
            for k in timeline::keys(ui, tl_id) {
                match k {
                    TimelineKey::Left { shift } => self.step(if shift { -10 } else { -1 }),
                    TimelineKey::Right { shift } => self.step(if shift { 10 } else { 1 }),
                    TimelineKey::Home => {
                        self.stop_playback();
                        self.edit(|s, d| s.seek(d, start));
                    }
                    TimelineKey::End => {
                        self.stop_playback();
                        self.edit(|s, d| s.seek(d, end));
                    }
                    TimelineKey::I => self.set_start_here(),
                    TimelineKey::O => self.set_end_here(),
                }
            }
        }
        y += timeline::HEIGHT + 8.0;
        // The progress bar (`IsWorking`; indeterminate while loading).
        let bar = Rect::from_min_size(pos2(q.min.x, y), vec2(q.width(), 4.0));
        if self.loading {
            sharex_ui::controls::indeterminate_progress(ui.painter(), bar, ui.input(|i| i.time));
            ui.ctx().request_repaint();
        } else if self.is_exporting() {
            sharex_ui::controls::determinate_progress(ui.painter(), bar, self.progress / 100.0);
        }
        y += 4.0 + 8.0;

        // Start, End, Duration, Reset (`ColumnDefinitions="*,*,Auto,Auto"`).
        let dur_label = s::VideoTrimmer_Duration();
        let dur_text = format_time(end - start);
        let dur_w = text_width(ui, dur_label, nf.clone()) + 6.0 + text_width(ui, &dur_text, nf.clone());
        let star = ((q.width() - dur_w - 40.0 - 3.0 * 16.0) / 2.0).max(0.0);
        let mut x = q.min.x;
        let start_text = format_time(start);
        let end_text = format_time(end);
        for (k, (label, shown, key)) in
            [(s::VideoTrimmer_Start(), &start_text, icon::ArrowLeftToLine), (s::VideoTrimmer_End(), &end_text, icon::ArrowRightToLine)].into_iter().enumerate()
        {
            let lw = text_width(ui, label, nf.clone());
            tools::row_label(ui, Rect::from_min_size(pos2(x, y), vec2(lw, 40.0)), label);
            let br = Rect::from_min_size(pos2(x + star - 40.0, y), vec2(40.0, 40.0));
            let tb = Rect::from_min_max(pos2(x + lw + 8.0, y), pos2(br.min.x - 8.0, y + 40.0));
            if let Some(text) = Self::time_box(ui, tb, Id::new(("gif-trimmer-time", k)), shown, can_edit) {
                if k == 0 {
                    self.edit(|s, d| s.set_start_time(d, &text));
                } else {
                    self.edit(|s, d| s.set_end_time(d, &text));
                }
            }
            if tools::icon_button(ui, br, Id::new(("gif-trimmer-set", k)), key, can_edit).clicked() {
                if k == 0 {
                    self.set_start_here();
                } else {
                    self.set_end_here();
                }
            }
            x += star + 16.0;
        }
        let row = Rect::from_min_size(pos2(x, y), vec2(dur_w, 40.0));
        let lw = text_width(ui, dur_label, nf.clone());
        tools::row_label(ui, Rect::from_min_size(row.min, vec2(lw, 40.0)), dur_label);
        tools::text_at(ui, pos2(x + lw + 6.0, center_y(row, line_height(ui, &nf))), &dur_text, nf.clone(), p.text);
        let br = Rect::from_min_size(pos2(q.max.x - 40.0, y), vec2(40.0, 40.0));
        if tools::icon_button(ui, br, Id::new("gif-trimmer-reset"), icon::Reset, can_edit).clicked() {
            self.reset();
        }
        y += 40.0 + 8.0;

        // Output size (`ColumnDefinitions="Auto,120,Auto,120,Auto,*"`).
        let label = s::AnimatedGifTrimmer_Size();
        let lw = text_width(ui, label, nf.clone());
        tools::row_label(ui, Rect::from_min_size(pos2(q.min.x, y), vec2(lw, 40.0)), label);
        let mut x = q.min.x + lw + 8.0;
        let size = self.sel.as_ref().map(|s| s.output_size());
        for k in 0..2 {
            let r = Rect::from_min_size(pos2(x, y), vec2(120.0, 40.0));
            let mut v = size.map_or(0.0, |s| if k == 0 { s.0 } else { s.1 } as f64);
            let spinner = DecimalUpDown { minimum: 1.0, maximum: 65535.0, increment: 1.0, decimals: 0, enabled: can_edit };
            // A null `Value` (no GIF) shows an empty box.
            let format = |v: f64| if size.is_some() { format!("{v:.0}") } else { String::new() };
            if spinner.show_with(ui, r, Id::new(("gif-trimmer-size", k)), &mut v, &format, true) && size.is_some() {
                self.edit(|s, _| if k == 0 { s.set_output_width(v) } else { s.set_output_height(v) });
            }
            x += 120.0 + 8.0;
            if k == 0 {
                let tw = text_width(ui, "×", nf.clone());
                tools::text_at(ui, pos2(x, center_y(r, line_height(ui, &nf))), "×", nf.clone(), p.text);
                x += tw + 8.0;
            }
        }
        let br = Rect::from_min_size(pos2(x, y), vec2(40.0, 40.0));
        if tools::icon_button(ui, br, Id::new("gif-trimmer-reset-crop"), icon::Reset, can_edit && self.is_transformed()).clicked() {
            self.edit(|s, _| s.reset_crop());
        }
        x += 48.0;
        let hint = tools::ellipsize(ui, s::AnimatedGifTrimmer_CropHint(), &nf, q.max.x - x);
        tools::text_at(ui, pos2(x, center_y(br, line_height(ui, &nf))), &hint, nf.clone(), p.text);
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

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        if let Some(e) = &self.export {
            e.cancel.store(true, Ordering::Relaxed);
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `frames`-frame 4x4 GIF made by the GIF maker (frames alternate
    /// black and white).
    fn gif(dir: &std::path::Path, frames: usize) -> Vec<u8> {
        let files: Vec<String> = (0..frames)
            .map(|i| {
                let path = dir.join(format!("f{i}.png"));
                let v = if i % 2 == 0 { 0 } else { 255 };
                image::RgbaImage::from_pixel(4, 4, image::Rgba([v, v, v, 255])).save(&path).unwrap();
                path.to_string_lossy().into_owned()
            })
            .collect();
        sharex_tools::video::gif_maker::create_bytes(&files, 100, true, 0).unwrap()
    }

    fn loaded(frames: usize) -> (AnimatedGifTrimmerWindow, tempfile_dir::Dir) {
        let dir = tempfile_dir::Dir::new();
        let path = dir.path.join("t.gif");
        std::fs::write(&path, gif(&dir.path, frames)).unwrap();
        let mut w = AnimatedGifTrimmerWindow::new(TaskSettings::default(), None);
        w.set_sync();
        w.load_input(&path.to_string_lossy());
        w.poll(&egui::Context::default());
        (w, dir)
    }

    /// A scratch folder removed on drop.
    mod tempfile_dir {
        pub struct Dir {
            pub path: std::path::PathBuf,
        }
        impl Dir {
            pub fn new() -> Self {
                let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
                let path = std::env::temp_dir().join(format!("sharexrs-gif-trimmer-{}-{n}", std::process::id()));
                std::fs::create_dir_all(&path).unwrap();
                Self { path }
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }

    #[test]
    fn loading_fills_the_timeline() {
        let (w, _dir) = loaded(4);
        assert!(w.doc.is_some() && !w.loading);
        assert_eq!(w.thumbnails.len(), 12);
        assert!(w.status_text.is_empty());
        assert_eq!(w.sel.as_ref().unwrap().end_index, 4);
        assert!(!w.can_trim());
    }

    #[test]
    fn set_start_and_end_here() {
        let (mut w, _dir) = loaded(4);
        w.step(1);
        w.set_start_here();
        assert_eq!(w.sel.as_ref().unwrap().start_index, 1);
        w.step(1);
        w.set_end_here();
        let sel = w.sel.as_ref().unwrap();
        assert_eq!((sel.start_index, sel.end_index, sel.position_index), (1, 3, 2));
        assert!(w.can_trim());
        w.reset();
        let sel = w.sel.as_ref().unwrap();
        assert_eq!((sel.start_index, sel.end_index, sel.position_index), (0, 4, 0));
    }

    #[test]
    fn an_invalid_file_is_reported() {
        let dir = tempfile_dir::Dir::new();
        let path = dir.path.join("bad.gif");
        std::fs::write(&path, b"nope").unwrap();
        let mut w = AnimatedGifTrimmerWindow::new(TaskSettings::default(), None);
        w.set_sync();
        w.load_input(&path.to_string_lossy());
        let effects = w.poll(&egui::Context::default());
        assert!(w.doc.is_none());
        // `GifLosslessTrimmer.Parse` runs out of bytes first.
        assert_eq!(w.status_text, "Unable to read beyond the end of the stream.");
        assert!(matches!(effects.as_slice(), [Effect::MessageBox(_)]));
    }

    #[test]
    fn a_crop_forces_reencode_and_export_writes() {
        let (mut w, dir) = loaded(4);
        w.edit(|s, _| s.set_crop(CropRect { x: 1, y: 1, width: 2, height: 2 }));
        assert!(w.is_transformed() && w.sel.as_ref().unwrap().reencode);
        let out = dir.path.join("out.gif").to_string_lossy().into_owned();
        w.start_export(out.clone());
        w.poll(&egui::Context::default());
        assert_eq!(w.output_file_path, out);
        assert!(!w.is_transformed());
        let doc = AnimatedGifTrimmerDocument::open_file(&out).unwrap();
        assert_eq!((doc.width(), doc.height(), doc.frame_count()), (2, 2, 4));
    }
}
