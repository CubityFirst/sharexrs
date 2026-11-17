//! Global hotkeys: `ShareX/HotkeyManager.cs` over a platform host
//! (`IHotkeyHost`: `WindowsHotkeyHost` in ShareX.HelpersLib).
//!
//! The host registers hotkeys with the OS and reports presses (their id) on
//! its own thread through a callback, which the app turns into an event on
//! its winit loop; [`HotkeyManager::on_hotkey_pressed`] then finds the
//! hotkey's settings. Registration status lives on each
//! [`HotkeyInfo`](sharex_settings::HotkeyInfo) (`Status`, `ID`), as in ShareX.
//!
//! - Windows: `RegisterHotKey` on a thread of its own ([`windows`]).
//! - macOS and Linux: livesplit-hotkey, as snapr uses ([`hook`]). On macOS
//!   it needs the Accessibility permission (MACOS-LEARNINGS 3.2), which
//!   livesplit-hotkey asks for when its hook fails; until it's granted every
//!   hotkey shows as failed.

use std::sync::Arc;
use std::time::{Duration, Instant};

use sharex_settings::{HotkeyInfo, HotkeySettings, HotkeyStatus, HotkeyType};
use sharex_strings::sharex as s;

#[cfg(not(windows))]
mod hook;
#[cfg(windows)]
mod windows;

/// Called (on the host's thread) with the id of a hotkey that was pressed.
pub type OnPress = Arc<dyn Fn(u16) + Send + Sync>;

/// `ShareX.HelpersLib.IHotkeyHost`: registers hotkeys with the OS.
pub trait HotkeyHost {
    /// `RegisterHotkey`: sets `status` (and `id`) on `info`.
    fn register(&mut self, info: &mut HotkeyInfo);
    /// `UnregisterHotkey`.
    fn unregister(&mut self, info: &mut HotkeyInfo) -> bool;
}

/// The platform's host.
pub fn platform_host(on_press: OnPress) -> Box<dyn HotkeyHost> {
    #[cfg(windows)]
    return Box::new(windows::WindowsHotkeyHost::new(on_press));
    #[cfg(not(windows))]
    return Box::new(hook::HookHotkeyHost::new(on_press));
}

/// `ShareX.HotkeyManager`. The hotkeys themselves are `HotkeysConfig.Hotkeys`,
/// passed in, as ShareX's manager shares that list.
pub struct HotkeyManager {
    host: Box<dyn HotkeyHost>,
    /// `IgnoreHotkeys`: presses are dropped (while the hotkey settings
    /// window records a new hotkey, in ShareX).
    pub ignore_hotkeys: bool,
    /// `IHotkeyHost.HotkeyRepeatLimit`: holding a hotkey fires it at most
    /// this often (`ApplicationConfig.HotkeyRepeatLimit`).
    pub repeat_limit: Duration,
    last_press: Option<Instant>,
}

impl HotkeyManager {
    pub fn new(host: Box<dyn HotkeyHost>) -> Self {
        // `WindowsHotkeyHost.HotkeyRepeatLimit`'s default, until settings apply theirs.
        Self { host, ignore_hotkeys: false, repeat_limit: Duration::from_millis(1000), last_press: None }
    }

    /// `UpdateHotkeys(hotkeys, showFailedHotkeys)`: registers every hotkey
    /// (unless hotkeys are disabled). Returns the "Hotkey registration
    /// failed" message to show, if any failed.
    pub fn update_hotkeys(&mut self, hotkeys: &mut [HotkeySettings], disable_hotkeys: bool) -> Option<FailedHotkeys> {
        self.register_all(hotkeys, disable_hotkeys);
        failed_hotkeys(hotkeys)
    }

    /// `RegisterHotkey`: with hotkeys disabled only the "Disable hotkeys"
    /// one stays registered, so it can turn them back on.
    pub fn register_hotkey(&mut self, hotkey: &mut HotkeySettings, disable_hotkeys: bool) {
        if !disable_hotkeys || hotkey.TaskSettings.Job == HotkeyType::DisableHotkeys {
            self.unregister_hotkey(hotkey);
            if hotkey.HotkeyInfo.status != HotkeyStatus::Registered && hotkey.HotkeyInfo.is_valid_hotkey() {
                self.host.register(&mut hotkey.HotkeyInfo);
                match hotkey.HotkeyInfo.status {
                    HotkeyStatus::Registered => log(&format!("Hotkey registered: {hotkey}")),
                    HotkeyStatus::Failed => log(&format!("Hotkey register failed: {hotkey}")),
                    HotkeyStatus::NotConfigured => {}
                }
            } else {
                hotkey.HotkeyInfo.status = HotkeyStatus::NotConfigured;
            }
        }
    }

