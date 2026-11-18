//! `ShareX/StartupManager.cs`: starting ShareXrs when you sign in, with
//! `-silent` (to the tray).
//!
//! - Windows, as ShareX: a shortcut in the Startup folder
//!   (`ShortcutHelpers.SetShortcut`), named after the app (`ShareXrs.lnk`,
//!   so it never touches ShareX's own), and "disabled by user" when Task
//!   Manager's Startup apps page turned it off (`StartupApproved\StartupFolder`).
//! - macOS: a LaunchAgent, `~/Library/LaunchAgents/com.sharexrs.app.plist`,
//!   with `RunAtLoad`; `launchctl` turning it off (`Disabled`) reads as
//!   disabled by the user.
//! - Linux: an XDG autostart entry, `~/.config/autostart/sharexrs.desktop`
//!   (as snapr's `autostart/linux.rs`); one the desktop's startup settings
//!   hid reads as disabled by the user.

/// `ShareX.StartupState` (the values `StartupTask.State` uses; the policy
/// ones come from the Microsoft Store build's startup task, which ShareXrs
/// doesn't have, but the views handle them as ShareX's do).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupState {
    Disabled,
    DisabledByUser,
    Enabled,
    DisabledByPolicy,
    EnabledByPolicy,
}

/// The file the entry starts: this executable, as `Environment.ProcessPath`.
fn target() -> Result<std::path::PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

#[cfg(windows)]
mod platform {
    use super::StartupState;
    use std::path::PathBuf;
    use windows::Win32::UI::Shell::FOLDERID_Startup;
    use windows::core::HSTRING;

    const NAME: &str = "ShareXrs.lnk";

    fn shortcut() -> Result<PathBuf, String> {
        Ok(crate::shortcut::known_folder(&FOLDERID_Startup)?.join(NAME))
    }

    /// Task Manager's "Disabled": the first byte of `ShareXrs.lnk`'s value
    /// under `StartupApproved\StartupFolder` is 3.
    fn disabled_by_user() -> bool {
        use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW};
        let key = HSTRING::from(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder");
        let name = HSTRING::from(NAME);
        let mut data = [0u8; 64];
        let mut len = data.len() as u32;
        // SAFETY: `data` is `len` bytes long.
        let r = unsafe {
            RegGetValueW(HKEY_CURRENT_USER, &key, &name, RRF_RT_REG_BINARY, None, Some(data.as_mut_ptr().cast()), Some(&mut len))
        };
        r.is_ok() && len > 0 && data[0] == 3
    }

    pub fn state() -> Result<StartupState, String> {
        let path = shortcut()?;
        let target = super::target()?;
        let ours = crate::shortcut::check(&path, &target);
        Ok(match (ours, ours && disabled_by_user()) {
            (true, true) => StartupState::DisabledByUser,
            (true, false) => StartupState::Enabled,
            _ => StartupState::Disabled,
        })
    }

