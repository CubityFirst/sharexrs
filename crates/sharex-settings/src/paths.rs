//! Where ShareX keeps its files: `ShareX/Infrastructure/AppPaths.cs`,
//! `PersonalPathManager.cs`, `SystemOptions.cs` and the paths in
//! `SettingManager.cs`.
//!
//! ShareXrs keeps ShareX's layout and file formats but in its own folder,
//! `Documents/ShareXrs` (or `ShareXrs` next to the executable when a
//! `Portable` file is there), and its own registry policy key, so it never
//! writes to, migrates or locks a real ShareX profile on the same machine.
//! The files are ShareX's format, so a ShareX profile can be copied in.

use std::fs;
use std::path::{Path, PathBuf};

use crate::ApplicationConfig;

/// The personal folder's name: ShareX's `ApplicationInfo.Name` is
/// "ShareX"; ShareXrs uses its own so the two apps' profiles stay apart.
pub const APPLICATION_NAME: &str = "ShareXrs";
pub const APPLICATION_CONFIG_FILE_NAME: &str = "ApplicationConfig.json";
pub const HOTKEYS_CONFIG_FILE_NAME: &str = "HotkeysConfig.json";
pub const UPLOADERS_CONFIG_FILE_NAME: &str = "UploadersConfig.json";
const PERSONAL_PATH_CONFIG_FILE_NAME: &str = "PersonalPath.cfg";

/// `SystemOptions`: policy values from `HKLM\SOFTWARE\ShareXrs`, then
/// `HKCU\SOFTWARE\ShareXrs` (ShareX's `SOFTWARE\ShareX`, under ShareXrs's
/// own name; Windows only, elsewhere all off).
#[derive(Debug, Clone, Default)]
pub struct SystemOptions {
    pub disable_update_check: bool,
    pub disable_upload: bool,
    pub disable_logging: bool,
    pub personal_path: Option<String>,
}

impl SystemOptions {
    /// `SystemOptions.UpdateSystemOptions()`.
    pub fn read() -> Self {
        Self {
            disable_update_check: registry::boolean("DisableUpdateCheck"),
            disable_upload: registry::boolean("DisableUpload"),
            disable_logging: registry::boolean("DisableLogging"),
            personal_path: registry::string("PersonalPath"),
        }
    }
}

/// The folders ShareX works in, as `PersonalPathManager.Initialize` decides them.
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// `AppPaths.PersonalFolder`.
    pub personal_folder: PathBuf,
    /// `StartupOptions.Portable`.
    pub portable: bool,
    /// `StartupOptions.Sandbox` (`-sandbox`): nothing is read or saved.
    pub sandbox: bool,
    /// `PersonalPathManager.DetectionMethod`.
    pub detection_method: Option<String>,
}

/// `FileHelpers.GetAbsolutePath`'s base: the executable's folder.
pub fn base_directory() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `FileHelpers.GetAbsolutePath`: expands folder variables, then resolves
/// relative paths against the executable's folder.
pub fn absolute_path(path: &str) -> PathBuf {
    let path = PathBuf::from(expand_folder_variables(path));
    if path.is_absolute() { path } else { base_directory().join(path) }
}

/// `AppPaths.DefaultPersonalFolder`: `Documents/ShareXrs`.
pub fn default_personal_folder() -> PathBuf {
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default().join(APPLICATION_NAME)
}

/// Whether a command line has `-name` or `--name` (any case), as
/// `ApplicationCommandLine.IsCommandPresent` checks.
pub fn is_command_present(args: &[String], names: &[&str]) -> bool {
    args.iter().any(|a| {
        let a = a.trim_start_matches(['-', '/']);
        names.iter().any(|n| a.eq_ignore_ascii_case(n))
    })
}

