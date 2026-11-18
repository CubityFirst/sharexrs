//! Putting a downloaded update in place.
//!
//! - Windows, as ShareX (`DownloaderWindow.Install`/`RunInstaller`): the
//!   installer's Authenticode signature is checked (`AuthenticodeSignatureVerifier`),
//!   then it runs with `/UPDATE /SILENT`, elevated (`runas`) when ShareXrs
//!   is in Program Files and the user isn't an administrator, and ShareXrs
//!   exits.
//! - macOS and Linux are ShareXrs additions, after snapr's updater
//!   (docs/MACOS-LEARNINGS.md §1.6): the zipped app replaces the whole
//!   `ShareXrs.app` (its signature stays sealed), with the old app moved
//!   out of the way rather than deleted; the tarball's files replace the
//!   ones in the folder ShareXrs runs from; the `.deb` is handed to the
//!   desktop's package installer. Both in-place updates need the release's
//!   `SHA256SUMS` to match first.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

/// Where the .deb installs ShareXrs (`/usr/bin/sharexrs` runs it from here).
pub const DEB_FOLDER: &str = "/usr/lib/sharexrs";

/// What ShareXrs asks of the system to finish an update, so tests and the
/// harness never open a browser, start an installer or exit.
pub trait UpdateHost: Send + Sync {
    /// `URLHelpers.OpenURL`.
    fn open_url(&self, url: &str);
    /// Opens a file with its default program (the `.deb`).
    fn open_file(&self, path: &Path) -> Result<(), String>;
    /// `Process.Start` with `UseShellExecute` (and `Verb = "runas"`).
    fn run_installer(&self, path: &Path, arguments: &str, elevate: bool) -> Result<(), String>;
    /// Starts the updated program again (what ShareX's installer does
    /// after `/UPDATE`).
    fn restart(&self, exe: &Path) -> Result<(), String>;
    /// `AvaloniaBootstrapper.Shutdown()`.
    fn shutdown(&self);
}

/// The program's own file as it was at startup: once an update has
/// replaced it, `current_exe` can name the moved-away copy.
static EXE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Remembers where the program is and removes what an earlier update left
/// behind. Call first thing at startup.
pub fn init() {
    if let Some(exe) = exe() {
        let installed = if cfg!(target_os = "macos") { app_of(exe).unwrap_or(exe) } else { exe };
        remove_leftovers(installed);
    }
}

pub fn exe() -> Option<&'static Path> {
    EXE.get_or_init(|| std::env::current_exe().ok()).as_deref()
}

/// The `.app` an executable runs from (`X.app/Contents/MacOS/x` → `X.app`).
pub fn app_of(exe: &Path) -> Option<&Path> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && app.extension()? == "app").then_some(app)
}

/// Launched from the disk image or Downloads, macOS runs a quarantined app
/// from a hidden read-only copy, which can't be replaced.
pub fn is_translocated(app: &Path) -> bool {
    app.to_string_lossy().contains("/AppTranslocation/")
}

/// `name.ext` → `name.ext.<suffix>` beside it.
pub fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

/// Removes the copies an update moved aside (`<name>.old…`), an unfinished
/// download (`.new`) and an unpacked app (`.unpacked`).
pub fn remove_leftovers(installed: &Path) {
    let (Some(dir), Some(name)) = (installed.parent(), installed.file_name()) else { return };
    let name = name.to_string_lossy();
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let file = entry.file_name();
        let file = file.to_string_lossy();
        if let Some(rest) = file.strip_prefix(&*name)
            && (rest.starts_with(".old") || rest == ".new" || rest == ".unpacked")
        {
            let path = entry.path();
            let _ = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
        }
    }
}

/// The checksum `SHA256SUMS` (`sha256sum`'s output) lists for `name`, in lower case.
pub fn checksum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        // `*` marks a file hashed in binary mode.
        let file = file.trim_start().trim_start_matches('*');
        (file == name).then(|| hash.to_ascii_lowercase())
    })
}

/// A file's SHA-256 in lower-case hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

// ---------------------------------------------------------------------------
// Windows (ShareX's flow).

/// `Helpers.IsDefaultInstallDir()`: the program is under Program Files.
pub fn is_default_install_dir() -> bool {
    let (Some(exe), Ok(program_files)) = (exe(), std::env::var("ProgramFiles")) else { return false };
    exe.to_string_lossy().starts_with(&program_files)
}

