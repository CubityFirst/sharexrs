//! `ShareX.Tools/Tools/ImageCombiner` (`ImageCombinerWindow.axaml`,
//! `ImageCombinerViewModel`, `TaskHelpers.OpenImageCombiner`): the image
//! list with Add/Remove/Move up/Move down, the layout options (orientation,
//! alignment, spacing, wrap after, auto fill background) saved into the
//! task's `ImageCombinerOptions` as they change, the live preview
//! (`CreateImageCombinerPreviewAsync`, two images at least) and Combine
//! images, which runs the result through the task's after capture tasks
//! (`UploadManager.RunImageTask`).

use egui::{Id, Rect, Ui, pos2, vec2};
use image::RgbaImage;
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::combiner::{Alignment, ImageCombinerOptions, Orientation};
use sharex_ui::controls::{CheckBox, ComboBox, ComboItem, NumericUpDown, RadioButton};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use super::{FileList, Job, Picture};
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 980.0;
pub const HEIGHT: f32 = 650.0;
pub const MIN_WIDTH: f32 = 820.0;
pub const MIN_HEIGHT: f32 = 560.0;

type Preview = Option<Vec<u8>>;

pub struct ImageCombinerWindow {
    pub list: FileList,
    pub options: ImageCombinerOptions,
    /// The task the combined image goes through (`activeTaskSettings`).
    task: TaskSettings,
    /// Whether the options are the default task settings' (saved there).
    save_to_defaults: bool,
    preview: Option<Picture>,
    job: Job<Preview>,
    status: String,
    busy: bool,
    options_changed: bool,
}

impl ImageCombinerWindow {
    /// `new ImageCombinerWindow(options, services, imageFiles)` over the
    /// task's `ToolsSettingsReference.ImageCombinerOptions`.
    pub fn new(task: TaskSettings, defaults: &TaskSettings, files: Vec<String>) -> Self {
        let save_to_defaults = task.UseDefaultToolsSettings;
        let tools = if save_to_defaults { defaults.ToolsSettings.as_ref() } else { task.ToolsSettings.as_ref() };
        let options = tools.map(|t| ImageCombinerOptions::from_json(&t.ImageCombinerOptions)).unwrap_or_default();
        let mut w = Self {
            list: FileList::default(),
            options,
            task,
            save_to_defaults,
            preview: None,
            job: Job::new(0),
            status: s::ImageCombinerViewModel_Add_two_images().into(),
            busy: false,
            options_changed: false,
        };
        w.add_files(files);
        w
    }

    /// `--render`: previews made at once.
    pub fn set_sync(&mut self) {
        self.job.sync = true;
    }

    /// `AddFiles`: the files that exist, appended.
    pub fn add_files(&mut self, files: Vec<String>) {
        self.list.items.extend(files.into_iter().filter(|f| super::file_exists(f)));
        self.collection_changed();
    }

    fn collection_changed(&mut self) {
        self.refresh_preview();
    }

    fn options_changed(&mut self) {
        self.options_changed = true;
        self.refresh_preview();
    }

    /// `RefreshPreviewAsync`: under two images, no preview.
    fn refresh_preview(&mut self) {
        if self.list.items.len() < 2 {
            self.job.cancel();
            self.preview = None;
            self.status = s::ImageCombinerViewModel_Add_two_images().into();
            return;
        }
        self.job.queue();
    }

    fn can_combine(&self) -> bool {
        self.list.items.len() > 1 && !self.busy
    }

    fn can_remove(&self) -> bool {
        !self.list.selected.is_empty() || self.list.primary.is_some()
    }

    fn image_count_text(&self) -> String {
        super::count_text(s::ImageCombinerViewModel_One_image(), s::ImageCombinerViewModel_Image_count(), self.list.items.len())
    }

