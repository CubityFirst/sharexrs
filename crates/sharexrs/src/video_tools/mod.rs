//! ShareX.Tools' Video windows (docs/TOOLS.md, "The Video windows in
//! ShareXrs"): the animated GIF maker and trimmer, the video converter,
//! player, thumbnailer and trimmer, each a [`Dialog`](crate::dialogs::Dialog)
//! over its engine in `sharex_tools::video`. They share the Images windows'
//! blocks (`crate::image_tools`); what's theirs is here.

pub mod converter;
pub mod gif_maker;
pub mod gif_trimmer;
pub mod media;
pub mod open;
pub mod player;
pub mod render;
pub mod thumbnailer;
pub mod timeline;
pub mod trimmer;

use egui::{Color32, Rect, Ui, pos2};

use crate::image_tools::Picture;

/// The video types the player's and trimmer's Open offer.
pub const VIDEO_PATTERNS: [&str; 10] = ["mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv", "ts", "mts", "m2ts"];

/// FFmpeg for playback: where the task's settings say, else the usual
/// places (`resolve_ffmpeg`), without ShareX's missing-FFmpeg message
/// (ShareX plays through Media Foundation, which needs no FFmpeg).
pub fn playback_ffmpeg(task: &sharex_settings::TaskSettings) -> Option<std::path::PathBuf> {
    let capture = task.CaptureSettings.clone().unwrap_or_default();
    sharex_screen_recording::discovery::resolve_ffmpeg(&capture.FFmpegOptions).ok()
}

/// `SaveFilePickerAsync` with a suggested name, start folder and one file
/// type (`ShowOverwritePrompt`).
pub fn save_file(title: &str, name: &str, folder: &str, filter: &str, extensions: &[&str]) -> Option<String> {
    let mut d = rfd::FileDialog::new().set_title(title).set_file_name(name).add_filter(filter, extensions);
    if !folder.trim().is_empty() && std::path::Path::new(folder).is_dir() {
        d = d.set_directory(folder);
    }
    d.save_file().map(|p| p.to_string_lossy().into_owned())
}

/// An `Image` with `Stretch="Uniform" StretchDirection="DownOnly"` filling
/// `area`: the picture shrunk to fit (never enlarged), centred on whole
/// pixels, drawn with `HighQuality` interpolation (Skia's linear filter
/// with mipmaps when shrinking).
pub fn draw_image(ui: &Ui, area: Rect, pic: &mut Picture) -> Rect {
    let dest = crate::image_tools::slider::image_bounds(area, pic.size(), crate::image_tools::slider::Stretch::DownOnly);
    let size = (dest.width().round().max(1.0) as u32, dest.height().round().max(1.0) as u32);
    let tex = pic.texture(ui.ctx(), size);
    ui.painter().image(tex, dest, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    dest
}

/// `TaskHelpers.PlayNotificationSoundAsync(NotificationSound.ActionCompleted,
/// taskSettings)`: the action sound, unless the task turns it off.
pub fn action_sound(task: &sharex_settings::TaskSettings) {
    if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction) {
        crate::sound::play_action_completed();
    }
}
