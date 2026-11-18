//! `ShareX.Tools/Tools/OCR/OCRWindow` (stub, being ported).

use crate::dialogs::render::Options;
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub struct OcrWindow {}

impl Dialog for OcrWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(sharex_strings::tools::OCRWindow_ShareX_Optical_character_recognition()), 1100.0, 680.0, 820.0, 520.0)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let _ = ui;
        Vec::new()
    }
}

/// The `--render` screens of the OCR window.
pub fn is_render_screen(name: &str) -> bool {
    name.starts_with("ocr")
}

pub fn render_dialog(_name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let _ = o;
    Ok(Box::new(OcrWindow {}))
}