    /// `CombineAsync` → `ProcessCombinedImagesAsync`.
    fn combine(&mut self) -> Vec<Effect> {
        if !self.can_combine() {
            return Vec::new();
        }
        self.status = s::ImageCombinerViewModel_Combining_images().into();
        match sharex_tools::combiner::combine_files(&self.list.items, &self.options) {
            Some(bmp) => {
                self.status = s::ImageCombinerViewModel_Images_combined().into();
                let image = RgbaImage::from_raw(bmp.width(), bmp.height(), bmp.to_straight()).unwrap_or_default();
                vec![Effect::RunImageTask(image, Box::new(self.task.clone()))]
            }
            None => Vec::new(),
        }
    }

    fn poll_preview(&mut self, ctx: &egui::Context) {
        let files = self.list.items.clone();
        let options = self.options.clone();
        if let Some(result) = self.job.poll(ctx, move || Box::new(move || sharex_tools::combiner::create_preview(&files, &options)))
            && let Some(pic) = result.and_then(|png| Picture::decode(&png))
        {
            self.preview = Some(pic);
            self.status = s::ImageCombinerViewModel_Previewing().replace("{0}", &self.image_count_text());
        }
    }

    fn left_panel(&mut self, ui: &Ui, rect: Rect, live: bool) {
        let p = palette(ui.ctx());
        super::panel(ui, rect);
        let c = rect.shrink(13.0);
        let bar = Rect::from_min_size(c.min, vec2(c.width(), super::BUTTON_HEIGHT));
        let add = s::ImageCombinerWindow_Add();
        let aw = super::icon_text_button_width(ui, add);
        let mut x = bar.min.x;
        if super::icon_text_button(ui, Rect::from_min_size(pos2(x, bar.min.y), vec2(aw, 40.0)), Id::new("combiner-add"), icon::Plus, add, live).clicked() {
            let files = super::pick_images(s::ImageCombinerWindow_Add_images_dialog());
            self.add_files(files);
        }
        x += aw + 8.0;
        let iw = super::ICON_BUTTON_WIDTH;
        let can_remove = self.can_remove();
        let can_up = self.list.primary.is_some_and(|i| i > 0);
        let can_down = self.list.primary.is_some_and(|i| i + 1 < self.list.items.len());
        let buttons = [(icon::Trash, can_remove, 0), (icon::ArrowUp, can_up, 1), (icon::ArrowDown, can_down, 2)];
        for (key, enabled, k) in buttons {
            let r = Rect::from_min_size(pos2(x, bar.min.y), vec2(iw, 40.0));
            if super::icon_button(ui, r, Id::new(("combiner-btn", k)), key, enabled && live).clicked() {
                match k {
                    0 => {
                        self.list.remove_selected();
                        self.collection_changed();
                    }
                    1 => {
                        if self.list.move_selected(-1) {
                            self.collection_changed();
                        }
                    }
                    _ => {
                        if self.list.move_selected(1) {
                            self.collection_changed();
                        }
                    }
                }
            }
            x += iw + 8.0;
        }
        let count = self.image_count_text();
        let nf = super::normal_font();
        let cw = text_width(ui, &count, nf.clone());
        super::text_at(ui, pos2(bar.max.x - cw, center_y(bar, line_height(ui, &nf))), &count, nf.clone(), p.text_secondary);
        let list_rect = Rect::from_min_max(pos2(c.min.x, bar.max.y + 10.0), c.max);
        self.list.ui(ui, list_rect, Id::new("combiner-list"), live);
        if self.list.items.is_empty() {
            super::empty_state(ui, list_rect, icon::DragDropUpload, 42.0, 8.0, &[(s::ImageCombinerWindow_Drop_images_here_or_use_the_Add_button(), nf, p.text_secondary)]);
        }
    }

