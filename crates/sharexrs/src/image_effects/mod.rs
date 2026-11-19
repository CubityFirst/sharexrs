//! `ShareX.ImageEffectsLib/Presentation/ImageEffectsWindow` with its view
//! model (`ImageEffectsViewModel`), `ImageEffectsIntegration` and
//! `TaskHelpers.OpenImageEffects*`: the presets (new, remove, duplicate,
//! move, rename), the selected preset's effects (enable, add from the
//! catalog menu, remove, duplicate, move, clear with a confirmation,
//! rename), the selected effect's options ([`options`]), the preview of the
//! preset on the image (or ShareX's sample image) 100 ms after the last
//! change, on a thread, the newest only, and the three modes: Tool (an
//! image: open, paste, save, upload), Presets (the task settings' presets)
//! and Editor (after capture: Continue/Cancel, the task waits). Presets
//! are written back to the settings as they change; `.sxie` packages are
//! made with the packager window and imported by dropping them.

pub mod confirm;
pub mod options;
pub mod packager;
pub mod render;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use egui::{Color32, Id, Rect, Ui, Vec2, pos2, vec2};
use image::RgbaImage;
use serde_json::Value;
use sharex_image_effects::preset::{ImageEffect, ImageEffectPreset, TypeNames};
use sharex_image_effects::{Bitmap, CATALOG, Context, EffectKind, RandomFast};
use sharex_settings::TaskSettings;
use sharex_strings::image_effects_lib as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{self, CheckBox, ComboBox, ComboItem, TextBox};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::menu::{Flyout, FlyoutOutput, MenuEntry, MenuStyle, Placement};
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools::{self as tools, Job, Picture};
use options::OptionsPanel;

pub const WIDTH: f32 = 1180.0;
pub const HEIGHT: f32 = 760.0;
pub const MIN_WIDTH: f32 = 1040.0;
pub const MIN_HEIGHT: f32 = 620.0;
/// `Button.picker-button`: 36x36, its icon 16px.
const PICKER: f32 = 36.0;

/// `ImageEffectsWindowMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Presets,
    Tool,
    Editor,
}

/// Where the presets are kept: the default task settings' image settings
/// (saved as they change), or a task's own (kept by the window's caller).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Default,
    Task,
}

/// What a preview run gives: the image, its size, the time it took; or the error.
type PreviewResult = Result<Option<(RgbaImage, u128)>, String>;

pub struct ImageEffectsWindow {
    pub mode: Mode,
    pub presets: Vec<ImageEffectPreset>,
    pub selected_preset: usize,
    pub selected_effect: Option<usize>,
    source: Bitmap,
    pub file_path: String,
    preview: Option<Picture>,
    title: String,
    title_sent: String,
    error: String,
    job: Job<PreviewResult>,
    options: OptionsPanel,
    effects_scroll: f32,
    add_menu: Option<Flyout<&'static str>>,
    effects_folder: PathBuf,
    target: Target,
    presets_changed: bool,
    started: bool,
    /// The clear confirmation's answer (`ImageEffectsConfirmationWindow`).
    confirm: Option<mpsc::Receiver<bool>>,
    /// Editor mode: the task waiting for the presets and the selected index.
    pub reply: Option<mpsc::Sender<(Vec<Value>, i32)>>,
    rng: RandomFast,
    /// The task an uploaded image goes through (`CreateImageEffectsCallbacks(taskSettings)`).
    task: TaskSettings,
    pending_effects: Vec<Effect>,
    pending_dialog: Option<Box<dyn Dialog>>,
    /// Open the Add menu on the next frame (`--render --state add-menu`).
    add_menu_pending: bool,
    /// The add menu opened this frame: the press of a one-frame click on
    /// the button isn't a press outside it.
    add_menu_just_opened: bool,
}

/// An engine bitmap from an image.
pub fn bitmap(image: &RgbaImage) -> Bitmap {
    Bitmap::from_straight(image.width(), image.height(), image.as_raw())
}

/// The theme colours `CreateSampleImage` reads.
fn sample_colors(ctx: &egui::Context) -> sharex_image_effects::sample::SampleColors {
    let p = palette(ctx);
    let c = |c: Color32| [c.r(), c.g(), c.b(), c.a()];
    sharex_image_effects::sample::SampleColors {
        background_main: c(p.background_main),
        background_panel: c(p.background_panel),
        accent_start: c(p.accent_start),
        accent_end: c(p.accent_end),
        accent_foreground: c(p.accent_foreground),
    }
}

