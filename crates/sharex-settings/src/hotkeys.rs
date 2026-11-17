//! Hotkey settings: `InputKey` and `HotkeyInfo` (ShareX.HelpersLib/Input),
//! `HotkeySettings`, `HotkeysConfig` and the default hotkeys
//! (`HotkeyManager.GetDefaultHotkeyList`) from ShareX/.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use sharex_strings::helpers_lib as hs;

use crate::enums::HotkeyType;
use crate::task_settings::TaskSettings;
use crate::{dotnet_flags, settings};

dotnet_flags! {
    /// `ShareX.HelpersLib.InputKey`: a Windows virtual-key code in the low
    /// 16 bits and Shift/Control/Alt above (`System.Windows.Forms.Keys`).
    /// Where several names share a value, the one listed here is the one
    /// .NET's `ToString()` prints (checked against .NET 11).
    pub struct InputKey: u32 {
        const None = 0;
        const LButton = 1;
        const RButton = 2;
        const Cancel = 3;
        const MButton = 4;
        const XButton1 = 5;
        const XButton2 = 6;
        const Back = 8;
        const Tab = 9;
        const LineFeed = 10;
        const Clear = 12;
        const Enter = 13;
        const ShiftKey = 16;
        const ControlKey = 17;
        const Menu = 18;
        const Pause = 19;
        const Capital = 20;
        const KanaMode = 21;
        const JunjaMode = 23;
        const FinalMode = 24;
        const KanjiMode = 25;
        const Escape = 27;
        const IMEConvert = 28;
        const IMENonconvert = 29;
        const IMEAceept = 30;
        const IMEModeChange = 31;
        const Space = 32;
        const PageUp = 33;
        const Next = 34;
        const End = 35;
        const Home = 36;
        const Left = 37;
        const Up = 38;
        const Right = 39;
        const Down = 40;
        const Select = 41;
        const Print = 42;
        const Execute = 43;
        const PrintScreen = 44;
        const Insert = 45;
        const Delete = 46;
        const Help = 47;
        const D0 = 48;
        const D1 = 49;
        const D2 = 50;
        const D3 = 51;
        const D4 = 52;
        const D5 = 53;
        const D6 = 54;
        const D7 = 55;
        const D8 = 56;
        const D9 = 57;
        const A = 65;
        const B = 66;
        const C = 67;
        const D = 68;
        const E = 69;
        const F = 70;
        const G = 71;
        const H = 72;
        const I = 73;
        const J = 74;
        const K = 75;
        const L = 76;
        const M = 77;
        const N = 78;
        const O = 79;
        const P = 80;
        const Q = 81;
        const R = 82;
        const S = 83;
        const T = 84;
        const U = 85;
        const V = 86;
        const W = 87;
        const X = 88;
        const Y = 89;
        const Z = 90;
        const LWin = 91;
        const RWin = 92;
        const Apps = 93;
        const Sleep = 95;
        const NumPad0 = 96;
        const NumPad1 = 97;
        const NumPad2 = 98;
        const NumPad3 = 99;
        const NumPad4 = 100;
        const NumPad5 = 101;
        const NumPad6 = 102;
        const NumPad7 = 103;
        const NumPad8 = 104;
        const NumPad9 = 105;
        const Multiply = 106;
        const Add = 107;
        const Separator = 108;
        const Subtract = 109;
        const Decimal = 110;
        const Divide = 111;
        const F1 = 112;
        const F2 = 113;
        const F3 = 114;
        const F4 = 115;
        const F5 = 116;
        const F6 = 117;
        const F7 = 118;
        const F8 = 119;
        const F9 = 120;
        const F10 = 121;
        const F11 = 122;
        const F12 = 123;
        const F13 = 124;
        const F14 = 125;
        const F15 = 126;
        const F16 = 127;
        const F17 = 128;
        const F18 = 129;
        const F19 = 130;
        const F20 = 131;
        const F21 = 132;
        const F22 = 133;
        const F23 = 134;
        const F24 = 135;
        const NumLock = 144;
        const Scroll = 145;
        const LShiftKey = 160;
        const RShiftKey = 161;
        const LControlKey = 162;
        const RControlKey = 163;
        const LMenu = 164;
        const RMenu = 165;
        const BrowserBack = 166;
        const BrowserForward = 167;
        const BrowserRefresh = 168;
        const BrowserStop = 169;
        const BrowserSearch = 170;
        const BrowserFavorites = 171;
        const BrowserHome = 172;
        const VolumeMute = 173;
        const VolumeDown = 174;
        const VolumeUp = 175;
        const MediaNextTrack = 176;
        const MediaPreviousTrack = 177;
        const MediaStop = 178;
        const MediaPlayPause = 179;
        const LaunchMail = 180;
        const SelectMedia = 181;
        const LaunchApplication1 = 182;
        const LaunchApplication2 = 183;
        const OemSemicolon = 186;
        const Oemplus = 187;
        const Oemcomma = 188;
        const OemMinus = 189;
        const OemPeriod = 190;
        const Oem2 = 191;
        const Oemtilde = 192;
        const Oem4 = 219;
        const OemPipe = 220;
        const Oem6 = 221;
        const OemQuotes = 222;
        const Oem8 = 223;
        const Oem102 = 226;
        const ProcessKey = 229;
        const Packet = 231;
        const Attn = 246;
        const Crsel = 247;
        const Exsel = 248;
        const EraseEof = 249;
        const Play = 250;
        const Zoom = 251;
        const NoName = 252;
        const Pa1 = 253;
        const OemClear = 254;
        const KeyCode = 0xFFFF;
        const Shift = 0x10000;
        const Control = 0x20000;
        const Alt = 0x40000;
        const Modifiers = 0xFFFF0000u32;
    }
    aliases {
        const Return = 13;
        const CapsLock = 20;
        const HangulMode = 21;
        const HanguelMode = 21;
        const HanjaMode = 25;
        const IMEAccept = 30;
        const Prior = 33;
        const PageDown = 34;
        const Snapshot = 44;
        const Oem1 = 186;
        const OemQuestion = 191;
        const Oem3 = 192;
        const OemOpenBrackets = 219;
        const Oem5 = 220;
        const OemCloseBrackets = 221;
        const Oem7 = 222;
        const OemBackslash = 226;
    }
}