    /// `RegisterAllHotkeys`.
    pub fn register_all(&mut self, hotkeys: &mut [HotkeySettings], disable_hotkeys: bool) {
        for h in hotkeys {
            self.register_hotkey(h, disable_hotkeys);
        }
    }

    /// `RegisterFailedHotkeys` (the hotkey settings window's retry).
    #[allow(dead_code)]
    pub fn register_failed(&mut self, hotkeys: &mut [HotkeySettings], disable_hotkeys: bool) {
        for h in hotkeys.iter_mut().filter(|h| h.HotkeyInfo.status == HotkeyStatus::Failed) {
            self.register_hotkey(h, disable_hotkeys);
        }
    }

    /// `UnregisterHotkey` (keeping it in the list).
    pub fn unregister_hotkey(&mut self, hotkey: &mut HotkeySettings) {
        if hotkey.HotkeyInfo.status == HotkeyStatus::Registered {
            self.host.unregister(&mut hotkey.HotkeyInfo);
            match hotkey.HotkeyInfo.status {
                HotkeyStatus::NotConfigured => log(&format!("Hotkey unregistered: {hotkey}")),
                HotkeyStatus::Failed => log(&format!("Hotkey unregister failed: {hotkey}")),
                HotkeyStatus::Registered => {}
            }
        }
    }

    /// `UnregisterAllHotkeys(removeFromList: false, temporary)`: a temporary
    /// unregister (disabling hotkeys) keeps the "Disable hotkeys" hotkey.
    pub fn unregister_all(&mut self, hotkeys: &mut [HotkeySettings], temporary: bool) {
        for h in hotkeys {
            if !temporary || h.TaskSettings.Job != HotkeyType::DisableHotkeys {
                self.unregister_hotkey(h);
            }
        }
    }

    /// `ToggleHotkeys(hotkeysDisabled)`.
    pub fn toggle_hotkeys(&mut self, hotkeys: &mut [HotkeySettings], disabled: bool) {
        if disabled {
            self.unregister_all(hotkeys, true);
        } else {
            self.register_all(hotkeys, false);
        }
    }

    /// `OnHotkeyPressed`: the hotkey with that id, unless presses are
    /// ignored, come faster than the repeat limit
    /// (`WindowsHotkeyHost.CheckRepeatLimitTime`), or the active window is
    /// fullscreen and `DisableHotkeysOnFullscreen` is on.
    pub fn on_hotkey_pressed(
        &mut self,
        id: u16,
        hotkeys: &[HotkeySettings],
        disable_on_fullscreen: bool,
    ) -> Option<usize> {
        if !self.check_repeat_limit_time() {
            return None;
        }
        if self.ignore_hotkeys || (disable_on_fullscreen && is_active_window_fullscreen()) {
            return None;
        }
        hotkeys.iter().position(|h| h.HotkeyInfo.id == id && h.HotkeyInfo.status == HotkeyStatus::Registered)
    }

    fn check_repeat_limit_time(&mut self) -> bool {
        if !self.repeat_limit.is_zero() {
            if self.last_press.is_some_and(|t| t.elapsed() < self.repeat_limit) {
                return false;
            }
            self.last_press = Some(Instant::now());
        }
        true
    }
}

/// The "Hotkey registration failed" message box (`ShowFailedHotkeys`).
#[derive(Debug, Clone, PartialEq)]
pub struct FailedHotkeys {
    pub caption: String,
    pub text: String,
}

/// `HotkeyManager.ShowFailedHotkeys()`'s message, if any hotkey failed.
pub fn failed_hotkeys(hotkeys: &[HotkeySettings]) -> Option<FailedHotkeys> {
    let failed: Vec<&HotkeySettings> = hotkeys.iter().filter(|h| h.HotkeyInfo.status == HotkeyStatus::Failed).collect();
    if failed.is_empty() {
        return None;
    }
    let list: Vec<String> =
        failed.iter().map(|h| format!("[{}] {}", h.HotkeyInfo, h.TaskSettings.display_name())).collect();
    let noun =
        if failed.len() > 1 { s::HotkeyManager_ShowFailedHotkeys_hotkeys() } else { s::HotkeyManager_ShowFailedHotkeys_hotkey() };
    let text = s::HotkeyManager_ShowFailedHotkeys_Unable_to_register_hotkey()
        .replace("{0}", noun)
        .replace("{1}", &list.join("\n"))
        .replace("\r\n", "\n");
    // "ShareX - " in ShareX: ShareXrs's windows carry its own name.
    let caption = format!("ShareXrs - {}", s::HotkeyManager_ShowFailedHotkeys_Hotkey_registration_failed());
    Some(FailedHotkeys { caption, text })
}

