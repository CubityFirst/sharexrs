//! `DebugHelper.Logger` (`ShareX.HelpersLib/Logger.cs`): every line the app
//! writes, kept in memory for the debug log window and appended to
//! `Logs/ShareX-Log-yyyy-MM.txt` (unless `DisableLogging`), each as
//! `{0:yyyy-MM-dd HH:mm:ss.fff} - {1}`.
//!
//! ShareXrs writes its log with `eprintln!`, so [`init`] puts a pipe in
//! place of the process's stderr: a thread reads it line by line into the
//! logger and passes the bytes on to the original stderr.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// `Logger`'s state.
#[derive(Default)]
pub struct Logger {
    /// `sbMessages`.
    messages: String,
    /// `LogFilePath`.
    log_file_path: Option<PathBuf>,
    /// Bumped on every message (`MessageAdded`), for windows to notice.
    version: u64,
}

static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

/// `Logger.MessageFormat`.
pub fn format_line(time: chrono::NaiveDateTime, message: &str) -> String {
    format!("{} - {message}\r\n", time.format("%Y-%m-%d %H:%M:%S%.3f"))
}

/// `AppPaths.LogsFilePath`: this month's file, or none with `DisableLogging`.
pub fn log_file_path(logs_folder: &Path, disable_logging: bool) -> Option<PathBuf> {
    (!disable_logging).then(|| logs_folder.join(format!("ShareX-Log-{}.txt", chrono::Local::now().format("%Y-%m"))))
}

/// `Logger.WriteLine`: a line with its time, kept, appended to the file.
pub fn write_line(message: &str) {
    let line = format_line(chrono::Local::now().naive_local(), message);
    let mut guard = LOGGER.lock().expect("logger");
    let logger = guard.get_or_insert_with(Logger::default);
    logger.messages.push_str(&line);
    logger.version += 1;
    if let Some(path) = &logger.log_file_path
        && let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// `Logger.ToString()`: everything so far.
#[cfg(test)]
pub fn text() -> String {
    LOGGER.lock().expect("logger").as_ref().map(|l| l.messages.clone()).unwrap_or_default()
}

/// What's been added since `version`, and the version now.
pub fn since(version: u64, len: usize) -> (String, u64) {
    let guard = LOGGER.lock().expect("logger");
    match guard.as_ref() {
        Some(l) if l.version != version => (l.messages.get(len..).unwrap_or_default().to_owned(), l.version),
        Some(l) => (String::new(), l.version),
        None => (String::new(), 0),
    }
}

/// `Logger.LogFilePath`.
pub fn log_file() -> Option<PathBuf> {
    LOGGER.lock().expect("logger").as_ref().and_then(|l| l.log_file_path.clone())
}

/// `DebugHelper.Init(AppPaths.LogsFilePath)`, and stderr into the logger.
pub fn init(log_file_path: Option<PathBuf>) {
    if let Some(path) = &log_file_path
        && let Some(parent) = path.parent()
    {
        let _ = std::fs::create_dir_all(parent);
    }
    LOGGER.lock().expect("logger").get_or_insert_with(Logger::default).log_file_path = log_file_path;
    if let Err(e) = capture_stderr() {
        write_line(&format!("The debug log can't read the app's output: {e}"));
    }
}

/// Splits what the pipe gives into lines for the logger.
struct LineSplitter(Vec<u8>);

impl LineSplitter {
    fn push(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
        while let Some(i) = self.0.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.0.drain(..=i).collect();
            let text = String::from_utf8_lossy(&line);
            write_line(text.trim_end_matches(['\n', '\r']));
        }
    }
}