/// `Helpers.IsMemberOfAdministratorsGroup()`: the token's groups include
/// BUILTIN\Administrators (S-1-5-32-544), deny-only (UAC filtered) or not.
#[cfg(windows)]
pub fn is_member_of_administrators_group() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_GROUPS, TOKEN_QUERY, TokenGroups};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: our own process's token, closed below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let mut found = false;
    let mut size = 0u32;
    // SAFETY: sizing call, then a buffer of that size (u64-aligned).
    unsafe { GetTokenInformation(token, TokenGroups, std::ptr::null_mut(), 0, &mut size) };
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    if size > 0 && unsafe { GetTokenInformation(token, TokenGroups, buf.as_mut_ptr().cast(), size, &mut size) } != 0 {
        // SAFETY: the buffer holds a TOKEN_GROUPS with GroupCount entries.
        let groups = unsafe { &*(buf.as_ptr() as *const TOKEN_GROUPS) };
        let list = unsafe { std::slice::from_raw_parts(groups.Groups.as_ptr(), groups.GroupCount as usize) };
        for group in list {
            let mut text: *mut u16 = std::ptr::null_mut();
            // SAFETY: a SID from the token; the string is freed with LocalFree.
            if unsafe { ConvertSidToStringSidW(group.Sid, &mut text) } != 0 && !text.is_null() {
                let len = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
                let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
                unsafe { LocalFree(text.cast()) };
                if sid.contains("S-1-5-32-544") {
                    found = true;
                    break;
                }
            }
        }
    }
    // SAFETY: opened above.
    unsafe { CloseHandle(token) };
    found
}

#[cfg(not(windows))]
pub fn is_member_of_administrators_group() -> bool {
    false
}

/// `AuthenticodeSignatureVerifier.IsTrusted`: `WinVerifyTrust` with the
/// generic verify action, no UI and whole-chain revocation checks.
#[cfg(windows)]
pub fn is_trusted(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::WinTrust::{
        WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO, WTD_CHOICE_FILE,
        WTD_REVOKE_WHOLECHAIN, WTD_STATEACTION_IGNORE, WTD_UI_NONE, WTD_UICONTEXT_EXECUTE, WinVerifyTrust,
    };
    if !path.is_file() {
        return false;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide.as_ptr(),
        hFile: std::ptr::null_mut(),
        pgKnownSubject: std::ptr::null_mut(),
    };
    // SAFETY: zeroed is a valid WINTRUST_DATA; the members are set below.
    let mut data: WINTRUST_DATA = unsafe { std::mem::zeroed() };
    data.cbStruct = std::mem::size_of::<WINTRUST_DATA>() as u32;
    data.dwUIChoice = WTD_UI_NONE;
    data.fdwRevocationChecks = WTD_REVOKE_WHOLECHAIN;
    data.dwUnionChoice = WTD_CHOICE_FILE;
    data.Anonymous = WINTRUST_DATA_0 { pFile: &mut file };
    data.dwStateAction = WTD_STATEACTION_IGNORE;
    data.dwUIContext = WTD_UICONTEXT_EXECUTE;
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: INVALID_HANDLE_VALUE (no UI), and structures that outlive the call.
    let result = unsafe { WinVerifyTrust(-1isize as _, &mut action, (&mut data as *mut WINTRUST_DATA).cast()) };
    result == 0
}

#[cfg(not(windows))]
pub fn is_trusted(_path: &Path) -> bool {
    false
}

/// `ShellExecute` of the installer: `Process.Start` with `UseShellExecute`.
#[cfg(windows)]
pub fn shell_execute(path: &Path, arguments: &str, elevate: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
    let file = wide(path.as_os_str());
    let args = wide(std::ffi::OsStr::new(arguments));
    let verb = wide(std::ffi::OsStr::new(if elevate { "runas" } else { "open" }));
    // SAFETY: NUL-terminated strings that outlive the call.
    let result = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), args.as_ptr(), std::ptr::null(), SW_SHOWNORMAL)
    };
    if result as isize > 32 { Ok(()) } else { Err(format!("ShellExecute failed ({})", result as isize)) }
}

