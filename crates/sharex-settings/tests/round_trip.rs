//! Settings files written by ShareX (the reference build, `fixtures/`)
//! survive a load and save byte for byte, and new settings are written the
//! way a fresh ShareX profile is.
//!
//! `fixtures/sharex-profile`: the reference ShareX's portable profile after
//! some use. `fixtures/sharex-fresh`: a first run of the same build in an
//! empty folder (start screen closed, then exited): `Backup/` holds the
//! `ApplicationConfig.json` it saved when the start screen closed.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sharex_settings::settings_base::{self, SaveOptions, load_file, to_json};
use sharex_settings::types::{DateTime, Point, Size};
use sharex_settings::*;

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(path)
}

/// The file as ShareX on this OS would have written it (the fixtures were
/// written on Windows: CRLF).
fn expected(path: &str) -> String {
    let text = fs::read_to_string(fixture(path)).unwrap();
    if cfg!(windows) { text } else { text.replace("\r\n", "\n") }
}

fn round_trip<T: SettingsFile>(path: &str) {
    let settings: T = load_file(&fixture(path)).unwrap();
    let written = to_json(&settings);
    let want = expected(path);
    if written != want {
        let (w, e): (Vec<_>, Vec<_>) = (written.lines().collect(), want.lines().collect());
        let first = w.iter().zip(&e).position(|(a, b)| a != b).unwrap_or(w.len().min(e.len()));
        panic!(
            "{path} differs at line {}:\n  wrote:    {:?}\n  expected: {:?}",
            first + 1,
            w.get(first),
            e.get(first)
        );
    }
}

#[test]
fn sharex_profile_round_trips_byte_for_byte() {
    round_trip::<ApplicationConfig>("sharex-profile/ApplicationConfig.json");
    round_trip::<ApplicationConfig>("sharex-profile/Backup/ApplicationConfig.json");
    round_trip::<ApplicationConfig>("sharex-profile/Backup/ApplicationConfig-2026-11-W47.json");
    round_trip::<HotkeysConfig>("sharex-profile/HotkeysConfig.json");
    round_trip::<HotkeysConfig>("sharex-profile/Backup/HotkeysConfig-2026-11-W47.json");
}

#[test]
fn fresh_profile_round_trips_byte_for_byte() {
    round_trip::<ApplicationConfig>("sharex-fresh/ApplicationConfig.json");
    round_trip::<ApplicationConfig>("sharex-fresh/Backup/ApplicationConfig.json");
    round_trip::<HotkeysConfig>("sharex-fresh/HotkeysConfig.json");
    round_trip::<UploadersConfig>("sharex-fresh/UploadersConfig.json");
}

#[test]
fn profile_is_read_as_sharex_wrote_it() {
    let settings: ApplicationConfig = load_file(&fixture("sharex-profile/ApplicationConfig.json")).unwrap();
    assert!(settings.extra.is_empty(), "unexpected properties: {:?}", settings.extra.keys().collect::<Vec<_>>());
    assert_eq!(settings.ThemeOptions.as_ref().unwrap().Theme, "Dark");
    assert_eq!(settings.MainFormSize, Size::new(900, 703));
    assert_eq!(settings.ApplicationVersion.as_deref(), Some("22.4.0"));
    let task = &settings.DefaultTaskSettings;
    assert_eq!(task.AfterCaptureJob, AfterCaptureTasks::CopyImageToClipboard | AfterCaptureTasks::SaveImageToFile);
    let capture = task.CaptureSettings.as_ref().unwrap();
    assert!(capture.extra.is_empty() && capture.RegionCaptureOptions.extra.is_empty());
    assert_eq!(capture.RegionCaptureOptions.MagnifierPixelCount, 15);

    let hotkeys: HotkeysConfig = load_file(&fixture("sharex-profile/HotkeysConfig.json")).unwrap();
    let keys: Vec<(String, String)> =
        hotkeys.Hotkeys.iter().map(|h| (h.HotkeyInfo.to_string(), h.TaskSettings.Job.name().to_owned())).collect();
    assert_eq!(
        keys,
        [
            ("Ctrl + Print Screen", "RectangleRegion"),
            ("Print Screen", "PrintScreen"),
            ("Alt + Print Screen", "ActiveWindow"),
            ("Shift + Print Screen", "ScreenRecorder"),
            ("Ctrl + Shift + Print Screen", "ScreenRecorderGIF"),
        ]
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
    );
    assert!(hotkeys.Hotkeys.iter().all(|h| h.TaskSettings.GeneralSettings.is_none()));
}