impl AppPaths {
    /// `PersonalPathManager.Initialize()`: `-sandbox`; `-portable`/`-p` or a
    /// `Portable` file next to the executable; the `PersonalPath` policy; a
    /// `PersonalPath.cfg`; else `Documents/ShareXrs`. Creates the folder.
    pub fn initialize(args: &[String], system: &SystemOptions) -> AppPaths {
        let mut paths = AppPaths {
            personal_folder: default_personal_folder(),
            portable: false,
            sandbox: is_command_present(args, &["sandbox"]),
            detection_method: None,
        };
        if paths.sandbox {
            return paths;
        }
        let portable_folder = absolute_path(APPLICATION_NAME);
        let portable_check = absolute_path("Portable");
        let mut custom: Option<PathBuf> = None;
        if is_command_present(args, &["portable", "p"]) {
            paths.portable = true;
            custom = Some(portable_folder);
            paths.detection_method = Some("Portable CLI flag".into());
        } else if portable_check.is_file() {
            paths.portable = true;
            custom = Some(portable_folder);
            paths.detection_method = Some(format!("Portable file ({})", portable_check.display()));
        } else if let Some(p) = system.personal_path.as_deref().filter(|p| !p.is_empty()) {
            custom = Some(PathBuf::from(expand_folder_variables(p)));
            paths.detection_method = Some("Registry".into());
        } else {
            migrate_personal_path_config();
            let configured = read_personal_path_config();
            if !configured.is_empty() {
                custom = Some(absolute_path(&configured));
                paths.detection_method =
                    Some(format!("PersonalPath.cfg file ({})", personal_path_config_file().display()));
            }
        }
        if let Some(custom) = custom {
            paths.personal_folder = custom;
        }
        // `EnsurePersonalFolder`: if it can't be made, fall back to the default.
        if !paths.personal_folder.is_dir() && fs::create_dir_all(&paths.personal_folder).is_err() {
            crate::settings_base::log::write(&format!(
                "Unable to create folder \"{}\" (personal path detection method: {})",
                paths.personal_folder.display(),
                paths.detection_method.as_deref().unwrap_or("default")
            ));
            paths.personal_folder = default_personal_folder();
        }
        paths
    }

    /// Paths rooted at `folder`, for tests and tools.
    pub fn in_folder(folder: impl Into<PathBuf>) -> AppPaths {
        AppPaths { personal_folder: folder.into(), portable: false, sandbox: false, detection_method: None }
    }

    /// `SettingManager.BackupFolder`.
    pub fn backup_folder(&self) -> PathBuf {
        self.personal_folder.join("Backup")
    }

    /// `AppPaths.LogsFolder`.
    pub fn logs_folder(&self) -> PathBuf {
        self.personal_folder.join("Logs")
    }

    /// `AppPaths.ImageEffectsFolder`.
    pub fn image_effects_folder(&self) -> PathBuf {
        self.personal_folder.join("ImageEffects")
    }

    /// `AppPaths.ModelsFolder`.
    pub fn models_folder(&self) -> PathBuf {
        self.personal_folder.join("Models")
    }

    /// `AppPaths.HistoryFilePath`.
    pub fn history_file(&self) -> Option<PathBuf> {
        (!self.sandbox).then(|| self.personal_folder.join("History.db"))
    }

    /// `SettingManager.ApplicationConfigFilePath`.
    pub fn application_config_file(&self) -> Option<PathBuf> {
        (!self.sandbox).then(|| self.personal_folder.join(APPLICATION_CONFIG_FILE_NAME))
    }

    /// `SettingManager.HotkeysConfigFilePath`: in `CustomHotkeysConfigPath`
    /// when that's set.
    pub fn hotkeys_config_file(&self, settings: Option<&ApplicationConfig>) -> Option<PathBuf> {
        if self.sandbox {
            return None;
        }
        let folder = match settings.and_then(|s| s.CustomHotkeysConfigPath.as_deref()).filter(|p| !p.is_empty()) {
            Some(custom) => PathBuf::from(expand_folder_variables(custom)),
            None => self.personal_folder.clone(),
        };
        Some(folder.join(HOTKEYS_CONFIG_FILE_NAME))
    }

    /// `SettingManager.UploadersConfigFilePath` (without the machine-specific
    /// copy, which comes with the uploaders).
    pub fn uploaders_config_file(&self, settings: Option<&ApplicationConfig>) -> Option<PathBuf> {
        if self.sandbox {
            return None;
        }
        let folder = match settings.and_then(|s| s.CustomUploadersConfigPath.as_deref()).filter(|p| !p.is_empty()) {
            Some(custom) => PathBuf::from(expand_folder_variables(custom)),
            None => self.personal_folder.clone(),
        };
        Some(folder.join(UPLOADERS_CONFIG_FILE_NAME))
    }

    /// `AppPaths.ScreenshotsParentFolder`.
    pub fn screenshots_parent_folder(&self, settings: Option<&ApplicationConfig>) -> PathBuf {
        if let Some(s) = settings.filter(|s| s.UseCustomScreenshotsPath) {
            let fallback = s.CustomScreenshotsPath2.as_deref().filter(|p| !p.is_empty());
            if !s.CustomScreenshotsPath.is_empty() {
                let primary = PathBuf::from(expand_folder_variables(&s.CustomScreenshotsPath));
                if fallback.is_none() || primary.is_dir() {
                    return primary;
                }
            }
            if let Some(fallback) = fallback {
                let fallback = PathBuf::from(expand_folder_variables(fallback));
                if fallback.is_dir() {
                    return fallback;
                }
            }
        }
        self.personal_folder.join("Screenshots")
    }