#[cfg(not(windows))]
pub fn shell_execute(path: &Path, arguments: &str, _elevate: bool) -> Result<(), String> {
    std::process::Command::new(path).args(arguments.split_whitespace()).spawn().map(drop).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// macOS (addition).

/// Puts the downloaded zip of the app, `zip`, in place of `app`, which may
/// be running: `ditto` keeps the signature and attributes, the old app is
/// renamed aside, the new one renamed in (rolled back on failure), and the
/// old one moved to the temporary folder (App Management can refuse
/// deleting another app's files but allows moving the bundle).
pub fn replace_app(app: &Path, zip: &Path) -> Result<(), String> {
    let unpacked = sibling(app, "unpacked");
    let old = sibling(app, "old");
    let fail = |e: std::io::Error| format!("couldn't replace {}: {e}", app.display());
    let _ = std::fs::remove_dir_all(&unpacked);
    let _ = std::fs::remove_dir_all(&old);
    let status = std::process::Command::new("ditto")
        .arg("-x")
        .arg("-k")
        .arg(zip)
        .arg(&unpacked)
        .status()
        .map_err(|e| format!("couldn't unpack the update: {e}"))?;
    let _ = std::fs::remove_file(zip);
    let name = app.file_name().unwrap_or_default();
    let new = unpacked.join(name);
    let result = if !status.success() {
        Err(format!("couldn't unpack the update (ditto {status})"))
    } else if !new.join("Contents/MacOS/sharexrs").is_file() {
        Err(format!("the update doesn't hold {}", name.to_string_lossy()))
    } else {
        swap(app, &new, &old).map_err(fail)
    };
    let _ = std::fs::remove_dir_all(&unpacked);
    let away = std::env::temp_dir().join(format!("ShareXrs-old-{}.app", std::process::id()));
    if std::fs::rename(&old, away).is_err() {
        let _ = std::fs::remove_dir_all(&old);
    }
    result
}

/// `rename(target, old)` then `rename(new, target)`, undoing the first if
/// the second fails.
fn swap(target: &Path, new: &Path, old: &Path) -> std::io::Result<()> {
    std::fs::rename(target, old)?;
    std::fs::rename(new, target).inspect_err(|_| {
        let _ = std::fs::rename(old, target);
    })
}

// ---------------------------------------------------------------------------
// Linux (addition).

/// Replaces the files the tarball (`ShareXrs/{sharexrs, ffmpeg,
/// libonnxruntime.so, Models/, Licenses/}`) holds in `folder`, which
/// ShareXrs runs from. Only the tarball's own entries are touched, so a
/// portable profile beside them stays. Each file is written beside its
/// target and renamed over it, which a running program allows; a folder
/// from the tarball (`Models/`) is replaced as a whole.
pub fn replace_tarball(folder: &Path, tarball: &Path) -> Result<(), String> {
    let unpacked = sibling(folder, "unpacked");
    let _ = std::fs::remove_dir_all(&unpacked);
    std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
    let result = unpack_tarball(tarball, &unpacked).and_then(|()| {
        let root = unpacked.join("ShareXrs");
        if !root.join("sharexrs").is_file() {
            return Err("the update doesn't hold ShareXrs/sharexrs".to_owned());
        }
        let entries = std::fs::read_dir(&root).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let target = folder.join(entry.file_name());
            let old = sibling(&target, "old");
            let _ = if old.is_dir() { std::fs::remove_dir_all(&old) } else { std::fs::remove_file(&old) };
            if target.exists() {
                swap(&target, &entry.path(), &old).map_err(|e| format!("couldn't replace {}: {e}", target.display()))?;
                let _ = if old.is_dir() { std::fs::remove_dir_all(&old) } else { std::fs::remove_file(&old) };
            } else {
                std::fs::rename(entry.path(), &target).map_err(|e| format!("couldn't write {}: {e}", target.display()))?;
            }
        }
        Ok(())
    });
    let _ = std::fs::remove_dir_all(&unpacked);
    let _ = std::fs::remove_file(tarball);
    result
}

/// Unpacks a `.tar.gz` into `to`, refusing entries that would land outside it.
fn unpack_tarball(tarball: &Path, to: &Path) -> Result<(), String> {
    let file = std::fs::File::open(tarball).map_err(|e| e.to_string())?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    archive.set_preserve_permissions(true);
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        // `unpack_in` skips entries with `..` or absolute paths.
        if !entry.unpack_in(to).map_err(|e| e.to_string())? {
            return Err("the update has a file outside its folder".to_owned());
        }
    }
    Ok(())
}

/// The real system: the browser, ShellExecute, `xdg-open`/`open`, exit.
pub struct SystemHost {
    /// Called by [`UpdateHost::shutdown`]: the app's exit.
    pub exit: Box<dyn Fn() + Send + Sync>,
}

impl UpdateHost for SystemHost {
    fn open_url(&self, url: &str) {
        if let Err(e) = open_with_default(std::ffi::OsStr::new(url)) {
            eprintln!("update: couldn't open {url}: {e}");
        }
    }

    fn open_file(&self, path: &Path) -> Result<(), String> {
        open_with_default(path.as_os_str())
    }

    fn run_installer(&self, path: &Path, arguments: &str, elevate: bool) -> Result<(), String> {
        shell_execute(path, arguments, elevate)
    }

    fn restart(&self, exe: &Path) -> Result<(), String> {
        std::process::Command::new(exe).spawn().map(drop).map_err(|e| format!("couldn't start ShareXrs again: {e}"))
    }