impl ImageEffectsWindow {
    /// `new ImageEffectsWindow(sourceImage, presets, selectedPresetIndex, mode, callbacks, filePath)`.
    /// Without an image the sample image is made on the first frame (it
    /// takes the theme's colours).
    #[allow(clippy::too_many_arguments)]
    pub fn new(source: Option<RgbaImage>, presets: &[Value], selected: i32, mode: Mode, file_path: Option<String>, effects_folder: PathBuf, target: Target, task: TaskSettings) -> Self {
        let mut presets: Vec<ImageEffectPreset> = presets.iter().map(|v| ImageEffectPreset::from_json(v).unwrap_or_default()).collect();
        if presets.is_empty() {
            presets.push(ImageEffectPreset::default());
        }
        let selected_preset = selected.clamp(0, presets.len() as i32 - 1) as usize;
        let selected_effect = (!presets[selected_preset].effects.is_empty()).then_some(0);
        let source = source.as_ref().map(bitmap).unwrap_or_else(|| Bitmap::new(0, 0));
        Self {
            mode,
            presets,
            selected_preset,
            selected_effect,
            source,
            file_path: file_path.unwrap_or_default(),
            preview: None,
            title: s::ImageEffectsWindow_Title().to_string(),
            title_sent: String::new(),
            error: String::new(),
            job: Job::new(100),
            options: OptionsPanel::default(),
            effects_scroll: 0.0,
            add_menu: None,
            effects_folder,
            target,
            presets_changed: false,
            started: false,
            confirm: None,
            reply: None,
            rng: RandomFast::new(),
            task,
            pending_effects: Vec::new(),
            pending_dialog: None,
            add_menu_pending: false,
            add_menu_just_opened: false,
        }
    }

    /// The Add effect menu, as its button opens it.
    pub fn open_add_menu(&mut self) {
        self.add_menu_pending = true;
    }

    pub fn set_sync(&mut self) {
        self.job.sync = true;
    }

    /// The presets as ShareX's settings write them.
    pub fn presets_json(&self) -> Vec<Value> {
        self.presets.iter().map(|p| p.to_json(TypeNames::Full)).collect()
    }

    fn preset(&self) -> &ImageEffectPreset {
        &self.presets[self.selected_preset]
    }

    fn preset_mut(&mut self) -> &mut ImageEffectPreset {
        &mut self.presets[self.selected_preset]
    }

    /// `QueuePreview`.
    fn queue_preview(&mut self) {
        self.job.queue();
    }

    fn changed(&mut self) {
        self.presets_changed = true;
        self.queue_preview();
    }

    /// `OnSelectedPresetChanged`.
    fn select_preset(&mut self, i: usize) {
        self.selected_preset = i.min(self.presets.len() - 1);
        self.selected_effect = (!self.preset().effects.is_empty()).then_some(0);
        self.queue_preview();
    }

    fn new_preset(&mut self) {
        self.presets.push(ImageEffectPreset::default());
        self.select_preset(self.presets.len() - 1);
        self.presets_changed = true;
    }

    fn remove_preset(&mut self) {
        let i = self.selected_preset;
        self.presets.remove(i);
        if self.presets.is_empty() {
            self.new_preset();
        } else {
            self.select_preset(i.min(self.presets.len() - 1));
        }
        self.presets_changed = true;
    }

    fn duplicate_preset(&mut self) {
        let copy = self.preset().clone();
        self.presets.push(copy);
        self.select_preset(self.presets.len() - 1);
        self.presets_changed = true;
    }

    fn move_preset(&mut self, d: isize) {
        let (i, j) = (self.selected_preset as isize, self.selected_preset as isize + d);
        if j < 0 || j as usize >= self.presets.len() {
            return;
        }
        self.presets.swap(i as usize, j as usize);
        self.selected_preset = j as usize;
        self.presets_changed = true;
    }

    /// `AddEffect`: after the selected effect, selected.
    pub fn add_effect(&mut self, effect: ImageEffect) {
        let i = self.selected_effect.map_or(self.preset().effects.len(), |i| i + 1);
        self.preset_mut().effects.insert(i, effect);
        self.selected_effect = Some(i);
        self.changed();
    }

    fn remove_effect(&mut self) {
        let Some(i) = self.selected_effect else { return };
        self.preset_mut().effects.remove(i);
        let n = self.preset().effects.len();
        self.selected_effect = (n > 0).then(|| i.min(n - 1));
        self.changed();
    }

    fn duplicate_effect(&mut self) {
        if let Some(e) = self.selected_effect.and_then(|i| self.preset().effects.get(i)).cloned() {
            self.add_effect(e);
        }
    }

    fn clear_effects(&mut self) {
        self.preset_mut().effects.clear();
        self.selected_effect = None;
        self.changed();
    }

    fn move_effect(&mut self, d: isize) {
        let Some(i) = self.selected_effect else { return };
        let j = i as isize + d;
        if j < 0 || j as usize >= self.preset().effects.len() {
            return;
        }
        self.preset_mut().effects.swap(i, j as usize);
        self.selected_effect = Some(j as usize);
        self.changed();
    }