/// What ShareX's first run changes from `new ApplicationConfig()` before it
/// saves: the start screen (`StartScreenWindow.OnClosed`) and the main
/// window's bounds (`SaveWindowBounds`, saved whether or not they're
/// remembered).
fn first_run(settings: &mut ApplicationConfig, written: &ApplicationConfig) {
    settings.FirstTimeRunDate = written.FirstTimeRunDate.clone();
    settings.ShowStartScreen = false;
    settings.ShowIconPackPrompt = false;
    settings.MainFormPosition = Point::new(54, 4);
    settings.MainFormSize = Size::new(900, 703);
    settings.ApplicationVersion = Some(settings_base::application_version().to_owned());
}

#[test]
fn new_settings_are_written_as_a_fresh_sharex_profile() {
    let sharex: ApplicationConfig = load_file(&fixture("sharex-fresh/ApplicationConfig.json")).unwrap();
    let mut ours = ApplicationConfig::default();
    first_run(&mut ours, &sharex);
    assert_eq!(to_json(&ours), expected("sharex-fresh/ApplicationConfig.json"));

    let mut hotkeys = HotkeysConfig::new(&ours.DefaultTaskSettings);
    hotkeys.cleanup();
    hotkeys.ApplicationVersion = Some(settings_base::application_version().to_owned());
    assert_eq!(to_json(&hotkeys), expected("sharex-fresh/HotkeysConfig.json"));
}

#[test]
fn unknown_properties_and_values_survive() {
    let path = fixture("sharex-profile/ApplicationConfig.json");
    let mut doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    doc["SomethingNew"] = json!({ "Nested": [1, 2.5, "x"] });
    doc["Language"] = json!("Klingon");
    doc["DefaultTaskSettings"]["Job"] = json!("SomeFutureJob");
    doc["DefaultTaskSettings"]["CaptureSettings"]["NewCaptureOption"] = json!(true);
    doc["DefaultTaskSettings"]["CaptureSettings"]["RegionCaptureOptions"]["NewRegionOption"] = json!(42);
    doc["DefaultTaskSettings"]["ToolsSettings"]["ImageEditorOptions"]["NewEditorOption"] = json!("y");
    doc["ThemeOptions"]["NewThemeOption"] = json!(null);
    let text = serde_json::to_string_pretty(&doc).unwrap();
    let settings: ApplicationConfig = serde_json::from_str(&text).unwrap();
    assert_eq!(settings.Language, SupportedLanguage::Unknown("Klingon".into()));
    let again: Value = serde_json::from_str(&to_json(&settings)).unwrap();
    assert_eq!(again, doc);

    let mut hotkeys: Value =
        serde_json::from_str(&fs::read_to_string(fixture("sharex-profile/HotkeysConfig.json")).unwrap()).unwrap();
    hotkeys["Hotkeys"][0]["HotkeyInfo"]["NewHotkeyOption"] = json!(1);
    hotkeys["Hotkeys"][0]["NewHotkeySetting"] = json!("z");
    hotkeys["Hotkeys"][1]["TaskSettings"]["Job"] = json!(999);
    let config: HotkeysConfig = serde_json::from_value(hotkeys.clone()).unwrap();
    assert_eq!(serde_json::to_value(&config).unwrap(), hotkeys);
}