    fn shutdown(&self) {
        (self.exit)();
    }
}

fn open_with_default(target: &std::ffi::OsStr) -> Result<(), String> {
    #[cfg(windows)]
    {
        shell_execute(Path::new(target), "", false)
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(target).spawn().map(drop).map_err(|e| e.to_string())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(target).spawn().map(drop).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_app() {
        assert_eq!(
            app_of(Path::new("/Applications/ShareXrs.app/Contents/MacOS/sharexrs")),
            Some(Path::new("/Applications/ShareXrs.app"))
        );
        assert_eq!(app_of(Path::new("/usr/local/bin/sharexrs")), None);
        assert!(is_translocated(Path::new("/private/var/folders/AppTranslocation/X/d/ShareXrs.app")));
        assert_eq!(sibling(Path::new("dir/ShareXrs.app"), "old"), Path::new("dir/ShareXrs.app.old"));
    }

    #[test]
    fn reads_checksums() {
        let sums = "ABC123  ShareXrs-22.5.0-setup-x64.exe\ndef456 *ShareXrs-macos-aarch64.app.zip\n";
        assert_eq!(checksum_for(sums, "ShareXrs-22.5.0-setup-x64.exe").as_deref(), Some("abc123"));
        assert_eq!(checksum_for(sums, "ShareXrs-macos-aarch64.app.zip").as_deref(), Some("def456"));
        assert_eq!(checksum_for(sums, "ShareXrs-macos-aarch64.dmg"), None);
    }

    fn tarball(path: &Path, files: &[(&str, &[u8])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(file, flate2::Compression::fast()));
        for (name, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append_data(&mut header, name, *data).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn replaces_the_tarballs_files_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("ShareXrs");
        std::fs::create_dir_all(folder.join("Models/old-model")).unwrap();
        std::fs::write(folder.join("sharexrs"), b"old").unwrap();
        std::fs::write(folder.join("Portable"), b"").unwrap();
        // A portable profile is `ShareXrs/` beside `sharexrs`, which only a
        // case-sensitive file system (Linux; not Windows' or macOS's
        // defaults) can hold.
        let profile = folder.join(if cfg!(target_os = "linux") { "ShareXrs" } else { "profile" });
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(profile.join("ApplicationConfig.json"), b"{}").unwrap();
        let tar = dir.path().join("update.tar.gz");
        tarball(&tar, &[("ShareXrs/sharexrs", b"new"), ("ShareXrs/ffmpeg", b"ff"), ("ShareXrs/Models/det/inference.onnx", b"m")]);
        replace_tarball(&folder, &tar).unwrap();
        assert_eq!(std::fs::read(folder.join("sharexrs")).unwrap(), b"new");
        assert_eq!(std::fs::read(folder.join("ffmpeg")).unwrap(), b"ff");
        assert!(folder.join("Models/det/inference.onnx").is_file());
        assert!(!folder.join("Models/old-model").exists());
        // The portable profile and marker beside it stay.
        assert!(folder.join("Portable").is_file());
        assert!(profile.join("ApplicationConfig.json").is_file());
        assert!(!tar.exists());
        assert!(!sibling(&folder, "unpacked").exists());
        assert!(!folder.join("sharexrs.old").exists());
    }

    #[test]
    fn refuses_a_tarball_without_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("app");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("sharexrs"), b"old").unwrap();
        let tar = dir.path().join("u.tar.gz");
        tarball(&tar, &[("Other/sharexrs", b"new")]);
        assert!(replace_tarball(&folder, &tar).is_err());
        assert_eq!(std::fs::read(folder.join("sharexrs")).unwrap(), b"old");
    }

    #[test]
    fn removes_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("sharexrs");
        std::fs::write(&exe, b"x").unwrap();
        std::fs::write(dir.path().join("sharexrs.old"), b"x").unwrap();
        std::fs::create_dir_all(dir.path().join("sharexrs.unpacked/a")).unwrap();
        std::fs::write(dir.path().join("sharexrs.keep"), b"x").unwrap();
        remove_leftovers(&exe);
        assert!(exe.exists());
        assert!(!dir.path().join("sharexrs.old").exists());
        assert!(!dir.path().join("sharexrs.unpacked").exists());
        assert!(dir.path().join("sharexrs.keep").exists());
    }

    #[cfg(windows)]
    #[test]
    fn an_unsigned_file_is_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("setup.exe");
        std::fs::write(&file, b"MZ not really an installer").unwrap();
        assert!(!is_trusted(&file));
        assert!(!is_trusted(&dir.path().join("missing.exe")));
    }
}
