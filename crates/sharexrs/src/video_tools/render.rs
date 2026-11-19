//! `sharexrs --render <screen>` for the Video tool windows, in the states
//! of `tools-video` in the reference shot manifest:
//!
//! - `gif-maker` (`--files a,b,c`, `--select N`, `--state repeat-count`
//!   (Loop on, repeat count 3) or `created` (the GIF written to `--text`)).
//!
//! - `video-converter` (`--image <input>`, `--state codec-no-rate-control`
//!   (NVENC), `bitrate`, `encoding` (at `--count` percent), `result` (a
//!   real conversion with FFmpeg on the PATH or `SHAREXRS_FFMPEG`), and
//!   `--content <codec index>`).
//!
//! - `video-thumbnailer` (`--image <video>`, `--state custom-output`,
//!   `progress` (at `--count` of 9), `results` (a real run into `--text`,
//!   uploads off), `invalid` (a run on a file FFmpeg can't read)).
//!
//! Previews are made at once, on the drawing thread.

use sharex_settings::TaskSettings;
use sharex_tools::video::converter::Codec;

use super::{converter, gif_maker, thumbnailer};
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "gif-maker" | "video-converter" | "video-thumbnailer")
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
        "video-converter" => {
            let defaults = TaskSettings::default();
            let task = TaskSettings::default_task_settings(&defaults);
            let mut w = converter::VideoConverterWindow::new(task, &defaults, ffmpeg_path(), o.image.as_ref().map(|p| p.to_string_lossy().into_owned()));
            if let Some(i) = o.content.as_deref().and_then(|c| c.parse::<usize>().ok()) {
                w.codec = *Codec::ALL.get(i).ok_or("--content <codec index>")?;
            }
            match state {
                "" | "empty" | "input" | "codec-menu" | "rate-control-menu" => {}
                "codec-no-rate-control" => w.codec = Codec::H264Nvenc,
                "bitrate" => w.use_bitrate = true,
                "encoding" => w.set_encoding(o.count as f64),
                "result" => {
                    let mut settings = sharex_settings::ApplicationConfig::default();
                    let store = crate::upload::UploadersConfigStore::default();
                    let mut cx = crate::dialogs::Cx { settings: &mut settings, uploaders: &store };
                    w.convert_now(&mut cx);
                }
                other => return Err(format!("no video-converter state {other}")),
            }
            Box::new(w)
        }
        "video-thumbnailer" => {
            let defaults = TaskSettings::default();
            let task = TaskSettings::default_task_settings(&defaults);
            let mut w = thumbnailer::VideoThumbnailerWindow::new(task, &defaults, ffmpeg_path().unwrap_or_default(), o.text.clone().unwrap_or_default());
            if let Some(p) = &o.image {
                w.load_video(&p.to_string_lossy());
            }
            match state {
                "" | "empty" | "input" | "format-menu" | "output-location-menu" => {}
                "custom-output" => {
                    w.options.output_location = sharex_tools::video::thumbnailer::OutputLocation::CustomFolder;
                    w.options.custom_output_directory = o.text.clone().unwrap_or_default();
                }
                "progress" => w.set_busy(o.count as i32),
                "results" | "invalid" => {
                    w.options.upload_thumbnails = false;
                    w.options.output_location = sharex_tools::video::thumbnailer::OutputLocation::CustomFolder;
                    w.options.custom_output_directory = o.text.clone().ok_or("--text <output folder> is required")?;
                    w.run_now();
                }
                other => return Err(format!("no video-thumbnailer state {other}")),
            }
            Box::new(w)
        }
        other => return Err(format!("no video tool window {other}")),
    })
}

/// FFmpeg for the renders that run it: `SHAREXRS_FFMPEG`, else the
/// platform's usual places.
fn ffmpeg_path() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("SHAREXRS_FFMPEG") {
        return Some(p.into());
    }
    sharex_screen_recording::discovery::fallback_locations(&std::env::var_os("PATH").unwrap_or_default()).into_iter().find(|p| p.is_file())
}