    /// `ImportPreset(json)`: a preset with effects, its image paths kept
    /// inside the ImageEffects folder, added and selected.
    pub fn import_preset(&mut self, json: &str) {
        let Some(mut preset) = ImageEffectPreset::from_json_str(json) else {
            self.error = "The preset couldn't be read.".into();
            return;
        };
        if preset.effects.is_empty() {
            return;
        }
        let ctx = self.context();
        sharex_image_effects::preset::sanitize_imported(&mut preset, &self.effects_folder, &ctx);
        self.presets.push(preset);
        self.select_preset(self.presets.len() - 1);
        self.presets_changed = true;
    }

    fn context(&self) -> Context {
        Context { special_folders: vec![("ShareXImageEffects".into(), self.effects_folder.clone())], ..Default::default() }
    }

    /// `ReplaceSource`.
    pub fn replace_source(&mut self, image: RgbaImage, path: Option<String>) {
        self.source = bitmap(&image);
        self.file_path = path.unwrap_or_default();
        self.queue_preview();
    }

    /// The selected preset applied to the image (`ApplySelectedPresetAsync`).
    fn apply_selected(&self) -> Option<RgbaImage> {
        let out = self.preset().apply(&self.source, &mut self.context())?;
        RgbaImage::from_raw(out.width(), out.height(), out.to_straight())
    }

    /// `UpdatePreviewAsync`.
    fn poll_preview(&mut self, ctx: &egui::Context) {
        if self.source.is_empty() {
            // `CreateSampleImage` on the theme's colours.
            self.source = sharex_image_effects::sample::create_sample_image(&sample_colors(ctx));
        }
        let source = self.source.clone();
        let preset = self.preset().clone();
        let mut context = self.context();
        let result = self.job.poll(ctx, move || {
            Box::new(move || {
                let started = Instant::now();
                Ok(preset.apply(&source, &mut context).and_then(|out| RgbaImage::from_raw(out.width(), out.height(), out.to_straight())).map(|img| (img, started.elapsed().as_millis())))
            })
        });
        match result {
            Some(Ok(Some((img, ms)))) => {
                self.error.clear();
                self.title = s::ImageEffectsWindow_Title_with_preview()
                    .replace("{0}", s::ImageEffectsWindow_Title())
                    .replace("{1}", &img.width().to_string())
                    .replace("{2}", &img.height().to_string())
                    .replace("{3}", &ms.to_string());
                self.preview = Some(Picture::new(img));
            }
            Some(Ok(None)) => {}
            Some(Err(e)) => self.error = e,
            None => {}
        }
    }

    fn picker(ui: &Ui, rect: Rect, id: Id, key: &str, enabled: bool, tip: &str) -> bool {
        picker_button(ui, rect, id, key, enabled).on_hover_text(tip).clicked()
    }