    pub fn set(enabled: bool) -> Result<(), String> {
        let path = shortcut()?;
        if enabled { crate::shortcut::create(&path, &super::target()?, "-silent") } else { crate::shortcut::delete(&path) }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::StartupState;
    use std::path::PathBuf;

    const LABEL: &str = "com.sharexrs.app";

    fn plist() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or("couldn't find your home folder")?;
        Ok(home.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
    }

    fn escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    pub fn state() -> Result<StartupState, String> {
        let Ok(text) = std::fs::read_to_string(plist()?) else { return Ok(StartupState::Disabled) };
        let target = escape(&super::target()?.to_string_lossy());
        if !text.contains(&format!("<string>{target}</string>")) {
            return Ok(StartupState::Disabled);
        }
        // `launchctl disable` (System Settings › Login Items) keeps the file.
        let uid = std::process::Command::new("id").arg("-u").output().ok();
        let uid = uid.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let disabled = std::process::Command::new("launchctl")
            .args(["print-disabled", &format!("gui/{uid}")])
            .output()
            .ok()
            .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{LABEL}\" => disabled")));
        Ok(if disabled { StartupState::DisabledByUser } else { StartupState::Enabled })
    }

    pub fn set(enabled: bool) -> Result<(), String> {
        let path = plist()?;
        if !enabled {
            return match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            };
        }
        // An app bundle starts through `open`, so it launches as an app
        // (Dock, activation) rather than as a bare process.
        let target = super::target()?;
        let bundle = target.ancestors().find(|p| p.extension().is_some_and(|e| e == "app"));
        let args: Vec<String> = match bundle {
            Some(app) => vec!["/usr/bin/open".into(), "-a".into(), app.to_string_lossy().into(), "--args".into(), "-silent".into()],
            None => vec![target.to_string_lossy().into(), "-silent".into()],
        };
        let args: String = args.iter().map(|a| format!("\n        <string>{}</string>", escape(a))).collect();
        let text = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n    <key>Label</key>\n    <string>{LABEL}</string>\n    \
             <key>ProgramArguments</key>\n    <array>{args}\n    </array>\n    <key>RunAtLoad</key>\n    <true/>\n</dict>\n</plist>\n"
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, text).map_err(|e| e.to_string())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use super::StartupState;
    use std::path::{Path, PathBuf};

    fn entry() -> Result<PathBuf, String> {
        let dir = dirs::config_dir().ok_or("couldn't find your config folder")?;
        Ok(dir.join("autostart").join("sharexrs.desktop"))
    }

    /// Turned off by the desktop's startup settings without removing it.
    fn disabled(entry: &str) -> bool {
        entry.lines().map(str::trim).any(|l| {
            l.eq_ignore_ascii_case("Hidden=true") || l.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
        })
    }

    /// `path` as a desktop entry's `Exec` argument (snapr's `quote`).
    fn quote(path: &Path) -> String {
        let mut arg = String::from("\"");
        for c in path.to_string_lossy().chars() {
            if matches!(c, '"' | '`' | '$' | '\\') {
                arg.push('\\');
            }
            arg.push(c);
        }
        arg.push('"');
        arg.replace('\\', "\\\\").replace('%', "%%")
    }

    pub fn state() -> Result<StartupState, String> {
        let Ok(text) = std::fs::read_to_string(entry()?) else { return Ok(StartupState::Disabled) };
        let exec = format!("Exec={} -silent", quote(&super::target()?));
        if !text.lines().any(|l| l.trim() == exec) {
            return Ok(StartupState::Disabled);
        }
        Ok(if disabled(&text) { StartupState::DisabledByUser } else { StartupState::Enabled })
    }

    pub fn set(enabled: bool) -> Result<(), String> {
        let path = entry()?;
        if !enabled {
            return match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            };
        }
        let text = format!(
            "[Desktop Entry]\nType=Application\nName=ShareXrs\nComment=Screen capture, file sharing and productivity tool\n\
             Exec={} -silent\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            quote(&super::target()?)
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, text).map_err(|e| e.to_string())
    }
}

/// `StartupManager.State`'s getter.
pub fn state() -> Result<StartupState, String> {
    platform::state()
}

/// `StartupManager.State`'s setter: only `Enabled` and `Disabled` can be set.
pub fn set_state(state: StartupState) -> Result<(), String> {
    eprintln!("Startup: {state:?}");
    match state {
        StartupState::Enabled => platform::set(true),
        StartupState::Disabled => platform::set(false),
        _ => Err("not supported".into()),
    }
}

/// What a "Run ShareX on startup" control shows for the current state
/// (`RefreshStartWithWindows`): whether it's checked, whether it can be
/// changed, and its text.
pub struct StartupView {
    pub checked: bool,
    pub enabled: bool,
    pub text: &'static str,
}

pub fn view() -> StartupView {
    use sharex_strings::sharex as s;
    let mut v = StartupView { checked: false, enabled: false, text: s::ApplicationSettingsForm_cbStartWithWindows_Text() };
    match state() {
        Ok(state) => {
            v.checked = matches!(state, StartupState::Enabled | StartupState::EnabledByPolicy);
            match state {
                StartupState::DisabledByUser => v.text = s::ApplicationSettingsForm_cbStartWithWindows_DisabledByUser_Text(),
                StartupState::DisabledByPolicy => v.text = s::ApplicationSettingsForm_cbStartWithWindows_DisabledByPolicy_Text(),
                StartupState::EnabledByPolicy => v.text = s::ApplicationSettingsForm_cbStartWithWindows_EnabledByPolicy_Text(),
                _ => v.enabled = true,
            }
        }
        Err(e) => eprintln!("startup: {e}"),
    }
    v
}
