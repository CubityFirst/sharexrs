//! `ShareX/IntegrationHelpers.cs` and `Infrastructure/FileTypeRegistration.cs`:
//! the Explorer context menus, the `.sxcu` and `.sxie` file types, the
//! browsers' native messaging hosts and the Send To shortcut, registered at
//! run time in `HKEY_CURRENT_USER` as ShareX registers them, but under
//! ShareXrs's own names, so an installed ShareX is never touched:
//!
//! | ShareX | ShareXrs |
//! |---|---|
//! | `*\shell\ShareX`, `Directory\shell\ShareX` | `*\shell\ShareXrs`, `Directory\shell\ShareXrs` |
//! | `image\shell\ShareXImageEditor`, `ShareXPhotoEditor` | `ShareXrsImageEditor`, `ShareXrsPhotoEditor` |
//! | `.sxcu` → `ShareX.sxcu`, `.sxie` → `ShareX.sxie` | `ShareXrs.sxcu`, `ShareXrs.sxie`, see below |
//! | `NativeMessagingHosts\com.getsharex.sharex`, `\ShareX` | `\com.sharexrs.sharexrs`, `\ShareXrs` |
//! | `SendTo\ShareX.lnk` | `SendTo\ShareXrs.lnk` |
//!
//! `.sxcu` and `.sxie` are ShareX's extensions too. ShareX takes their
//! default over and deletes the whole extension key when it unregisters;
//! ShareXrs lists its ProgID in the extension's `OpenWithProgids`, takes the
//! default only when nothing else has it, and on unregistering removes only
//! what is its own (the same as its uninstaller does), so ShareX's
//! registration stays.
//!
//! The file type icon is the program's own (ShareX ships a separate
//! `ShareX_File_Icon.ico`).
//!
//! The registry goes through [`Registry`], so the tests can run on a
//! scratch key (`HKCU\Software\ShareXrsTest`) or in memory. Outside Windows
//! there's no registry: every check is false and every change does nothing.

use std::io;
use std::path::{Path, PathBuf};

use sharexrs_native_messaging as nm;

/// What [`Integration`] needs of `HKEY_CURRENT_USER` (`RegistryHelpers`).
pub trait Registry {
    /// A string value (`None` for the default value), if there is one.
    fn get_string(&self, path: &str, name: Option<&str>) -> Option<String>;
    /// `CreateRegistry(path, name, value)`: the key made if needed, and a
    /// string value set.
    fn set_string(&mut self, path: &str, name: Option<&str>, value: &str) -> io::Result<()>;
    /// An empty `REG_NONE` value, as `OpenWithProgids` lists ProgIDs.
    fn set_empty(&mut self, path: &str, name: &str) -> io::Result<()>;
    /// Whether the key has a value by this name (any type).
    fn has_value(&self, path: &str, name: Option<&str>) -> bool;
    fn delete_value(&mut self, path: &str, name: Option<&str>) -> io::Result<()>;
    /// `RemoveRegistry`: the key and everything under it, if it's there.
    fn delete_tree(&mut self, path: &str) -> io::Result<()>;
    /// Deletes the key if it has no values and no sub keys.
    fn delete_if_empty(&mut self, path: &str) -> io::Result<()>;
    /// `SHChangeNotify(SHCNE_ASSOCCHANGED)`.
    fn notify_association_changed(&self) {}
}

/// `IntegrationHelpers`, for the program at `app`.
pub struct Integration<R: Registry> {
    pub registry: R,
    /// `Environment.ProcessPath`.
    pub app: PathBuf,
}

const MENU_FILES: &str = r"Software\Classes\*\shell\ShareXrs";
const MENU_DIRECTORY: &str = r"Software\Classes\Directory\shell\ShareXrs";
const EDIT_IMAGE: &str = r"Software\Classes\SystemFileAssociations\image\shell\ShareXrsImageEditor";
const PHOTO_EDITOR_IMAGE: &str = r"Software\Classes\SystemFileAssociations\image\shell\ShareXrsPhotoEditor";

/// A file type: ShareX's `.sxcu` or `.sxie` registration, ShareXrs's way.
#[derive(Clone, Copy)]
pub struct FileType {
    pub extension: &'static str,
    pub prog_id: &'static str,
    pub description: &'static str,
    pub command: &'static str,
}

/// `ShellCustomUploader*`.
pub const CUSTOM_UPLOADER: FileType =
    FileType { extension: ".sxcu", prog_id: "ShareXrs.sxcu", description: "ShareXrs custom uploader", command: "-CustomUploader" };
/// `ShellImageEffect*`.
pub const IMAGE_EFFECT: FileType =
    FileType { extension: ".sxie", prog_id: "ShareXrs.sxie", description: "ShareXrs image effect", command: "-ImageEffect" };

