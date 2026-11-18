//! `sharexrs --render <screen>` for the Screen tools' windows, set up as
//! ShareX's were captured:
//!
//! - `color-picker` (`--state standard`: Standard colors chosen;
//!   `--state swatch`: then its seventh swatch clicked, red the previous).

use super::color_picker::ColorPickerWindow;
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "color-picker")
}

pub fn render_dialog(name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let state = o.state.as_deref().unwrap_or("");
    Ok(match name {
        "color-picker" => {
            let mut w = ColorPickerWindow::new(state.is_empty(), Vec::new());
            w.skip_clipboard();
            if state == "swatch" {
                w.choose(sharex_tools::color::Color::rgb(76, 255, 0));
            }
            Box::new(w)
        }
        _ => return Err(format!("unknown screen {name}")),
    })
}
