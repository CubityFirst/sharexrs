//! `ShareX.HelpersLib/Helpers/ShortcutHelpers.cs` (Windows): `.lnk` files
//! through `IShellLinkW`, for the Startup folder (`startup.rs`) and the
//! Send To menu (`integration.rs`).

use std::path::{Path, PathBuf};

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, STGM_READ,
};
use windows::Win32::UI::Shell::{IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink};
use windows::core::{GUID, HSTRING, Interface, PCWSTR};

/// `Environment.GetFolderPath(specialFolder)`.
pub fn known_folder(id: &GUID) -> Result<PathBuf, String> {
    // SAFETY: the returned string is copied, then freed with CoTaskMemFree.
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).map_err(|e| e.to_string())?;
        let s = p.to_string().map_err(|e| e.to_string());
        CoTaskMemFree(Some(p.0 as *const _));
        Ok(PathBuf::from(s?))
    }
}

fn com() {
    // SAFETY: initialising COM on this thread; a repeat call is harmless.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

/// `ShortcutHelpers.GetShortcutTargetPath`.
pub fn target(path: &Path) -> Option<PathBuf> {
    com();
    // SAFETY: COM calls on interfaces we created; the buffer outlives
    // the call that fills it.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let file: IPersistFile = link.cast().ok()?;
        file.Load(&HSTRING::from(path.as_os_str()), STGM_READ).ok()?;
        let mut buf = [0u16; 1024];
        link.GetPath(&mut buf, std::ptr::null_mut(), 0).ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
    }
}

/// `ShortcutHelpers.CheckShortcut`: the shortcut is there and points at
/// `target_path` (ignoring case).
pub fn check(path: &Path, target_path: &Path) -> bool {
    path.is_file()
        && target(path).is_some_and(|t| {
            sharex_helpers::culture::ordinal_ignore_case_eq(&t.to_string_lossy(), &target_path.to_string_lossy())
        })
}

/// `ShortcutHelpers.CreateShortcut`: target, arguments and its folder
/// as the working directory, over any shortcut there.
pub fn create(path: &Path, target: &Path, arguments: &str) -> Result<(), String> {
    com();
    let _ = std::fs::remove_file(path);
    // SAFETY: as above.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
        link.SetPath(&HSTRING::from(target.as_os_str())).map_err(|e| e.to_string())?;
        link.SetArguments(&HSTRING::from(arguments)).map_err(|e| e.to_string())?;
        if let Some(dir) = target.parent() {
            link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str())).map_err(|e| e.to_string())?;
        }
        let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        file.Save(PCWSTR(HSTRING::from(path.as_os_str()).as_ptr()), true).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `ShortcutHelpers.DeleteShortcut`.
pub fn delete(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