const CHROME_HOST: &str = r"SOFTWARE\Google\Chrome\NativeMessagingHosts\com.sharexrs.sharexrs";
const FIREFOX_HOST: &str = r"SOFTWARE\Mozilla\NativeMessagingHosts\ShareXrs";

/// ShareX's string with ShareXrs's name ("Upload with ShareX" →
/// "Upload with ShareXrs", in every language).
pub fn rs_name(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    let mut rest = text;
    while let Some(i) = rest.find("ShareX") {
        out.push_str(&rest[..i + 6]);
        rest = &rest[i + 6..];
        if !rest.starts_with("rs") {
            out.push_str("rs");
        }
    }
    out.push_str(rest);
    out
}

fn same(a: &str, b: &str) -> bool {
    sharex_helpers::culture::ordinal_ignore_case_eq(a, b)
}

impl<R: Registry> Integration<R> {
    /// `ApplicationPath`: the program, quoted.
    fn application_path(&self) -> String {
        format!("\"{}\"", self.app.display())
    }

    /// `ShellExtIcon`: the program's first icon.
    fn icon(&self) -> String {
        format!("{},0", self.application_path())
    }

    /// `RegistryHelpers.CheckStringValue`: there, and equal ignoring case.
    fn check(&self, path: &str, value: &str) -> bool {
        self.registry.get_string(path, None).is_some_and(|v| same(&v, value))
    }

