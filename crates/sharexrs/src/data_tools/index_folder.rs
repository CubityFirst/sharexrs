//! `ShareX.Tools/Tools/DirectoryIndexer` (`DirectoryIndexerWindow.axaml`,
//! `DirectoryIndexerViewModel`) with `TaskHelpers.OpenDirectoryIndexer`'s
//! upload: the folder (picked when the window opens, or dropped), the
//! index options and each output's own, the output made on a thread
//! (`sharex_tools::indexer::index`) and shown in a read-only text box (an
//! HTML index shows a notice instead), Save as... (`Index for <folder>`
//! with the output's extension, UTF-8 with a BOM) and Upload (a text
//! upload named with the extension). Options are written into the task's
//! `IndexerSettings` as they change.

use std::path::PathBuf;

use egui::{FontId, Id, Rect, Ui, pos2, vec2};
use sharex_settings::TaskSettings;
use sharex_strings::tools as s;
use sharex_tools::indexer::{IndexerOutput, IndexerSettings};
use sharex_ui::controls::{self, ComboBox, ComboItem, NumericUpDown, TextBox, ToggleSwitch};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec, UploadRequest};
use crate::image_tools::{self as tools, Job};

pub const WIDTH: f32 = 980.0;
pub const HEIGHT: f32 = 680.0;
/// The options column (`ColumnDefinitions="360,*"`), less the
/// `StackPanel`'s `Margin="0,0,10,0"`.
const OPTIONS_COLUMN: f32 = 360.0;
const OPTIONS_MARGIN: f32 = 10.0;
/// `ColumnDefinitions="115,*"`.
const LABEL_W: f32 = 115.0;
/// A toggle row: `MinHeight="30"`, the `ToggleSwitch` 32 tall.
const TOGGLE_ROW: f32 = 32.0;

/// `OutputChoices`, in the window's order.
pub const OUTPUTS: [IndexerOutput; 4] = [IndexerOutput::Html, IndexerOutput::Txt, IndexerOutput::Xml, IndexerOutput::Json];

/// An output's `Name`.
pub fn output_name(o: IndexerOutput) -> &'static str {
    match o {
        IndexerOutput::Html => "HTML",
        IndexerOutput::Txt => s::DirectoryIndexerViewModel_Text(),
        IndexerOutput::Xml => "XML",
        IndexerOutput::Json => "JSON",
    }
}

pub fn output_items() -> Vec<ComboItem> {
    OUTPUTS.iter().map(|o| ComboItem::normal(output_name(*o))).collect()
}

pub struct DirectoryIndexerWindow {
    pub folder_path: String,
    pub settings: IndexerSettings,
    /// `MaxDepthLevel` (the `NumericUpDown`'s value).
    max_depth: i64,
    pub source: String,
    pub error_message: String,
    job: Job<Result<String, String>>,
    task: TaskSettings,
    settings_changed: bool,
    /// `OnOpened` → `InitializeAsync` (the folder picker) has run.
    opened: bool,
    /// The options column's scroll offset.
    scroll: f32,
}

impl DirectoryIndexerWindow {
    /// `new DirectoryIndexerWindow(indexerSettings, upload)` over the
    /// task's `ToolsSettingsReference.IndexerSettings`, `BinaryUnits` from
    /// the application settings.
    pub fn new(task: TaskSettings, defaults: &TaskSettings, binary_units: bool) -> Self {
        let tools_settings = super::tools_settings(&task, defaults);
        let settings = IndexerSettings::from_json(&tools_settings.IndexerSettings, binary_units);
        Self {
            folder_path: String::new(),
            max_depth: settings.max_depth_level.max(0) as i64,
            settings,
            source: String::new(),
            error_message: String::new(),
            job: Job::new(0),
            task,
            settings_changed: false,
            opened: false,
            scroll: 0.0,
        }
    }

    /// No folder picker when the window opens (`--render`), work on the
    /// drawing thread.
    pub fn set_sync(&mut self) {
        self.job.sync = true;
        self.opened = true;
    }

    pub fn is_busy(&self) -> bool {
        self.job.busy
    }

    /// `CanIndex`.
    pub fn can_index(&self) -> bool {
        !self.is_busy() && tools::dir_exists(&self.folder_path)
    }

