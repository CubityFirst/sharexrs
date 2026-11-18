//! `sharexrs --render <screen>` for the Screen tools' windows, set up as
//! ShareX's were captured:
//!
//! - `color-picker` (`--state standard`: Standard colors chosen;
//!   `--state swatch`: then its seventh swatch clicked, red the previous),
//! - `pin-startup` (`--state error`: the clipboard had no image),
//! - `pin-options` (the default options),
//! - `mouse-highlighter` (`--state circle`: Circle highlight).

use super::color_picker::ColorPickerWindow;
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "color-picker" | "pin-startup" | "pin-options" | "mouse-highlighter")
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
        "pin-startup" => {
            let mut w = super::pin::PinStartupWindow::new();
            if state == "error" {
                w.failed(super::pin::Source::Clipboard);
            }
            Box::new(w)
        }
        "pin-options" => Box::new(super::pin::PinOptionsWindow::new(winit::window::WindowId::dummy(), Default::default())),
        "mouse-highlighter" => {
            let mut o = sharex_tools::mouse_highlighter::MouseHighlighterOptions::default();
            if state == "circle" {
                o.mode = sharex_tools::mouse_highlighter::Mode::Circle;
            }
            Box::new(super::highlighter::HighlighterWindow::new(o))
        }
        _ => return Err(format!("unknown screen {name}")),
    })
}