    fn folder(&self) -> PathBuf {
        self.app.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    // ---- Upload with ShareXrs (`ShellExtMenu*`)

    fn shell_ext_path(&self) -> String {
        format!("{} \"%1\"", self.application_path())
    }

    pub fn check_shell_context_menu_button(&self) -> bool {
        let cmd = self.shell_ext_path();
        self.check(&format!(r"{MENU_FILES}\command"), &cmd) && self.check(&format!(r"{MENU_DIRECTORY}\command"), &cmd)
    }

    pub fn create_shell_context_menu_button(&mut self, create: bool) -> io::Result<()> {
        self.registry.delete_tree(MENU_FILES)?;
        self.registry.delete_tree(MENU_DIRECTORY)?;
        if create {
            let desc = rs_name(sharex_strings::sharex::IntegrationHelpers_UploadWithShareX());
            let (icon, cmd) = (self.icon(), self.shell_ext_path());
            for key in [MENU_FILES, MENU_DIRECTORY] {
                self.registry.set_string(key, None, &desc)?;
                self.registry.set_string(key, Some("Icon"), &icon)?;
                self.registry.set_string(&format!(r"{key}\command"), None, &cmd)?;
            }
        }
        Ok(())
    }

    // ---- Edit with ShareXrs Image editor / Photo editor

    fn edit_command(&self, which: &str) -> String {
        format!("{} {which} \"%1\"", self.application_path())
    }

    pub fn check_edit_shell_context_menu_button(&self) -> bool {
        self.check(&format!(r"{EDIT_IMAGE}\command"), &self.edit_command("-ImageEditor"))
    }

    pub fn create_edit_shell_context_menu_button(&mut self, create: bool) -> io::Result<()> {
        let desc = rs_name(sharex_strings::sharex::IntegrationHelpers_EditWithShareX());
        self.image_menu(EDIT_IMAGE, create, &desc, "-ImageEditor")
    }

    pub fn check_photo_editor_shell_context_menu_button(&self) -> bool {
        self.check(&format!(r"{PHOTO_EDITOR_IMAGE}\command"), &self.edit_command("-PhotoEditor"))
    }

    pub fn create_photo_editor_shell_context_menu_button(&mut self, create: bool) -> io::Result<()> {
        let desc = rs_name(sharex_strings::sharex::IntegrationHelpers_EditWithShareXPhotoEditor());
        self.image_menu(PHOTO_EDITOR_IMAGE, create, &desc, "-PhotoEditor")
    }

    fn image_menu(&mut self, key: &str, create: bool, desc: &str, which: &str) -> io::Result<()> {
        self.registry.delete_tree(key)?;
        if create {
            self.registry.set_string(key, None, desc)?;
            self.registry.set_string(key, Some("Icon"), &self.icon())?;
            self.registry.set_string(&format!(r"{key}\command"), None, &self.edit_command(which))?;
        }
        Ok(())
    }

    // ---- .sxcu and .sxie

    fn file_type_command(&self, t: FileType) -> String {
        format!("{} {} \"%1\"", self.application_path(), t.command)
    }

    /// `CheckCustomUploaderExtension` / `CheckImageEffectExtension`: the
    /// ProgID opens with this program and is listed for the extension,
    /// which has a default (this one, or another program's).
    pub fn check_file_type(&self, t: FileType) -> bool {
        let ext = format!(r"Software\Classes\{}", t.extension);
        self.check(&format!(r"Software\Classes\{}\shell\open\command", t.prog_id), &self.file_type_command(t))
            && self.registry.has_value(&format!(r"{ext}\OpenWithProgids"), Some(t.prog_id))
            && self.registry.get_string(&ext, None).is_some_and(|v| !v.is_empty())
    }

    /// `CreateCustomUploaderExtension` / `CreateImageEffectExtension`.
    pub fn create_file_type(&mut self, t: FileType, create: bool) -> io::Result<()> {
        self.unregister_file_type(t)?;
        if create {
            self.register_file_type(t)?;
        }
        Ok(())
    }

    fn register_file_type(&mut self, t: FileType) -> io::Result<()> {
        let ext = format!(r"Software\Classes\{}", t.extension);
        let prog = format!(r"Software\Classes\{}", t.prog_id);
        self.registry.set_empty(&format!(r"{ext}\OpenWithProgids"), t.prog_id)?;
        if self.registry.get_string(&ext, None).is_none_or(|v| v.is_empty()) {
            self.registry.set_string(&ext, None, t.prog_id)?;
        }
        self.registry.set_string(&prog, None, t.description)?;
        self.registry.set_string(&format!(r"{prog}\DefaultIcon"), None, &self.icon())?;
        self.registry.set_string(&format!(r"{prog}\shell\open\command"), None, &self.file_type_command(t))?;
        self.registry.notify_association_changed();
        Ok(())
    }

    fn unregister_file_type(&mut self, t: FileType) -> io::Result<()> {
        let ext = format!(r"Software\Classes\{}", t.extension);
        let open_with = format!(r"{ext}\OpenWithProgids");
        self.registry.delete_tree(&format!(r"Software\Classes\{}", t.prog_id))?;
        self.registry.delete_value(&open_with, Some(t.prog_id))?;
        if self.registry.get_string(&ext, None).is_some_and(|v| same(&v, t.prog_id)) {
            self.registry.delete_value(&ext, None)?;
        }
        self.registry.delete_if_empty(&open_with)?;
        self.registry.delete_if_empty(&ext)
    }

    // ---- Browser extension support

    fn chrome_manifest(&self) -> PathBuf {
        self.folder().join(nm::CHROME_MANIFEST_FILE_NAME)
    }

    fn firefox_manifest(&self) -> PathBuf {
        self.folder().join(nm::FIREFOX_MANIFEST_FILE_NAME)
    }

    /// `CheckChromeExtensionSupport`: registered here, and the manifest is there.
    pub fn check_chrome_extension_support(&self) -> bool {
        let m = self.chrome_manifest();
        self.check(CHROME_HOST, &m.to_string_lossy()) && m.is_file()
    }

    pub fn create_chrome_extension_support(&mut self, create: bool) -> io::Result<()> {
        self.registry.delete_tree(CHROME_HOST)?;
        if create {
            let m = self.chrome_manifest();
            self.registry.set_string(CHROME_HOST, None, &m.to_string_lossy())?;
        }
        Ok(())
    }

    pub fn check_firefox_addon_support(&self) -> bool {
        let m = self.firefox_manifest();
        self.check(FIREFOX_HOST, &m.to_string_lossy()) && m.is_file()
    }

    pub fn create_firefox_addon_support(&mut self, create: bool) -> io::Result<()> {
        self.registry.delete_tree(FIREFOX_HOST)?;
        if create {
            let m = self.firefox_manifest();
            self.registry.set_string(FIREFOX_HOST, None, &m.to_string_lossy())?;
        }
        Ok(())
    }

    /// `FileTypeRegistration.RegisterMissingExtensions`.
    pub fn register_missing_extensions(&mut self) {
        for t in [CUSTOM_UPLOADER, IMAGE_EFFECT] {
            if !self.check_file_type(t)
                && let Err(e) = self.create_file_type(t, true)
            {
                eprintln!("couldn't register {}: {e}", t.extension);
            }
        }
    }

    /// `IntegrationHelpers.Uninstall` (ShareX runs it for `-uninstall` in its
    /// Steam build only; ShareXrs's uninstaller removes the same keys).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn uninstall(&mut self) -> io::Result<()> {
        self.create_shell_context_menu_button(false)?;
        self.create_edit_shell_context_menu_button(false)?;
        self.create_photo_editor_shell_context_menu_button(false)?;
        self.create_file_type(CUSTOM_UPLOADER, false)?;
        self.create_file_type(IMAGE_EFFECT, false)?;
        self.create_chrome_extension_support(false)?;
        self.create_firefox_addon_support(false)
    }
}

