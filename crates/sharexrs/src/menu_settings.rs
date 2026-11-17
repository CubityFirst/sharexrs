//! What the After capture tasks, After upload tasks and Destinations menus
//! show of the default task settings and change in them
//! (`MainMenuBuilder.BuildAfterCaptureMenu`, `BuildAfterUploadMenu`,
//! `BuildDestinationsMenu` with `ApplicationState.DefaultTaskSettings`).

use sharex_settings::{AfterCaptureTasks, AfterUploadTasks, FileDestination, ImageDestination, TaskSettings, TextDestination, URLSharingServices, UrlShortenerType};

use crate::main_menu::{MenuCommand, MenuState};

/// The `'static` name of an enum value, as `ALL` lists it (`fallback` for
/// a value ShareX doesn't know).
macro_rules! static_name {
    ($ty:ty, $value:expr, $fallback:expr) => {
        <$ty>::ALL.iter().find(|v| **v == $value).map(|v| v.name()).unwrap_or($fallback)
    };
}

/// The flags set in `value`, by their `'static` names.
fn flag_names(value: u64, names: &'static [(u64, &'static str)]) -> Vec<&'static str> {
    names.iter().filter(|(v, _)| *v != 0 && value & v == *v).map(|(_, n)| *n).collect()
}

/// The menus' state from the default task settings.
pub fn fill(m: &mut MenuState, t: &TaskSettings) {
    m.image_destination = static_name!(ImageDestination, t.ImageDestination, "Imgur");
    m.image_file_destination = static_name!(FileDestination, t.ImageFileDestination, "Dropbox");
    m.text_destination = static_name!(TextDestination, t.TextDestination, "Pastebin");
    m.text_file_destination = static_name!(FileDestination, t.TextFileDestination, "Dropbox");
    m.file_destination = static_name!(FileDestination, t.FileDestination, "Dropbox");
    m.url_shortener = static_name!(UrlShortenerType, t.URLShortenerDestination, "BITLY");
    m.url_sharing_service = static_name!(URLSharingServices, t.URLSharingServiceDestination, "Email");
    m.after_capture = flag_names(t.AfterCaptureJob.0 as u64, AfterCaptureTasks::NAMES);
    m.after_upload = flag_names(t.AfterUploadJob.0 as u64, AfterUploadTasks::NAMES);
    let image = t.ImageSettings.clone().unwrap_or_default();
    m.image_effect_presets = image
        .ImageEffectPresets
        .iter()
        .map(|p| p.get("Name").and_then(serde_json::Value::as_str).filter(|n| !n.is_empty()).unwrap_or("Name").to_owned())
        .collect();
    m.selected_image_effect_preset = image.SelectedImageEffectPreset.max(0) as usize;
}

fn by_name<T: Clone>(all: &[T], name: &str, name_of: fn(&T) -> &str) -> Option<T> {
    all.iter().find(|v| name_of(v) == name).cloned()
}

/// A menu entry's change to the default task settings; false for other
/// commands.
pub fn apply(t: &mut TaskSettings, command: &MenuCommand) -> bool {
    match command {
        MenuCommand::SetImageDestination(n) => {
            if let Some(v) = by_name(ImageDestination::ALL, n, |v| v.name()) {
                t.ImageDestination = v;
            }
        }
        MenuCommand::SetImageFileDestination(n) => {
            if let Some(v) = by_name(FileDestination::ALL, n, |v| v.name()) {
                t.ImageDestination = ImageDestination::FileUploader;
                t.ImageFileDestination = v;
            }
        }
        MenuCommand::SetTextDestination(n) => {
            if let Some(v) = by_name(TextDestination::ALL, n, |v| v.name()) {
                t.TextDestination = v;
            }
        }
        MenuCommand::SetTextFileDestination(n) => {
            if let Some(v) = by_name(FileDestination::ALL, n, |v| v.name()) {
                t.TextDestination = TextDestination::FileUploader;
                t.TextFileDestination = v;
            }
        }
        MenuCommand::SetFileDestination(n) => {
            if let Some(v) = by_name(FileDestination::ALL, n, |v| v.name()) {
                t.FileDestination = v;
            }
        }
        MenuCommand::SetUrlShortener(n) => {
            if let Some(v) = by_name(UrlShortenerType::ALL, n, |v| v.name()) {
                t.URLShortenerDestination = v;
            }
        }
        MenuCommand::SetUrlSharingService(n) => {
            if let Some(v) = by_name(URLSharingServices::ALL, n, |v| v.name()) {
                t.URLSharingServiceDestination = v;
            }
        }
        // `MainMenuBuilder.ToggleAfterCaptureTask` / `ToggleAfterUploadTask`.
        MenuCommand::ToggleAfterCaptureTask(n) => {
            if let Some((v, _)) = AfterCaptureTasks::NAMES.iter().find(|(_, name)| name == n) {
                let flag = AfterCaptureTasks(*v as _);
                if t.AfterCaptureJob.contains(flag) { t.AfterCaptureJob.remove(flag) } else { t.AfterCaptureJob.insert(flag) }
            }
        }
        MenuCommand::ToggleAfterUploadTask(n) => {
            if let Some((v, _)) = AfterUploadTasks::NAMES.iter().find(|(_, name)| name == n) {
                let flag = AfterUploadTasks(*v as _);
                if t.AfterUploadJob.contains(flag) { t.AfterUploadJob.remove(flag) } else { t.AfterUploadJob.insert(flag) }
            }
        }
        MenuCommand::SelectImageEffectPreset(i) => {
            t.ImageSettings.get_or_insert_default().SelectedImageEffectPreset = *i as i32;
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_follow_and_change_the_task_settings() {
        let mut t = TaskSettings::default();
        let mut m = MenuState::default();
        assert!(apply(&mut t, &MenuCommand::SetImageFileDestination("Uguu")));
        assert!(apply(&mut t, &MenuCommand::SetUrlShortener("ISGD")));
        assert!(apply(&mut t, &MenuCommand::ToggleAfterUploadTask("ShowQRCode")));
        assert!(apply(&mut t, &MenuCommand::ToggleAfterCaptureTask("SaveImageToFile")));
        fill(&mut m, &t);
        assert_eq!((m.image_destination, m.image_file_destination, m.url_shortener), ("FileUploader", "Uguu", "ISGD"));
        assert_eq!(m.after_upload, ["CopyURLToClipboard", "ShowQRCode"]);
        assert_eq!(m.after_capture, ["CopyImageToClipboard"]);
    }
}
