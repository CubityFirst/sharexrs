//! `ShareX/Presentation/AfterCapture`: `AfterCaptureWindow` and
//! `AfterCaptureGoals`. With `ShowAfterCaptureWindow`, a capture (or a
//! finished recording) waits here: the user picks what to leave on the
//! clipboard (a goal, which brings the tasks it needs), extras, the upload
//! destination and the file name, then Continue starts the task.
//! 1040x680 (at least 820x540), topmost.

use std::path::PathBuf;

use egui::{Color32, Id, Rect, Sense, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_settings::{AfterCaptureTasks, AfterUploadTasks, FileDestination, ImageDestination, TaskSettings};
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, CheckBox, ComboBox, ComboItem, RadioButton, TextBox, card, content_font, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, fluent, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_ui::icons;
use sharex_uploaders::{UploadersConfig, factory, validator};

use super::layout;
use super::{Cx, Dialog, Effect, Spec};

/// `AfterCaptureGoal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    None,
    CopyImage,
    CopyFile,
    CopyFilePath,
    CopyFolderPath,
    CopyUrl,
}

/// A task the goal needs (`GetRequiredTasks`): after capture or after upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Required {
    Capture(AfterCaptureTasks),
    Upload(AfterUploadTasks),
}

/// `AfterCaptureGoals.GoalCaptureTasks`.
pub const GOAL_CAPTURE_TASKS: AfterCaptureTasks = AfterCaptureTasks(
    AfterCaptureTasks::CopyImageToClipboard.0
        | AfterCaptureTasks::CopyFileToClipboard.0
        | AfterCaptureTasks::CopyFilePathToClipboard.0
        | AfterCaptureTasks::CopyFolderPathToClipboard.0
        | AfterCaptureTasks::UploadImageToHost.0,
);

/// `GetAvailableGoals`.
pub fn available_goals(has_image: bool, can_upload: bool) -> Vec<Goal> {
    let mut goals = Vec::new();
    if has_image {
        goals.push(Goal::CopyImage);
    }
    goals.extend([Goal::CopyFile, Goal::CopyFilePath, Goal::CopyFolderPath]);
    if can_upload {
        goals.push(Goal::CopyUrl);
    }
    goals
}

/// `InferGoal`.
pub fn infer_goal(capture: AfterCaptureTasks, available: &[Goal]) -> Goal {
    let goal = if capture.contains(AfterCaptureTasks::UploadImageToHost) {
        Goal::CopyUrl
    } else if capture.contains(AfterCaptureTasks::CopyFileToClipboard) {
        Goal::CopyFile
    } else if capture.contains(AfterCaptureTasks::CopyFilePathToClipboard) {
        Goal::CopyFilePath
    } else if capture.contains(AfterCaptureTasks::CopyFolderPathToClipboard) {
        Goal::CopyFolderPath
    } else if capture.contains(AfterCaptureTasks::CopyImageToClipboard) {
        Goal::CopyImage
    } else {
        Goal::None
    };
    if available.contains(&goal) { goal } else { Goal::None }
}

/// `NeedsSavedFile`.
pub fn needs_saved_file(goal: Goal) -> bool {
    matches!(goal, Goal::CopyFile | Goal::CopyFilePath | Goal::CopyFolderPath)
}

/// `GetRequiredTasks`.
pub fn required_tasks(goal: Goal, file_already_saved: bool) -> Vec<Required> {
    let mut tasks = Vec::new();
    if needs_saved_file(goal) && !file_already_saved {
        tasks.push(Required::Capture(AfterCaptureTasks::SaveImageToFile));
    }
    match goal {
        Goal::CopyImage => tasks.push(Required::Capture(AfterCaptureTasks::CopyImageToClipboard)),
        Goal::CopyFile => tasks.push(Required::Capture(AfterCaptureTasks::CopyFileToClipboard)),
        Goal::CopyFilePath => tasks.push(Required::Capture(AfterCaptureTasks::CopyFilePathToClipboard)),
        Goal::CopyFolderPath => tasks.push(Required::Capture(AfterCaptureTasks::CopyFolderPathToClipboard)),
        Goal::CopyUrl => {
            tasks.push(Required::Capture(AfterCaptureTasks::UploadImageToHost));
            tasks.push(Required::Upload(AfterUploadTasks::CopyURLToClipboard));
        }
        Goal::None => {}
    }
    tasks
}

