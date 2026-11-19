//! Application settings › Settings › Export, Import and Reset settings:
//! `SettingManager.Export`, `Import` and `ResetSettings`
//! (`ShareX/SettingManager.cs`) with the view model's `ExportAsync`,
//! `ImportAsync` and `ResetAsync` (`ApplicationSettingsViewModel.cs`) and the
//! window's pickers (`OnExportClick`, `OnImportClick`).
//!
//! A backup (`.sxb`) is a zip (`sharex_helpers::zip_manager`, checked against
//! ShareX's by `tools/settings-harness`) of `ApplicationConfig.json`,
//! `UploadersConfig.json` (secrets in plain text: `SaveToMemoryStream(false)`)
//! and `HotkeysConfig.json` with Settings, and `History.db` with History.
//! An import extracts only its `.json` and `.xml` entries into the personal
//! folder, so, as in ShareX, a backup's history isn't imported.
//!
//! The zip work runs on a thread while the window shows "Exporting
//! backup..." or "Importing backup..." and its buttons are disabled
//! (`IsBusy`); the thread wakes the event loop with
//! [`crate::UserEvent::SettingsBackup`].

use std::path::{Path, PathBuf};

use sharex_helpers::zip_manager::{self, ZipEntryInfo};
use sharex_settings::settings_base::{self, SettingsFile};
use sharex_strings::sharex as s;
use winit::event_loop::ActiveEventLoop;

use crate::message_box::{MessageBox, MessageBoxIcon};
use crate::{App, DialogPurpose, UserEvent, appearance, history};

/// `SettingManager.Import`'s limit on the archive's uncompressed size.
pub const IMPORT_MAX_UNCOMPRESSED_SIZE: u64 = 1_000_000_000;

/// The file names in a backup.
pub const APPLICATION_CONFIG: &str = "ApplicationConfig.json";
pub const UPLOADERS_CONFIG: &str = "UploadersConfig.json";
pub const HOTKEYS_CONFIG: &str = "HotkeysConfig.json";

/// What a backup thread did.
#[derive(Debug)]
pub struct Done {
    pub import: bool,
    pub path: PathBuf,
    /// `Err` holds the exception's message.
    pub result: Result<(), String>,
    /// History was closed for it (`HistoryClose`) and reconnects now.
    pub history_closed: bool,
}

/// `SettingManager.Export`'s entries: the settings files (from their JSON)
/// with `settings`, the history file with `history` (left out when it's
/// missing, as `CreateEntry` does).
pub fn export_entries(application: String, uploaders: Option<String>, hotkeys: String, history_file: Option<&Path>, settings: bool, history: bool) -> Vec<ZipEntryInfo> {
    let mut entries = Vec::new();
    if settings {
        entries.push(ZipEntryInfo::data(application.into_bytes(), APPLICATION_CONFIG));
        // `UploadersConfig.SaveToMemoryStream`: the config ShareX has loaded;
        // ShareXrs's loads in the background, so it may still be loading.
        if let Some(u) = uploaders {
            entries.push(ZipEntryInfo::data(u.into_bytes(), UPLOADERS_CONFIG));
        }
        entries.push(ZipEntryInfo::data(hotkeys.into_bytes(), HOTKEYS_CONFIG));
    }
    if history && let Some(file) = history_file {
        entries.push(ZipEntryInfo::file(file));
    }
    entries
}

/// `SettingManager.Import`'s extraction: the archive's `.json` and `.xml`
/// entries into `personal_folder`, folders kept.
pub fn import_archive(archive: &Path, personal_folder: &Path) -> Result<(), String> {
    let filter = |_: &str, name: &str| sharex_helpers::file_helpers::check_extension(name, &["json", "xml"]);
    zip_manager::extract(archive, personal_folder, true, Some(&filter), IMPORT_MAX_UNCOMPRESSED_SIZE).map(|_| ()).map_err(|e| e.to_string())
}

/// `OnExportClick`'s suggested name: `ShareX-<version>-<machine>-backup.sxb`
/// under ShareXrs's name.
pub fn suggested_file_name() -> String {
    let machine = sharex_helpers::file_helpers::sanitize_file_name(&machine_name().to_lowercase());
    format!("ShareXrs-{}-{machine}-backup.sxb", settings_base::application_version())
}

/// `Environment.MachineName`.
fn machine_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|n| !n.is_empty())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_owned()))
        .unwrap_or_default()
}

/// `StorageProvider.SaveFilePickerAsync` for the export.
pub fn pick_export_path() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(s::ApplicationSettingsWindow_ExportShareXBackup())
        .set_file_name(suggested_file_name())
        .add_filter(s::ApplicationSettingsWindow_ShareXBackup(), &["sxb"])
        // `FilePickerFileTypes.All`.
        .add_filter("All", &["*"])
        .save_file()
}

/// `StorageProvider.OpenFilePickerAsync` for the import (the file must exist).
pub fn pick_import_path() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(s::ApplicationSettingsWindow_ImportShareXBackup())
        .add_filter(s::ApplicationSettingsWindow_ShareXBackup(), &["sxb"])
        .pick_file()
        .filter(|p| p.is_file())
}