    /// The presets panel (`tool-panel`, `Padding="12"`).
    fn presets_panel(&mut self, ui: &mut Ui, rect: Rect, live: bool) {
        tools::panel(ui, rect);
        let c = rect.shrink(13.0);
        let row = Rect::from_min_size(c.min, vec2(c.width(), PICKER));
        tools::heading(ui, row, s::ImageEffectsWindow_Presets());
        let buttons = [
            (icon::Plus, s::ImageEffectsWindow_New_preset()),
            (icon::Minus, s::ImageEffectsWindow_Remove_preset()),
            (icon::Copy, s::ImageEffectsWindow_Duplicate_preset()),
            (icon::ArrowUp, s::ImageEffectsWindow_Move_up()),
            (icon::ArrowDown, s::ImageEffectsWindow_Move_down()),
        ];
        let mut x = row.max.x - 5.0 * PICKER - 4.0 * 4.0;
        for (k, (key, tip)) in buttons.into_iter().enumerate() {
            if Self::picker(ui, Rect::from_min_size(pos2(x, row.min.y), Vec2::splat(PICKER)), Id::new(("effects-preset-btn", k)), key, live, tip) {
                match k {
                    0 => self.new_preset(),
                    1 => self.remove_preset(),
                    2 => self.duplicate_preset(),
                    3 => self.move_preset(-1),
                    _ => self.move_preset(1),
                }
            }
            x += PICKER + 4.0;
        }
        let combo_r = Rect::from_min_size(pos2(c.min.x, row.max.y + 10.0), vec2(c.width(), 40.0));
        let items: Vec<ComboItem> = self.presets.iter().map(|p| ComboItem::normal(if p.name.trim().is_empty() { s::ImageEffectsWindow_Unnamed_preset().to_string() } else { p.name.clone() })).collect();
        if let Some(i) = (ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: live }).show(ui, combo_r, Id::new("effects-presets"), &items, Some(self.selected_preset)) {
            self.select_preset(i);
        }
        let name_r = Rect::from_min_size(pos2(c.min.x, combo_r.max.y + 10.0), vec2(c.width(), 40.0));
        let mut name = self.preset().name.clone();
        TextBox { placeholder: s::ImageEffectsWindow_Preset_name(), enabled: live, ..TextBox::default() }.show(ui, name_r, Id::new("effects-preset-name"), &mut name);
        if name != self.preset().name {
            self.preset_mut().name = name;
            self.presets_changed = true;
        }
    }

    /// The effects panel.
    fn effects_panel(&mut self, ui: &mut Ui, rect: Rect, live: bool) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let c = rect.shrink(13.0);
        let row = Rect::from_min_size(c.min, vec2(c.width(), PICKER));
        tools::heading(ui, row, s::ImageEffectsWindow_Effects());
        if Self::picker(ui, Rect::from_min_size(pos2(row.max.x - PICKER, row.min.y), Vec2::splat(PICKER)), Id::new("effects-refresh"), icon::Refresh, live, s::ImageEffectsWindow_Refresh_preview()) {
            self.queue_preview();
        }
        let name_r = Rect::from_min_size(pos2(c.min.x, row.max.y + 10.0), vec2(c.width(), 40.0));
        let selected = self.selected_effect;
        let mut name = selected.and_then(|i| self.preset().effects.get(i)).and_then(|e| e.name.clone()).unwrap_or_default();
        let before = name.clone();
        TextBox { placeholder: s::ImageEffectsWindow_Custom_effect_name(), enabled: live && selected.is_some(), ..TextBox::default() }.show(ui, name_r, Id::new("effects-effect-name"), &mut name);
        if name != before
            && let Some(i) = selected
        {
            self.preset_mut().effects[i].name = Some(name);
            self.presets_changed = true;
        }
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - PICKER), c.max);
        let list = Rect::from_min_max(pos2(c.min.x, name_r.max.y + 10.0), pos2(c.max.x, bottom.min.y - 10.0));
        // `ListBox.compact`: items `Padding="4,0" MinHeight="32"`.
        let f = sharex_ui::theme::fluent(ui.ctx());
        ui.painter().rect_filled(list, 0.0, f.chrome_medium_low);
        let row_h = 32.0;
        let n = self.preset().effects.len();
        let content_h = row_h * n as f32;
        let max_scroll = (content_h - list.height()).max(0.0);
        if live && ui.rect_contains_pointer(list) {
            self.effects_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.effects_scroll = self.effects_scroll.clamp(0.0, max_scroll);
        let painter = ui.painter().with_clip_rect(list.intersect(ui.clip_rect()));
        let nf = font(13.0, 400);
        let lh = line_height(ui, &nf);
        let mut clicked = None;
        let mut toggled = None;
        for i in 0..n {
            let y = list.min.y + i as f32 * row_h - self.effects_scroll;
            if y + row_h < list.min.y || y > list.max.y {
                continue;
            }
            let r = Rect::from_min_size(pos2(list.min.x, y), vec2(list.width(), row_h));
            let sel = Some(i) == self.selected_effect;
            let resp = live.then(|| ui.interact(r.intersect(list), Id::new(("effects-row", i)), egui::Sense::click()));
            let (hovered, pressed) = resp.as_ref().map_or((false, false), |r| (r.hovered(), r.is_pointer_button_down_on()));
            controls::list_item_background(&painter, r, 0.0, hovered, pressed, sel);
            let e = &self.preset().effects[i];
            let mut enabled = e.enabled;
            let cb = CheckBox { text: "", text_font: nf.clone(), text_color: None, min_height: 32.0, enabled: live };
            let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(list));
            cui.set_clip_rect(list.intersect(ui.clip_rect()));
            cb.show(&cui, Rect::from_min_size(pos2(r.min.x + 4.0, r.min.y), vec2(28.0, 32.0)), Id::new(("effects-enabled", i)), &mut enabled);
            if enabled != e.enabled {
                toggled = Some((i, enabled));
            }
            let fg = if sel { p.accent_foreground } else { p.text_secondary };
            let tx = r.min.x + 4.0 + 28.0 + 6.0;
            let label = tools::ellipsize(ui, &e.label(), &nf, r.max.x - 4.0 - tx);
            painter.galley(pos2(tx, center_y(r, lh)), sharex_ui::widgets::galley(ui, &label, nf.clone(), fg, f32::INFINITY), fg);
            if resp.is_some_and(|r| r.clicked()) {
                clicked = Some(i);
            }
        }
        if max_scroll > 0.0 {
            crate::dialogs::layout::scroll_bar(ui, list, content_h, self.effects_scroll, 0.0);
        }
        if let Some((i, on)) = toggled {
            self.preset_mut().effects[i].enabled = on;
            self.changed();
        }
        if let Some(i) = clicked {
            self.selected_effect = Some(i);
        }
        // The six buttons (`ColumnDefinitions="*,*,*,*,*,*" ColumnSpacing="4"`).
        let col = (bottom.width() - 20.0) / 6.0;
        let has_effect = self.selected_effect.is_some();
        let buttons = [
            (icon::Plus, s::ImageEffectsWindow_Add_effect(), true),
            (icon::Minus, s::ImageEffectsWindow_Remove_effect(), has_effect),
            (icon::Copy, s::ImageEffectsWindow_Duplicate_effect(), has_effect),
            (icon::ArrowUp, s::ImageEffectsWindow_Move_up(), has_effect),
            (icon::ArrowDown, s::ImageEffectsWindow_Move_down(), has_effect),
            (icon::Eraser, s::ImageEffectsWindow_Clear_effects(), true),
        ];
        for (k, (key, tip, enabled)) in buttons.into_iter().enumerate() {
            let x = (bottom.min.x + k as f32 * (col + 4.0)).round();
            let br = Rect::from_min_size(pos2(x, bottom.min.y), Vec2::splat(PICKER));
            if k == 0 && std::mem::take(&mut self.add_menu_pending) {
                self.add_menu = Some(add_menu(br));
                self.add_menu_just_opened = true;
            }
            if Self::picker(ui, br, Id::new(("effects-effect-btn", k)), key, enabled && live, tip) {
                match k {
                    0 => {
                        self.add_menu = Some(add_menu(br));
                        self.add_menu_just_opened = true;
                    }
                    1 => self.remove_effect(),
                    2 => self.duplicate_effect(),
                    3 => self.move_effect(-1),
                    4 => self.move_effect(1),
                    _ => {}
                }
                if k == 5 {
                    // `OnClearEffectsClick`: asks first.
                    let (tx, rx) = mpsc::channel();
                    self.confirm = Some(rx);
                    self.pending_dialog = Some(Box::new(confirm::ConfirmWindow::new(s::ImageEffectsWindow_Clear_confirmation(), tx)));
                }
            }
        }
    }

    /// The preview (`Border` on `ShareX.Brush.Checkered`, the image
    /// `Stretch="Uniform" StretchDirection="DownOnly"`, the error badge).
    fn preview_area(&mut self, ui: &Ui, rect: Rect, live: bool) {
        let p = palette(ui.ctx());
        border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::Checkered { colors: p.checkered, cell: 15.0 }), Some(p.border));
        let inner = rect.shrink(1.0);
        if let Some(pic) = &mut self.preview {
            let dest = tools::slider::image_bounds(inner, pic.size(), tools::slider::Stretch::DownOnly);
            let tex = pic.texture(ui.ctx(), (dest.width().round() as u32, dest.height().round() as u32));
            ui.painter().with_clip_rect(inner.intersect(ui.clip_rect())).image(tex, dest, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            let resp = if live { Some(ui.interact(inner, Id::new("effects-preview"), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)) } else { None };
            if resp.is_some_and(|r| r.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed())) {
                // `OnPreviewPointerPressed`: `ImageViewerWindowIntegration.ShowImage(imageData,
                // displayName)`, the file's name or "Preview".
                let name = std::path::Path::new(&self.file_path)
                    .file_name()
                    .filter(|_| !self.file_path.trim().is_empty())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| s::ImageEffectsWindow_Preview_name().to_owned());
                self.pending_effects.push(Effect::ShowImageNamed(pic.image.clone(), name));
            }
        }
        if self.job.busy && !self.job.sync {
            controls::indeterminate_progress(ui.painter(), Rect::from_min_size(inner.min, vec2(inner.width(), 4.0)), self.job.busy_since.elapsed().as_secs_f64());
            ui.ctx().request_repaint();
        }
        if !self.error.trim().is_empty() {
            let nf = font(13.0, 400);
            let tw = text_width(ui, &self.error, nf.clone());
            let lh = line_height(ui, &nf);
            let badge = Rect::from_min_size(pos2(inner.max.x - 8.0 - tw - 16.0, inner.max.y - 8.0 - lh - 8.0), vec2(tw + 16.0, lh + 8.0));
            border::border(ui.painter(), badge, 4.0, 0.0, Some(Brush::Solid(p.background_panel)), None);
            tools::text_at(ui, pos2(badge.min.x + 8.0, badge.min.y + 4.0), &self.error, nf, p.cancel_border);
        }
    }

    /// The bottom row of buttons.
    fn buttons(&mut self, ui: &Ui, row: Rect, live: bool) {
        let tool = self.mode == Mode::Tool;
        let mut x = row.min.x;
        let left = [
            (icon::FolderOpen, s::ImageEffectsWindow_Open_image(), tool, true, 0),
            (icon::ClipboardPaste, s::ImageEffectsWindow_Load_from_clipboard(), tool, true, 1),
            (icon::Package, s::ImageEffectsWindow_Package_preset(), true, true, 2),
            (icon::ExternalLink, s::ImageEffectsWindow_Image_effects_page(), true, true, 3),
        ];
        for (key, text, visible, enabled, k) in left {
            if !visible {
                continue;
            }
            let w = tools::icon_text_button_width(ui, text);
            if tools::icon_text_button(ui, Rect::from_min_size(pos2(x, row.min.y), vec2(w, 40.0)), Id::new(("effects-bottom", k)), key, text, enabled && live).clicked() {
                match k {
                    0 => {
                        if let Some(f) = tools::open_image_file_dialog()
                            && let Some(img) = crate::thumbnail::load(std::path::Path::new(&f))
                        {
                            self.replace_source(img, Some(f));
                        }
                    }
                    1 => self.pending_effects.push(Effect::PasteImage),
                    2 => self.package_preset(),
                    _ => self.pending_effects.push(Effect::OpenUrl("https://getsharex.com/image-effects".into())),
                }
            }
            x += w + 8.0;
        }
        let mut right = row.max.x;
        let close_text = if self.mode == Mode::Editor { s::ImageEffectsWindow_Cancel() } else { s::ImageEffectsWindow_Close() };
        let cw = tools::text_button_width(ui, close_text);
        right -= cw;
        if tools::text_button(ui, Rect::from_min_size(pos2(right, row.min.y), vec2(cw, 40.0)), Id::new("effects-close"), close_text, live).clicked() {
            self.pending_effects.push(Effect::Close);
        }
        if self.mode == Mode::Editor {
            let t = s::ImageEffectsWindow_Continue();
            let w = tools::text_button_width(ui, t);
            right -= 8.0 + w;
            if tools::text_button(ui, Rect::from_min_size(pos2(right, row.min.y), vec2(w, 40.0)), Id::new("effects-continue"), t, live).clicked() {
                self.pending_effects.push(Effect::Close);
            }
        }
        if tool {
            for (k, (key, text)) in [(icon::Upload, s::ImageEffectsWindow_Upload_image()), (icon::Save, s::ImageEffectsWindow_Save_image())].into_iter().enumerate() {
                let w = tools::icon_text_button_width(ui, text);
                right -= 8.0 + w;
                if tools::icon_text_button(ui, Rect::from_min_size(pos2(right, row.min.y), vec2(w, 40.0)), Id::new(("effects-io", k)), key, text, live).clicked() {
                    if k == 0 {
                        if let Some(img) = self.apply_selected() {
                            self.pending_effects.push(Effect::RunImageTask(img, Box::new(self.task.clone())));
                        }
                    } else {
                        self.save_image();
                    }
                }
            }
        }
    }

    /// `SaveAsync` → `SkiaImageHelpers.SaveImageFileDialog(image, path)`.
    fn save_image(&mut self) {
        let Some(img) = self.apply_selected() else { return };
        if let Some(out) = crate::image_tools::save_image_file_dialog(&img, std::path::Path::new(&self.file_path)) {
            self.file_path = out.to_string_lossy().into_owned();
        }
    }

    /// `PackagePreset`: a name is needed, then the packager window.
    fn package_preset(&mut self) {
        if self.preset().name.trim().is_empty() {
            self.error = s::ImageEffectsWindow_Enter_preset_name().into();
            return;
        }
        self.error.clear();
        let json = self.preset().to_json_string(TypeNames::Short);
        let name = self.preset().name.clone();
        self.pending_dialog = Some(Box::new(packager::PackagerWindow::new(json, &name, self.effects_folder.clone())));
    }

    /// What the frame decided (effects for the app, a dialog to open).
    fn take_effects(&mut self) -> Vec<Effect> {
        let mut out = std::mem::take(&mut self.pending_effects);
        if let Some(d) = self.pending_dialog.take() {
            out.push(Effect::OpenDialog(d));
        }
        out
    }
}

