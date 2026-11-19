//! `ThemeManager.ApplyConfiguredSettings` (ShareX.Avalonia/Theming) and
//! `LanguageHelper.ChangeLanguage`: what the theme options and the
//! language in `ApplicationConfig` do to the running app.
//!
//! The accent palette, the spacing and the icon pack are app-wide
//! (`sharex_ui::theme::set_accent`, `set_spacing`, `icons::set_pack`), so
//! every window shows a change on its next frame; the theme is per window
//! (each egui context's), so the caller applies [`Appearance::theme`] to
//! each window.

use sharex_settings::{ApplicationConfig, ApplicationThemeOptions, IconPack, SupportedLanguage};
use sharex_ui::theme::{self, Spacing};

/// What the theme options resolve to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Appearance {
    pub theme: egui::Theme,
}

/// `ThemeManager.IsLightTheme`: any name containing "Light".
pub fn is_light(name: &str) -> bool {
    name.to_ascii_lowercase().contains("light")
}

/// `ApplyConfiguredSettings`: the theme (`UseSystemTheme` follows
/// `system`, the OS's), the accent (`UseSystemAccentColor` follows the
/// OS's when it has one), the icon pack and the spacing.
pub fn apply(settings: &ApplicationConfig, system: Option<winit::window::Theme>) -> Appearance {
    let o = settings.ThemeOptions.clone().unwrap_or_default();
    let light = match (o.UseSystemTheme, system) {
        (true, Some(t)) => t == winit::window::Theme::Light,
        _ => is_light(&o.Theme),
    };
    let mut accent = accent_color(&o);
    if o.UseSystemAccentColor
        && let Some(system) = system_accent_color()
    {
        accent = system;
    }
    theme::set_accent(accent);
    sharex_ui::icons::set_pack(match o.IconPack {
        IconPack::Legacy => sharex_ui::icons::IconPack::Legacy,
        _ => sharex_ui::icons::IconPack::Modern,
    });
    theme::set_spacing(spacing(&o));
    Appearance { theme: if light { egui::Theme::Light } else { egui::Theme::Dark } }
}

/// `ApplicationThemeOptions.AccentColor`: the hex colour, or the default
/// when it doesn't parse or is transparent.
pub fn accent_color(o: &ApplicationThemeOptions) -> egui::Color32 {
    sharex_ui::color_view::to_color32(accent_rgba(o))
}

/// [`accent_color`] as unmultiplied `[R, G, B, A]`, the exact value.
pub fn accent_rgba(o: &ApplicationThemeOptions) -> [u8; 4] {
    sharex_ui::accent::parse_hex_rgba(&o.AccentColorHex)
        .filter(|c| c[3] > 0)
        .or_else(|| sharex_ui::accent::parse_hex_rgba(ApplicationThemeOptions::DEFAULT_ACCENT_COLOR_HEX))
        .unwrap_or([0x09, 0xAA, 0xFF, 0xFF])
}

/// `ApplicationThemeOptions.GetSpacing` for each area: a per-area value, or
/// the compact (0) or default (100) preset.
pub fn spacing(o: &ApplicationThemeOptions) -> Spacing {
    let preset = if o.CompactMode { 0 } else { 100 };
    let get = |v: Option<i32>| v.unwrap_or(preset).clamp(0, 200) as f32;
    Spacing {
        sidebar: get(o.SidebarSpacing),
        menus: get(o.MenuSpacing),
        tool_menus: get(o.ToolMenuSpacing),
        settings: get(o.SettingsSpacing),
    }
}

/// `ApplicationThemeOptions.ResetSpacing`.
pub fn reset_spacing(o: &mut ApplicationThemeOptions) {
    o.SidebarSpacing = None;
    o.MenuSpacing = None;
    o.ToolMenuSpacing = None;
    o.SettingsSpacing = None;
}

/// The OS's accent colour (Avalonia's `PlatformColorValues.AccentColor1`):
/// on Windows `UISettings`' accent, which Explorer keeps as the fourth
/// entry of `Explorer\Accent\AccentPalette`.
#[cfg(windows)]
pub fn system_accent_color() -> Option<egui::Color32> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW};
    use windows::core::HSTRING;
    let key = HSTRING::from(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent");
    let name = HSTRING::from("AccentPalette");
    let mut data = [0u8; 32];
    let mut len = data.len() as u32;
    // SAFETY: `data` is `len` bytes long.
    let r = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, &key, &name, RRF_RT_REG_BINARY, None, Some(data.as_mut_ptr().cast()), Some(&mut len))
    };
    (r.is_ok() && len >= 16).then(|| egui::Color32::from_rgb(data[12], data[13], data[14]))
}

#[cfg(not(windows))]
pub fn system_accent_color() -> Option<egui::Color32> {
    None
}

/// The OS's UI language as a culture name (`CultureInfo.InstalledUICulture`).
#[cfg(windows)]
fn system_culture() -> Option<String> {
    use windows::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
    let mut buf = [0u16; 85];
    // SAFETY: `buf` is LOCALE_NAME_MAX_LENGTH long.
    let n = unsafe { LCIDToLocaleName(GetUserDefaultUILanguage() as u32, Some(&mut buf), 0) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
}

#[cfg(not(windows))]
fn system_culture() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .map(|v| v.split(['.', '@']).next().unwrap_or_default().replace('_', "-"))
}

/// The culture `language` means (`Automatic`: the OS's).
fn culture(language: &SupportedLanguage) -> String {
    match language {
        SupportedLanguage::Automatic => system_culture().unwrap_or_else(|| "en-US".into()),
        l => l.culture_name().into(),
    }
}

/// `LanguageHelper.ChangeLanguage`: switches every string to `language`.
/// Returns whether that changed the language in use.
pub fn change_language(language: &SupportedLanguage) -> bool {
    let before = sharex_strings::language();
    sharex_strings::set_language(&culture(language)) != before
}

/// Whether `language` would show other text than what's in use, without
/// switching (the settings window's "restart to apply").
pub fn language_differs(language: &SupportedLanguage) -> bool {
    let before = sharex_strings::language();
    let now = sharex_strings::set_language(&culture(language));
    sharex_strings::set_language(before);
    now != before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_follows_compact_mode_unless_set() {
        let mut o = ApplicationThemeOptions { CompactMode: true, ..Default::default() };
        assert_eq!(spacing(&o).sidebar, 0.0);
        o.MenuSpacing = Some(150);
        assert_eq!(spacing(&o).menus, 150.0);
        reset_spacing(&mut o);
        assert_eq!(spacing(&o).menus, 0.0);
    }

    #[test]
    fn bad_accent_falls_back_to_the_default() {
        let o = ApplicationThemeOptions { AccentColorHex: "#00000000".into(), ..Default::default() };
        assert_eq!(accent_color(&o), egui::Color32::from_rgb(0x09, 0xAA, 0xFF));
    }
}
