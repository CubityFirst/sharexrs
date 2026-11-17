//! `ShareX/SettingManager.cs`: loads `ApplicationConfig.json` and
//! `HotkeysConfig.json` at startup and saves them, with ShareX's backups.
//!
//! ShareX saves all settings on exit (`ApplicationLifecycle.CloseSequence`)
//! and in the background when the main window hides to the tray, the tray
//! menu closes, a task completes (`SaveSettingsAfterTaskCompleted`) or a
//! settings window closes. Background saves here go through one writer
//! thread, in order, so an older snapshot never overwrites a newer one.

use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use crate::enums::{AfterCaptureTasks, UpdateChannel};
use crate::paths::{AppPaths, SystemOptions};
use crate::settings_base::{self, Loaded, SaveOptions, SettingsFile, log};
use crate::{ApplicationConfig, HotkeysConfig, defaults};

/// What a load found about a file, besides its settings.
#[derive(Debug, Clone, Default)]
pub struct LoadInfo {
    pub source: Option<PathBuf>,
    pub is_first_time_run: bool,
    pub is_upgrade: bool,
}

/// `ShareX.SettingManager` with `ApplicationState`'s settings.
pub struct SettingManager {
    pub paths: AppPaths,
    pub system: SystemOptions,
    /// `ApplicationState.Settings`.
    pub settings: ApplicationConfig,
    pub settings_info: LoadInfo,
    /// `ApplicationState.HotkeysConfig`.
    pub hotkeys: HotkeysConfig,
    pub hotkeys_info: LoadInfo,
    saver: Saver,
}

impl SettingManager {
    /// `SettingManager.LoadInitialSettings()` (the hotkeys synchronously:
    /// ShareX loads them on a worker and waits before registering them).
    pub fn load(paths: AppPaths, system: SystemOptions) -> Self {
        let mut m = SettingManager {
            paths,
            system,
            settings: ApplicationConfig::default(),
            settings_info: LoadInfo::default(),
            hotkeys: HotkeysConfig::default(),
            hotkeys_info: LoadInfo::default(),
            saver: Saver::start(),
        };
        m.load_application_config(true);
        m.load_hotkeys_config(true);
        m
    }

    fn options(&self) -> SaveOptions {
        SaveOptions { backup_folder: Some(self.paths.backup_folder()), create_backup: true, create_weekly_backup: true }
    }

    /// `LoadApplicationConfig`.
    pub fn load_application_config(&mut self, fallback: bool) {
        let path = self.paths.application_config_file();
        let backup = self.paths.backup_folder();
        let loaded: Loaded<ApplicationConfig> = settings_base::load(path.as_deref(), Some(&backup), fallback);
        self.settings_info = info(&loaded);
        self.settings = loaded.settings;
        self.application_config_backward_compatibility_tasks();
        self.settings.theme_options();
    }

    /// `LoadHotkeysConfig`. A new config gets the default hotkeys with the
    /// application's default task settings, as `new HotkeysConfig()` does.
    pub fn load_hotkeys_config(&mut self, fallback: bool) {
        let path = self.paths.hotkeys_config_file(Some(&self.settings));
        let backup = self.paths.backup_folder();
        let loaded: Loaded<HotkeysConfig> = settings_base::load(path.as_deref(), Some(&backup), fallback);
        self.hotkeys_info = info(&loaded);
        self.hotkeys = loaded.settings;
        if self.hotkeys_info.source.is_none() {
            self.hotkeys = HotkeysConfig::new(&self.settings.DefaultTaskSettings);
        }
        self.hotkeys_config_backward_compatibility_tasks();
    }

    fn is_upgrade_from(&self, version: &str) -> bool {
        self.settings_info.is_upgrade
            && settings_base::compare_version(self.settings.ApplicationVersion.as_deref().unwrap_or(""), version).is_le()
    }