#[test]
fn bad_values_keep_their_defaults() {
    // Newtonsoft's `Serializer_Error` handling: "Error converting value" keeps the default.
    let settings: ApplicationConfig = serde_json::from_value(json!({
        "ShowTray": "not a bool",
        "HotkeyRepeatLimit": 50,
        "ThumbnailSize": "garbage",
        "showstartscreen": false,
        "DefaultTaskSettings": { "AfterCaptureJob": "NoSuchTask, SaveImageToFile" }
    }))
    .unwrap();
    assert!(settings.ShowTray);
    assert_eq!(settings.hotkey_repeat_limit(), 200);
    assert_eq!(settings.ThumbnailSize, Size::new(200, 150));
    assert!(!settings.ShowStartScreen, "names match case-insensitively");
    assert_eq!(
        settings.DefaultTaskSettings.AfterCaptureJob,
        AfterCaptureTasks::CopyImageToClipboard | AfterCaptureTasks::SaveImageToFile
    );
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sharex-settings-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn save_keeps_backups_and_load_falls_back() {
    let dir = temp_dir("backups");
    let backup = dir.join("Backup");
    let file = dir.join("ApplicationConfig.json");
    let options = SaveOptions { backup_folder: Some(backup.clone()), create_backup: true, create_weekly_backup: true };

    let mut settings = ApplicationConfig::default();
    settings_base::save(&mut settings, &file, &options).unwrap();
    assert!(file.is_file() && !dir.join("ApplicationConfig.json.temp").exists());
    assert!(!backup.join("ApplicationConfig.json").exists(), "nothing to back up on the first save");
    let weekly: Vec<_> = fs::read_dir(&backup).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(weekly.len(), 1);
    assert!(weekly[0].to_string_lossy().starts_with("ApplicationConfig-") && weekly[0].to_string_lossy().contains("-W"));

    settings.ShowTray = false;
    settings_base::save(&mut settings, &file, &options).unwrap();
    let previous: ApplicationConfig = load_file(&backup.join("ApplicationConfig.json")).unwrap();
    assert!(previous.ShowTray, "the backup is the file before this save");
    assert_eq!(fs::read_dir(&backup).unwrap().count(), 2, "one weekly backup a week");

    let loaded = settings_base::load::<ApplicationConfig>(Some(&file), Some(&backup), true);
    assert!(!loaded.settings.ShowTray && !loaded.is_first_time_run && !loaded.is_upgrade);

    // A corrupt file falls back to its backup, then to the weekly backup.
    fs::write(&file, "{ not json").unwrap();
    let loaded = settings_base::load::<ApplicationConfig>(Some(&file), Some(&backup), true);
    assert_eq!(loaded.source.as_deref(), Some(backup.join("ApplicationConfig.json").as_path()));
    assert!(loaded.settings.ShowTray);
    fs::remove_file(backup.join("ApplicationConfig.json")).unwrap();
    let loaded = settings_base::load::<ApplicationConfig>(Some(&file), Some(&backup), true);
    assert!(loaded.source.unwrap().file_name().unwrap().to_string_lossy().contains("-W"));
    // Without fallbacks (`ResetSettings`), or with nothing to fall back on: new settings.
    let loaded = settings_base::load::<ApplicationConfig>(Some(&file), Some(&backup), false);
    assert!(loaded.source.is_none() && loaded.is_first_time_run);
    // An interrupted save leaves the temp file, which comes before the backups.
    fs::write(dir.join("ApplicationConfig.json.temp"), to_json(&settings)).unwrap();
    let loaded = settings_base::load::<ApplicationConfig>(Some(&file), Some(&backup), true);
    assert!(loaded.source.unwrap().to_string_lossy().ends_with(".temp"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn upgrade_detection() {
    let dir = temp_dir("upgrade");
    let file = dir.join("ApplicationConfig.json");
    fs::write(&file, r#"{ "ApplicationVersion": "15.0.0", "CheckPreReleaseUpdates": true }"#).unwrap();
    let loaded = settings_base::load::<ApplicationConfig>(Some(&file), None, true);
    assert!(loaded.is_upgrade && loaded.is_upgrade_from("16.0.2") && !loaded.is_upgrade_from("14.0"));

    // `SettingManager` applies ShareX's backward compatibility tasks.
    let manager = SettingManager::load(AppPaths::in_folder(&dir), SystemOptions::default());
    assert_eq!(manager.settings.UpdateChannel, UpdateChannel::PreRelease);
    assert!(manager.hotkeys_info.source.is_none());
    assert_eq!(manager.hotkeys.Hotkeys.len(), 5);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn setting_manager_saves_like_sharex() {
    let dir = temp_dir("manager");
    let mut manager = SettingManager::load(AppPaths::in_folder(&dir), SystemOptions { disable_upload: true, ..Default::default() });
    assert!(manager.settings_info.is_first_time_run);
    manager.save_all_async();
    manager.settings.DisableHotkeys = true;
    manager.save_all().unwrap();
    let app: ApplicationConfig = load_file(&dir.join("ApplicationConfig.json")).unwrap();
    assert!(app.DisableHotkeys, "the synchronous save runs after the background ones");
    let hotkeys: HotkeysConfig = load_file(&dir.join("HotkeysConfig.json")).unwrap();
    assert!(hotkeys.Hotkeys.iter().all(|h| h.TaskSettings.CaptureSettings.is_none()), "cleaned up before saving");
    assert!(dir.join("Backup").join("ApplicationConfig.json").is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn first_run_date_format() {
    let now = DateTime::now();
    assert!(now.0.ends_with('Z') && now.parse().is_some());
}