#[cfg(windows)]
fn capture_stderr() -> Result<(), String> {
    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, SetStdHandle};
    use windows_sys::Win32::System::Pipes::CreatePipe;
    let (mut read, mut write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
    // SAFETY: plain Win32 calls on handles this function owns.
    unsafe {
        if CreatePipe(&mut read, &mut write, std::ptr::null(), 0) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let original = GetStdHandle(STD_ERROR_HANDLE);
        if SetStdHandle(STD_ERROR_HANDLE, write) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let (read, original) = (read as usize, original as usize);
        std::thread::Builder::new()
            .name("debug-log".into())
            .spawn(move || {
                let mut splitter = LineSplitter(Vec::new());
                let mut buf = [0u8; 4096];
                loop {
                    let mut n = 0u32;
                    if ReadFile(read as HANDLE, buf.as_mut_ptr(), buf.len() as u32, &mut n, std::ptr::null_mut()) == 0 || n == 0 {
                        break;
                    }
                    let bytes = &buf[..n as usize];
                    if original != 0 && original as HANDLE != INVALID_HANDLE_VALUE {
                        let mut written = 0u32;
                        WriteFile(original as HANDLE, bytes.as_ptr(), n, &mut written, std::ptr::null_mut());
                    }
                    splitter.push(bytes);
                }
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(unix)]
fn capture_stderr() -> Result<(), String> {
    let mut fds = [0i32; 2];
    // SAFETY: plain POSIX calls on descriptors this function owns.
    unsafe {
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let original = libc::dup(2);
        if libc::dup2(fds[1], 2) < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        libc::close(fds[1]);
        let read = fds[0];
        std::thread::Builder::new()
            .name("debug-log".into())
            .spawn(move || {
                let mut splitter = LineSplitter(Vec::new());
                let mut buf = [0u8; 4096];
                loop {
                    let n = libc::read(read, buf.as_mut_ptr().cast(), buf.len());
                    if n <= 0 {
                        break;
                    }
                    let bytes = &buf[..n as usize];
                    if original >= 0 {
                        libc::write(original, bytes.as_ptr().cast(), bytes.len());
                    }
                    splitter.push(bytes);
                }
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The modules loaded from the program's folder (`OnLoadedAssembliesClick`'s
/// assemblies): its own executable and the libraries beside it.
pub fn loaded_modules() -> Vec<String> {
    let exe = std::env::current_exe().ok();
    let folder = exe.as_ref().and_then(|e| e.parent().map(Path::to_path_buf));
    let mut names: Vec<String> = Vec::new();
    for path in module_paths() {
        let p = Path::new(&path);
        if folder.as_ref().is_some_and(|f| p.parent().is_some_and(|d| d.to_string_lossy().eq_ignore_ascii_case(&f.to_string_lossy())))
            && let Some(name) = p.file_name()
        {
            let name = name.to_string_lossy().into_owned();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if names.is_empty()
        && let Some(name) = exe.as_ref().and_then(|e| e.file_name())
    {
        names.push(name.to_string_lossy().into_owned());
    }
    names
}

#[cfg(windows)]
fn module_paths() -> Vec<String> {
    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::System::ProcessStatus::{EnumProcessModules, GetModuleFileNameExW};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut modules: Vec<HMODULE> = vec![std::ptr::null_mut(); 1024];
    let mut needed = 0u32;
    // SAFETY: the buffer is as large as the size passed.
    unsafe {
        let process = GetCurrentProcess();
        if EnumProcessModules(process, modules.as_mut_ptr(), (modules.len() * std::mem::size_of::<HMODULE>()) as u32, &mut needed) == 0 {
            return Vec::new();
        }
        let count = (needed as usize / std::mem::size_of::<HMODULE>()).min(modules.len());
        modules[..count]
            .iter()
            .filter_map(|m| {
                let mut buf = [0u16; 1024];
                let n = GetModuleFileNameExW(process, *m, buf.as_mut_ptr(), buf.len() as u32);
                (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
            })
            .collect()
    }
}

#[cfg(target_os = "linux")]
fn module_paths() -> Vec<String> {
    std::fs::read_to_string("/proc/self/maps")
        .map(|maps| maps.lines().filter_map(|l| l.split_whitespace().nth(5).filter(|p| p.starts_with('/')).map(str::to_owned)).collect())
        .unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn module_paths() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_have_the_sharex_format() {
        let t = chrono::NaiveDate::from_ymd_opt(2026, 11, 19).unwrap().and_hms_milli_opt(9, 5, 7, 42).unwrap();
        assert_eq!(format_line(t, "Startup"), "2026-11-19 09:05:07.042 - Startup\r\n");
    }

    #[test]
    fn the_splitter_keeps_partial_lines() {
        let before = text();
        let mut s = LineSplitter(Vec::new());
        s.push(b"one\r\ntw");
        assert!(text().ends_with(" - one\r\n"));
        s.push(b"o\n");
        assert!(text().ends_with(" - two\r\n"));
        assert!(text().len() > before.len());
    }
}