impl App {
    /// `ExportAsync(path)`: settings saved, then the archive written on a
    /// thread.
    pub(crate) fn export_settings(&mut self, path: PathBuf, settings: bool, history_on: bool) {
        // `SettingManager.SaveAllSettings()`.
        if let Err(e) = self.settings.save_all() {
            eprintln!("couldn't save the settings: {e}");
        }
        self.save_uploaders_config(true);
        let s = &mut self.settings;
        s.settings.set_application_version(settings_base::application_version().to_owned());
        let application = settings_base::to_json(&s.settings);
        s.hotkeys.set_application_version(settings_base::application_version().to_owned());
        let hotkeys = settings_base::to_json(&s.hotkeys);
        let uploaders = self.uploads.config.to_plain_json();
        let history_file = self.settings.paths.history_file();
        let entries = export_entries(application, uploaders, hotkeys, history_file.as_deref(), settings, history_on);
        // `HistoryClose()` while the file is copied.
        if history_on {
            history::close();
        }
        let proxy = self.proxy.clone();
        let spawned = std::thread::Builder::new().name("settings-export".into()).spawn({
            let path = path.clone();
            move || {
                let result = zip_manager::compress(&path, &entries).map_err(|e| e.to_string());
                let _ = proxy.send_event(UserEvent::SettingsBackup(Box::new(Done { import: false, path, result, history_closed: history_on })));
            }
        });
        if let Err(e) = spawned {
            self.backup_finished_inner(Done { import: false, path, result: Err(e.to_string()), history_closed: history_on }, None);
        }
    }

    /// `ImportAsync(path)`: the archive extracted on a thread; the settings
    /// load when it's done ([`App::backup_finished`]).
    pub(crate) fn import_settings(&mut self, path: PathBuf) {
        history::close();
        let folder = self.settings.paths.personal_folder.clone();
        let proxy = self.proxy.clone();
        let spawned = std::thread::Builder::new().name("settings-import".into()).spawn({
            let path = path.clone();
            move || {
                let result = import_archive(&path, &folder);
                let _ = proxy.send_event(UserEvent::SettingsBackup(Box::new(Done { import: true, path, result, history_closed: true })));
            }
        });
        if let Err(e) = spawned {
            self.backup_finished_inner(Done { import: true, path, result: Err(e.to_string()), history_closed: true }, None);
        }
    }

    /// A backup thread finished: the rest of `ExportAsync` / `ImportAsync`.
    pub(crate) fn backup_finished(&mut self, event_loop: &ActiveEventLoop, done: Done) {
        self.backup_finished_inner(done, Some(event_loop));
    }

    fn backup_finished_inner(&mut self, done: Done, event_loop: Option<&ActiveEventLoop>) {
        // `finally { HistoryConnect(); }`.
        if done.history_closed {
            history::connect(&self.settings.paths);
        }
        let path = done.path.display().to_string();
        let status = match (&done.result, done.import) {
            (Ok(()), false) => s::ApplicationSettingsWindow_BackupExportedTo().replace("{0}", &path),
            (Ok(()), true) => {
                // `SettingManager.LoadAllSettings()`, then the language,
                // `Reload()` and `UpdateApplicationAsync()`.
                self.load_all_settings();
                appearance::change_language(&self.settings.settings.Language);
                self.reload_application();
                s::ApplicationSettingsWindow_BackupImportedFrom().replace("{0}", &path)
            }
            (Err(e), import) => {
                // `MessageBox.Show(string.Format(Strings.SettingManager_*BackupError, e), ...)`.
                let text = if import { s::SettingManager_ImportBackupError() } else { s::SettingManager_ExportBackupError() }.replace("{0}", e);
                if let Some(el) = event_loop {
                    self.queue_dialog(el, MessageBox::ok(crate::rs_title(s::SettingManager_ErrorTitle()), text, MessageBoxIcon::Error), DialogPurpose::Information);
                } else {
                    eprintln!("{text}");
                }
                if import { s::ApplicationSettingsWindow_BackupImportFailed() } else { s::ApplicationSettingsWindow_BackupExportFailed() }.to_owned()
            }
        };
        if let Some(w) = &mut self.settings_window {
            if done.import && done.result.is_ok() {
                w.state.reload(&self.settings.settings);
            }
            w.state.busy = false;
            w.state.status = status;
            w.window.window.request_redraw();
        }
    }

    /// `ResetAsync`'s question (`MessageBoxIcon.Exclamation`).
    pub(crate) fn ask_reset_settings(&mut self, event_loop: &ActiveEventLoop) {
        let caption = crate::rs_title(&format!("ShareX - {}", s::Confirmation()));
        let message = MessageBox::yes_no(caption, s::ApplicationSettingsForm_btnResetSettings_Click_WouldYouLikeToResetShareXSettings(), MessageBoxIcon::Warning);
        self.queue_dialog(event_loop, message, DialogPurpose::ResetSettings);
    }

