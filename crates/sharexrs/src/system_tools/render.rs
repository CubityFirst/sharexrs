//! `sharexrs --render <screen>` for the System tool windows.

use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(_name: &str) -> bool {
    false
}

pub fn render_dialog(name: &str, _o: &Options) -> Result<Box<dyn Dialog>, String> {
    Err(format!("no system tool window {name}"))
}