/// `IsExtraAvailable`.
pub fn is_extra_available(task: AfterCaptureTasks, goal: Goal, file_already_saved: bool) -> bool {
    use AfterCaptureTasks as T;
    if GOAL_CAPTURE_TASKS.contains(task) {
        return false;
    }
    let in_memory = [
        T::SaveImageToFile,
        T::SaveImageToFileWithDialog,
        T::SaveThumbnailImageToFile,
        T::BeautifyImage,
        T::AddImageEffects,
        T::AnnotateImage,
        T::OpenInPhotoEditor,
        T::PinToScreen,
        T::SendImageToPrinter,
    ];
    if file_already_saved && in_memory.contains(&task) {
        return false;
    }
    if task == T::SaveImageToFile || task == T::DeleteFile {
        return !needs_saved_file(goal);
    }
    if task == T::ShowBeforeUploadWindow {
        return goal == Goal::CopyUrl;
    }
    true
}

/// `Resolve`: the goal's tasks plus the extras offered for it.
pub fn resolve(goal: Goal, capture_extras: AfterCaptureTasks, upload_extras: AfterUploadTasks, file_already_saved: bool) -> (AfterCaptureTasks, AfterUploadTasks) {
    let mut capture = AfterCaptureTasks::None;
    for &(bit, _) in AfterCaptureTasks::NAMES {
        let task = AfterCaptureTasks(bit as u32);
        if bit != 0 && capture_extras.contains(task) && is_extra_available(task, goal, file_already_saved) {
            capture.insert(task);
        }
    }
    let mut upload = upload_extras;
    for r in required_tasks(goal, file_already_saved) {
        match r {
            Required::Capture(t) => capture.insert(t),
            Required::Upload(t) => upload.insert(t),
        }
    }
    (capture, upload)
}

