//! `ShareX.ImageEffectsLib/Presentation/ImageEffectPackagerWindow`: the
//! assets folder (`<ImageEffects>/<name>`, which must be inside the
//! ImageEffects folder) and the package file (`<ImageEffects>/<name>.sxie`)
//! with their pickers, Overwrite existing, the status line, and Open effects
//! folder / Create package / Close (`ImageEffectPackager.Package`).

use std::path::PathBuf;

use egui::{Id, Rect, Ui, pos2, vec2};
use sharex_strings::image_effects_lib as s;
use sharex_ui::controls::{self, TextBox, ToggleSwitch};
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, line_height};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 680.0;

pub struct PackagerWindow {
    json: String,
    effects_folder: PathBuf,
    pub assets_folder: String,
    pub package_file: String,
    pub overwrite: bool,
    pub status: String,
}

impl PackagerWindow {
    pub fn new(json: String, name: &str, effects_folder: PathBuf) -> Self {
        let assets_folder = effects_folder.join(name).to_string_lossy().into_owned();
        let package_file = effects_folder.join(format!("{name}.sxie")).to_string_lossy().into_owned();
        Self { json, effects_folder, assets_folder, package_file, overwrite: false, status: String::new() }
    }

    fn status_height(&self, ui: &Ui) -> f32 {
        controls::wrapped(ui, pos2(0.0, 0.0), WIDTH - 32.0, &self.status, font(13.0, 400), palette(ui.ctx()).cancel_border, false)
    }

    fn panel_height(ui: &Ui) -> f32 {
        2.0 + 24.0 + tools::heading_height(ui) + 10.0 + 40.0 + 10.0 + 40.0 + 10.0 + 32.0
    }

    /// `OnPackageClick`.
    fn package(&mut self) -> Vec<Effect> {
        let assets = self.assets_folder.trim().to_string();
        let output = self.package_file.trim().to_string();
        let folder = self.effects_folder.to_string_lossy().into_owned();
        let prefix = format!("{folder}{}", std::path::MAIN_SEPARATOR);
        if !assets.is_empty() && !assets.to_lowercase().starts_with(&prefix.to_lowercase()) {
            self.status = s::ImageEffectPackagerWindow_Assets_must_be_inside().into();
            return Vec::new();
        }
        if output.trim().is_empty() {
            self.status = s::ImageEffectPackagerWindow_Choose_package_path().into();
            return Vec::new();
        }
        if std::path::Path::new(&output).exists() && !self.overwrite {
            self.status = s::ImageEffectPackagerWindow_Already_exists().into();
            return Vec::new();
        }
        let assets_path = (!assets.is_empty()).then(|| PathBuf::from(&assets));
        match sharex_image_effects::package::package(std::path::Path::new(&output), &self.json, assets_path.as_deref()) {
            Ok(result) if result.exists() => {
                self.status = s::ImageEffectPackagerWindow_Created_successfully().into();
                vec![Effect::OpenFolderWithFile(result)]
            }
            Ok(_) => Vec::new(),
            Err(e) => {
                self.status = e.to_string();
                Vec::new()
            }
        }
    }
}

impl Dialog for PackagerWindow {
    fn spec(&self) -> Spec {
        Spec::fit_height(crate::rs_title(s::ImageEffectPackagerWindow_Title()), WIDTH, WIDTH)
    }

    fn content_height(&mut self, ui: &mut Ui) -> f32 {
        16.0 + Self::panel_height(ui) + 12.0 + self.status_height(ui) + 12.0 + tools::BUTTON_HEIGHT + 16.0
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let c = tools::background(ui);
        let ph = Self::panel_height(ui);
        let panel = Rect::from_min_size(c.min, vec2(c.width(), ph));
        tools::panel(ui, panel);
        let inner = panel.shrink(13.0);
        let hh = tools::heading_height(ui);
        tools::heading(ui, Rect::from_min_size(inner.min, vec2(inner.width(), hh)), s::ImageEffectPackagerWindow_Package_preset());
        let mut y = inner.min.y + hh + 10.0;
        let browse = s::ImageEffectPackagerWindow_Browse();
        let bw = tools::text_button_width(ui, browse);
        for k in 0..2 {
            let row = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner.width(), 40.0));
            let label = if k == 0 { s::ImageEffectPackagerWindow_Assets_folder() } else { s::ImageEffectPackagerWindow_Package_file() };
            tools::row_label(ui, Rect::from_min_size(row.min, vec2(110.0, 40.0)), label);
            let br = Rect::from_min_size(pos2(row.max.x - bw, row.min.y), vec2(bw, 40.0));
            let tr = Rect::from_min_max(pos2(row.min.x + 118.0, row.min.y), pos2(br.min.x - 8.0, row.max.y));
            let text = if k == 0 { &mut self.assets_folder } else { &mut self.package_file };
            TextBox::default().show(ui, tr, Id::new(("packager-text", k)), text);
            if tools::text_button(ui, br, Id::new(("packager-browse", k)), browse, true).clicked() {
                if k == 0 {
                    if let Some(f) = tools::pick_folder(s::ImageEffectPackagerWindow_Select_assets_folder(), "") {
                        self.assets_folder = f;
                    }
                } else {
                    let current = std::path::Path::new(&self.package_file);
                    let mut d = rfd::FileDialog::new().set_title(s::ImageEffectPackagerWindow_Save_package()).add_filter(s::ImageEffectPackagerWindow_File_type(), &["sxie"]);
                    if let Some(name) = current.file_name() {
                        d = d.set_file_name(name.to_string_lossy());
                    }
                    if let Some(f) = d.save_file() {
                        self.package_file = f.to_string_lossy().into_owned();
                    }
                }
            }
            y += 50.0;
        }
        let row = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner.width(), 32.0));
        let nf = font(13.0, 400);
        tools::text_at(ui, pos2(row.min.x, center_y(row, line_height(ui, &nf))), s::ImageEffectPackagerWindow_Overwrite_existing(), nf.clone(), p.text_secondary);
        ToggleSwitch { enabled: true }.show(ui, pos2(row.max.x - ToggleSwitch::SIZE.x, row.min.y), Id::new("packager-overwrite"), &mut self.overwrite);
        let sy = panel.max.y + 12.0;
        controls::wrapped(ui, pos2(c.min.x, sy), c.width(), &self.status, nf, p.cancel_border, true);
        let by = sy + self.status_height(ui) + 12.0;
        let open = s::ImageEffectPackagerWindow_Open_effects_folder();
        let create = s::ImageEffectPackagerWindow_Create_package();
        let close = s::ImageEffectPackagerWindow_Close();
        let ow = tools::text_button_width(ui, open);
        let (cw, xw) = (tools::text_button_width(ui, create), tools::text_button_width(ui, close));
        let mut effects = Vec::new();
        if tools::text_button(ui, Rect::from_min_size(pos2(c.min.x, by), vec2(ow, 40.0)), Id::new("packager-open"), open, true).clicked() {
            let _ = std::fs::create_dir_all(&self.effects_folder);
            // `FileHelpers.OpenFolder(_effectsFolder)`.
            crate::shell::open_folder(&self.effects_folder);
        }
        let xr = Rect::from_min_size(pos2(c.max.x - xw, by), vec2(xw, 40.0));
        let cr = Rect::from_min_size(pos2(xr.min.x - 8.0 - cw, by), vec2(cw, 40.0));
        if tools::text_button(ui, cr, Id::new("packager-create"), create, true).clicked() {
            effects = self.package();
        }
        if tools::text_button(ui, xr, Id::new("packager-close"), close, true).clicked() {
            effects.push(Effect::Close);
        }
        effects
    }
}
