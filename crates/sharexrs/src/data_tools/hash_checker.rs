//! `ShareX.Tools/Tools/HashChecker` (`HashCheckerWindow.axaml`,
//! `HashCheckerViewModel`) with `TaskHelpers.OpenHashCheck`'s services:
//! one or two files (picked or dropped), the algorithm, Check (the file
//! hashed on a thread by `sharex_tools::hash::hash_file`, with progress
//! and Stop), the result in upper case, the target pasted or the second
//! file's hash, and the green/red line under it when they match or differ.
//! `SizeToContent="Height"`: the second file's row comes and goes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, mpsc};

use egui::{Color32, FontId, Id, Rect, Ui, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::hash::{self, HashAlgorithm};
use sharex_ui::controls::{self, CheckBox, ComboBox, ComboItem, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 700.0;
/// `Grid ColumnDefinitions="100,..."`: the labels' column.
const LABEL_W: f32 = 100.0;
/// The algorithm combo's column (`ColumnDefinitions="100,180,*,Auto"`).
pub const COMBO_W: f32 = 180.0;
/// `Button.browse-button`'s `MinWidth`.
const BROWSE_MIN_W: f32 = 88.0;
/// The match and mismatch lines (`Background="#36B37E"` / `"#E05252"`).
const MATCH: Color32 = Color32::from_rgb(0x36, 0xB3, 0x7E);
const DIFFER: Color32 = Color32::from_rgb(0xE0, 0x52, 0x52);

/// What the hashing thread reports.
enum Done {
    /// The first hash (and the second's when comparing), or `None` when it
    /// couldn't be made (stopped or unreadable).
    First(Option<String>),
    Second(Option<String>),
}

pub struct HashCheckerWindow {
    pub first_file_path: String,
    pub second_file_path: String,
    pub compare_two_files: bool,
    pub algorithm: HashAlgorithm,
    pub result_hash: String,
    pub target_hash: String,
    pub is_working: bool,
    /// `Progress` (0..100), written by the thread as `f32` bits.
    progress: Arc<AtomicU32>,
    /// `StatusText` (not shown by the window, kept as ShareX's is).
    pub status_text: String,
    cancel: Arc<AtomicBool>,
    rx: Option<mpsc::Receiver<Done>>,
    /// The notification sound (`PlayNotificationSoundAsync(ActionCompleted, taskSettings)`).
    play_sound: bool,
    /// `OnOpened` → `StartAutomaticallyAsync` has run.
    started: bool,
    /// The height last given to the window (`SizeToContent`).
    height: f32,
    /// Hash on the drawing thread (`--render`).
    pub sync: bool,
}

impl HashCheckerWindow {
    /// `ToolsIntegration.ShowHashCheckerWindow(…, filePath)`.
    pub fn new(file_path: Option<String>, task: &sharex_settings::TaskSettings) -> Self {
        Self {
            first_file_path: file_path.unwrap_or_default(),
            second_file_path: String::new(),
            compare_two_files: false,
            algorithm: HashAlgorithm::DEFAULT,
            result_hash: String::new(),
            target_hash: String::new(),
            is_working: false,
            progress: Arc::new(AtomicU32::new(0)),
            status_text: String::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            rx: None,
            play_sound: task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction),
            started: false,
            height: 0.0,
            sync: false,
        }
    }

    fn file_exists(path: &str) -> bool {
        !path.is_empty() && std::path::Path::new(path).is_file()
    }

    /// `CanCheck`.
    pub fn can_check(&self) -> bool {
        !self.is_working && Self::file_exists(&self.first_file_path) && (!self.compare_two_files || Self::file_exists(&self.second_file_path))
    }

    /// `ResultLabel`, `TargetLabel`.
    fn labels(&self) -> (&'static str, &'static str) {
        if self.compare_two_files {
            (s::HashCheckerViewModel_First_file_hash(), s::HashCheckerViewModel_Second_file_hash())
        } else {
            (s::HashCheckerViewModel_Result(), s::HashCheckerViewModel_Target_hash())
        }
    }

    pub fn progress(&self) -> f64 {
        f32::from_bits(self.progress.load(Ordering::Relaxed)) as f64
    }

    fn set_progress(&self, v: f64) {
        self.progress.store((v.clamp(0.0, 100.0) as f32).to_bits(), Ordering::Relaxed);
    }

    /// Hashes `path` off the UI thread (`CalculateFileHashAsync`), or at once when `sync`.
    fn start_hash(&mut self, path: String, second: bool, ctx: Option<&egui::Context>) {
        let algorithm = self.algorithm;
        let cancel = self.cancel.clone();
        let progress = self.progress.clone();
        let wrap = move |r: Option<String>| if second { Done::Second(r) } else { Done::First(r) };
        let work = move || {
            let r = hash::hash_file(&path, algorithm, |v| progress.store(v.clamp(0.0, 100.0).to_bits(), Ordering::Relaxed), &cancel);
            wrap(r)
        };
        let (tx, rx) = mpsc::channel();
        if self.sync {
            let _ = tx.send(work());
        } else {
            let ctx = ctx.cloned();
            std::thread::spawn(move || {
                let _ = tx.send(work());
                if let Some(ctx) = ctx {
                    ctx.request_repaint();
                }
            });
        }
        self.rx = Some(rx);
    }

    /// `CheckAsync` (its start; [`Self::poll`] carries on as the hashes come).
    pub fn check(&mut self, ctx: Option<&egui::Context>) {
        if !self.can_check() {
            self.status_text = if self.compare_two_files { s::HashCheckerViewModel_Select_two_existing_files() } else { s::HashCheckerViewModel_Select_existing_file() }.to_owned();
            return;
        }
        self.is_working = true;
        self.set_progress(0.0);
        self.result_hash.clear();
        if self.compare_two_files {
            self.target_hash.clear();
        }
        self.status_text = s::HashCheckerViewModel_Calculating_hash().to_owned();
        self.cancel = Arc::new(AtomicBool::new(false));
        let path = self.first_file_path.clone();
        self.start_hash(path, false, ctx);
    }

    /// A hash under way at `progress` percent (`--render`).
    pub fn set_running(&mut self, progress: f64) {
        self.is_working = true;
        self.started = true;
        self.set_progress(progress);
        self.status_text = s::HashCheckerViewModel_Calculating_hash().to_owned();
    }

    /// `Stop`.
    pub fn stop(&mut self) {
        if self.is_working {
            self.status_text = s::HashCheckerViewModel_Stopping_hash().to_owned();
            self.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// The hashing thread's answers; true when the notification sound is due.
    pub fn poll(&mut self, ctx: Option<&egui::Context>) -> bool {
        let Some(rx) = &self.rx else { return false };
        let Ok(done) = rx.try_recv() else {
            if let Some(ctx) = ctx {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            return false;
        };
        self.rx = None;
        let cancelled = self.cancel.load(Ordering::Relaxed);
        let stopped = || s::HashCheckerViewModel_Hash_check_stopped().to_owned();
        match done {
            Done::First(None) => {
                self.status_text = if cancelled { stopped() } else { s::HashCheckerViewModel_Unable_calculate_file_hash().to_owned() };
                self.is_working = false;
            }
            Done::Second(None) => {
                self.status_text = if cancelled { stopped() } else { s::HashCheckerViewModel_Unable_calculate_second_hash().to_owned() };
                self.is_working = false;
            }
            Done::First(Some(r)) => {
                self.result_hash = hash::display_hash(&r);
                if self.compare_two_files {
                    self.set_progress(0.0);
                    self.status_text = s::HashCheckerViewModel_Calculating_second_hash().to_owned();
                    let path = self.second_file_path.clone();
                    self.start_hash(path, true, ctx);
                    if self.sync {
                        return self.poll(ctx);
                    }
                    return false;
                }
                return self.finish();
            }
            Done::Second(Some(r)) => {
                self.target_hash = hash::display_hash(&r);
                return self.finish();
            }
        }
        false
    }

    fn finish(&mut self) -> bool {
        self.is_working = false;
        if self.cancel.load(Ordering::Relaxed) {
            return false;
        }
        self.set_progress(100.0);
        self.status_text = if self.compare_two_files {
            if self.hashes_match() { s::HashCheckerViewModel_Files_matching_hashes() } else { s::HashCheckerViewModel_Files_different_hashes() }
        } else {
            s::HashCheckerViewModel_Hash_complete()
        }
        .to_owned();
        true
    }

    /// `HashesMatch`.
    pub fn hashes_match(&self) -> bool {
        hash::hashes_match(&self.result_hash, &self.target_hash)
    }

    /// `HashesDiffer`.
    pub fn hashes_differ(&self) -> bool {
        hash::has_comparison(&self.result_hash, &self.target_hash) && !self.hashes_match()
    }

    /// `SetDroppedFile`.
    pub fn set_dropped_file(&mut self, path: String, second: bool) {
        if self.is_working {
            return;
        }
        if second {
            self.second_file_path = path;
        } else {
            self.first_file_path = path;
        }
    }

    /// The top panel's height: the heading/check box row, the file rows.
    fn files_height(&self) -> f32 {
        let rows = if self.compare_two_files { 2.0 } else { 1.0 };
        13.0 + 32.0 + rows * (10.0 + tools::CONTROL_HEIGHT) + 13.0
    }

    fn total_height(&self, ui: &Ui) -> f32 {
        let options = 13.0 + tools::CONTROL_HEIGHT + 13.0;
        let results = 13.0 + tools::heading_height(ui) + 10.0 + tools::CONTROL_HEIGHT + 10.0 + tools::CONTROL_HEIGHT + 13.0;
        tools::MARGIN * 2.0 + self.files_height() + options + results + 12.0 * 2.0
    }

    /// `FontFamily="Consolas"` at the text box's 14px.
    fn hash_font() -> FontId {
        FontId::new(14.0, sharex_ui::fonts::consolas())
    }

    pub fn selected_algorithm(&self) -> Option<usize> {
        HashAlgorithm::ALL.iter().position(|a| *a == self.algorithm)
    }

    /// The algorithm list (`Algorithms`: `TextBlock`s, SemiBold).
    pub fn algorithm_items() -> Vec<ComboItem> {
        HashAlgorithm::ALL.iter().map(|a| ComboItem::text(a.display_name())).collect()
    }

    /// A file row: label, the path box, Browse...; true when Browse was clicked.
    fn file_row(ui: &mut Ui, rect: Rect, k: usize, label: &str, placeholder: &str, path: &mut String, idle: bool) -> bool {
        tools::row_label(ui, Rect::from_min_size(rect.min, vec2(LABEL_W, rect.height())), label);
        let browse = s::HashCheckerWindow_Browse();
        let bw = tools::text_button_width(ui, browse).max(BROWSE_MIN_W);
        let br = Rect::from_min_size(pos2(rect.max.x - bw, rect.min.y), vec2(bw, rect.height()));
        let tb = Rect::from_min_max(pos2(rect.min.x + LABEL_W + 10.0, rect.min.y), pos2(br.min.x - 10.0, rect.max.y));
        TextBox { placeholder, enabled: idle, ..TextBox::default() }.show(ui, tb, Id::new(("hash-file", k)), path);
        tools::text_button(ui, br, Id::new(("hash-browse", k)), browse, idle).clicked()
    }

    /// `SelectFileAsync`: the picker in the current file's folder.
    fn pick(title: &str, current: &str) -> Option<String> {
        let mut d = rfd::FileDialog::new().set_title(title);
        let folder = tools::directory_name(current);
        if !folder.trim().is_empty() && std::path::Path::new(&folder).is_dir() {
            d = d.set_directory(folder);
        }
        d.pick_file().map(|p| p.to_string_lossy().into_owned())
    }
}

impl Dialog for HashCheckerWindow {
    fn spec(&self) -> Spec {
        let mut spec = Spec::fit_height(crate::rs_title(s::HashCheckerWindow_ShareX_Hash_Checker()), WIDTH, WIDTH);
        // `SizeToContent="Height"` keeps `CanResize` (the width).
        spec.resizable = true;
        spec
    }

    fn content_height(&mut self, ui: &mut Ui) -> f32 {
        self.height = self.total_height(ui);
        self.height
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let mut out = Vec::new();
        let ctx = ui.ctx().clone();
        if !self.started {
            // `OnOpened` → `StartAutomaticallyAsync`.
            self.started = true;
            if Self::file_exists(&self.first_file_path) {
                self.check(Some(&ctx));
            }
        }
        if self.poll(Some(&ctx)) && self.play_sound {
            crate::sound::play_action_completed();
        }
        let p = palette(ui.ctx());
        let idle = !self.is_working;
        let c = tools::background(ui);
        let mut y = c.min.y;

        // Files.
        let fh = self.files_height();
        let panel = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), fh));
        tools::panel(ui, panel);
        let inner = panel.shrink(13.0);
        let top = Rect::from_min_size(inner.min, vec2(inner.width(), 32.0));
        tools::heading(ui, top, s::HashCheckerWindow_Files());
        let text = s::HashCheckerWindow_Compare_two_files();
        let cb = CheckBox { text, text_font: font(13.0, 400), text_color: Some(palette(ui.ctx()).text), min_height: 32.0, enabled: idle };
        let cw = cb.width(ui);
        let before = self.compare_two_files;
        cb.show(ui, Rect::from_min_size(pos2(top.max.x - cw, top.min.y), vec2(cw, 32.0)), Id::new("hash-compare"), &mut self.compare_two_files);
        let mut row = Rect::from_min_size(pos2(inner.min.x, top.max.y + 10.0), vec2(inner.width(), tools::CONTROL_HEIGHT));
        if Self::file_row(ui, row, 0, s::HashCheckerWindow_File(), s::HashCheckerWindow_Choose_a_file_or_drop_it_here(), &mut self.first_file_path, idle)
            && let Some(f) = Self::pick(s::HashCheckerViewModel_Select_file_dialog(), &self.first_file_path)
        {
            self.first_file_path = f;
        }
        if self.compare_two_files {
            row = row.translate(vec2(0.0, tools::CONTROL_HEIGHT + 10.0));
            if Self::file_row(ui, row, 1, s::HashCheckerWindow_Second_file(), s::HashCheckerWindow_Choose_a_second_file_or_drop_it_here(), &mut self.second_file_path, idle)
                && let Some(f) = Self::pick(s::HashCheckerViewModel_Select_second_file_dialog(), &self.second_file_path)
            {
                self.second_file_path = f;
            }
        }
        y += fh + 12.0;

        // Hash type, progress, Check/Stop.
        let oh = 13.0 + tools::CONTROL_HEIGHT + 13.0;
        let panel = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), oh));
        tools::panel(ui, panel);
        let row = panel.shrink(13.0);
        tools::row_label(ui, Rect::from_min_size(row.min, vec2(LABEL_W, row.height())), s::HashCheckerWindow_Hash_type());
        let items = Self::algorithm_items();
        let selected = self.selected_algorithm();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: idle };
        // Fluent's `HorizontalAlignment="Left"`: as wide as the selected item
        // in its 180px column.
        let combo_rect = Rect::from_min_size(pos2(row.min.x + LABEL_W + 10.0, row.min.y), vec2(combo.auto_width(ui, &items, selected).min(COMBO_W), row.height()));
        let combo_column_end = row.min.x + LABEL_W + 10.0 + COMBO_W;
        if let Some(i) = combo.show(ui, combo_rect, Id::new("hash-algorithm"), &items, selected) {
            self.algorithm = HashAlgorithm::ALL[i];
        }
        let (button_text, button_icon) = if idle { (s::HashCheckerWindow_Check(), icon::SearchCheck) } else { (s::HashCheckerWindow_Stop(), icon::Stop) };
        let bw = tools::icon_text_button_width(ui, button_text);
        let br = Rect::from_min_size(pos2(row.max.x - bw, row.min.y), vec2(bw, tools::BUTTON_HEIGHT));
        if self.is_working {
            // `ProgressBar Height="6" VerticalAlignment="Center"` in the `*` column.
            let bar = Rect::from_min_max(pos2(combo_column_end + 10.0, center_y(row, 6.0)), pos2(br.min.x - 10.0, center_y(row, 6.0) + 6.0));
            controls::determinate_progress(ui.painter(), bar, self.progress() / 100.0);
        }
        let enabled = if idle { self.can_check() } else { true };
        if tools::icon_text_button(ui, br, Id::new(("hash-check", idle)), button_icon, button_text, enabled).clicked() {
            if idle {
                self.check(Some(&ctx));
                if self.poll(Some(&ctx)) && self.play_sound {
                    crate::sound::play_action_completed();
                }
            } else {
                self.stop();
            }
        }
        y += oh + 12.0;

        // Results.
        let hh = tools::heading_height(ui);
        let rh = 13.0 + hh + 10.0 + tools::CONTROL_HEIGHT * 2.0 + 10.0 + 13.0;
        let panel = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), rh));
        tools::panel(ui, panel);
        let inner = panel.shrink(13.0);
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), s::HashCheckerWindow_Results());
        let (result_label, target_label) = self.labels();
        let r1 = Rect::from_min_size(pos2(inner.min.x, inner.min.y + hh + 10.0), vec2(inner.width(), tools::CONTROL_HEIGHT));
        let r2 = r1.translate(vec2(0.0, tools::CONTROL_HEIGHT + 10.0));
        let boxes = |r: Rect| Rect::from_min_max(pos2(r.min.x + LABEL_W + 10.0, r.min.y), r.max);
        tools::row_label(ui, Rect::from_min_size(r1.min, vec2(LABEL_W, r1.height())), result_label);
        let tb = TextBox { read_only: true, placeholder: s::HashCheckerWindow_Calculated_hash_will_appear_here(), font: Some(Self::hash_font()), ..TextBox::default() };
        tb.show(ui, boxes(r1), Id::new("hash-result"), &mut self.result_hash);
        tools::row_label(ui, Rect::from_min_size(r2.min, vec2(LABEL_W, r2.height())), target_label);
        let tb = TextBox { read_only: self.compare_two_files, placeholder: s::HashCheckerWindow_Paste_a_hash_to_compare(), font: Some(Self::hash_font()), ..TextBox::default() };
        let target_rect = boxes(r2);
        tb.show(ui, target_rect, Id::new("hash-target"), &mut self.target_hash);
        let line = if self.hashes_match() { Some(MATCH) } else if self.hashes_differ() { Some(DIFFER) } else { None };
        if let Some(color) = line {
            // `Height="2" Margin="1,0,1,1" VerticalAlignment="Bottom"`.
            let r = Rect::from_min_max(pos2(target_rect.min.x + 1.0, target_rect.max.y - 3.0), pos2(target_rect.max.x - 1.0, target_rect.max.y - 1.0));
            ui.painter().rect_filled(r, 0.0, color);
        }
        let _ = p;

        if before != self.compare_two_files {
            out.push(Effect::SetHeight(self.total_height(ui)));
        }
        out
    }

    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        // ShareX takes a drop on the second row as the second file; winit
        // gives no drop position, so a drop goes to the second file only
        // when it's the one still missing.
        if let Some(f) = files.first() {
            let second = self.compare_two_files && !self.first_file_path.is_empty() && self.second_file_path.is_empty();
            self.set_dropped_file(f.to_string_lossy().into_owned(), second);
        }
        Vec::new()
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        // `Dispose`: a running hash stops.
        self.cancel.store(true, Ordering::Relaxed);
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, data: &[u8]) -> String {
        let dir = std::env::temp_dir().join(format!("sharexrs-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, data).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn checks_one_file_and_compares_two() {
        let a = temp_file("a.txt", b"hello");
        let b = temp_file("b.txt", b"hello");
        let c = temp_file("c.txt", b"other");
        let mut w = HashCheckerWindow::new(Some(a.clone()), &sharex_settings::TaskSettings::default());
        w.sync = true;
        w.check(None);
        assert!(w.poll(None));
        assert_eq!(w.result_hash, "2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824");
        assert_eq!(w.status_text, s::HashCheckerViewModel_Hash_complete());
        // A pasted target, compared ignoring case and surrounding blanks.
        w.target_hash = " 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824 ".into();
        assert!(w.hashes_match());
        w.compare_two_files = true;
        w.second_file_path = b;
        w.check(None);
        assert!(w.poll(None));
        assert!(w.hashes_match());
        assert_eq!(w.labels().1, s::HashCheckerViewModel_Second_file_hash());
        w.second_file_path = c;
        w.check(None);
        assert!(w.poll(None));
        assert!(w.hashes_differ());
        assert_eq!(w.status_text, s::HashCheckerViewModel_Files_different_hashes());
        w.second_file_path = "missing".into();
        assert!(!w.can_check());
    }
}