fn describe(enum_type: &str, flag: u64, names: &[(u64, &'static str)]) -> String {
    let name = names.iter().find(|(b, _)| *b == flag).map_or("", |(_, n)| *n);
    sharex_strings::helpers_lib::lookup(&format!("{enum_type}_{name}")).unwrap_or(name).to_owned()
}

fn goal_texts(goal: Goal) -> (&'static str, &'static str) {
    match goal {
        Goal::CopyImage => (s::AfterCaptureWindow_GoalCopyImage(), s::AfterCaptureWindow_GoalCopyImageDescription()),
        Goal::CopyFile => (s::AfterCaptureWindow_GoalCopyFile(), s::AfterCaptureWindow_GoalCopyFileDescription()),
        Goal::CopyFilePath => (s::AfterCaptureWindow_GoalCopyFilePath(), s::AfterCaptureWindow_GoalCopyFilePathDescription()),
        Goal::CopyFolderPath => (s::AfterCaptureWindow_GoalCopyFolderPath(), s::AfterCaptureWindow_GoalCopyFolderPathDescription()),
        Goal::CopyUrl | Goal::None => (s::AfterCaptureWindow_GoalCopyUrl(), s::AfterCaptureWindow_GoalCopyUrlDescription()),
    }
}

/// An upload destination the combo box offers.
#[derive(Clone, Debug, PartialEq)]
pub enum Destination {
    Image(ImageDestination),
    File(FileDestination),
}

/// `CreateDestinationOptions`: image destinations (not the file uploader)
/// and file destinations whose settings are complete.
pub fn destination_options(task: &TaskSettings, config: &UploadersConfig) -> Vec<(String, Destination, bool)> {
    let custom = |index: i32, fallback: String| {
        let list = &config.CustomUploadersList.0;
        let index = if task.OverrideCustomUploader {
            if (0..list.len() as i32).contains(&task.CustomUploaderIndex) { task.CustomUploaderIndex } else { 0 }
        } else {
            index
        };
        usize::try_from(index).ok().and_then(|i| list.get(i)).map_or(fallback, |u| s::AfterCaptureWindow_CustomUploader().replace("{0}", &u.display_name()))
    };
    let mut out = Vec::new();
    for d in ImageDestination::ALL.iter().filter(|d| **d != ImageDestination::FileUploader && validator::image(d, config)) {
        let mut label = factory::service_name("ImageDestination", d.name());
        if *d == ImageDestination::CustomImageUploader {
            label = custom(config.CustomImageUploaderSelected, label);
        }
        out.push((label, Destination::Image(d.clone()), task.ImageDestination == *d));
    }
    for d in FileDestination::ALL.iter().filter(|d| validator::file(d, config)) {
        let mut label = factory::service_name("FileDestination", d.name());
        if *d == FileDestination::CustomFileUploader {
            label = custom(config.CustomFileUploaderSelected, label);
        }
        let selected = task.ImageDestination == ImageDestination::FileUploader && task.ImageFileDestination == *d;
        out.push((label, Destination::File(d.clone()), selected));
    }
    out
}

/// What the window was opened for.
pub enum Target {
    /// An image task (`UploadManager.RunImageTask`), started on Continue.
    Image(Box<crate::capture_job::ImageTask>),
    /// A finished recording (`ScreenRecordManager.FinishRecording`).
    Recording(PathBuf),
    /// Nothing (`--render`).
    Preview,
}

/// What Continue (or the copy button) hands the app.
pub struct Accepted {
    pub target: Target,
    pub task: TaskSettings,
    /// `Result.FileName`.
    pub file_name: String,
}

pub struct AfterCaptureWindow {
    target: Option<Target>,
    pub task: TaskSettings,
    image: Option<std::sync::Arc<RgbaImage>>,
    file_already_saved: bool,
    /// The file that's already there (a recording), for the empty preview.
    file_path: Option<PathBuf>,
    goals: Vec<Goal>,
    pub goal: Goal,
    /// `AfterCaptureOptions`: (task, checked).
    capture_options: Vec<(AfterCaptureTasks, bool)>,
    upload_options: Vec<(AfterUploadTasks, bool)>,
    destinations: Vec<(String, Destination)>,
    selected_destination: Option<usize>,
    pub file_name: String,
    opened: bool,
    texture: Option<egui::TextureHandle>,
    /// Pick a goal when drawn (`--render`).
    pub select_on_open: bool,
}

/// The body's `Margin="18,14,18,18"`, columns `390,*` 14 apart.
const MARGIN: (f32, f32, f32) = (18.0, 14.0, 18.0);
const LEFT: f32 = 390.0;
const COLUMN_SPACING: f32 = 14.0;
const ROW_SPACING: f32 = 10.0;
/// `Button`'s 40px; `Button.icon-button` 32.
const BUTTON_HEIGHT: f32 = 40.0;
const ICON_BUTTON: f32 = 32.0;
/// The options `StackPanel`: `Margin="8,6"`, `Spacing="12"`.
const STACK_MARGIN: (f32, f32) = (8.0, 6.0);
const STACK_SPACING: f32 = 12.0;
/// A `CheckBox.task-option`'s row.
const CHECK_ROW: f32 = 32.0;

impl AfterCaptureWindow {
    /// `new AfterCaptureWindow(taskSettings, metadata, filePath)`.
    /// `can_upload`: uploads aren't turned off (`SystemOptions` and the
    /// settings' `DisableUpload`).
    pub fn new(target: Target, task: TaskSettings, image: Option<RgbaImage>, file_path: Option<PathBuf>, file_name: String, can_upload: bool, config: &UploadersConfig) -> Self {
        let file_already_saved = file_path.is_some();
        let options = destination_options(&task, config);
        let selected_destination = options.iter().position(|(_, _, sel)| *sel);
        let destinations: Vec<(String, Destination)> = options.into_iter().map(|(l, d, _)| (l, d)).collect();
        let can_upload = can_upload && !destinations.is_empty();
        // A saved image file shows as the preview.
        let source = image.or_else(|| file_path.as_ref().filter(|p| sharex_helpers::file_helpers::is_image_file(&p.to_string_lossy())).and_then(|p| crate::thumbnail::load(p)));
        let has_image = source.is_some() && !file_already_saved;
        let goals = available_goals(has_image, can_upload);
        let goal = infer_goal(task.AfterCaptureJob, &goals);
        let capture_options = AfterCaptureTasks::NAMES
            .iter()
            .map(|&(bit, _)| AfterCaptureTasks(bit as u32))
            .filter(|t| t.0 != 0 && *t != AfterCaptureTasks::ShowQuickTaskMenu && *t != AfterCaptureTasks::ShowAfterCaptureWindow && !GOAL_CAPTURE_TASKS.contains(*t))
            .map(|t| (t, task.AfterCaptureJob.contains(t)))
            .collect();
        let upload_options = AfterUploadTasks::NAMES
            .iter()
            .map(|&(bit, _)| AfterUploadTasks(bit as u32))
            .filter(|t| t.0 != 0 && *t != AfterUploadTasks::CopyURLToClipboard)
            .map(|t| (t, task.AfterUploadJob.contains(t)))
            .collect();
        let file_name = match &file_path {
            Some(p) => p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            None => file_name,
        };
        Self {
            target: Some(target),
            task,
            image: source.map(std::sync::Arc::new),
            file_already_saved,
            file_path,
            goals,
            goal,
            capture_options,
            upload_options,
            destinations,
            selected_destination,
            file_name,
            opened: false,
            texture: None,
            select_on_open: true,
        }
    }

    /// Whether the capture's image is in memory (the copy goal and button).
    pub fn has_image(&self) -> bool {
        self.image.is_some() && !self.file_already_saved
    }

    /// `OnContinueClick`: the goal, the extras and the destination into the
    /// task settings.
    pub fn continue_jobs(&mut self) -> TaskSettings {
        let mut capture_extras = AfterCaptureTasks::None;
        for (t, checked) in &self.capture_options {
            if *checked {
                capture_extras.insert(*t);
            }
        }
        // Copying the URL stays as saved unless the goal asks for it.
        let mut upload_extras = AfterUploadTasks(self.task.AfterUploadJob.0 & AfterUploadTasks::CopyURLToClipboard.0);
        for (t, checked) in &self.upload_options {
            if *checked {
                upload_extras.insert(*t);
            }
        }
        let (capture, upload) = resolve(self.goal, capture_extras, upload_extras, self.file_already_saved);
        let mut task = self.task.clone();
        task.AfterCaptureJob = capture;
        task.AfterUploadJob = upload;
        if self.goal == Goal::CopyUrl
            && let Some((_, d)) = self.selected_destination.and_then(|i| self.destinations.get(i))
        {
            match d {
                Destination::Image(d) => task.ImageDestination = d.clone(),
                Destination::File(d) => {
                    task.ImageDestination = ImageDestination::FileUploader;
                    task.ImageFileDestination = d.clone();
                }
            }
        }
        task
    }

    fn accept(&mut self, task: TaskSettings) -> Vec<Effect> {
        let target = self.target.take().unwrap_or(Target::Preview);
        vec![Effect::AfterCaptureAccepted(Box::new(Accepted { target, task, file_name: self.file_name.clone() })), Effect::Close]
    }

    fn title_font() -> egui::FontId {
        font(13.0, 600)
    }

    fn caption_font() -> egui::FontId {
        font(13.0, 400)
    }

    fn content_width() -> f32 {
        LEFT - 2.0 - 8.0 - 2.0 * STACK_MARGIN.0
    }

    /// A goal's `RadioButton`: label and description, `Padding="7,1"`.
    fn goal_height(ui: &egui::Ui, goal: Goal) -> f32 {
        let (_, desc) = goal_texts(goal);
        let w = Self::content_width() - 4.0 - 20.0 - 7.0;
        let lines = wrap_lines(ui, desc, &Self::caption_font(), w).len().max(1);
        let content = line_height(ui, &Self::title_font()) + 1.0 + line_height(ui, &Self::caption_font()) * lines as f32;
        (content + 2.0).max(32.0)
    }

    /// The options' stack, drawn (when `paint`) from `top`; its height.
    fn options(&mut self, ui: &mut egui::Ui, x: f32, top: f32, paint: bool) -> f32 {
        let p = palette(ui.ctx());
        let f = fluent(ui.ctx());
        let w = Self::content_width();
        let (tf, cf) = (Self::title_font(), Self::caption_font());
        let (th, ch) = (line_height(ui, &tf), line_height(ui, &cf));
        let mut y = top;
        let title = |ui: &egui::Ui, y: f32, text: &str| {
            if paint {
                ui.painter().galley(pos2(x, y), galley(ui, text, tf.clone(), p.accent_text, w), p.accent_text);
            }
        };
        // The goal.
        title(ui, y, s::AfterCaptureWindow_GoalHeading());
        y += th + 2.0;
        for line in wrap_lines(ui, s::AfterCaptureWindow_GoalDescription(), &cf, w) {
            if paint {
                ui.painter().galley(pos2(x, y), galley(ui, &line, cf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            }
            y += ch;
        }
        y += STACK_SPACING;
        for i in 0..self.goals.len() {
            let goal = self.goals[i];
            y += 3.0;
            let h = Self::goal_height(ui, goal);
            if paint {
                let rect = Rect::from_min_size(pos2(x + 2.0, y), vec2(w - 4.0, h));
                let circle_row = Rect::from_min_size(rect.min, vec2(rect.width(), 32.0));
                let rb = RadioButton { text: "", text_font: tf.clone(), text_color: None, padding_left: 7.0, enabled: true };
                let r = rb.show(ui, circle_row, ui.id().with(("goal", i)), self.goal == goal);
                let whole = ui.interact(rect, ui.id().with(("goal-row", i)), Sense::click());
                if r.clicked() || whole.clicked() {
                    // `SelectGoal`.
                    self.goal = goal;
                }
                let (label, desc) = goal_texts(goal);
                let tx = rect.min.x + 20.0 + 7.0;
                let tw = rect.max.x - tx;
                ui.painter().galley(pos2(tx, rect.min.y + 1.0), galley(ui, label, tf.clone(), p.text_secondary, tw), p.text_secondary);
                let mut dy = rect.min.y + 1.0 + th + 1.0;
                for line in wrap_lines(ui, desc, &cf, tw) {
                    ui.painter().galley(pos2(tx, dy), galley(ui, &line, cf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
                    dy += ch;
                }
            }
            y += h + 3.0;
        }
        // A check box row (`CheckBox.task-option`): string content, Inter
        // Normal, `Padding="7,1"`, `Margin="2,0"`.
        let check = |ui: &egui::Ui, y: f32, id: Id, text: &str, checked: &mut bool, enabled: bool| {
            let rect = Rect::from_min_size(pos2(x + 2.0, y), vec2(w - 4.0, CHECK_ROW));
            let cb = CheckBox { text: "", text_font: content_font(13.0, 400), text_color: None, min_height: CHECK_ROW, enabled };
            let r = cb.show(ui, rect, id, checked);
            let over = enabled && (r.hovered() || r.is_pointer_button_down_on());
            let color = if !enabled { f.base_medium_low } else if *checked || over { f.base_high } else { p.text };
            let font = content_font(13.0, 400);
            let ty = center_y(rect, line_height(ui, &font));
            ui.painter().galley(pos2(rect.min.x + 20.0 + 7.0, ty), galley(ui, text, font, color, rect.width() - 27.0), color);
        };
        // What the goal needs.
        let required = required_tasks(self.goal, self.file_already_saved);
        if !required.is_empty() {
            y += STACK_SPACING;
            title(ui, y, s::AfterCaptureWindow_RequiredHeading());
            y += th + 2.0;
            for (i, r) in required.iter().enumerate() {
                if paint {
                    let text = match r {
                        Required::Capture(t) => describe("AfterCaptureTasks", t.0 as u64, AfterCaptureTasks::NAMES),
                        Required::Upload(t) => describe("AfterUploadTasks", t.0 as u64, AfterUploadTasks::NAMES),
                    };
                    check(ui, y, ui.id().with(("required", i)), &text, &mut true, false);
                }
                y += CHECK_ROW;
            }
        }
        // The destination.
        if self.goal == Goal::CopyUrl {
            y += STACK_SPACING;
            title(ui, y, s::AfterCaptureWindow_Destinations());
            y += th + 4.0;
            // ShareX's `ComboBox`: `MinHeight="40"`, `Padding="8,6"`.
            let combo_h = 40.0;
            if paint {
                let items: Vec<ComboItem> = self.destinations.iter().map(|(l, _)| ComboItem::text(l.clone())).collect();
                let rect = Rect::from_min_size(pos2(x, y), vec2(w, combo_h));
                if let Some(i) = (ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true }).show(ui, rect, ui.id().with("destination"), &items, self.selected_destination) {
                    self.selected_destination = Some(i);
                }
            }
            y += combo_h;
        }
        // The extras.
        y += STACK_SPACING;
        title(ui, y, s::AfterCaptureWindow_AlsoHeading());
        y += th + 2.0;
        for i in 0..self.capture_options.len() {
            let (t, mut checked) = self.capture_options[i];
            if !is_extra_available(t, self.goal, self.file_already_saved) {
                continue;
            }
            if paint {
                let text = describe("AfterCaptureTasks", t.0 as u64, AfterCaptureTasks::NAMES);
                check(ui, y, ui.id().with(("also", i)), &text, &mut checked, true);
                self.capture_options[i].1 = checked;
            }
            y += CHECK_ROW;
        }
        if self.goal == Goal::CopyUrl {
            y += STACK_SPACING;
            title(ui, y, s::AfterCaptureWindow_AfterUpload());
            y += th + 2.0;
            for i in 0..self.upload_options.len() {
                let (t, mut checked) = self.upload_options[i];
                if paint {
                    let text = describe("AfterUploadTasks", t.0 as u64, AfterUploadTasks::NAMES);
                    check(ui, y, ui.id().with(("upload", i)), &text, &mut checked, true);
                    self.upload_options[i].1 = checked;
                }
                y += CHECK_ROW;
            }
        }
        y - top
    }
}

impl Dialog for AfterCaptureWindow {
    fn spec(&self) -> Spec {
        Spec { topmost: true, ..Spec::sized(crate::rs_title(s::AfterCaptureWindow_Title()), 1040.0, 680.0, 820.0, 540.0) }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = layout::background(ui);
        let header = layout::header(ui, full, icon::Image, s::AfterCaptureWindow_Heading(), Some(s::AfterCaptureWindow_Subtitle()), false);
        let body = Rect::from_min_max(pos2(full.min.x + MARGIN.0, full.min.y + header + MARGIN.1), pos2(full.max.x - MARGIN.0, full.max.y - MARGIN.2));
        let left = Rect::from_min_size(body.min, vec2(LEFT, body.height()));
        let mut out = Vec::new();

        // The bottom rows: the buttons, the file name.
        let buttons_top = left.max.y - BUTTON_HEIGHT;
        let lf = font(13.0, 600);
        let tb_h = (line_height(ui, &TextBox::font()) + 12.0).max(32.0);
        let name_top = buttons_top - ROW_SPACING - tb_h - 4.0 - line_height(ui, &lf);
        layout::text(ui, pos2(left.min.x, name_top), s::AfterCaptureWindow_FileName(), lf.clone(), p.text_secondary, LEFT);
        let tb_rect = Rect::from_min_size(pos2(left.min.x, name_top + line_height(ui, &lf) + 4.0), vec2(LEFT, tb_h));
        let tb_id = ui.id().with("file-name");
        if !self.opened {
            self.opened = true;
            if self.select_on_open {
                super::text_upload::focus_text_box(ui, tb_id, &self.file_name, true);
            }
        }
        TextBox { padding: Thickness::hv(8.0, 5.0), ..TextBox::default() }.show(ui, tb_rect, tb_id, &mut self.file_name);

        // Copy (an image in memory only), Continue, Cancel.
        let icon_rect = Rect::from_min_size(pos2(left.min.x, center_y(Rect::from_min_size(pos2(left.min.x, buttons_top), vec2(LEFT, BUTTON_HEIGHT)), ICON_BUTTON)), Vec2::splat(ICON_BUTTON));
        let copy_enabled = self.image.is_some() && !self.file_already_saved;
        let b = Button { padding: Thickness::all(0.0), ..Button::default() };
        let r = b.show(ui, icon_rect, ui.id().with("copy"), "", copy_enabled);
        let ic = b.foreground(ui, &r, copy_enabled, p.background_main);
        icons::paint(ui.painter(), Rect::from_center_size(icon_rect.center(), Vec2::splat(15.0)), icon::Copy, ic);
        if r.clicked() {
            // `OnCopyClick`: only the image to the clipboard.
            let mut task = self.task.clone();
            task.AfterCaptureJob = AfterCaptureTasks::CopyImageToClipboard;
            return self.accept(task);
        }
        let (cont, cancel) = (s::AfterCaptureWindow_Continue(), s::AfterCaptureWindow_Cancel());
        let compact = |primary: bool| Button { padding: Thickness::hv(14.0, 5.0), primary, ..Button::default() };
        let cw = compact(true).width(ui, cont).max(96.0);
        let xw = compact(false).width(ui, cancel).max(96.0);
        let cancel_rect = Rect::from_min_size(pos2(left.max.x - xw, buttons_top), vec2(xw, BUTTON_HEIGHT));
        let cont_rect = Rect::from_min_size(pos2(cancel_rect.min.x - 7.0 - cw, buttons_top), vec2(cw, BUTTON_HEIGHT));
        let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        if compact(true).show(ui, cont_rect, ui.id().with("continue"), cont, true).clicked() || enter {
            let task = self.continue_jobs();
            return self.accept(task);
        }
        if compact(false).show(ui, cancel_rect, ui.id().with("cancel"), cancel, true).clicked() || escape {
            out.push(Effect::Close);
            return out;
        }

        // The options card, scrolling.
        let card_rect = Rect::from_min_max(left.min, pos2(left.max.x, name_top - ROW_SPACING));
        card(ui, card_rect, 6.0);
        let view = card_rect.shrink(5.0);
        let content = self.options(ui, 0.0, 0.0, false) + 2.0 * STACK_MARGIN.1;
        let scroll = layout::scroll_offset(ui, ui.id().with("options-scroll"), view, content);
        let clip = ui.clip_rect();
        ui.set_clip_rect(view.intersect(clip));
        self.options(ui, view.min.x + STACK_MARGIN.0, view.min.y + STACK_MARGIN.1 - scroll, true);
        ui.set_clip_rect(clip);
        layout::scroll_bar(ui, view, content, scroll, 0.0);

        // The preview.
        let preview = Rect::from_min_max(pos2(left.max.x + COLUMN_SPACING, body.min.y), body.max);
        card(ui, preview, 6.0);
        let inner = preview.shrink(1.0);
        match &self.image {
            Some(img) => {
                if self.texture.is_none() {
                    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                    self.texture = Some(ui.ctx().load_texture("after-capture-preview", ci, egui::TextureOptions::LINEAR));
                }
                let area = inner.shrink(10.0);
                let (iw, ih) = (img.width() as f32, img.height() as f32);
                let scale = (area.width() / iw).min(area.height() / ih);
                let size = vec2((iw * scale).round(), (ih * scale).round());
                let r = Rect::from_min_size(pos2(area.min.x + ((area.width() - size.x) / 2.0).round_ties_even(), area.min.y + ((area.height() - size.y) / 2.0).round_ties_even()), size);
                if let Some(t) = &self.texture {
                    ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                // The size badge.
                let text = format!("{} × {}", img.width(), img.height());
                let bf = font(12.0, 600);
                let bw = text_width(ui, &text, bf.clone()) + 14.0;
                let bh = line_height(ui, &bf) + 6.0;
                let badge = Rect::from_min_size(pos2(inner.max.x - 8.0 - bw, inner.max.y - 8.0 - bh), vec2(bw, bh));
                border::border(ui.painter(), badge, 4.0, 0.0, Some(Brush::Solid(Color32::from_rgba_unmultiplied(0x20, 0x20, 0x20, 0xCC))), None);
                ui.painter().galley(pos2(badge.min.x + 7.0, badge.min.y + 3.0), galley(ui, &text, bf, Color32::WHITE, f32::INFINITY), Color32::WHITE);
                let pr = ui.interact(inner, ui.id().with("preview"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if pr.clicked() {
                    // `ImageViewerWindowIntegration.ShowImage(_sourceImage)`.
                    out.push(Effect::ShowImage((**img).clone()));
                }
            }
            None => {
                // `EmptyPreview`: the video icon at 45%, the file's name.
                let text = self.file_path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| s::AfterCaptureWindow_NoPreview().to_owned());
                let tf = font(13.0, 600);
                let lines = wrap_lines(ui, &text, &tf, 360.0);
                let lh = line_height(ui, &tf);
                let h = 48.0 + 8.0 + lh * lines.len() as f32;
                let mut y = center_y(inner, h);
                let c = p.text.gamma_multiply(0.45);
                icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.center().x - 24.0, y), Vec2::splat(48.0)), icon::FileVideo, c);
                y += 56.0;
                for line in lines {
                    let w = text_width(ui, &line, tf.clone());
                    ui.painter().galley(pos2(inner.center().x - (w / 2.0).round(), y), galley(ui, &line, tf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
                    y += lh;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goals_follow_sharex() {
        let all = available_goals(true, true);
        assert_eq!(all, [Goal::CopyImage, Goal::CopyFile, Goal::CopyFilePath, Goal::CopyFolderPath, Goal::CopyUrl]);
        assert_eq!(available_goals(false, false), [Goal::CopyFile, Goal::CopyFilePath, Goal::CopyFolderPath]);
        let t = AfterCaptureTasks::CopyImageToClipboard | AfterCaptureTasks::SaveImageToFile | AfterCaptureTasks::UploadImageToHost;
        assert_eq!(infer_goal(t, &all), Goal::CopyUrl);
        assert_eq!(infer_goal(AfterCaptureTasks::SaveImageToFile, &all), Goal::None);
        assert_eq!(infer_goal(AfterCaptureTasks::CopyImageToClipboard, &available_goals(false, true)), Goal::None);
    }

    #[test]
    fn resolve_keeps_one_clipboard_task() {
        let extras = AfterCaptureTasks::CopyImageToClipboard | AfterCaptureTasks::AnnotateImage | AfterCaptureTasks::DeleteFile;
        let (c, u) = resolve(Goal::CopyFilePath, extras, AfterUploadTasks::None, false);
        assert_eq!(c, AfterCaptureTasks::AnnotateImage | AfterCaptureTasks::SaveImageToFile | AfterCaptureTasks::CopyFilePathToClipboard);
        assert_eq!(u, AfterUploadTasks::None);
        let (c, u) = resolve(Goal::CopyUrl, AfterCaptureTasks::ShowBeforeUploadWindow, AfterUploadTasks::OpenURL, false);
        assert_eq!(c, AfterCaptureTasks::ShowBeforeUploadWindow | AfterCaptureTasks::UploadImageToHost);
        assert_eq!(u, AfterUploadTasks::OpenURL | AfterUploadTasks::CopyURLToClipboard);
        // A recording is already saved: no image work, no save.
        assert!(!is_extra_available(AfterCaptureTasks::AnnotateImage, Goal::CopyFile, true));
        assert_eq!(required_tasks(Goal::CopyFile, true), [Required::Capture(AfterCaptureTasks::CopyFileToClipboard)]);
    }
}