/// The integration checkboxes of Application settings › Integration, in
/// the page's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    ShellContextMenu,
    EditWithShareX,
    EditWithShareXPhotoEditor,
    SendToMenu,
    ChromeExtensionSupport,
    FirefoxAddonSupport,
}

pub const SWITCHES: [Switch; 6] = [
    Switch::ShellContextMenu,
    Switch::EditWithShareX,
    Switch::EditWithShareXPhotoEditor,
    Switch::SendToMenu,
    Switch::ChromeExtensionSupport,
    Switch::FirefoxAddonSupport,
];

/// `ApplicationSettingsViewModel.RefreshIntegration`'s checks.
pub fn check(switch: Switch) -> bool {
    match switch {
        Switch::SendToMenu => send_to::check(),
        _ => with_registry(|i| match switch {
            Switch::ShellContextMenu => i.check_shell_context_menu_button(),
            Switch::EditWithShareX => i.check_edit_shell_context_menu_button(),
            Switch::EditWithShareXPhotoEditor => i.check_photo_editor_shell_context_menu_button(),
            Switch::ChromeExtensionSupport => i.check_chrome_extension_support(),
            Switch::FirefoxAddonSupport => i.check_firefox_addon_support(),
            Switch::SendToMenu => unreachable!(),
        })
        .unwrap_or(false),
    }
}

/// A checkbox changed: `IntegrationHelpers.Create*(value)`. Errors are
/// logged (ShareX's `DebugHelper.WriteException`), except Send To's, which
/// ShareX shows; that one comes back as the error text.
pub fn set(switch: Switch, on: bool) -> Result<(), String> {
    if switch == Switch::SendToMenu {
        return send_to::set(on);
    }
    let r = with_registry(|i| match switch {
        Switch::ShellContextMenu => i.create_shell_context_menu_button(on),
        Switch::EditWithShareX => i.create_edit_shell_context_menu_button(on),
        Switch::EditWithShareXPhotoEditor => i.create_photo_editor_shell_context_menu_button(on),
        Switch::ChromeExtensionSupport => i.create_chrome_extension_support(on),
        Switch::FirefoxAddonSupport => i.create_firefox_addon_support(on),
        Switch::SendToMenu => unreachable!(),
    });
    match r {
        Some(Err(e)) => eprintln!("{switch:?}: {e}"),
        None => eprintln!("{switch:?}: there's no registry on this system"),
        Some(Ok(())) => {}
    }
    Ok(())
}

/// `FileTypeRegistration.RegisterMissingExtensions` for this program.
pub fn register_missing_extensions() {
    let _ = with_registry(|i| i.register_missing_extensions());
}

/// Runs `f` on this program's registration in the real registry (Windows).
fn with_registry<T>(f: impl FnOnce(&mut Integration<windows_registry::CurrentUser>) -> T) -> Option<T> {
    #[cfg(windows)]
    {
        let app = std::env::current_exe().ok()?;
        Some(f(&mut Integration { registry: windows_registry::CurrentUser::root(), app }))
    }
    #[cfg(not(windows))]
    {
        let _ = f;
        None
    }
}

/// `IntegrationHelpers.CheckSendToMenuButton` / `CreateSendToMenuButton`:
/// `SendTo\ShareXrs.lnk` to this program.
mod send_to {
    #[cfg(windows)]
    fn shortcut() -> Result<std::path::PathBuf, String> {
        Ok(crate::shortcut::known_folder(&windows::Win32::UI::Shell::FOLDERID_SendTo)?.join("ShareXrs.lnk"))
    }

    pub fn check() -> bool {
        #[cfg(windows)]
        {
            let (Ok(path), Ok(app)) = (shortcut(), std::env::current_exe()) else { return false };
            crate::shortcut::check(&path, &app)
        }
        #[cfg(not(windows))]
        false
    }

    pub fn set(create: bool) -> Result<(), String> {
        #[cfg(windows)]
        {
            let path = shortcut()?;
            if create {
                let app = std::env::current_exe().map_err(|e| e.to_string())?;
                crate::shortcut::create(&path, &app, "")
            } else {
                crate::shortcut::delete(&path)
            }
        }
        #[cfg(not(windows))]
        {
            let _ = create;
            Ok(())
        }
    }
}

/// `HKEY_CURRENT_USER`, under a prefix (empty for the real keys).
pub mod windows_registry {
    /// `HKEY_CURRENT_USER\<prefix><path>`.
    pub struct CurrentUser {
        #[cfg_attr(not(windows), allow(dead_code))]
        prefix: String,
    }

