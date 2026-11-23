//! Global hotkeys on macOS and Linux through livesplit-hotkey, as snapr
//! does (`src/hotkey/mod.rs` there): a keyboard hook that consumes the
//! hotkeys it knows if it can (`PreferConsume`).
//!
//! macOS: the hook needs the Accessibility permission; livesplit-hotkey
//! asks macOS to prompt for it when the hook can't be made, and every
//! hotkey shows as failed until ShareXrs is restarted with it granted.
//! Linux: X11 grabs. On Wayland only the input devices (evdev, which needs
//! the input group; the keys aren't consumed): X11 grabs there go to
//! XWayland, which sees keys only while an X11 window has the focus, so
//! they'd register and never fire. Without evdev every hotkey fails, as
//! the user is told. snapr also uses the desktop's global shortcuts portal
//! on Wayland; not ported here.
//!
//! ShareX's hotkeys are Windows virtual-key codes (`InputKey`); they map to
//! livesplit's (W3C) key codes below.

use std::collections::HashMap;

use livesplit_hotkey::{ConsumePreference, Hook, Hotkey, KeyCode, Modifiers};
use sharex_settings::hotkeys::modifiers as m;
use sharex_settings::{HotkeyInfo, HotkeyStatus};

use super::{HotkeyHost, OnPress};

pub struct HookHotkeyHost {
    hook: Option<Hook>,
    on_press: OnPress,
    registered: HashMap<u16, Hotkey>,
    next_id: u16,
}

impl HookHotkeyHost {
    pub fn new(on_press: OnPress) -> Self {
        #[cfg(target_os = "linux")]
        let wayland = sharex_capture::platform::linux::wayland();
        #[cfg(not(target_os = "linux"))]
        let wayland = false;
        let preference = if wayland { ConsumePreference::MustNotConsume } else { ConsumePreference::PreferConsume };
        let hook = match Hook::with_consume_preference(preference) {
            Ok(h) => Some(h),
            Err(e) if wayland => {
                eprintln!("global hotkeys unavailable: on Wayland they read the input devices, which needs the input group ({e})");
                None
            }
            Err(e) => {
                eprintln!("global hotkeys unavailable: {e}");
                None
            }
        };
        Self { hook, on_press, registered: HashMap::new(), next_id: 1 }
    }
}

impl HotkeyHost for HookHotkeyHost {
    fn register(&mut self, info: &mut HotkeyInfo) {
        if info.status == HotkeyStatus::Registered {
            return;
        }
        if !info.is_valid_hotkey() {
            info.status = HotkeyStatus::NotConfigured;
            return;
        }
        let (Some(hook), Some(hotkey)) = (&self.hook, to_hotkey(info)) else {
            info.status = HotkeyStatus::Failed;
            return;
        };
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let on_press = self.on_press.clone();
        match hook.register(hotkey, move || on_press(id)) {
            Ok(()) => {
                info.id = id;
                info.status = HotkeyStatus::Registered;
                self.registered.insert(id, hotkey);
            }
            Err(e) => {
                eprintln!("Unable to register hotkey: {info}: {e}");
                info.id = 0;
                info.status = HotkeyStatus::Failed;
            }
        }
    }

    fn unregister(&mut self, info: &mut HotkeyInfo) -> bool {
        if let (Some(hook), Some(hotkey)) = (&self.hook, self.registered.remove(&info.id))
            && hook.unregister(hotkey).is_ok()
        {
            info.id = 0;
            info.status = HotkeyStatus::NotConfigured;
            return true;
        }
        info.status = HotkeyStatus::Failed;
        false
    }
}

fn to_hotkey(info: &HotkeyInfo) -> Option<Hotkey> {
    let bits = info.modifiers_enum();
    let mut modifiers = Modifiers::empty();
    for (bit, modifier) in
        [(m::ALT, Modifiers::ALT), (m::CONTROL, Modifiers::CONTROL), (m::SHIFT, Modifiers::SHIFT), (m::WIN, Modifiers::META)]
    {
        if bits & bit != 0 {
            modifiers |= modifier;
        }
    }
    Some(Hotkey { key_code: key_code(info.key_code().0)?, modifiers })
}

/// A Windows virtual-key code as a W3C key code.
fn key_code(vk: u32) -> Option<KeyCode> {
    use KeyCode::*;
    const LETTERS: [KeyCode; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR,
        KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    const NUMPAD: [KeyCode; 10] =
        [Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9];
    const FUNCTION: [KeyCode; 24] = [
        F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    ];
    Some(match vk {
        0x41..=0x5A => LETTERS[(vk - 0x41) as usize],
        0x30..=0x39 => DIGITS[(vk - 0x30) as usize],
        0x60..=0x69 => NUMPAD[(vk - 0x60) as usize],
        0x70..=0x87 => FUNCTION[(vk - 0x70) as usize],
        0x08 => Backspace,
        0x09 => Tab,
        0x0C => NumpadClear,
        0x0D => Enter,
        0x13 => Pause,
        0x14 => CapsLock,
        0x1B => Escape,
        0x1C => Convert,
        0x1D => NonConvert,
        0x20 => Space,
        0x21 => PageUp,
        0x22 => PageDown,
        0x23 => End,
        0x24 => Home,
        0x25 => ArrowLeft,
        0x26 => ArrowUp,
        0x27 => ArrowRight,
        0x28 => ArrowDown,
        0x29 => Select,
        0x2C => PrintScreen,
        0x2D => Insert,
        0x2E => Delete,
        0x2F => Help,
        0x5D => ContextMenu,
        0x5F => Sleep,
        0x6A => NumpadMultiply,
        0x6B => NumpadAdd,
        0x6C => NumpadComma,
        0x6D => NumpadSubtract,
        0x6E => NumpadDecimal,
        0x6F => NumpadDivide,
        0x90 => NumLock,
        0x91 => ScrollLock,
        0xA6 => BrowserBack,
        0xA7 => BrowserForward,
        0xA8 => BrowserRefresh,
        0xA9 => BrowserStop,
        0xAA => BrowserSearch,
        0xAB => BrowserFavorites,
        0xAC => BrowserHome,
        0xAD => AudioVolumeMute,
        0xAE => AudioVolumeDown,
        0xAF => AudioVolumeUp,
        0xB0 => MediaTrackNext,
        0xB1 => MediaTrackPrevious,
        0xB2 => MediaStop,
        0xB3 => MediaPlayPause,
        0xB4 => LaunchMail,
        0xB5 => MediaSelect,
        0xB6 => LaunchApp1,
        0xB7 => LaunchApp2,
        0xBA => Semicolon,
        0xBB => Equal,
        0xBC => Comma,
        0xBD => Minus,
        0xBE => Period,
        0xBF => Slash,
        0xC0 => Backquote,
        0xDB => BracketLeft,
        0xDC => Backslash,
        0xDD => BracketRight,
        0xDE => Quote,
        0xE2 => IntlBackslash,
        _ => return None,
    })
}