/// `OnAddEffectClick`: a menu of the catalog's categories, each a submenu
/// of its effects.
fn add_menu(anchor: Rect) -> Flyout<&'static str> {
    let mut categories: Vec<(sharex_image_effects::Category, Vec<MenuEntry<&'static str>>)> = Vec::new();
    for &(cat, ty) in CATALOG {
        let name = EffectKind::create(ty, &mut RandomFast::seeded(0)).map(|k| k.display_name()).unwrap_or(ty);
        match categories.iter_mut().find(|(c, _)| *c == cat) {
            Some((_, items)) => items.push(MenuEntry::item(name, "", ty)),
            None => categories.push((cat, vec![MenuEntry::item(name, "", ty)])),
        }
    }
    let entries = categories.into_iter().map(|(c, items)| MenuEntry::parent(c.name(), "", items)).collect();
    let mut f = Flyout::menu(Id::new("effects-add-menu"), entries, anchor, Placement::BottomEdgeAlignedLeft);
    f.style = MenuStyle::Standard;
    f
}

/// `Button.picker-button`: transparent, borderless, `ShareX.Brush.Control.Background.Disabled`
/// under the pointer and pressed, its 16px icon in the text colour.
pub fn picker_button(ui: &Ui, rect: Rect, id: Id, key: &str, enabled: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let r = ui.interact(rect, id, if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    if enabled && (r.hovered() || r.is_pointer_button_down_on()) {
        border::border(ui.painter(), rect, 4.0, 0.0, Some(Brush::Solid(p.control_background_disabled)), None);
    }
    let color = if enabled { p.text } else { p.text.gamma_multiply(0.35) };
    let size = 16.0;
    let ir = Rect::from_min_size(pos2(rect.min.x + ((rect.width() - size) / 2.0).round_ties_even(), center_y(rect, size)), Vec2::splat(size));
    sharex_ui::icons::paint(ui.painter(), ir, key, color);
    r
}

impl Dialog for ImageEffectsWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ImageEffectsWindow_Title()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        if !self.started {
            // `InitializeAsync`: the first preview at once.
            self.started = true;
            self.job.queue();
            self.job.due_now();
        }
        if let Some(rx) = &self.confirm
            && let Ok(yes) = rx.try_recv()
        {
            self.confirm = None;
            if yes {
                self.clear_effects();
            }
        }
        self.poll_preview(ui.ctx());
        // `OnWindowKeyDown` (ShareX #9304): Escape closes the window, after
        // the menus and flyouts that take it.
        if self.add_menu.is_none() && !self.options.flyout_open() && sharex_ui::popup::escape_pressed(ui.ctx()) {
            self.pending_effects.push(Effect::Close);
        }
        let live = self.add_menu.is_none();
        let text_focused = crate::history_ui::common::text_box_focused(ui.ctx());
        if live && !text_focused {
            if ui.input(|i| i.key_pressed(egui::Key::Delete)) && self.selected_effect.is_some() {
                self.remove_effect();
            }
            if ui.input(|i| i.key_pressed(egui::Key::F5)) {
                self.queue_preview();
            }
        }
        let c = tools::background(ui);
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - 40.0), c.max);
        let main = Rect::from_min_max(c.min, pos2(c.max.x, bottom.min.y - 12.0));
        let col0 = Rect::from_min_size(main.min, vec2(320.0, main.height()));
        let col1 = Rect::from_min_size(pos2(col0.max.x + 12.0, main.min.y), vec2(360.0, main.height()));
        let col2 = Rect::from_min_max(pos2(col1.max.x + 12.0, main.min.y), main.max);
        let presets_h = 2.0 + 24.0 + PICKER + 10.0 + 40.0 + 10.0 + 40.0;
        self.presets_panel(ui, Rect::from_min_size(col0.min, vec2(col0.width(), presets_h)), live);
        self.effects_panel(ui, Rect::from_min_max(pos2(col0.min.x, col0.min.y + presets_h + 12.0), col0.max), live);
        tools::panel(ui, col1);
        self.options.set_effect(self.selected_effect.map(|i| i + self.selected_preset * 100_000));
        let sel = self.selected_effect;
        let preset = &mut self.presets[self.selected_preset];
        let effect = sel.and_then(|i| preset.effects.get_mut(i));
        if self.options.ui(ui, col1.shrink(13.0), effect, live) {
            self.changed();
        }
        self.preview_area(ui, col2, live);
        self.buttons(ui, bottom, live);
        if let Some(menu) = self.add_menu.as_mut() {
            let full = ui.max_rect();
            let FlyoutOutput { command, closed } = menu.show(ui, full);
            let just_opened = std::mem::take(&mut self.add_menu_just_opened);
            let pressed_outside = !just_opened && ui.input(|i| i.pointer.any_pressed()) && ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| !menu.contains(pos));
            if let Some(ty) = command {
                self.add_menu = None;
                if let Some(kind) = EffectKind::create(ty, &mut self.rng) {
                    self.add_effect(ImageEffect::new(kind));
                }
            } else if closed || pressed_outside {
                self.add_menu = None;
            }
        }
        if self.title != self.title_sent {
            self.title_sent = self.title.clone();
            self.pending_effects.push(Effect::SetTitle(crate::rs_title(&self.title)));
        }
        if std::mem::take(&mut self.presets_changed) && self.target == Target::Default {
            let image = cx.settings.DefaultTaskSettings.ImageSettings.get_or_insert_default();
            image.ImageEffectPresets = self.presets_json();
        }
        self.take_effects()
    }

    /// `Closed`: the selected index back to the settings (`selectedPresetChanged`),
    /// or the waiting task (Editor mode).
    fn closed(&mut self, cx: &mut Cx) -> Vec<Effect> {
        let index = self.selected_preset as i32;
        if self.target == Target::Default {
            let image = cx.settings.DefaultTaskSettings.ImageSettings.get_or_insert_default();
            image.ImageEffectPresets = self.presets_json();
            image.SelectedImageEffectPreset = index;
        }
        if let Some(reply) = self.reply.take() {
            let _ = reply.send((self.presets_json(), index));
        }
        Vec::new()
    }

    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        for f in files {
            let path = f.to_string_lossy().into_owned();
            if self.mode == Mode::Tool && sharex_helpers::file_helpers::is_image_file(&path) {
                if let Some(img) = crate::thumbnail::load(&f) {
                    self.replace_source(img, Some(path));
                }
                break;
            }
            if !path.to_ascii_lowercase().ends_with(".sxie") {
                continue;
            }
            match sharex_image_effects::package::extract(&f, &self.effects_folder) {
                Ok(Some(json)) => self.import_preset(&json),
                Ok(None) => {}
                Err(e) => eprintln!("couldn't import {path}: {e}"),
            }
        }
        Vec::new()
    }

    fn replace_image(&mut self, image: RgbaImage, _ctx: &egui::Context) {
        self.replace_source(image, None);
    }

    fn import(&mut self, json: &str) {
        self.import_preset(json);
    }

    fn single(&self) -> Option<&'static str> {
        (self.mode == Mode::Presets).then_some("image-effects-presets")
    }
}