    /// `PersonalPathManager.CreateApplicationFolders()`.
    pub fn create_application_folders(&self, settings: Option<&ApplicationConfig>) {
        if !self.sandbox && self.personal_folder.is_dir() {
            for folder in [self.backup_folder(), self.image_effects_folder(), self.screenshots_parent_folder(settings)] {
                let _ = fs::create_dir_all(folder);
            }
        }
    }
}

/// `PersonalPathManager.ConfigFilePath`: next to the executable if there's
/// one there, else in `Documents/ShareX`.
fn personal_path_config_file() -> PathBuf {
    let relative = absolute_path(PERSONAL_PATH_CONFIG_FILE_NAME);
    if relative.is_file() { relative } else { default_personal_folder().join(PERSONAL_PATH_CONFIG_FILE_NAME) }
}

/// `PersonalPathManager.ReadConfig()`.
pub fn read_personal_path_config() -> String {
    fs::read_to_string(personal_path_config_file())
        .map(|s| s.trim_start_matches('\u{feff}').trim().to_owned())
        .unwrap_or_default()
}

/// `PersonalPathManager.WriteConfig()`: stores `path` (trimmed) unless it's
/// what's there already, or empty with no file. Returns whether it wrote.
pub fn write_personal_path_config(path: &str) -> Result<bool, String> {
    let path = path.trim();
    let file = personal_path_config_file();
    if (path.is_empty() && !file.exists()) || path.eq_ignore_ascii_case(&read_personal_path_config()) {
        return Ok(false);
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&file, path).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(true)
}

/// `PersonalPathManager.MigrateConfig()`: moves a `PersonalPath.cfg` from
/// ShareX's old place (`%LOCALAPPDATA%/ShareX`) to `Documents/ShareX`.
fn migrate_personal_path_config() {
    let Some(previous) = dirs::data_local_dir().map(|d| d.join(APPLICATION_NAME).join(PERSONAL_PATH_CONFIG_FILE_NAME))
    else {
        return;
    };
    if !previous.is_file() {
        return;
    }
    let current = default_personal_folder().join(PERSONAL_PATH_CONFIG_FILE_NAME);
    if !current.exists() {
        let _ = current.parent().map(fs::create_dir_all);
        let _ = fs::rename(&previous, &current);
    }
    let _ = fs::remove_file(&previous);
    if let Some(dir) = previous.parent() {
        let _ = fs::remove_dir(dir);
    }
}

/// `FileHelpers.ExpandFolderVariables`: `%SpecialFolder%` names
/// (`Environment.SpecialFolder`), then environment variables.
pub fn expand_folder_variables(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut out = path.to_owned();
    for (name, dir) in special_folders() {
        let token = format!("%{name}%");
        out = replace_ignore_case(&out, &token, &dir.to_string_lossy());
    }
    expand_environment_variables(&out)
}

fn special_folders() -> Vec<(&'static str, PathBuf)> {
    let mut v = Vec::new();
    let mut add = |name, dir: Option<PathBuf>| {
        if let Some(d) = dir {
            v.push((name, d));
        }
    };
    add("Desktop", dirs::desktop_dir());
    add("DesktopDirectory", dirs::desktop_dir());
    add("MyDocuments", dirs::document_dir());
    add("Personal", dirs::document_dir());
    add("MyMusic", dirs::audio_dir());
    add("MyVideos", dirs::video_dir());
    add("MyPictures", dirs::picture_dir());
    add("ApplicationData", dirs::config_dir());
    add("LocalApplicationData", dirs::data_local_dir());
    add("UserProfile", dirs::home_dir());
    add("Fonts", dirs::font_dir());
    v
}

fn replace_ignore_case(text: &str, token: &str, with: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let token_lower = token.to_ascii_lowercase();
    let mut out = String::new();
    let mut last = 0;
    for (i, _) in lower.match_indices(&token_lower) {
        out.push_str(&text[last..i]);
        out.push_str(with);
        last = i + token.len();
    }
    out.push_str(&text[last..]);
    out
}