    pub fn has_source(&self) -> bool {
        !self.source.trim().is_empty()
    }

    /// `SetFolder` / a picked folder (`BrowseFolderAsync` then indexes).
    pub fn set_folder(&mut self, folder: &str, index: bool) {
        if !self.is_busy() && tools::dir_exists(folder) {
            self.folder_path = folder.to_owned();
            if index {
                self.index();
            }
        }
    }

    /// `IndexAsync`.
    pub fn index(&mut self) {
        if !self.can_index() {
            return;
        }
        self.settings_changed = true;
        self.source.clear();
        self.error_message.clear();
        self.job.queue();
    }

    /// The output selected (`OnSelectedOutputChanged`): re-indexed when it can.
    pub fn set_output(&mut self, output: IndexerOutput) {
        self.settings.output = output;
        self.settings_changed = true;
        if self.can_index() {
            self.index();
        }
    }

    /// The index made on a thread is in.
    fn poll(&mut self, ctx: &egui::Context) {
        let folder = self.folder_path.clone();
        let mut settings = self.settings.clone();
        settings.max_depth_level = self.max_depth.max(0) as i32;
        if let Some(r) = self.job.poll(ctx, move || Box::new(move || Ok(sharex_tools::indexer::index(&folder, &settings)))) {
            match r {
                Ok(source) => self.source = source,
                Err(e) => {
                    eprintln!("DirectoryIndexerViewModel: Directory indexing failed. {e}");
                    self.error_message = e;
                }
            }
        }
    }