/// `CaptureHelpers.IsActiveWindowFullscreen()`: the foreground window
/// (other than the desktop) covers its whole monitor.
#[cfg(windows)]
pub fn is_active_window_fullscreen() -> bool {
    windows::is_active_window_fullscreen()
}

/// Not ported outside Windows: never fullscreen.
#[cfg(not(windows))]
pub fn is_active_window_fullscreen() -> bool {
    false
}

fn log(message: &str) {
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sharex_settings::{HotkeysConfig, InputKey, TaskSettings};

    /// A host that fails the keys in `taken`.
    struct FakeHost {
        taken: Vec<InputKey>,
        next: u16,
        registered: Vec<u16>,
    }

    impl HotkeyHost for FakeHost {
        fn register(&mut self, info: &mut HotkeyInfo) {
            if self.taken.contains(&info.hotkey) {
                info.status = HotkeyStatus::Failed;
                return;
            }
            self.next += 1;
            info.id = self.next;
            info.status = HotkeyStatus::Registered;
            self.registered.push(info.id);
        }

        fn unregister(&mut self, info: &mut HotkeyInfo) -> bool {
            self.registered.retain(|&id| id != info.id);
            info.id = 0;
            info.status = HotkeyStatus::NotConfigured;
            true
        }
    }

    fn manager(taken: Vec<InputKey>) -> HotkeyManager {
        HotkeyManager::new(Box::new(FakeHost { taken, next: 0, registered: Vec::new() }))
    }

    #[test]
    fn failed_hotkeys_message() {
        let mut config = HotkeysConfig::new(&TaskSettings::default());
        let mut m = manager(vec![InputKey::Control | InputKey::PrintScreen, InputKey::Shift | InputKey::PrintScreen]);
        let failed = m.update_hotkeys(&mut config.Hotkeys, false).unwrap();
        assert_eq!(failed.caption, "ShareXrs - Hotkey registration failed");
        assert_eq!(
            failed.text,
            "Unable to register hotkeys:\n\n[Ctrl + Print Screen] Capture region\n[Shift + Print Screen] Start/Stop screen recording\n\nPlease select a different hotkey or quit the conflicting application and reopen ShareX."
        );
        let statuses: Vec<HotkeyStatus> = config.Hotkeys.iter().map(|h| h.HotkeyInfo.status).collect();
        use HotkeyStatus::*;
        assert_eq!(statuses, [Failed, Registered, Registered, Failed, Registered]);
    }

    #[test]
    fn disable_hotkeys_keeps_the_toggle() {
        let mut config = HotkeysConfig::new(&TaskSettings::default());
        config.Hotkeys.push(HotkeySettings::new(HotkeyType::DisableHotkeys, InputKey::F12, &TaskSettings::default()));
        config.Hotkeys.push(HotkeySettings::new(HotkeyType::OpenMainWindow, InputKey::None, &TaskSettings::default()));
        let mut m = manager(Vec::new());
        assert_eq!(m.update_hotkeys(&mut config.Hotkeys, true), None);
        let registered = |c: &HotkeysConfig| c.Hotkeys.iter().filter(|h| h.HotkeyInfo.status == HotkeyStatus::Registered).count();
        assert_eq!(registered(&config), 1, "only Disable hotkeys while disabled");
        m.toggle_hotkeys(&mut config.Hotkeys, false);
        assert_eq!(registered(&config), 6);
        assert_eq!(config.Hotkeys[6].HotkeyInfo.status, HotkeyStatus::NotConfigured);
        m.toggle_hotkeys(&mut config.Hotkeys, true);
        assert_eq!(registered(&config), 1);
    }

    #[test]
    fn presses_find_their_hotkey_with_a_repeat_limit() {
        let mut config = HotkeysConfig::new(&TaskSettings::default());
        let mut m = manager(Vec::new());
        m.update_hotkeys(&mut config.Hotkeys, false);
        let id = config.Hotkeys[2].HotkeyInfo.id;
        assert_eq!(m.on_hotkey_pressed(id, &config.Hotkeys, false), Some(2));
        assert_eq!(m.on_hotkey_pressed(id, &config.Hotkeys, false), None, "within the repeat limit");
        m.repeat_limit = Duration::ZERO;
        assert_eq!(m.on_hotkey_pressed(id, &config.Hotkeys, false), Some(2));
        m.ignore_hotkeys = true;
        assert_eq!(m.on_hotkey_pressed(id, &config.Hotkeys, false), None);
    }
}