/// `Environment.ExpandEnvironmentVariables`: `%NAME%`, left as is when unset.
fn expand_environment_variables(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else { break };
        let name = &after[..end];
        match std::env::var(name) {
            Ok(value) if !name.is_empty() => {
                out.push_str(&rest[..start]);
                out.push_str(&value);
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&rest[..start + 1]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(windows)]
mod registry {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_DWORD, REG_QWORD, REG_SZ, RRF_RT_ANY, RegGetValueW,
    };

    const PATH: &str = "SOFTWARE\\ShareXrs";

    enum RegValue {
        Number(u64),
        Text(String),
    }

    fn read(hive: HKEY, name: &str) -> Option<RegValue> {
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (path, name) = (wide(PATH), wide(name));
        let mut kind = 0u32;
        let mut size = 0u32;
        let status = unsafe {
            RegGetValueW(hive, path.as_ptr(), name.as_ptr(), RRF_RT_ANY, &mut kind, std::ptr::null_mut(), &mut size)
        };
        if status != 0 || size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let status = unsafe {
            RegGetValueW(hive, path.as_ptr(), name.as_ptr(), RRF_RT_ANY, &mut kind, buf.as_mut_ptr().cast(), &mut size)
        };
        if status != 0 {
            return None;
        }
        buf.truncate(size as usize);
        match kind {
            REG_DWORD if buf.len() >= 4 => Some(RegValue::Number(u32::from_le_bytes(buf[..4].try_into().ok()?) as u64)),
            REG_QWORD if buf.len() >= 8 => Some(RegValue::Number(u64::from_le_bytes(buf[..8].try_into().ok()?))),
            REG_SZ => {
                let units: Vec<u16> = buf.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
                let text = String::from_utf16_lossy(&units);
                Some(RegValue::Text(text.trim_end_matches('\0').to_owned()))
            }
            _ => None,
        }
    }

    /// `GetSystemOptionBoolean`: `Convert.ToBoolean` of the value (a number,
    /// or "true"/"false"), HKLM first.
    pub fn boolean(name: &str) -> bool {
        for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            match read(hive, name) {
                Some(RegValue::Number(n)) => return n != 0,
                Some(RegValue::Text(t)) if t.trim().eq_ignore_ascii_case("true") => return true,
                Some(RegValue::Text(t)) if t.trim().eq_ignore_ascii_case("false") => return false,
                _ => {}
            }
        }
        false
    }

    /// `GetSystemOptionString`.
    pub fn string(name: &str) -> Option<String> {
        [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER].into_iter().find_map(|hive| match read(hive, name) {
            Some(RegValue::Text(t)) => Some(t),
            Some(RegValue::Number(n)) => Some(n.to_string()),
            None => None,
        })
    }
}

#[cfg(not(windows))]
mod registry {
    pub fn boolean(_name: &str) -> bool {
        false
    }

    pub fn string(_name: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_variables() {
        // SAFETY: tests in this module don't read this variable concurrently.
        unsafe { std::env::set_var("SHAREXRS_TEST_DIR", "C:/x") };
        assert_eq!(expand_folder_variables("%SHAREXRS_TEST_DIR%/a"), "C:/x/a");
        // .NET matches environment variable names ignoring case only on
        // Windows, where the variables themselves do.
        if cfg!(windows) {
            assert_eq!(expand_folder_variables("%sharexrs_test_dir%/a"), "C:/x/a");
        }
        assert_eq!(expand_folder_variables("%NO_SUCH_VAR_X%/a"), "%NO_SUCH_VAR_X%/a");
        assert_eq!(expand_folder_variables("100% done"), "100% done");
        // Headless Linux (CI) has no XDG Documents folder.
        if let Some(docs) = dirs::document_dir() {
            assert_eq!(expand_folder_variables("%mydocuments%"), docs.to_string_lossy());
        }
    }

    #[test]
    fn command_line() {
        let args = vec!["-Portable".to_string(), "--silent".into()];
        assert!(is_command_present(&args, &["portable", "p"]));
        assert!(is_command_present(&args, &["silent", "s"]));
        assert!(!is_command_present(&args, &["sandbox"]));
    }

    #[test]
    fn file_locations() {
        let paths = AppPaths::in_folder("/p");
        let mut settings = ApplicationConfig::default();
        assert_eq!(paths.hotkeys_config_file(Some(&settings)), Some(PathBuf::from("/p").join("HotkeysConfig.json")));
        settings.CustomHotkeysConfigPath = Some("/q".into());
        assert_eq!(paths.hotkeys_config_file(Some(&settings)), Some(PathBuf::from("/q").join("HotkeysConfig.json")));
        assert_eq!(paths.backup_folder(), PathBuf::from("/p").join("Backup"));
    }
}
