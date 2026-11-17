//! `ShareX.HelpersLib/Settings/SettingsBase.cs` and `JsonHelpers`: how a
//! settings file is read (falling back to its temp file, its backup and its
//! latest weekly backup) and written (to a temp file, verified, swapped in
//! with the old one kept as a backup, plus one copy a week).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{ApplicationConfig, HotkeysConfig};

/// A settings file: `SettingsBase<T>`.
pub trait SettingsFile: Serialize + DeserializeOwned + Default {
    /// `SettingsBase.ApplicationVersion`: the version that last saved it.
    fn application_version(&self) -> Option<&str>;
    fn set_application_version(&mut self, version: String);
}

macro_rules! impl_settings_file {
    ($($t:ty),*) => {$(
        impl SettingsFile for $t {
            fn application_version(&self) -> Option<&str> {
                self.ApplicationVersion.as_deref()
            }
            fn set_application_version(&mut self, version: String) {
                self.ApplicationVersion = Some(version);
            }
        }
    )*};
}
impl_settings_file!(ApplicationConfig, HotkeysConfig);

/// `Helpers.GetApplicationVersion()`: what ShareXrs writes as
/// `ApplicationVersion` (ShareX's version it ports: "22.4.0").
pub fn application_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// What [`load`] found.
#[derive(Debug)]
pub struct Loaded<T> {
    pub settings: T,
    /// The file it came from, or `None` for new settings.
    pub source: Option<PathBuf>,
    /// `IsFirstTimeRun`: no `ApplicationVersion` (new settings).
    pub is_first_time_run: bool,
    /// `IsUpgrade`: saved by an older version.
    pub is_upgrade: bool,
}

impl<T: SettingsFile> Loaded<T> {
    /// `IsUpgradeFrom(version)`: saved by `version` or older.
    pub fn is_upgrade_from(&self, version: &str) -> bool {
        self.is_upgrade && compare_version(self.settings.application_version().unwrap_or(""), version).is_le()
    }
}

/// `SettingsBase.Load`: `path`, else (with `fallback`) `path.temp`, the copy
/// in `backup_folder`, and the latest weekly backup there; else new settings.
pub fn load<T: SettingsFile>(path: Option<&Path>, backup_folder: Option<&Path>, fallback: bool) -> Loaded<T> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = path {
        candidates.push(path.to_owned());
        if fallback {
            candidates.push(temp_path(path));
            if let Some(folder) = backup_folder.filter(|f| f.is_dir())
                && let Some(name) = path.file_name()
            {
                candidates.push(folder.join(name));
                // A folder that can't be listed only means no weekly backup.
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    && let Some(latest) = latest_weekly_backup(folder, stem)
                {
                    candidates.push(latest);
                }
            }
        }
    }
    for candidate in candidates {
        if !candidate.is_file() {
            log::write(&format!("{} file does not exist: {}", type_name::<T>(), candidate.display()));
            continue;
        }
        match load_file::<T>(&candidate) {
            Ok(settings) => {
                let version = settings.application_version().unwrap_or("");
                let is_first_time_run = version.is_empty();
                let is_upgrade = !is_first_time_run && compare_version(version, application_version()).is_lt();
                return Loaded { settings, source: Some(candidate), is_first_time_run, is_upgrade };
            }
            Err(e) => log::write(&format!("{} load failed: {}: {e}", type_name::<T>(), candidate.display())),
        }
    }
    log::write(&format!("Loading new {} instance.", type_name::<T>()));
    Loaded { settings: T::default(), source: None, is_first_time_run: true, is_upgrade: false }
}

fn type_name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("Settings")
}

/// `Directory.GetFiles(backupFolder, name + "-*").OrderBy(x => x).LastOrDefault()`.
fn latest_weekly_backup(folder: &Path, stem: &str) -> Option<PathBuf> {
    let prefix = format!("{stem}-");
    let mut found: Vec<PathBuf> = fs::read_dir(folder)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.len() >= prefix.len() && n[..prefix.len()].eq_ignore_ascii_case(&prefix))
        })
        .collect();
    found.sort();
    found.pop()
}

/// Reads one settings file (`LoadInternal` without the fallbacks).
pub fn load_file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("file stream length is 0".into());
    }
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    serde_json::from_str::<T>(text).map_err(|e| e.to_string())
}

/// The settings as ShareX writes them: Newtonsoft's indented JSON (two
/// spaces, `Environment.NewLine` between lines, no final newline).
pub fn to_json<T: Serialize>(settings: &T) -> String {
    let text = serde_json::to_string_pretty(settings).expect("settings serialise");
    // A raw newline only ever separates tokens: in strings it's escaped.
    if cfg!(windows) { text.replace('\n', "\r\n") } else { text }
}

/// How a settings file is saved (`CreateBackup`, `CreateWeeklyBackup`,
/// `BackupFolder`). ShareX turns both backups on for all three files.
#[derive(Debug, Clone, Default)]
pub struct SaveOptions {
    pub backup_folder: Option<PathBuf>,
    pub create_backup: bool,
    pub create_weekly_backup: bool,
}

/// `SettingsBase.Save(filePath)`: stamps `ApplicationVersion`, then writes.
pub fn save<T: SettingsFile>(settings: &mut T, path: &Path, options: &SaveOptions) -> io::Result<()> {
    settings.set_application_version(application_version().to_owned());
    write(&to_json(settings), path, options)
}