    /// `SaveAsAsync`: the save dialog (`Index for <folder>`, the output's
    /// type), the text written as UTF-8 with a BOM; true when saved.
    fn save_as(&self) -> bool {
        let name = self.folder_path.trim_end_matches(['\\', '/']);
        let folder_name = std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let suggested = super::format(s::DirectoryIndexerViewModel_Index_for(), &[&folder_name]);
        let ext = self.settings.output.extension();
        let filter = super::format(s::DirectoryIndexerWindow_File_type(), &[&ext.to_uppercase()]);
        let Some(path) = rfd::FileDialog::new()
            .set_title(s::DirectoryIndexerWindow_Save_directory_index_dialog())
            .set_file_name(format!("{suggested}.{ext}"))
            .add_filter(filter, &[ext])
            .add_filter("All files", &["*"])
            .save_file()
        else {
            return false;
        };
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(self.source.as_bytes());
        match std::fs::write(&path, bytes) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("Save directory index: {e}");
                false
            }
        }
    }

    fn heading_h(ui: &Ui) -> f32 {
        tools::heading_height(ui)
    }

    /// The Index options panel's height.
    fn options_height(ui: &Ui) -> f32 {
        26.0 + tools::grid_height(&[Self::heading_h(ui), 40.0, 40.0, TOGGLE_ROW, TOGGLE_ROW, TOGGLE_ROW, TOGGLE_ROW * 2.0 + 8.0], 8.0)
    }

    /// The selected output's panel height.
    fn output_panel_height(&self, ui: &Ui) -> f32 {
        let h = Self::heading_h(ui);
        26.0 + match self.settings.output {
            IndexerOutput::Txt => tools::grid_height(&[h, 40.0, TOGGLE_ROW], 10.0),
            IndexerOutput::Html => tools::grid_height(&[h, TOGGLE_ROW, TOGGLE_ROW, TOGGLE_ROW, 40.0], 9.0),
            IndexerOutput::Xml | IndexerOutput::Json => tools::grid_height(&[h, TOGGLE_ROW], 9.0),
        }
    }

    /// A toggle row: the text (Normal, the text colour) and the switch at the right.
    fn toggle_row(ui: &Ui, rect: Rect, id: &str, text: &str, on: &mut bool, enabled: bool) -> bool {
        let p = palette(ui.ctx());
        let f = tools::normal_font();
        let color = if enabled { p.text } else { p.text.gamma_multiply(0.5) };
        controls::label(ui, Rect::from_min_max(rect.min, pos2(rect.max.x - ToggleSwitch::SIZE.x, rect.max.y)), text, f, color);
        let before = *on;
        ToggleSwitch { enabled }.show(ui, pos2(rect.max.x - ToggleSwitch::SIZE.x, center_y(rect, ToggleSwitch::SIZE.y)), Id::new(("indexer-toggle", id)), on);
        before != *on
    }

    /// The options column (scrolled); true when an option changed.
    fn options_column(&mut self, ui: &mut Ui, area: Rect) -> bool {
        let content_h = Self::options_height(ui) + 12.0 + self.output_panel_height(ui);
        let max_scroll = (content_h - area.height()).max(0.0);
        if ui.rect_contains_pointer(area) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let width = area.width() - OPTIONS_MARGIN;
        let clip = area.intersect(ui.clip_rect());
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
        child.set_clip_rect(clip);
        let ui = &mut child;
        let mut changed = false;
        let x = area.min.x;
        let mut y = area.min.y - self.scroll;
        let hh = Self::heading_h(ui);

        // Index options.
        let oh = Self::options_height(ui);
        let panel = Rect::from_min_size(pos2(x, y), vec2(width, oh));
        tools::panel(ui, panel);
        let inner = panel.shrink(13.0);
        let mut ry = inner.min.y;
        tools::heading(ui, Rect::from_min_size(pos2(inner.min.x, ry), vec2(inner.width(), hh)), s::DirectoryIndexerWindow_Index_options());
        ry += hh + 8.0;
        let row = Rect::from_min_size(pos2(inner.min.x, ry), vec2(inner.width(), 40.0));
        let control = tools::label_row(ui, row, LABEL_W, s::DirectoryIndexerWindow_Output_format());
        let items = output_items();
        let selected = OUTPUTS.iter().position(|o| *o == self.settings.output);
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let cw = combo.auto_width(ui, &items, selected).min(control.width());
        if let Some(i) = combo.show(ui, Rect::from_min_size(control.min, vec2(cw, 40.0)), Id::new("indexer-output"), &items, selected) {
            self.set_output(OUTPUTS[i]);
        }
        ry += 40.0 + 8.0;
        let row = Rect::from_min_size(pos2(inner.min.x, ry), vec2(inner.width(), 40.0));
        let control = tools::label_row(ui, row, LABEL_W, s::DirectoryIndexerWindow_Maximum_depth());
        if (NumericUpDown { minimum: 0, maximum: 1000, enabled: true }).show(ui, control, Id::new("indexer-depth"), &mut self.max_depth) {
            self.settings.max_depth_level = self.max_depth.max(0) as i32;
            changed = true;
        }
        ry += 40.0 + 8.0;
        let toggles: [(&str, &str, bool); 5] = [
            ("skip-folders", s::DirectoryIndexerWindow_Skip_hidden_folders(), self.settings.skip_hidden_folders),
            ("skip-files", s::DirectoryIndexerWindow_Skip_hidden_files(), self.settings.skip_hidden_files),
            ("folders-only", s::DirectoryIndexerWindow_Folders_only(), self.settings.skip_files),
            ("sizes", s::DirectoryIndexerWindow_Show_sizes(), self.settings.show_size_info),
            ("footer", s::DirectoryIndexerWindow_Add_generated_by_footer(), self.settings.add_footer),
        ];
        for (k, (id, text, mut on)) in toggles.into_iter().enumerate() {
            let row = Rect::from_min_size(pos2(inner.min.x, ry), vec2(inner.width(), TOGGLE_ROW));
            if Self::toggle_row(ui, row, id, text, &mut on, true) {
                changed = true;
                match k {
                    0 => self.settings.skip_hidden_folders = on,
                    1 => self.settings.skip_hidden_files = on,
                    2 => self.settings.skip_files = on,
                    3 => self.settings.show_size_info = on,
                    _ => self.settings.add_footer = on,
                }
            }
            ry += TOGGLE_ROW + 8.0;
        }
        y += oh + 12.0;

        // The selected output's options.
        let ph = self.output_panel_height(ui);
        let panel = Rect::from_min_size(pos2(x, y), vec2(width, ph));
        tools::panel(ui, panel);
        let inner = panel.shrink(13.0);
        let heading = match self.settings.output {
            IndexerOutput::Txt => s::DirectoryIndexerWindow_Text_output(),
            IndexerOutput::Html => s::DirectoryIndexerWindow_HTML_output(),
            IndexerOutput::Xml => s::DirectoryIndexerWindow_XML_output(),
            IndexerOutput::Json => s::DirectoryIndexerWindow_JSON_output(),
        };
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), heading);
        let row_at = |ry: f32, h: f32| Rect::from_min_size(pos2(inner.min.x, ry), vec2(inner.width(), h));
        match self.settings.output {
            IndexerOutput::Txt => {
                let mut ry = inner.min.y + hh + 10.0;
                let control = tools::label_row(ui, row_at(ry, 40.0), LABEL_W, s::DirectoryIndexerWindow_Indentation());
                let before = self.settings.indentation_text.clone();
                TextBox::default().show(ui, control, Id::new("indexer-indentation"), &mut self.settings.indentation_text);
                changed |= before != self.settings.indentation_text;
                ry += 40.0 + 10.0;
                changed |= Self::toggle_row(ui, row_at(ry, TOGGLE_ROW), "empty-line", s::DirectoryIndexerWindow_Empty_line_after_folders(), &mut self.settings.add_empty_line_after_folders, true);
            }
            IndexerOutput::Html => {
                let mut ry = inner.min.y + hh + 9.0;
                changed |= Self::toggle_row(ui, row_at(ry, TOGGLE_ROW), "display-path", s::DirectoryIndexerWindow_Display_folder_paths(), &mut self.settings.display_path, true);
                ry += TOGGLE_ROW + 9.0;
                // `CanLimitDisplayPath`.
                let can_limit = self.settings.display_path;
                changed |= Self::toggle_row(ui, row_at(ry, TOGGLE_ROW), "limit-path", s::DirectoryIndexerWindow_Limit_paths_to_root_folder(), &mut self.settings.display_path_limited, can_limit);
                ry += TOGGLE_ROW + 9.0;
                changed |= Self::toggle_row(ui, row_at(ry, TOGGLE_ROW), "custom-css", s::DirectoryIndexerWindow_Use_custom_CSS_file(), &mut self.settings.use_custom_css_file, true);
                ry += TOGGLE_ROW + 9.0;
                let row = row_at(ry, 40.0);
                let use_css = self.settings.use_custom_css_file;
                let browse = s::DirectoryIndexerWindow_Browse();
                let bw = tools::text_button_width(ui, browse);
                let br = Rect::from_min_size(pos2(row.max.x - bw, row.min.y), vec2(bw, 40.0));
                let before = self.settings.custom_css_file_path.clone();
                TextBox { placeholder: s::DirectoryIndexerWindow_Select_a_CSS_file(), enabled: use_css, ..TextBox::default() }.show(
                    ui,
                    Rect::from_min_max(row.min, pos2(br.min.x - 8.0, row.max.y)),
                    Id::new("indexer-css"),
                    &mut self.settings.custom_css_file_path,
                );
                if tools::text_button(ui, br, Id::new("indexer-css-browse"), browse, use_css).clicked()
                    && let Some(f) = rfd::FileDialog::new()
                        .set_title(s::DirectoryIndexerWindow_Select_CSS_file_dialog())
                        .add_filter(s::DirectoryIndexerWindow_CSS_files(), &["css"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                {
                    self.settings.custom_css_file_path = f.to_string_lossy().into_owned();
                }
                changed |= before != self.settings.custom_css_file_path;
            }
            IndexerOutput::Xml => {
                let ry = inner.min.y + hh + 9.0;
                changed |= Self::toggle_row(ui, row_at(ry, TOGGLE_ROW), "attributes", s::DirectoryIndexerWindow_Write_information_as_attributes(), &mut self.settings.use_attribute, true);
            }
            IndexerOutput::Json => {
                let ry = inner.min.y + hh + 9.0;
                changed |= Self::toggle_row(ui, row_at(ry, TOGGLE_ROW), "parseable", s::DirectoryIndexerWindow_Create_parseable_structured_JSON(), &mut self.settings.create_parseable_json, true);
            }
        }
        if max_scroll > 0.0 {
            crate::dialogs::layout::scroll_bar(ui, area, content_h, self.scroll, 0.0);
        }
        changed
    }

    /// The Generated output panel.
    fn output_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let inner = rect.shrink(1.0);
        let hh = Self::heading_h(ui);
        let head = Rect::from_min_size(pos2(inner.min.x + 12.0, inner.min.y + 12.0), vec2(inner.width() - 24.0, hh));
        tools::heading(ui, head, s::DirectoryIndexerWindow_Generated_output());
        if self.has_source() {
            let t = s::DirectoryIndexerWindow_Ready_to_save_or_upload();
            let f = tools::normal_font();
            let w = text_width(ui, t, f.clone());
            tools::text_at(ui, pos2(head.max.x - w, center_y(head, line_height(ui, &f))), t, f, p.accent_text);
        }
        let mut top = head.max.y + 12.0;
        if self.is_busy() {
            let bar = Rect::from_min_size(pos2(inner.min.x, top), vec2(inner.width(), 4.0));
            controls::indeterminate_progress(ui.painter(), bar, self.job.busy_since.elapsed().as_secs_f64());
            ui.ctx().request_repaint();
            top += 4.0;
        }
        let body = Rect::from_min_max(pos2(inner.min.x + 12.0, top), pos2(inner.max.x - 12.0, inner.max.y - 12.0));
        if self.has_source() && self.settings.output != IndexerOutput::Html {
            // `CanPreview`: the read-only Consolas text box, no wrapping.
            let tb = TextBox { read_only: true, multiline: true, font: Some(FontId::new(14.0, sharex_ui::fonts::consolas())), ..TextBox::default() };
            tb.show(ui, body, Id::new("indexer-source"), &mut self.source);
        } else if self.has_source() {
            super::notice(ui, body, icon::FileCode, 44.0, 390.0, s::DirectoryIndexerWindow_The_HTML_index_was_generated_successfully_HTML_preview_is_unavailable_but_the_output_is_ready_to_save_or_upload());
        } else {
            super::notice(ui, body, icon::FolderTree, 44.0, 390.0, s::DirectoryIndexerWindow_Choose_a_folder_and_generate_output_Nothing_is_saved_until_you_use_Save_as_or_Upload());
        }
    }
}

