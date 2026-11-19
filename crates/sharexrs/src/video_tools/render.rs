//! `sharexrs --render <screen>` for the Video tool windows, in the states
//! of `tools-video` in the reference shot manifest:
//!
//! - `gif-maker` (`--files a,b,c`, `--select N`, `--state repeat-count`
//!   (Loop on, repeat count 3) or `created` (the GIF written to `--text`)).
//!
//! - `video-converter` (`--image <input>`, `--state codec-no-rate-control`
//!   (NVENC), `bitrate`, `encoding` (at `--count` percent), `result` (the
//!   existing output probed with FFmpeg from `SHAREXRS_FFMPEG` or the
//!   PATH), and `--content <codec index>`).
//!
//! - `video-thumbnailer` (`--image <video>`, `--state custom-output`,
//!   `progress` (at `--count` of 9), `results` (a real run into `--text`,
//!   uploads off), `invalid` (a run on a file FFmpeg can't read)).
//!
//! - `gif-trimmer` (`--image <gif>`, `--state trimmed` (Next frame, Set start here),
//!   `crop` (a crop of the middle half), `resize-reencode` (half size),
//!   `exporting` (at `--count` percent), `saved` (written to `--text`),
//!   `invalid`; `--select N` steps to frame N).
//!
//! - `video-player` (`--image <video>`, paused at `--count` ms; `--state
//!   paused`, `controls-hidden` (also what a client-area capture of ShareX
//!   shows: its controls are a popup window of their own), `muted`,
//!   `invalid`).
//!
//! - `video-trimmer` (`--image <video>`, `--state paused` (at `--count`
//!   seconds), `trimmed-copy` (the start at `--count` ms), `reencode`
//!   (that, Re-encode), `exporting` (40%), `saved` (a real copy trim to
//!   `--text`), `invalid`).
//!
//! Previews are made at once, on the drawing thread.

use sharex_settings::TaskSettings;
use sharex_tools::video::converter::Codec;

use super::{converter, gif_maker, gif_trimmer, player, thumbnailer, trimmer};
use crate::dialogs::Dialog;
use crate::dialogs::render::Options;

pub fn is_render_screen(name: &str) -> bool {
    matches!(name, "gif-maker" | "gif-trimmer" | "video-converter" | "video-player" | "video-thumbnailer" | "video-trimmer")
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
            // The reference profile has the folder opening turned off.
            w.auto_open_folder = false;
            match state {
                "" | "empty" | "input" | "codec-menu" | "rate-control-menu" => {}
                "codec-no-rate-control" => w.codec = Codec::H264Nvenc,
                "bitrate" => w.use_bitrate = true,
                "encoding" => w.set_encoding(o.count as f64),
                // The output a conversion left (probed, not written again).
                "result" => w.show_result(),
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
            // The reference profile has uploads off (never on in a render).
            w.options.upload_thumbnails = false;
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
        "gif-trimmer" => {
            let mut w = gif_trimmer::AnimatedGifTrimmerWindow::new(TaskSettings::default(), None);
            w.set_sync();
            if let Some(p) = &o.image {
                w.load_input(&p.to_string_lossy());
                w.poll(&egui::Context::default());
            }
            w.render_state(state, o.select.first().copied(), o.count as f64, o.text.as_deref())?;
            Box::new(w)
        }
        "video-player" => {
            let mut w = player::VideoPlayerWindow::new(ffmpeg_path(), None);
            w.player.sync = true;
            w.force_controls = Some(state != "controls-hidden");
            if let Some(p) = &o.image {
                w.load_input(&p.to_string_lossy());
                w.poll(&egui::Context::default());
                // Paused where the shot is (`--count` milliseconds).
                w.player.pause();
                if o.count > 0 {
                    w.player.seek(o.count as f64 / 1000.0);
                }
                w.position = w.player.position();
            }
            match state {
                "" | "empty" | "paused" | "controls-hidden" | "invalid" | "seek-pressed" | "fullscreen-controls" => {}
                "muted" => w.muted = true,
                other => return Err(format!("no video-player state {other}")),
            }
            Box::new(w)
        }
        "video-trimmer" => {
            let ffmpeg = ffmpeg_path();
            let mut w = trimmer::VideoTrimmerWindow::new(TaskSettings::default(), ffmpeg.clone(), None);
            w.player.sync = true;
            if let Some(p) = &o.image {
                w.load_input(&p.to_string_lossy());
                w.poll(&egui::Context::default());
            }
            // The shots' route: Next frame, Set start here. Media Foundation's
            // step left the playhead at `--count` ms (249 in the shots).
            let at = o.count as f64 / 1000.0;
            let trimmed = |w: &mut trimmer::VideoTrimmerWindow| {
                w.start = at.min(w.duration);
                w.player.seek(at);
                w.position = at;
            };
            match state {
                "" | "empty" | "invalid" | "timeline-drag" => {}
                "paused" => {
                    w.player.seek(o.count as f64);
                    w.position = w.player.position();
                }
                "trimmed-copy" => trimmed(&mut w),
                "reencode" => {
                    trimmed(&mut w);
                    w.precise = true;
                }
                "exporting" => {
                    trimmed(&mut w);
                    w.set_exporting(40.0);
                }
                "saved" => {
                    trimmed(&mut w);
                    let out = o.text.clone().ok_or("--text <output> is required")?;
                    w.start_export(ffmpeg.ok_or("no FFmpeg")?, out);
                    w.finish_export();
                }
                other => return Err(format!("no video-trimmer state {other}")),
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
