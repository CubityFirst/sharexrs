//! `ShareX.HelpersLib/Helpers/FileHelpers.cs`'s shell parts: opening a file
//! with its program (`OpenFile`), showing it in its folder
//! (`OpenFolderWithFile`) and deleting it to the recycle bin (`DeleteFile`).

use std::path::Path;

/// `FileHelpers.OpenFile`: `Process.Start` with `UseShellExecute`.
pub fn open_file(path: &Path) -> bool {
    if !path.is_file() {
        // ShareX says so in a message box (`Helpers_OpenFile_File_not_exist_`).
        eprintln!("{} {}", sharex_strings::helpers_lib::Helpers_OpenFile_File_not_exist_(), path.display());
        return false;
    }
    match shell_open(path) {
        Ok(()) => {
            eprintln!("File opened: {}", path.display());
            true
        }
        Err(e) => {
            eprintln!("OpenFile({}) failed: {e}", path.display());
            false
        }
    }
}

/// `URLHelpers.OpenURL`'s `Process.Start(url)` with `UseShellExecute`:
/// the default browser (a custom `BrowserPath` isn't supported yet).
pub fn open_url(url: &str) -> bool {
    match shell_open(Path::new(url)) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("OpenURL({url}) failed: {e}");
            false
        }
    }
}

/// `FileHelpers.OpenFolderWithFile`: the folder, with the file selected
/// (`NativeMethods.OpenFolderAndSelectFile`).
pub fn open_folder_with_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    match select_in_folder(path) {
        Ok(()) => {
            eprintln!("Folder opened with file: {}", path.display());
            true
        }
        Err(e) => {
            eprintln!("OpenFolderWithFile({}) failed: {e}", path.display());
            false
        }
    }
}

/// `RegistryHelpers.SearchProgramPath`: an installed program by its file
/// name, from `HKCR\Applications\<file>\shell\open|edit\command`, then
/// from the programs Explorer has run (`MuiCache`). Windows only.
#[cfg(windows)]
pub fn search_program_path(file_name: &str) -> Option<String> {
    use windows::Win32::System::Registry::*;
    use windows::core::{HSTRING, PCWSTR, PWSTR};
    for command in ["open", "edit"] {
        let key = HSTRING::from(format!(r"Applications\{file_name}\shell\{command}\command"));
        let mut data = vec![0u16; 2048];
        let mut len = (data.len() * 2) as u32;
        // SAFETY: `data` is `len` bytes long; the default value (no name).
        let r = unsafe { RegGetValueW(HKEY_CLASSES_ROOT, &key, PCWSTR::null(), RRF_RT_REG_SZ, None, Some(data.as_mut_ptr().cast()), Some(&mut len)) };
        if r.is_ok() {
            let value = String::from_utf16_lossy(&data[..(len as usize / 2)]).trim_end_matches('\0').to_owned();
            let path = sharex_settings::external_program::parse_quote_string(&value);
            if Path::new(&path).is_file() {
                return Some(path);
            }
        }
    }
    let mut hkey = HKEY::default();
    let sub = HSTRING::from(r"Software\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache");
    // SAFETY: `hkey` receives the opened key, closed below.
    if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &sub, None, KEY_READ, &mut hkey) }.is_err() {
        return None;
    }
    let mut found = None;
    let mut index = 0;
    loop {
        let mut name = vec![0u16; 32768];
        let mut len = name.len() as u32;
        // SAFETY: `name` holds `len` UTF-16 units.
        let r = unsafe { RegEnumValueW(hkey, index, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None) };
        if r.is_err() {
            break;
        }
        index += 1;
        let mut program = String::from_utf16_lossy(&name[..len as usize]);
        for trim in [".ApplicationCompany", ".FriendlyAppName"] {
            if program.len() >= trim.len() && program[program.len() - trim.len()..].eq_ignore_ascii_case(trim) {
                program.truncate(program.len() - trim.len());
            }
        }
        if program.to_lowercase().ends_with(&file_name.to_lowercase()) && Path::new(&program).is_file() {
            found = Some(program);
            break;
        }
    }
    // SAFETY: opened above.
    let _ = unsafe { RegCloseKey(hkey) };
    found
}

#[cfg(not(windows))]
pub fn search_program_path(_file_name: &str) -> Option<String> {
    None
}

/// `FileHelpers.DeleteFile(path, sendToRecycleBin: true)`.
pub fn delete_file_to_recycle_bin(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    match recycle(path) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("DeleteFile({}) failed: {e}", path.display());
            false
        }
    }
}

#[cfg(windows)]
fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain([0]).collect()
}

#[cfg(windows)]
fn shell_open(path: &Path) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let file = wide(path.as_os_str());
    let verb = wide("open".as_ref());
    // SAFETY: NUL-terminated strings that outlive the call.
    let r = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL)
    };
    if r as isize > 32 { Ok(()) } else { Err(format!("ShellExecute returned {}", r as isize)) }
}

/// `NativeMethods.OpenFolderAndSelectFile`: `SHOpenFolderAndSelectItems`
/// on the file's ID list, on a thread of its own with COM set up.
#[cfg(windows)]
fn select_in_folder(path: &Path) -> Result<(), String> {
    use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows_sys::Win32::UI::Shell::{ILCreateFromPathW, ILFree, SHOpenFolderAndSelectItems};
    let file = wide(path.as_os_str());
    std::thread::spawn(move || {
        // SAFETY: COM is set up and torn down on this thread; the ID list is
        // freed after use; `file` is NUL-terminated.
        unsafe {
            CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
            let pidl = ILCreateFromPathW(file.as_ptr());
            let result = if pidl.is_null() {
                Err("ILCreateFromPath failed".to_string())
            } else {
                let hr = SHOpenFolderAndSelectItems(pidl, 0, std::ptr::null(), 0);
                ILFree(pidl);
                if hr >= 0 { Ok(()) } else { Err(format!("SHOpenFolderAndSelectItems returned {hr:#x}")) }
            };
            CoUninitialize();
            result
        }
    })
    .join()
    .map_err(|_| "the shell thread panicked".to_string())?
}

#[cfg(windows)]
fn recycle(path: &Path) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT, SHFILEOPSTRUCTW, SHFileOperationW};
    // `UIOption.OnlyErrorDialogs`: no confirmation or progress, errors shown.
    let mut from = wide(path.as_os_str());
    from.push(0); // a double-NUL-terminated list
    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT) as u16,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    // SAFETY: `op` points at `from`, which outlives the call.
    let r = unsafe { SHFileOperationW(&mut op) };
    if r == 0 { Ok(()) } else { Err(format!("SHFileOperation returned {r}")) }
}

#[cfg(target_os = "macos")]
fn shell_open(path: &Path) -> Result<(), String> {
    std::process::Command::new("open").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn select_in_folder(path: &Path) -> Result<(), String> {
    std::process::Command::new("open").arg("-R").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn shell_open(path: &Path) -> Result<(), String> {
    std::process::Command::new("xdg-open").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn select_in_folder(path: &Path) -> Result<(), String> {
    let folder = path.parent().ok_or("no folder")?;
    std::process::Command::new("xdg-open").arg(folder).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// No recycle bin API outside Windows yet: deleted outright.
#[cfg(not(windows))]
fn recycle(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}
