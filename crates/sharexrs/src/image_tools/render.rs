//! `sharexrs --render <screen>` for the Images tool windows, set up as
//! `scripts/image-tools-shots.ps1` leaves ShareX's:
//!
//! - `image-combiner` (`--files a,b,c`, `--select N`, `--state horizontal`),
//! - `image-splitter` (`--image <file>`, `--state 3x2` for columns × rows),
//! - `image-thumbnailer`, `image-resizer` (`--state jpeg`),
//! - `image-converter` (`--state png|jpeg|webp|gif|bmp`),
//! - `image-watermark` (`--state image --image <watermark>`),
//! - `image-comparer` (`--files a,b`, `--state diff`),
//! - `icon-converter` (`--image <file>`).
//!
//! Previews are made at once, on the drawing thread.

use sharex_settings::TaskSettings;

use super::{combiner, comparer, converter, icon_converter, resizer, splitter, thumbnailer, watermark};
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "image-combiner" | "image-splitter" | "image-thumbnailer" | "image-resizer" | "image-converter" | "image-watermark" | "image-comparer" | "icon-converter")
}

fn files(o: &Options) -> Vec<String> {
    o.files.iter().map(|f| f.to_string_lossy().into_owned()).collect()
}

pub fn render_dialog(name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let state = o.state.as_deref().unwrap_or("");
    Ok(match name {
        "image-combiner" => {
            let defaults = TaskSettings::default();
            let task = TaskSettings::default_task_settings(&defaults);
            let mut w = combiner::ImageCombinerWindow::new(task, &defaults, Vec::new());
            w.set_sync();
            if state == "horizontal" {
                // As clicking Horizontal leaves it (the alignment combo blank).
                w.set_horizontal();
            }
            w.add_files(files(o));
            if let Some(&i) = o.select.first() {
                w.list.select(Some(i));
            }
            Box::new(w)
        }
        "image-splitter" => {
            let mut w = splitter::ImageSplitterWindow::default();
            if let Some(p) = &o.image {
                w.set_image_file(&p.to_string_lossy());
            }
            if let Some((c, r)) = state.split_once('x') {
                w.column_count = c.parse().map_err(|_| "--state CxR")?;
                w.row_count = r.parse().map_err(|_| "--state CxR")?;
            }
            Box::new(w)
        }
        "image-thumbnailer" => {
            let mut w = thumbnailer::ImageThumbnailerWindow::default();
            w.add_files(files(o));
            Box::new(w)
        }
        "image-resizer" => {
            let mut w = resizer::ImageResizerWindow::default();
            w.set_sync();
            if state == "jpeg" {
                w.format = sharex_tools::resizer::OutputFormat::Jpeg;
            }
            w.add_files(files(o));
            Box::new(w)
        }
        "image-converter" => {
            let mut w = converter::ImageConverterWindow::default();
            w.set_sync();
            w.format = match state {
                "" | "png" => 0,
                "jpeg" => 1,
                "webp" => 2,
                "gif" => 3,
                "bmp" => 4,
                other => return Err(format!("no converter state {other}")),
            };
            w.add_files(files(o));
            Box::new(w)
        }
        "image-watermark" => {
            let mut w = watermark::ImageWatermarkWindow::default();
            w.set_sync();
            if state == "image" {
                w.kind = sharex_tools::watermark::WatermarkType::Image;
                if let Some(p) = &o.image {
                    w.image_path = p.to_string_lossy().into_owned();
                }
            }
            w.add_files(files(o));
            Box::new(w)
        }
        "image-comparer" => {
            let mut w = comparer::ImageComparerWindow::default();
            for (k, f) in files(o).iter().take(2).enumerate() {
                w.set_image(k, f);
            }
            w.diff_mode = state == "diff";
            Box::new(w)
        }
        "icon-converter" => {
            let mut w = icon_converter::IconConverterWindow::default();
            if let Some(p) = &o.image {
                w.load_image(&p.to_string_lossy());
            }
            Box::new(w)
        }
        other => return Err(format!("no tool window {other}")),
    })
}