/// `SettingsBase.SaveInternal`: writes `json` to `path.temp` (write
/// through), checks it starts with `{` and ends with `}`, then swaps it in,
/// keeping the old file in the backup folder, and makes this week's backup
/// if there isn't one yet.
pub fn write(json: &str, path: &Path, options: &SaveOptions) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let temp = temp_path(path);
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    if !quick_verify_json_file(&temp) {
        return Err(io::Error::other(format!("settings file is corrupt: {}", temp.display())));
    }
    if path.exists() {
        let backup = match (&options.backup_folder, options.create_backup, path.file_name()) {
            (Some(folder), true, Some(name)) => {
                fs::create_dir_all(folder)?;
                Some(folder.join(name))
            }
            _ => None,
        };
        replace_file(&temp, path, backup.as_deref())?;
    } else {
        fs::rename(&temp, path)?;
    }
    if options.create_weekly_backup
        && let Some(folder) = &options.backup_folder
    {
        backup_file_weekly(path, folder)?;
    }
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".temp");
    PathBuf::from(s)
}

/// `JsonHelpers.QuickVerifyJsonFile`.
pub fn quick_verify_json_file(path: &Path) -> bool {
    fs::read(path).is_ok_and(|b| b.len() > 1 && b[0] == b'{' && b[b.len() - 1] == b'}')
}

/// `File.Replace(source, destination, backup, ignoreMetadataErrors: true)`.
#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path, backup: Option<&Path>) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        REPLACEFILE_IGNORE_ACL_ERRORS, REPLACEFILE_IGNORE_MERGE_ERRORS, REPLACEFILE_WRITE_THROUGH, ReplaceFileW,
    };
    let wide = |p: &Path| p.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let (replaced, replacement) = (wide(destination), wide(source));
    let backup = backup.map(wide);
    let ok = unsafe {
        ReplaceFileW(
            replaced.as_ptr(),
            replacement.as_ptr(),
            backup.as_ref().map_or(std::ptr::null(), |b| b.as_ptr()),
            REPLACEFILE_WRITE_THROUGH | REPLACEFILE_IGNORE_MERGE_ERRORS | REPLACEFILE_IGNORE_ACL_ERRORS,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

/// `File.Replace` on Unix (.NET's `FileSystem.Unix`): the destination
/// becomes the backup, then the source moves into its place.
#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path, backup: Option<&Path>) -> io::Result<()> {
    if let Some(backup) = backup {
        if backup.exists() {
            fs::remove_file(backup)?;
        }
        if fs::hard_link(destination, backup).is_err() {
            fs::copy(destination, backup)?;
        }
    }
    fs::rename(source, destination)
}

/// `FileHelpers.BackupFileWeekly`: copies the file to
/// `<name>-<yyyy-MM>-W<week><ext>` in `folder` unless that exists.
pub fn backup_file_weekly(path: &Path, folder: &Path) -> io::Result<Option<PathBuf>> {
    if !path.is_file() {
        return Ok(None);
    }
    let now = chrono::Local::now().date_naive();
    let target = folder.join(weekly_backup_name(path, now));
    if target.exists() {
        return Ok(None);
    }
    fs::create_dir_all(folder)?;
    fs::copy(path, &target)?;
    Ok(Some(target))
}

fn weekly_backup_name(path: &Path, date: chrono::NaiveDate) -> String {
    let stem = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    format!("{stem}-{}-W{:02}{ext}", date.format("%Y-%m"), week_of_year(date))
}

/// `DateTime.WeekOfYear()`: `Calendar.GetWeekOfYear(date,
/// CalendarWeekRule.FirstDay, DayOfWeek.Monday)`: week 1 holds 1 January,
/// weeks start on Monday.
pub fn week_of_year(date: chrono::NaiveDate) -> u32 {
    use chrono::Datelike;
    let jan1 = chrono::NaiveDate::from_ymd_opt(date.year(), 1, 1).expect("1 January");
    let offset = jan1.weekday().num_days_from_monday();
    (date.ordinal0() + offset) / 7 + 1
}

/// `Helpers.CompareVersion`: compares dotted versions part by part.
pub fn compare_version(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<u64> { v.trim().split('.').map(|p| p.trim().parse().unwrap_or(0)).collect() };
    let (a, b) = (parse(a), parse(b));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x.cmp(&y);
        }
    }
    std::cmp::Ordering::Equal
}

/// `DebugHelper.WriteLine`, until ShareX's logger is ported: stderr.
pub mod log {
    pub fn write(message: &str) {
        eprintln!("{message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn weekly_names() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        assert_eq!(week_of_year(d), 47);
        assert_eq!(weekly_backup_name(Path::new("x/ApplicationConfig.json"), d), "ApplicationConfig-2026-11-W47.json");
        // 2026 starts on a Thursday: 4 January (Sunday) is still week 1, Monday 5 January week 2.
        assert_eq!(week_of_year(NaiveDate::from_ymd_opt(2026, 1, 4).unwrap()), 1);
        assert_eq!(week_of_year(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()), 2);
        assert_eq!(week_of_year(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()), 53);
    }

    #[test]
    fn versions() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_version("22.4.0", "22.4.0"), Equal);
        assert_eq!(compare_version("15.0.1", "22.4.0"), Less);
        assert_eq!(compare_version("22.4", "22.4.0"), Equal);
        assert_eq!(compare_version("22.10.0", "22.4.0"), Greater);
    }
}