impl Dialog for DirectoryIndexerWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::DirectoryIndexerWindow_ShareX_Directory_Indexer()), WIDTH, HEIGHT, 820.0, 580.0)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        let ctx = ui.ctx().clone();
        if !self.opened {
            // `OnOpened` → `InitializeAsync` → `BrowseFolderAsync`.
            self.opened = true;
            if let Some(f) = tools::pick_folder(s::DirectoryIndexerWindow_Select_folder_to_index_dialog(), "") {
                self.set_folder(&f, true);
            }
        }
        self.poll(&ctx);
        let p = palette(ui.ctx());
        let c = tools::background(ui);
        let mut effects = Vec::new();
        let idle = !self.is_busy();

        // Folder.
        let top = Rect::from_min_size(c.min, vec2(c.width(), 26.0 + tools::BUTTON_HEIGHT));
        tools::panel(ui, top);
        let row = top.shrink(13.0);
        let hf = tools::heading_font();
        let folder = s::DirectoryIndexerWindow_Folder();
        let hw = text_width(ui, folder, hf.clone());
        tools::heading(ui, Rect::from_min_size(row.min, vec2(hw, row.height())), folder);
        let gen_text = s::DirectoryIndexerWindow_Generate_output();
        let gw = tools::icon_text_button_width(ui, gen_text);
        let gr = Rect::from_min_size(pos2(row.max.x - gw, row.min.y), vec2(gw, tools::BUTTON_HEIGHT));
        let browse = s::DirectoryIndexerWindow_Browse();
        let bw = tools::text_button_width(ui, browse);
        let br = Rect::from_min_size(pos2(gr.min.x - 10.0 - bw, row.min.y), vec2(bw, tools::BUTTON_HEIGHT));
        tools::path_text(ui, Rect::from_min_max(pos2(row.min.x + hw + 10.0, row.min.y), pos2(br.min.x - 10.0, row.max.y)), &self.folder_path);
        if tools::text_button(ui, br, Id::new("indexer-browse"), browse, idle).clicked()
            && let Some(f) = tools::pick_folder(s::DirectoryIndexerWindow_Select_folder_to_index_dialog(), "")
        {
            self.set_folder(&f, true);
        }
        if tools::icon_text_button(ui, gr, Id::new("indexer-generate"), icon::FolderSearch, gen_text, self.can_index()).clicked() {
            self.index();
        }

        // Bottom: the error, Save as..., Upload.
        let bottom = Rect::from_min_max(pos2(c.min.x, c.max.y - tools::BUTTON_HEIGHT), c.max);
        let up_text = s::DirectoryIndexerWindow_Upload();
        let uw = tools::icon_text_button_width(ui, up_text);
        let ur = Rect::from_min_size(pos2(bottom.max.x - uw, bottom.min.y), vec2(uw, tools::BUTTON_HEIGHT));
        let save_text = s::DirectoryIndexerWindow_Save_as();
        let sw = tools::icon_text_button_width(ui, save_text);
        let sr = Rect::from_min_size(pos2(ur.min.x - 8.0 - sw, bottom.min.y), vec2(sw, tools::BUTTON_HEIGHT));
        if !self.error_message.trim().is_empty() {
            let f = font(13.0, 400);
            let width = sr.min.x - 8.0 - bottom.min.x;
            let h = crate::video_tools::wrapped_text(ui, pos2(0.0, 0.0), width, &self.error_message, f.clone(), p.cancel_border, false);
            crate::video_tools::wrapped_text(ui, pos2(bottom.min.x, center_y(bottom, h)), width, &self.error_message, f, p.cancel_border, true);
        }
        let has_source = self.has_source();
        if tools::icon_text_button(ui, sr, Id::new("indexer-save"), icon::Save, save_text, has_source).clicked() && self.save_as() {
            effects.push(Effect::Close);
        }
        if tools::icon_text_button(ui, ur, Id::new("indexer-upload"), icon::Upload, up_text, has_source).clicked() {
            effects.push(Effect::Upload(Box::new(UploadRequest::IndexFolder { source: self.source.clone(), output: self.settings.output, task: self.task.clone() })));
            effects.push(Effect::Close);
        }

        // The options and the output.
        let middle = Rect::from_min_max(pos2(c.min.x, top.max.y + 12.0), pos2(c.max.x, bottom.min.y - 12.0));
        let options = Rect::from_min_size(middle.min, vec2(OPTIONS_COLUMN, middle.height()));
        if self.options_column(ui, options) {
            self.settings_changed = true;
        }
        let output = Rect::from_min_max(pos2(options.max.x + 12.0, middle.min.y), middle.max);
        self.output_panel(ui, output);

        if std::mem::take(&mut self.settings_changed) {
            let json = self.settings.to_json();
            super::save_tools_setting(cx, &mut self.task, |t| t.IndexerSettings = json.clone());
        }
        effects
    }

    fn dropped(&mut self, files: Vec<PathBuf>, _cx: &mut Cx) -> Vec<Effect> {
        // `OnDrop`: the first folder dropped (not indexed until Generate).
        if let Some(f) = files.iter().find(|f| f.is_dir()) {
            self.set_folder(&f.to_string_lossy(), false);
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_a_folder_and_keeps_the_options() {
        let dir = std::env::temp_dir().join(format!("sharexrs-indexer-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("a.txt"), b"hello").unwrap();
        std::fs::write(dir.join("nested").join("b.txt"), b"hi").unwrap();
        let defaults = TaskSettings::default();
        let task = TaskSettings::default_task_settings(&defaults);
        let mut w = DirectoryIndexerWindow::new(task, &defaults, false);
        w.set_sync();
        assert!(!w.can_index());
        w.set_output(IndexerOutput::Json);
        w.set_folder(&dir.to_string_lossy(), true);
        w.poll(&egui::Context::default());
        assert!(w.has_source(), "{}", w.error_message);
        assert!(w.source.contains("a.txt") && w.source.contains("b.txt"));
        assert_eq!(w.settings.output, IndexerOutput::Json);
        assert_eq!(w.settings.to_json()["Output"], "Json");
    }
}