    fn layout_panel(&mut self, ui: &mut Ui, rect: Rect, live: bool) {
        super::panel(ui, rect);
        let c = rect.shrink(17.0);
        let hh = super::heading_height(ui);
        super::heading(ui, Rect::from_min_size(c.min, vec2(c.width(), hh)), s::ImageCombinerWindow_Layout());
        let mut y = c.min.y + hh + 10.0;
        // Orientation: two radio buttons (`Spacing="16"`).
        let row = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), 32.0));
        let control = super::label_row(ui, row, 120.0, s::ImageCombinerWindow_Orientation());
        let nf = super::normal_font();
        let mut rx = control.min.x;
        let horizontal = self.options.orientation == Orientation::Horizontal;
        for (k, text, checked) in [(0, s::ImageCombinerWindow_Horizontal(), horizontal), (1, s::ImageCombinerWindow_Vertical(), !horizontal)] {
            let rb = RadioButton { text, text_font: nf.clone(), text_color: None, padding_left: 8.0, enabled: live };
            let w = 20.0 + 8.0 + text_width(ui, text, nf.clone());
            if rb.show(ui, Rect::from_min_size(pos2(rx, row.min.y), vec2(w, 32.0)), Id::new(("combiner-orientation", k)), checked).clicked() && !checked {
                self.options.orientation = if k == 0 { Orientation::Horizontal } else { Orientation::Vertical };
                self.options_changed();
            }
            rx += w + 16.0;
        }
        y += 32.0 + 10.0;
        // Alignment and spacing (`ColumnDefinitions="120,*,120,*"`).
        let star = ((c.width() - 240.0 - 30.0) / 2.0).max(0.0);
        let cols = [c.min.x, c.min.x + 130.0, c.min.x + 130.0 + star + 10.0, c.min.x + 130.0 + star + 10.0 + 130.0];
        let row = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), 40.0));
        super::row_label(ui, Rect::from_min_max(pos2(cols[0], row.min.y), pos2(cols[0] + 120.0, row.max.y)), s::ImageCombinerWindow_Alignment());
        let items: Vec<ComboItem> = alignment_options(self.options.orientation).iter().map(|t| ComboItem::text(*t)).collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: live };
        let selected = self.options.alignment as usize;
        // `HorizontalAlignment="Left"`: as wide as the selected item.
        let cw = combo.auto_width(ui, &items, Some(selected)).min(star);
        if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(cols[1], row.min.y), vec2(cw, 40.0)), Id::new("combiner-alignment"), &items, Some(selected)) {
            self.options.alignment = [Alignment::LeftOrTop, Alignment::Center, Alignment::RightOrBottom][i.min(2)];
            self.options_changed();
        }
        super::row_label(ui, Rect::from_min_max(pos2(cols[2], row.min.y), pos2(cols[2] + 120.0, row.max.y)), s::ImageCombinerWindow_Spacing());
        let mut space = self.options.space as i64;
        if (NumericUpDown { minimum: 0, maximum: 1000, enabled: live }).show(ui, Rect::from_min_size(pos2(cols[3], row.min.y), vec2(star, 40.0)), Id::new("combiner-space"), &mut space) {
            self.options.space = space as i32;
            self.options_changed();
        }
        y += 40.0 + 10.0;
        let row = Rect::from_min_size(pos2(c.min.x, y), vec2(c.width(), 40.0));
        super::row_label(ui, Rect::from_min_max(pos2(cols[0], row.min.y), pos2(cols[0] + 120.0, row.max.y)), s::ImageCombinerWindow_Wrap_after());
        let mut wrap = self.options.wrap_after as i64;
        if (NumericUpDown { minimum: 0, maximum: 1000, enabled: live }).show(ui, Rect::from_min_size(pos2(cols[1], row.min.y), vec2(star, 40.0)), Id::new("combiner-wrap"), &mut wrap) {
            self.options.wrap_after = wrap as i32;
            self.options_changed();
        }
        let text = s::ImageCombinerWindow_Auto_fill_background();
        let cb = CheckBox { text, text_font: font(13.0, 400), text_color: Some(palette(ui.ctx()).text_secondary), min_height: 32.0, enabled: live };
        let cr = Rect::from_min_size(pos2(cols[2], center_y(row, 32.0)), vec2(cb.width(ui), 32.0));
        let mut fill = self.options.auto_fill_background;
        cb.show(ui, cr, Id::new("combiner-fill"), &mut fill);
        if fill != self.options.auto_fill_background {
            self.options.auto_fill_background = fill;
            self.options_changed();
        }
    }

    /// The layout panel's height.
    fn layout_height(ui: &Ui) -> f32 {
        2.0 + 32.0 + super::heading_height(ui) + 10.0 + 32.0 + 10.0 + 40.0 + 10.0 + 40.0
    }

    fn preview_panel(&mut self, ui: &Ui, rect: Rect) {
        let p = palette(ui.ctx());
        super::panel(ui, rect);
        let area = rect.shrink(17.0);
        if let Some(pic) = &mut self.preview {
            // `Image Stretch="Uniform"`: scaled to fit, centred.
            let dest = crate::dialogs::layout::uniform(area, pic.size());
            let tex = pic.texture(ui.ctx(), (dest.width().round() as u32, dest.height().round() as u32));
            ui.painter().with_clip_rect(area.intersect(ui.clip_rect())).image(tex, dest, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        } else {
            super::empty_state(ui, area, icon::Images, 46.0, 8.0, &[(s::ImageCombinerWindow_Add_at_least_two_images_to_preview(), super::normal_font(), p.text_secondary)]);
        }
    }
}