impl InputKey {
    /// `Hotkey & InputKey.KeyCode`.
    pub fn key_code(self) -> InputKey {
        InputKey(self.0 & InputKey::KeyCode.0)
    }

    /// `Hotkey & InputKey.Modifiers`.
    pub fn modifiers(self) -> InputKey {
        InputKey(self.0 & InputKey::Modifiers.0)
    }
}

/// `ShareX.HelpersLib.HotkeyStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HotkeyStatus {
    Registered,
    Failed,
    #[default]
    NotConfigured,
}

/// `ShareX.HelpersLib.Modifiers`: the modifier bits `RegisterHotKey` takes.
pub mod modifiers {
    pub const ALT: u32 = 1;
    pub const CONTROL: u32 = 2;
    pub const SHIFT: u32 = 4;
    pub const WIN: u32 = 8;
}

/// `ShareX.HelpersLib.HotkeyInfo`. Only `Hotkey` and `Win` are saved;
/// `id` and `status` are the registration's, at run time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HotkeyInfo {
    pub hotkey: InputKey,
    pub win: bool,
    /// The id it's registered under (0 when it isn't).
    pub id: u16,
    pub status: HotkeyStatus,
    /// Properties this version doesn't know.
    pub extra: Map<String, Value>,
}

impl HotkeyInfo {
    pub fn new(hotkey: InputKey) -> Self {
        Self { hotkey, ..Default::default() }
    }

    pub fn key_code(&self) -> InputKey {
        self.hotkey.key_code()
    }

    pub fn control(&self) -> bool {
        self.hotkey.contains(InputKey::Control)
    }

    pub fn shift(&self) -> bool {
        self.hotkey.contains(InputKey::Shift)
    }

    pub fn alt(&self) -> bool {
        self.hotkey.contains(InputKey::Alt)
    }

    /// `ModifiersEnum`: the [`modifiers`] bits.
    pub fn modifiers_enum(&self) -> u32 {
        let mut m = 0;
        if self.alt() {
            m |= modifiers::ALT;
        }
        if self.control() {
            m |= modifiers::CONTROL;
        }
        if self.shift() {
            m |= modifiers::SHIFT;
        }
        if self.win {
            m |= modifiers::WIN;
        }
        m
    }

