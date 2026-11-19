//! `sharexrs --render <screen>` for the Video tool windows, in the states
//! of `tools-video` in the reference shot manifest:
//!
//! - `gif-maker` (`--files a,b,c`, `--select N`, `--state repeat-count`
//!   (Loop on, repeat count 3) or `created` (the GIF written to `--text`)).
//!
//! Previews are made at once, on the drawing thread.

use super::gif_maker;
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "gif-maker")
}

fn files(o: &Options) -> Vec<String> {
    o.files.iter().map(|f| f.to_string_lossy().into_owned()).collect()
}

pub fn render_dialog(name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let state = o.state.as_deref().unwrap_or("");
    Ok(match name {
        "gif-maker" => {
            let mut w = gif_maker::AnimatedGifMakerWindow::default();
            w.set_sync();
            w.add_files(files(o));
            if let Some(&i) = o.select.first() {
                w.list.select(Some(i));
            }
            match state {
                "" | "frames" | "selected" | "empty" => {}
                "repeat-count" => w.repeat_count = o.count as i64,
                "created" => {
                    let out = o.text.clone().ok_or("--text <output.gif> is required")?;
                    w.start_create(out);
                    w.finish_create(&egui::Context::default());
                }
                other => return Err(format!("no gif-maker state {other}")),
            }
            Box::new(w)
        }
        other => return Err(format!("no video tool window {other}")),
    })
}