    /// The rest of `ResetAsync` after Yes: `SettingManager.ResetSettings()`
    /// (each file deleted and loaded again without its backups), saved,
    /// then the language, `Reload()` and `UpdateApplicationAsync()`.
    pub(crate) fn reset_settings(&mut self) {
        let paths = self.settings.paths.clone();
        if let Some(p) = paths.application_config_file() {
            let _ = std::fs::remove_file(p);
        }
        self.settings.load_application_config(false);
        if let Some(p) = sharex_uploaders::config::uploaders_config_file_path(&paths, Some(&self.settings.settings)) {
            let _ = std::fs::remove_file(p);
        }
        self.reload_uploaders_config(false);
        if let Some(p) = paths.hotkeys_config_file(Some(&self.settings.settings)) {
            let _ = std::fs::remove_file(p);
        }
        self.settings.load_hotkeys_config(false);
        if let Err(e) = self.settings.save_all() {
            eprintln!("couldn't save the settings: {e}");
        }
        self.save_uploaders_config(true);
        appearance::change_language(&self.settings.settings.Language);
        self.reload_application();
        if let Some(w) = &mut self.settings_window {
            w.state.reload(&self.settings.settings);
            w.state.busy = false;
            w.state.status = s::ApplicationSettingsWindow_SettingsReset().to_owned();
            w.window.window.request_redraw();
        }
    }

    /// `SettingManager.LoadAllSettings()`.
    fn load_all_settings(&mut self) {
        self.settings.load_application_config(true);
        self.reload_uploaders_config(true);
        self.settings.load_hotkeys_config(true);
    }

    /// `LoadUploadersConfig(fallback)`, now.
    fn reload_uploaders_config(&mut self, fallback: bool) {
        let path = sharex_uploaders::config::uploaders_config_file_path(&self.settings.paths, Some(&self.settings.settings));
        let loaded = sharex_uploaders::UploadersConfig::load(path.as_deref(), Some(&self.settings.paths.backup_folder()), fallback);
        self.uploads.config.replace(loaded.settings);
    }

    /// `ApplicationSettingsRuntime.ReloadAsync`: what the app takes from the
    /// settings (`ApplyCore`), the hotkeys registered again, the watch
    /// folders, the menus; and the theme options, which ShareX's
    /// `ThemeManager` follows as they change.
    fn reload_application(&mut self) {
        self.apply_appearance();
        self.apply_settings();
        let disabled = self.settings.settings.DisableHotkeys;
        let _ = self.hotkeys.update_hotkeys(&mut self.settings.hotkeys.Hotkeys, disabled);
        self.refresh_hotkey_tips();
        self.update_watch_folders();
        self.tray_progress = -2;
        self.update_tray_icon(-1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_entries_follow_the_check_boxes() {
        let names = |e: &[ZipEntryInfo]| {
            e.iter()
                .map(|x| match x {
                    ZipEntryInfo::Data { name, .. } | ZipEntryInfo::File { name, .. } => name.clone(),
                })
                .collect::<Vec<_>>()
        };
        let all = export_entries("{}".into(), Some("{}".into()), "{}".into(), Some(Path::new("/p/History.db")), true, true);
        assert_eq!(names(&all), ["ApplicationConfig.json", "UploadersConfig.json", "HotkeysConfig.json", "History.db"]);
        let history = export_entries("{}".into(), Some("{}".into()), "{}".into(), Some(Path::new("/p/History.db")), false, true);
        assert_eq!(names(&history), ["History.db"]);
        // Sandbox mode has no history file.
        let settings = export_entries("{}".into(), Some("{}".into()), "{}".into(), None, true, true);
        assert_eq!(names(&settings).len(), 3);
    }

    #[test]
    fn import_takes_only_json_and_xml() {
        let dir = std::env::temp_dir().join(format!("sharexrs-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let history = dir.join("History.db");
        std::fs::write(&history, b"db").unwrap();
        let archive = dir.join("b.sxb");
        let entries = export_entries("{\"a\":1}".into(), Some("{}".into()), "{}".into(), Some(&history), true, true);
        zip_manager::compress(&archive, &entries).unwrap();
        let personal = dir.join("personal");
        import_archive(&archive, &personal).unwrap();
        assert_eq!(std::fs::read_to_string(personal.join(APPLICATION_CONFIG)).unwrap(), "{\"a\":1}");
        assert!(personal.join(HOTKEYS_CONFIG).exists());
        assert!(!personal.join("History.db").exists());
        // Not a zip: the import fails and writes nothing.
        std::fs::write(dir.join("bad.sxb"), b"nope").unwrap();
        assert!(import_archive(&dir.join("bad.sxb"), &dir.join("other")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn suggested_name_has_the_version() {
        let name = suggested_file_name();
        assert!(name.starts_with(&format!("ShareXrs-{}-", settings_base::application_version())));
        assert!(name.ends_with("-backup.sxb"));
    }
}