    /// `IsOnlyModifiers`.
    pub fn is_only_modifiers(&self) -> bool {
        let k = self.key_code();
        k == InputKey::ControlKey || k == InputKey::ShiftKey || k == InputKey::Menu || (k == InputKey::None && self.win)
    }

    /// `IsValidHotkey`.
    pub fn is_valid_hotkey(&self) -> bool {
        self.key_code() != InputKey::None && !self.is_only_modifiers()
    }
}

/// `HotkeyInfo.ToString()`: "Ctrl + Shift + Print Screen".
impl std::fmt::Display for HotkeyInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut text = String::new();
        if self.control() {
            text += &format!("{} + ", hs::HotkeyInfo_Ctrl());
        }
        if self.shift() {
            text += &format!("{} + ", hs::HotkeyInfo_Shift());
        }
        if self.alt() {
            text += &format!("{} + ", hs::HotkeyInfo_Alt());
        }
        if self.win {
            text += &format!("{} + ", hs::HotkeyInfo_Win());
        }
        let k = self.key_code();
        if self.is_only_modifiers() {
            text += "...";
        } else if k == InputKey::Back {
            text += hs::HotkeyInfo_Backspace();
        } else if k == InputKey::Return {
            text += hs::HotkeyInfo_Enter();
        } else if k == InputKey::Capital {
            text += hs::HotkeyInfo_Caps_lock();
        } else if k == InputKey::Next {
            text += hs::HotkeyInfo_Page_down();
        } else if k == InputKey::Scroll {
            text += hs::HotkeyInfo_Scroll_lock();
        } else if (InputKey::D0.0..=InputKey::D9.0).contains(&k.0) {
            text += &(k.0 - InputKey::D0.0).to_string();
        } else if (InputKey::NumPad0.0..=InputKey::NumPad9.0).contains(&k.0) {
            text += &format!("{} {}", hs::HotkeyInfo_Numpad(), k.0 - InputKey::NumPad0.0);
        } else {
            text += &to_string_with_spaces(&k.to_dotnet_string());
        }
        f.write_str(&text)
    }
}

/// `HotkeyInfo.ToStringWithSpaces`: a space before every capital but the first.
fn to_string_with_spaces(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

impl Serialize for HotkeyInfo {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("Hotkey", &self.hotkey)?;
        m.serialize_entry("Win", &self.win)?;
        for (k, v) in &self.extra {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for HotkeyInfo {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut map = Map::<String, Value>::deserialize(d)?;
        let mut out = HotkeyInfo::default();
        if let Some(v) = crate::macros::take(&mut map, "Hotkey")
            && let Ok(v) = InputKey::deserialize(v)
        {
            out.hotkey = v;
        }
        if let Some(v) = crate::macros::take(&mut map, "Win")
            && let Ok(v) = bool::deserialize(v)
        {
            out.win = v;
        }
        out.extra = map;
        Ok(out)
    }
}

settings! {
    /// `ShareX.HotkeySettings`: a hotkey and the task it runs.
    pub struct HotkeySettings {
        HotkeyInfo: HotkeyInfo = HotkeyInfo::default(),
        TaskSettings: TaskSettings = TaskSettings::default(),
    }
}

impl HotkeySettings {
    /// `new HotkeySettings(job, hotkey)`: a task with the application's
    /// defaults (`TaskSettings.GetDefaultTaskSettings()`).
    pub fn new(job: HotkeyType, hotkey: InputKey, defaults: &TaskSettings) -> Self {
        let mut task = TaskSettings::default_task_settings(defaults);
        task.Job = job;
        Self { HotkeyInfo: HotkeyInfo::new(hotkey), TaskSettings: task, extra: Map::new() }
    }
}

/// `HotkeySettings.ToString()`.
impl std::fmt::Display for HotkeySettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Hotkey: {}, Description: {}, Job: {}",
            self.HotkeyInfo,
            self.TaskSettings.display_name(),
            self.TaskSettings.Job.name()
        )
    }
}