    impl CurrentUser {
        /// The real keys.
        #[cfg_attr(not(windows), allow(dead_code))]
        pub fn root() -> Self {
            Self { prefix: String::new() }
        }

        /// Everything under `HKCU\<prefix>\` instead (the tests' scratch key).
        #[cfg(all(test, windows))]
        pub fn under(prefix: &str) -> Self {
            Self { prefix: format!("{}\\", prefix.trim_end_matches('\\')) }
        }
    }

    #[cfg(windows)]
    mod imp {
        use super::CurrentUser;
        use std::io;
        use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
        use windows_sys::Win32::System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_NONE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_ANY,
            RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegDeleteTreeW, RegDeleteValueW, RegGetValueW,
            RegOpenKeyExW, RegQueryInfoKeyW, RegSetValueExW,
        };

        fn wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0)).collect()
        }

        fn check(status: u32) -> io::Result<()> {
            if status == ERROR_SUCCESS { Ok(()) } else { Err(io::Error::from_raw_os_error(status as i32)) }
        }

        fn name_ptr(name: &Option<Vec<u16>>) -> *const u16 {
            name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr())
        }

        struct Key(HKEY);

        impl Drop for Key {
            fn drop(&mut self) {
                // SAFETY: a key we opened.
                unsafe { RegCloseKey(self.0) };
            }
        }

        impl CurrentUser {
            fn full(&self, path: &str) -> Vec<u16> {
                wide(&format!("{}{path}", self.prefix))
            }

            fn create(&self, path: &str) -> io::Result<Key> {
                let mut key: HKEY = std::ptr::null_mut();
                // SAFETY: a null-terminated path; the key is closed on drop.
                check(unsafe {
                    RegCreateKeyExW(
                        HKEY_CURRENT_USER,
                        self.full(path).as_ptr(),
                        0,
                        std::ptr::null(),
                        REG_OPTION_NON_VOLATILE,
                        KEY_SET_VALUE | KEY_QUERY_VALUE,
                        std::ptr::null(),
                        &mut key,
                        std::ptr::null_mut(),
                    )
                })?;
                Ok(Key(key))
            }

            fn open(&self, path: &str, access: u32) -> Option<Key> {
                let mut key: HKEY = std::ptr::null_mut();
                // SAFETY: as above.
                let r = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, self.full(path).as_ptr(), 0, access, &mut key) };
                (r == ERROR_SUCCESS).then_some(Key(key))
            }
        }

        impl crate::integration::Registry for CurrentUser {
            fn get_string(&self, path: &str, name: Option<&str>) -> Option<String> {
                let key = self.open(path, KEY_QUERY_VALUE)?;
                let name = name.map(wide);
                let mut len = 0u32;
                // SAFETY: sizing call, then a buffer of that size.
                unsafe {
                    if RegGetValueW(key.0, std::ptr::null(), name_ptr(&name), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut len)
                        != ERROR_SUCCESS
                    {
                        return None;
                    }
                    let mut buf = vec![0u16; (len as usize).div_ceil(2)];
                    if RegGetValueW(
                        key.0,
                        std::ptr::null(),
                        name_ptr(&name),
                        RRF_RT_REG_SZ,
                        std::ptr::null_mut(),
                        buf.as_mut_ptr().cast(),
                        &mut len,
                    ) != ERROR_SUCCESS
                    {
                        return None;
                    }
                    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    Some(String::from_utf16_lossy(&buf[..n]))
                }
            }

            fn set_string(&mut self, path: &str, name: Option<&str>, value: &str) -> io::Result<()> {
                let key = self.create(path)?;
                let name = name.map(wide);
                let data = wide(value);
                // SAFETY: the data is `len` bytes, null-terminated.
                check(unsafe { RegSetValueExW(key.0, name_ptr(&name), 0, REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32) })
            }

            fn set_empty(&mut self, path: &str, name: &str) -> io::Result<()> {
                let key = self.create(path)?;
                let name = wide(name);
                // SAFETY: no data.
                check(unsafe { RegSetValueExW(key.0, name.as_ptr(), 0, REG_NONE, std::ptr::null(), 0) })
            }

            fn has_value(&self, path: &str, name: Option<&str>) -> bool {
                let Some(key) = self.open(path, KEY_QUERY_VALUE) else { return false };
                let name = name.map(wide);
                let mut len = 0u32;
                // SAFETY: sizing call only.
                unsafe {
                    RegGetValueW(key.0, std::ptr::null(), name_ptr(&name), RRF_RT_ANY, std::ptr::null_mut(), std::ptr::null_mut(), &mut len)
                        == ERROR_SUCCESS
                }
            }

            fn delete_value(&mut self, path: &str, name: Option<&str>) -> io::Result<()> {
                let Some(key) = self.open(path, KEY_SET_VALUE) else { return Ok(()) };
                let name = name.map(wide);
                // SAFETY: an open key.
                let r = unsafe { RegDeleteValueW(key.0, name_ptr(&name)) };
                if r == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(r) }
            }

            fn delete_tree(&mut self, path: &str) -> io::Result<()> {
                // SAFETY: a null-terminated path under HKCU.
                let r = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, self.full(path).as_ptr()) };
                if r == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(r) }
            }

            fn delete_if_empty(&mut self, path: &str) -> io::Result<()> {
                let Some(key) = self.open(path, KEY_QUERY_VALUE) else { return Ok(()) };
                let (mut subkeys, mut values) = (0u32, 0u32);
                // SAFETY: only the counts are asked for.
                check(unsafe {
                    RegQueryInfoKeyW(
                        key.0,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null(),
                        &mut subkeys,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        &mut values,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                })?;
                drop(key);
                if subkeys == 0 && values == 0 {
                    // SAFETY: as above.
                    check(unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, self.full(path).as_ptr()) })?;
                }
                Ok(())
            }

            fn notify_association_changed(&self) {
                if self.prefix.is_empty() {
                    use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_FLUSH, SHChangeNotify};
                    // SAFETY: no items.
                    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_FLUSH, std::ptr::null(), std::ptr::null()) };
                }
            }
        }
    }

    #[cfg(not(windows))]
    impl crate::integration::Registry for CurrentUser {
        fn get_string(&self, _: &str, _: Option<&str>) -> Option<String> {
            None
        }
        fn set_string(&mut self, _: &str, _: Option<&str>, _: &str) -> std::io::Result<()> {
            Err(std::io::ErrorKind::Unsupported.into())
        }
        fn set_empty(&mut self, _: &str, _: &str) -> std::io::Result<()> {
            Err(std::io::ErrorKind::Unsupported.into())
        }
        fn has_value(&self, _: &str, _: Option<&str>) -> bool {
            false
        }
        fn delete_value(&mut self, _: &str, _: Option<&str>) -> std::io::Result<()> {
            Ok(())
        }
        fn delete_tree(&mut self, _: &str) -> std::io::Result<()> {
            Ok(())
        }
        fn delete_if_empty(&mut self, _: &str) -> std::io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// The registry in memory: keys (ignoring case, as the registry does)
    /// to values (`""` is the default value; `None` an empty `REG_NONE`).
    #[derive(Default, Debug, PartialEq)]
    struct Memory(BTreeMap<String, BTreeMap<String, Option<String>>>);

    fn k(path: &str) -> String {
        path.to_ascii_lowercase()
    }

    impl Memory {
        fn ensure(&mut self, path: &str) -> &mut BTreeMap<String, Option<String>> {
            // Making a key makes its parents.
            let mut p = String::new();
            for part in path.split('\\') {
                if !p.is_empty() {
                    p.push('\\');
                }
                p.push_str(part);
                self.0.entry(k(&p)).or_default();
            }
            self.0.get_mut(&k(path)).expect("made")
        }
    }

    impl Registry for Memory {
        fn get_string(&self, path: &str, name: Option<&str>) -> Option<String> {
            self.0.get(&k(path))?.get(&k(name.unwrap_or(""))).cloned().flatten()
        }
        fn set_string(&mut self, path: &str, name: Option<&str>, value: &str) -> io::Result<()> {
            self.ensure(path).insert(k(name.unwrap_or("")), Some(value.to_owned()));
            Ok(())
        }
        fn set_empty(&mut self, path: &str, name: &str) -> io::Result<()> {
            self.ensure(path).insert(k(name), None);
            Ok(())
        }
        fn has_value(&self, path: &str, name: Option<&str>) -> bool {
            self.0.get(&k(path)).is_some_and(|v| v.contains_key(&k(name.unwrap_or(""))))
        }
        fn delete_value(&mut self, path: &str, name: Option<&str>) -> io::Result<()> {
            if let Some(v) = self.0.get_mut(&k(path)) {
                v.remove(&k(name.unwrap_or("")));
            }
            Ok(())
        }
        fn delete_tree(&mut self, path: &str) -> io::Result<()> {
            let p = k(path);
            self.0.retain(|key, _| key != &p && !key.starts_with(&format!("{p}\\")));
            Ok(())
        }
        fn delete_if_empty(&mut self, path: &str) -> io::Result<()> {
            let p = k(path);
            let has_sub = self.0.keys().any(|key| key.starts_with(&format!("{p}\\")));
            if self.0.get(&p).is_some_and(|v| v.is_empty()) && !has_sub {
                self.0.remove(&p);
            }
            Ok(())
        }
    }

    fn app() -> PathBuf {
        PathBuf::from(r"C:\Program Files\ShareXrs\sharexrs.exe")
    }

    /// Every key and value, as `path|name=value` lines (registry case).
    fn dump(m: &Memory) -> Vec<String> {
        let mut out = Vec::new();
        for (key, values) in &m.0 {
            for (name, value) in values {
                out.push(format!("{key}|{name}={}", value.as_deref().unwrap_or("<none>")));
            }
        }
        out
    }

    #[test]
    fn names() {
        assert_eq!(rs_name("Upload with ShareX"), "Upload with ShareXrs");
        assert_eq!(rs_name("Mit ShareX hochladen (ShareX)"), "Mit ShareXrs hochladen (ShareXrs)");
        assert_eq!(rs_name("ShareXrs"), "ShareXrs");
    }

    #[test]
    fn context_menus_as_sharex_writes_them() {
        let mut i = Integration { registry: Memory::default(), app: app() };
        assert!(!i.check_shell_context_menu_button());
        i.create_shell_context_menu_button(true).unwrap();
        assert!(i.check_shell_context_menu_button());
        i.create_edit_shell_context_menu_button(true).unwrap();
        i.create_photo_editor_shell_context_menu_button(true).unwrap();
        let exe = r#""C:\Program Files\ShareXrs\sharexrs.exe""#;
        let d = dump(&i.registry);
        for want in [
            r"software\classes\*\shell\sharexrs|=Upload with ShareXrs".to_owned(),
            format!(r"software\classes\*\shell\sharexrs|icon={exe},0"),
            format!(r#"software\classes\*\shell\sharexrs\command|={exe} "%1""#),
            r"software\classes\directory\shell\sharexrs|=Upload with ShareXrs".to_owned(),
            format!(r#"software\classes\directory\shell\sharexrs\command|={exe} "%1""#),
            r"software\classes\systemfileassociations\image\shell\sharexrsimageeditor|=Edit with ShareXrs Image editor".to_owned(),
            format!(r#"software\classes\systemfileassociations\image\shell\sharexrsimageeditor\command|={exe} -ImageEditor "%1""#),
            format!(r"software\classes\systemfileassociations\image\shell\sharexrsphotoeditor|icon={exe},0"),
            format!(r#"software\classes\systemfileassociations\image\shell\sharexrsphotoeditor\command|={exe} -PhotoEditor "%1""#),
        ] {
            assert!(d.contains(&want), "missing {want}\n{d:#?}");
        }
        assert!(i.check_edit_shell_context_menu_button() && i.check_photo_editor_shell_context_menu_button());
        // Another copy's command doesn't count as this one's.
        let other = Integration { registry: i.registry, app: PathBuf::from(r"D:\portable\sharexrs.exe") };
        assert!(!other.check_shell_context_menu_button());
        let mut i = Integration { registry: other.registry, app: app() };
        i.create_shell_context_menu_button(false).unwrap();
        i.create_edit_shell_context_menu_button(false).unwrap();
        i.create_photo_editor_shell_context_menu_button(false).unwrap();
        assert!(dump(&i.registry).iter().all(|l| !l.contains("sharexrs")), "{:#?}", dump(&i.registry));
    }

    #[test]
    fn file_types_leave_sharex_alone() {
        // ShareX registered .sxcu (`RegisterCustomUploaderExtension`).
        let mut m = Memory::default();
        m.set_string(r"Software\Classes\.sxcu", None, "ShareX.sxcu").unwrap();
        m.set_string(r"Software\Classes\ShareX.sxcu\shell\open\command", None, r#""C:\ShareX\ShareX.exe" -CustomUploader "%1""#).unwrap();
        let sharex = dump(&m);
        let mut i = Integration { registry: m, app: app() };
        assert!(!i.check_file_type(CUSTOM_UPLOADER));
        i.register_missing_extensions();
        assert!(i.check_file_type(CUSTOM_UPLOADER) && i.check_file_type(IMAGE_EFFECT));
        let exe = r#""C:\Program Files\ShareXrs\sharexrs.exe""#;
        let d = dump(&i.registry);
        for want in [
            // ShareX keeps the default; ShareXrs is listed beside it.
            r"software\classes\.sxcu|=ShareX.sxcu".to_owned(),
            r"software\classes\.sxcu\openwithprogids|sharexrs.sxcu=<none>".to_owned(),
            r"software\classes\sharexrs.sxcu|=ShareXrs custom uploader".to_owned(),
            format!(r"software\classes\sharexrs.sxcu\defaulticon|={exe},0"),
            format!(r#"software\classes\sharexrs.sxcu\shell\open\command|={exe} -CustomUploader "%1""#),
            // Nobody had .sxie: ShareXrs takes the default.
            r"software\classes\.sxie|=ShareXrs.sxie".to_owned(),
            r"software\classes\.sxie\openwithprogids|sharexrs.sxie=<none>".to_owned(),
            r"software\classes\sharexrs.sxie|=ShareXrs image effect".to_owned(),
            format!(r#"software\classes\sharexrs.sxie\shell\open\command|={exe} -ImageEffect "%1""#),
        ] {
            assert!(d.contains(&want), "missing {want}\n{d:#?}");
        }
        // Registered: the next start changes nothing.
        let before = dump(&i.registry);
        i.register_missing_extensions();
        assert_eq!(dump(&i.registry), before);
        // Unregistering takes only ShareXrs's out.
        i.create_file_type(CUSTOM_UPLOADER, false).unwrap();
        i.create_file_type(IMAGE_EFFECT, false).unwrap();
        assert_eq!(dump(&i.registry), sharex);
    }

    #[test]
    fn browser_hosts() {
        let dir = std::env::temp_dir().join(format!("sharexrs-integration-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut i = Integration { registry: Memory::default(), app: dir.join("sharexrs.exe") };
        i.create_chrome_extension_support(true).unwrap();
        i.create_firefox_addon_support(true).unwrap();
        // Registered, but the manifests aren't there yet.
        assert!(!i.check_chrome_extension_support() && !i.check_firefox_addon_support());
        std::fs::write(dir.join("host-manifest-chrome.json"), "{}").unwrap();
        std::fs::write(dir.join("host-manifest-firefox.json"), "{}").unwrap();
        assert!(i.check_chrome_extension_support() && i.check_firefox_addon_support());
        let d = dump(&i.registry);
        let chrome = format!(r"software\google\chrome\nativemessaginghosts\com.sharexrs.sharexrs|={}", dir.join("host-manifest-chrome.json").display());
        let firefox = format!(r"software\mozilla\nativemessaginghosts\sharexrs|={}", dir.join("host-manifest-firefox.json").display());
        assert!(d.contains(&chrome) && d.contains(&firefox), "{d:#?}");
        i.uninstall().unwrap();
        assert!(dump(&i.registry).iter().all(|l| !l.contains("nativemessaginghosts\\")), "{:#?}", dump(&i.registry));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The same, on the real registry API, under `HKCU\Software\ShareXrsTest`
    /// (deleted afterwards). Nothing outside it is touched.
    #[cfg(windows)]
    #[test]
    fn real_registry_under_a_scratch_key() {
        const SCRATCH: &str = r"Software\ShareXrsTest";
        let scratch = format!(r"{SCRATCH}\integration-{}", std::process::id());
        let mut root = windows_registry::CurrentUser::root();
        let mut i = Integration { registry: windows_registry::CurrentUser::under(&scratch), app: app() };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            i.registry.set_string(r"Software\Classes\.sxcu", None, "ShareX.sxcu").unwrap();
            i.register_missing_extensions();
            i.create_shell_context_menu_button(true).unwrap();
            assert!(i.check_file_type(CUSTOM_UPLOADER) && i.check_file_type(IMAGE_EFFECT));
            assert!(i.check_shell_context_menu_button());
            assert_eq!(i.registry.get_string(r"Software\Classes\.sxcu", None).as_deref(), Some("ShareX.sxcu"));
            assert_eq!(i.registry.get_string(r"Software\Classes\.sxie", None).as_deref(), Some("ShareXrs.sxie"));
            assert!(i.registry.has_value(r"Software\Classes\.sxcu\OpenWithProgids", Some("ShareXrs.sxcu")));
            assert_eq!(
                i.registry.get_string(r"Software\Classes\*\shell\ShareXrs", Some("Icon")).as_deref(),
                Some(r#""C:\Program Files\ShareXrs\sharexrs.exe",0"#)
            );
            i.uninstall().unwrap();
            assert!(!i.check_shell_context_menu_button());
            assert_eq!(i.registry.get_string(r"Software\Classes\.sxcu", None).as_deref(), Some("ShareX.sxcu"));
            assert!(!i.registry.has_value(r"Software\Classes\.sxcu\OpenWithProgids", Some("ShareXrs.sxcu")));
            // .sxie was only ShareXrs's: gone entirely.
            assert!(!i.registry.has_value(r"Software\Classes\.sxie", None));
            assert!(i.registry.get_string(r"Software\Classes\ShareXrs.sxie", None).is_none());
        }));
        root.delete_tree(&scratch).unwrap();
        let _ = root.delete_if_empty(SCRATCH);
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }
}
