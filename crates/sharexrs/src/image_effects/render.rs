//! `sharexrs --render` screens of the Image effects windows, on the default
//! preset (as the reference profile has it):
//!
//! - `image-effects` (`--image <file>`: Tool mode on it; else Presets mode
//!   on the sample image; `--select N`: the N-th effect; `--state
//!   add-menu|editor`; `--content add:<Type>`: that effect added, as Add
//!   effect... does, after the selected one),
//! - `image-effects-confirm`, `image-effects-packager`.

use std::path::PathBuf;

use sharex_image_effects::preset::{ImageEffectPreset, TypeNames};
use sharex_settings::TaskSettings;

use super::{ImageEffectsWindow, Mode, Target};
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "image-effects" | "image-effects-confirm" | "image-effects-packager")
}

pub fn render_dialog(name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let folder = PathBuf::from(r"C:\ShareXrs\ImageEffects");
    Ok(match name {
        "image-effects" => {
            let presets = vec![ImageEffectPreset::default_preset().to_json(TypeNames::Full)];
            let (image, path, mode) = match &o.image {
                Some(p) => (Some(image::open(p).map_err(|e| format!("{}: {e}", p.display()))?.to_rgba8()), Some(p.to_string_lossy().into_owned()), Mode::Tool),
                None => (None, None, if o.state.as_deref() == Some("editor") { Mode::Editor } else { Mode::Presets }),
            };
            let mut w = ImageEffectsWindow::new(image, &presets, 0, mode, path, folder, Target::Task, TaskSettings::default());
            w.set_sync();
            if let Some(&i) = o.select.first() {
                w.selected_effect = Some(i);
            }
            if let Some(ty) = o.content.as_deref().and_then(|c| c.strip_prefix("add:")) {
                let kind = sharex_image_effects::effects::EffectKind::create(ty, &mut sharex_image_effects::random::RandomFast::new()).ok_or_else(|| format!("no effect {ty}"))?;
                w.add_effect(sharex_image_effects::preset::ImageEffect::new(kind));
            }
            if o.state.as_deref() == Some("add-menu") {
                w.open_add_menu();
            }
            Box::new(w)
        }
        "image-effects-confirm" => {
            let (tx, _rx) = std::sync::mpsc::channel();
            Box::new(super::confirm::ConfirmWindow::new(sharex_strings::image_effects_lib::ImageEffectsWindow_Clear_confirmation(), tx))
        }
        "image-effects-packager" => {
            let json = ImageEffectPreset::default_preset().to_json_string(TypeNames::Short);
            Box::new(super::packager::PackagerWindow::new(json, o.text.as_deref().unwrap_or("Preset"), folder))
        }
        other => return Err(format!("no image effects screen {other}")),
    })
}