/// `HotkeyManager.GetDefaultHotkeyList()`.
pub fn default_hotkey_list(defaults: &TaskSettings) -> Vec<HotkeySettings> {
    let h = |job, key| HotkeySettings::new(job, key, defaults);
    vec![
        h(HotkeyType::RectangleRegion, InputKey::Control | InputKey::PrintScreen),
        h(HotkeyType::PrintScreen, InputKey::PrintScreen),
        h(HotkeyType::ActiveWindow, InputKey::Alt | InputKey::PrintScreen),
        h(HotkeyType::ScreenRecorder, InputKey::Shift | InputKey::PrintScreen),
        h(HotkeyType::ScreenRecorderGIF, InputKey::Control | InputKey::Shift | InputKey::PrintScreen),
    ]
}

settings! {
    /// `ShareX.HotkeysConfig` (`HotkeysConfig.json`).
    pub struct HotkeysConfig {
        Hotkeys: Vec<HotkeySettings> = default_hotkey_list(&TaskSettings::default()),
        /// `SettingsBase.ApplicationVersion`.
        ApplicationVersion: Option<String> = None,
    }
}

impl HotkeysConfig {
    /// A new config, as `new HotkeysConfig()` makes it once the application
    /// config is loaded: the default hotkeys with its default task settings.
    pub fn new(defaults: &TaskSettings) -> Self {
        Self { Hotkeys: default_hotkey_list(defaults), ..Default::default() }
    }

    /// `SettingManager.CleanupHotkeysConfig()`, done before every save.
    pub fn cleanup(&mut self) {
        for h in &mut self.Hotkeys {
            h.TaskSettings.cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(key: InputKey, win: bool) -> String {
        HotkeyInfo { hotkey: key, win, ..Default::default() }.to_string()
    }

    #[test]
    fn hotkey_text() {
        assert_eq!(text(InputKey::Control | InputKey::PrintScreen, false), "Ctrl + Print Screen");
        assert_eq!(text(InputKey::PrintScreen, false), "Print Screen");
        assert_eq!(text(InputKey::Control | InputKey::Shift | InputKey::PrintScreen, false), "Ctrl + Shift + Print Screen");
        assert_eq!(text(InputKey::Alt | InputKey::D5, true), "Alt + Win + 5");
        assert_eq!(text(InputKey::NumPad3, false), "Numpad 3");
        assert_eq!(text(InputKey::Control | InputKey::ControlKey, false), "Ctrl + ...");
        assert_eq!(text(InputKey::Next, false), "Page Down");
        assert_eq!(text(InputKey::PageUp, false), "Page Up");
        assert_eq!(text(InputKey::Return, false), "Enter");
        assert_eq!(text(InputKey::Oemtilde, false), "Oemtilde");
        assert_eq!(text(InputKey::MediaPlayPause, false), "Media Play Pause");
    }

    #[test]
    fn input_key_like_dotnet() {
        let s = |k: InputKey| serde_json::to_value(k).unwrap();
        assert_eq!(s(InputKey::Control | InputKey::PrintScreen), "PrintScreen, Control");
        assert_eq!(s(InputKey::Control | InputKey::Shift | InputKey::PrintScreen | InputKey::Alt), "PrintScreen, Shift, Control, Alt");
        assert_eq!(s(InputKey::Shift | InputKey::Control), "Shift, Control");
        assert_eq!(s(InputKey(7)), "LButton, XButton2");
        assert_eq!(s(InputKey(255)), "LButton, OemClear");
        assert_eq!(s(InputKey(237)), "Back, ProcessKey");
        assert_eq!(s(InputKey::None), "None");
        assert_eq!(serde_json::from_value::<InputKey>("Snapshot, control".into()).unwrap(), InputKey::Control | InputKey::PrintScreen);
    }

    #[test]
    fn validity() {
        assert!(HotkeyInfo::new(InputKey::PrintScreen).is_valid_hotkey());
        assert!(!HotkeyInfo::new(InputKey::None).is_valid_hotkey());
        assert!(!HotkeyInfo::new(InputKey::Control | InputKey::ControlKey).is_valid_hotkey());
        assert_eq!(HotkeyInfo::new(InputKey::Control | InputKey::Alt).modifiers_enum(), modifiers::CONTROL | modifiers::ALT);
    }
}