/// `AlignmentOptions`.
fn alignment_options(o: Orientation) -> [&'static str; 3] {
    match o {
        Orientation::Horizontal => [s::ImageCombinerViewModel_Top(), s::ImageCombinerViewModel_Center(), s::ImageCombinerViewModel_Bottom()],
        Orientation::Vertical => [s::ImageCombinerViewModel_Left(), s::ImageCombinerViewModel_Center(), s::ImageCombinerViewModel_Right()],
    }
}

impl Dialog for ImageCombinerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::ImageCombinerWindow_ShareX_Image_Combiner()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        self.poll_preview(ui.ctx());
        let live = true;
        let content = super::background(ui);
        let bottom = Rect::from_min_max(pos2(content.min.x, content.max.y - super::BUTTON_HEIGHT), content.max);
        let main = Rect::from_min_max(content.min, pos2(content.max.x, bottom.min.y - 12.0));
        let left = Rect::from_min_size(main.min, vec2(440.0, main.height()));
        let right = Rect::from_min_max(pos2(left.max.x + 12.0, main.min.y), main.max);
        self.left_panel(ui, left, live);
        let lh = Self::layout_height(ui);
        let layout = Rect::from_min_size(right.min, vec2(right.width(), lh));
        self.layout_panel(ui, layout, live);
        let preview = Rect::from_min_max(pos2(right.min.x, layout.max.y + 12.0), right.max);
        self.preview_panel(ui, preview);
        let text = s::ImageCombinerWindow_Combine_images();
        let bw = super::icon_text_button_width(ui, text);
        let br = Rect::from_min_size(pos2(bottom.max.x - bw, bottom.min.y), vec2(bw, super::BUTTON_HEIGHT));
        let mut effects = Vec::new();
        if super::icon_text_button(ui, br, Id::new("combiner-combine"), icon::Combine, text, self.can_combine()).clicked() {
            effects = self.combine();
        }
        if std::mem::take(&mut self.options_changed) {
            let json = self.options.to_json();
            if self.save_to_defaults {
                cx.settings.DefaultTaskSettings.ToolsSettings.get_or_insert_default().ImageCombinerOptions = json.clone();
            }
            if let Some(t) = self.task.ToolsSettings.as_mut() {
                t.ImageCombinerOptions = json;
            }
        }
        effects
    }

    fn dropped(&mut self, files: Vec<std::path::PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        self.add_files(files.into_iter().map(|f| f.to_string_lossy().into_owned()).collect());
        Vec::new()
    }
}