impl crate::App {
    /// `TaskHelpers.OpenImageEffects(filePath, taskSettings)`: an image (the
    /// file, or one picked first) in a Tool mode window over the task's
    /// presets (`ImageSettingsReference`: the default task settings' unless
    /// the task has its own).
    pub(crate) fn open_image_effects_tool(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, file: Option<PathBuf>, task: TaskSettings) {
        let path = match file {
            Some(f) => f,
            None => match tools::open_image_file_dialog() {
                Some(f) => PathBuf::from(f),
                None => return,
            },
        };
        let Some(image) = crate::thumbnail::load(&path) else { return };
        let defaults = &self.settings.settings.DefaultTaskSettings;
        let (settings, target) = if task.UseDefaultImageSettings { (defaults.ImageSettings.clone(), Target::Default) } else { (task.ImageSettings.clone(), Target::Task) };
        let settings = settings.unwrap_or_default();
        let folder = self.settings.paths.image_effects_folder();
        let w = ImageEffectsWindow::new(Some(image), &settings.ImageEffectPresets, settings.SelectedImageEffectPreset, Mode::Tool, Some(path.to_string_lossy().into_owned()), folder, target, task);
        self.open_dialog_window(event_loop, Box::new(w));
    }

    /// `TaskHelpers.OpenImageEffectsSingleton(taskSettings, importJson)`: the
    /// one Presets mode window (an import goes to the open one).
    pub(crate) fn open_image_effects_presets(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, hotkey: Option<usize>, import: Option<String>) {
        if let Some(d) = self.dialogs.iter_mut().find(|d| d.dialog.single() == Some("image-effects-presets")) {
            if let Some(json) = &import {
                d.dialog.import(json);
            }
            d.window.window.set_minimized(false);
            crate::window::activate(&d.window.window);
            d.window.window.request_redraw();
            return;
        }
        let (settings, target, task) = match hotkey.and_then(|i| self.settings.hotkeys.Hotkeys.get(i)) {
            Some(h) => (h.TaskSettings.ImageSettings.clone(), Target::Task, h.TaskSettings.clone()),
            None => {
                let d = &self.settings.settings.DefaultTaskSettings;
                (d.ImageSettings.clone(), Target::Default, TaskSettings::default_task_settings(d))
            }
        };
        let settings = settings.unwrap_or_default();
        let folder = self.settings.paths.image_effects_folder();
        let mut w = ImageEffectsWindow::new(None, &settings.ImageEffectPresets, settings.SelectedImageEffectPreset, Mode::Presets, None, folder, target, task);
        if let Some(json) = &import {
            w.import_preset(json);
        }
        self.open_dialog_window(event_loop, Box::new(w));
    }

    /// `ImageEffectsIntegration.ShowDialog(bmp, presets, selected, Editor)`
    /// from an after capture task: the task waits for the presets and the
    /// selected index.
    pub(crate) fn open_image_effects_editor(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, image: RgbaImage, task: TaskSettings, reply: mpsc::Sender<(Vec<Value>, i32)>) {
        let settings = task.ImageSettings.clone().unwrap_or_default();
        let target = if task.UseDefaultImageSettings { Target::Default } else { Target::Task };
        let folder = self.settings.paths.image_effects_folder();
        let mut w = ImageEffectsWindow::new(Some(image), &settings.ImageEffectPresets, settings.SelectedImageEffectPreset, Mode::Editor, None, folder, target, task);
        w.reply = Some(reply);
        self.open_dialog_window(event_loop, Box::new(w));
    }
}