    /// `ApplicationConfigBackwardCompatibilityTasks` (the FFmpeg ones wait
    /// for the screen recorder port).
    fn application_config_backward_compatibility_tasks(&mut self) {
        if self.system.disable_upload {
            self.settings.DefaultTaskSettings.AfterCaptureJob.remove(AfterCaptureTasks::UploadImageToHost);
        }
        if self.is_upgrade_from("15.0.1")
            && let Some(capture) = self.settings.DefaultTaskSettings.CaptureSettings.as_mut()
        {
            capture.ScrollingCaptureOptions = defaults::json("ScrollingCaptureOptions");
        }
        if self.is_upgrade_from("16.0.2") && self.settings.CheckPreReleaseUpdates {
            self.settings.UpdateChannel = UpdateChannel::PreRelease;
        }
    }

    /// `HotkeysConfigBackwardCompatibilityTasks`.
    fn hotkeys_config_backward_compatibility_tasks(&mut self) {
        let upgrade = self.is_upgrade_from("15.0.1");
        for h in &mut self.hotkeys.Hotkeys {
            if self.system.disable_upload {
                h.TaskSettings.AfterCaptureJob.remove(AfterCaptureTasks::UploadImageToHost);
            }
            if upgrade && let Some(capture) = h.TaskSettings.CaptureSettings.as_mut() {
                capture.ScrollingCaptureOptions = defaults::json("ScrollingCaptureOptions");
            }
        }
    }

    /// `SaveAllSettings()`: saves both files now (on exit). Waits for
    /// background saves still running first.
    pub fn save_all(&mut self) -> io::Result<()> {
        let a = self.save_application_config(true);
        let h = self.save_hotkeys_config(true);
        a.and(h)
    }

    /// `SaveAllSettingsAsync()`.
    pub fn save_all_async(&mut self) {
        let _ = self.save_application_config(false);
        let _ = self.save_hotkeys_config(false);
    }

    /// `SaveApplicationConfigAsync()` (or the synchronous `Save` with `wait`).
    pub fn save_application_config(&mut self, wait: bool) -> io::Result<()> {
        let Some(path) = self.paths.application_config_file() else { return Ok(()) };
        self.settings.set_application_version(settings_base::application_version().to_owned());
        let json = settings_base::to_json(&self.settings);
        self.saver.save(json, path, self.options(), wait)
    }

    /// `SaveHotkeysConfigAsync()`: cleans up first (`CleanupHotkeysConfig`),
    /// so hotkeys using the defaults save without their sections.
    pub fn save_hotkeys_config(&mut self, wait: bool) -> io::Result<()> {
        let Some(path) = self.paths.hotkeys_config_file(Some(&self.settings)) else { return Ok(()) };
        self.hotkeys.cleanup();
        self.hotkeys.set_application_version(settings_base::application_version().to_owned());
        let json = settings_base::to_json(&self.hotkeys);
        self.saver.save(json, path, self.options(), wait)
    }
}

fn info<T>(loaded: &Loaded<T>) -> LoadInfo {
    LoadInfo { source: loaded.source.clone(), is_first_time_run: loaded.is_first_time_run, is_upgrade: loaded.is_upgrade }
}

type Job = (String, PathBuf, SaveOptions, Option<mpsc::Sender<io::Result<()>>>);

/// One writer thread: ShareX's `SaveAsync` is `Task.Run(Save)` with
/// `lock(this)`; here the saves also stay in order.
struct Saver {
    jobs: mpsc::Sender<Job>,
}

impl Saver {
    fn start() -> Self {
        let (jobs, rx) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("settings-saver".into())
            .spawn(move || {
                for (json, path, options, done) in rx {
                    let result = settings_base::write(&json, &path, &options);
                    match &result {
                        Ok(()) => log::write(&format!("Settings save successful: {}", path.display())),
                        // `Settings_SettingsSaveFailed` shows a notification;
                        // notifications aren't ported yet.
                        Err(e) => log::write(&format!("Settings save failed: {}: {e}", path.display())),
                    }
                    if let Some(done) = done {
                        let _ = done.send(result);
                    }
                }
            })
            .expect("settings saver thread");
        Saver { jobs }
    }

    fn save(&self, json: String, path: PathBuf, options: SaveOptions, wait: bool) -> io::Result<()> {
        if wait {
            let (tx, rx) = mpsc::channel();
            self.jobs.send((json, path, options, Some(tx))).map_err(io::Error::other)?;
            rx.recv().map_err(io::Error::other)?
        } else {
            self.jobs.send((json, path, options, None)).map_err(io::Error::other)
        }
    }
}
