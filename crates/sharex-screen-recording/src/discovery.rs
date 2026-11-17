//! Where FFmpeg is.
//!
//! ShareX's rule (`FFmpegOptions.FFmpegPath`, `TaskHelpers.CheckFFmpeg`):
//! the user's path when "Use custom FFmpeg path" is on and it isn't empty,
//! otherwise `ffmpeg.exe` beside ShareX.exe, which ShareX's release ships
//! (its CI copies BtbN's `ffmpeg-n9.0-latest-win64-gpl-9.0` build there).
//! Relative paths are resolved against the program's folder after folder
//! variables (`%MyDocuments%`, `%APPDATA%`, ...) are expanded. Nothing else
//! is searched; a missing file is an error message. A 32-bit Windows needs
//! a custom path (ShareX ships 64-bit FFmpeg only).
//!
//! ShareXrs additions ([`resolve_ffmpeg`]), used only when ShareX's rule
//! finds nothing and no custom path is set: on Windows `ffmpeg.exe` on
//! `PATH`; on macOS `ffmpeg` beside the executable, in the bundle's
//! `Resources`, Homebrew's and MacPorts' folders (an app started from
//! Finder doesn't get the shell's `PATH`), then `PATH`; on Linux `PATH`,
//! then `/usr/bin` and `/usr/local/bin`.

use std::path::{Path, PathBuf};

use sharex_settings::FFmpegOptions;

/// The FFmpeg program's file name on this platform.
pub const FFMPEG_FILE_NAME: &str = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };

/// `FFmpegOptions.FFmpegPath`: the custom path, or FFmpeg beside the
/// program, made absolute (`FileHelpers.GetAbsolutePath`).
pub fn ffmpeg_path(options: &FFmpegOptions) -> PathBuf {
    if options.OverrideCLIPath && !options.CLIPath.is_empty() {
        return absolute_path(&options.CLIPath);
    }
    absolute_path(FFMPEG_FILE_NAME)
}

/// `FileHelpers.GetAbsolutePath`: folder variables expanded, relative to
/// the program's folder, then `Path.GetFullPath`.
pub fn absolute_path(path: &str) -> PathBuf {
    let p = sharex_settings::paths::absolute_path(path);
    std::path::absolute(&p).unwrap_or(p)
}

/// Why FFmpeg can't be used (`CheckFFmpeg`'s message boxes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FFmpegCheckError {
    /// `FFmpegOnlySupports64BitOperatingSystems`: 32-bit Windows without a
    /// custom path.
    Not64Bit,
    /// `FFmpegDoesNotExistAtTheFollowingPath` + the path.
    Missing(PathBuf),
}

/// `TaskHelpers.CheckFFmpeg`, exactly: ShareX's path, which must exist.
pub fn check_ffmpeg(options: &FFmpegOptions) -> Result<PathBuf, FFmpegCheckError> {
    if cfg!(all(windows, not(target_pointer_width = "64"))) && !options.OverrideCLIPath {
        return Err(FFmpegCheckError::Not64Bit);
    }
    let path = ffmpeg_path(options);
    if path.is_file() { Ok(path) } else { Err(FFmpegCheckError::Missing(path)) }
}

/// [`check_ffmpeg`], then (ShareXrs addition) the platform's usual places
/// when no custom path is set. The error is ShareX's, naming ShareX's path.
pub fn resolve_ffmpeg(options: &FFmpegOptions) -> Result<PathBuf, FFmpegCheckError> {
    match check_ffmpeg(options) {
        Ok(p) => Ok(p),
        Err(e) if options.OverrideCLIPath => Err(e),
        Err(e) => fallback_locations(&std::env::var_os("PATH").unwrap_or_default())
            .into_iter()
            .find(|p| p.is_file())
            .ok_or(e),
    }
}

/// The FFmpeg program's file name on `os` (`std::env::consts::OS`).
pub fn ffmpeg_file_name(os: &str) -> &'static str {
    if os == "windows" { "ffmpeg.exe" } else { "ffmpeg" }
}

/// The places [`resolve_ffmpeg`] tries after ShareX's, in order, for a
/// `PATH` value.
pub fn fallback_locations(path_var: &std::ffi::OsStr) -> Vec<PathBuf> {
    fallback_locations_for(std::env::consts::OS, &sharex_settings::paths::base_directory(), path_var)
}

/// [`fallback_locations`] for a platform (`std::env::consts::OS`), the
/// executable's folder and a `PATH` value. On macOS the release's bundle
/// has FFmpeg beside the executable (`Contents/MacOS`, ShareX's rule); an
/// older or hand-made one may have it in `Contents/Resources`.
pub fn fallback_locations_for(os: &str, exe_dir: &Path, path_var: &std::ffi::OsStr) -> Vec<PathBuf> {
    let name = ffmpeg_file_name(os);
    let mut v = Vec::new();
    if os == "macos" {
        v.push(exe_dir.join(name));
        // X.app/Contents/MacOS/X -> X.app/Contents/Resources/ffmpeg
        if exe_dir.file_name().is_some_and(|n| n == "MacOS") {
            v.push(exe_dir.with_file_name("Resources").join(name));
        }
        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"] {
            v.push(Path::new(dir).join(name));
        }
    }
    v.extend(std::env::split_paths(path_var).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join(name)));
    if os != "windows" && os != "macos" {
        for dir in ["/usr/bin", "/usr/local/bin"] {
            v.push(Path::new(dir).join(name));
        }
    }
    v
}

/// FFmpeg's version from `ffmpeg -version`'s first line ("ffmpeg version
/// n9.0.2-48-g... Copyright ..." gives "n9.0.2-48-g..."), for logs.
pub fn ffmpeg_version(ffmpeg: &Path) -> Option<String> {
    let out = crate::cli::command(ffmpeg, "-version").stdin(std::process::Stdio::null()).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().next()?.strip_prefix("ffmpeg version ")?.split_whitespace().next().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharex_rule() {
        let default = FFmpegOptions::default();
        assert_eq!(ffmpeg_path(&default), sharex_settings::paths::base_directory().join(FFMPEG_FILE_NAME));
        // A custom path only counts when it's switched on and not empty.
        let custom = FFmpegOptions { CLIPath: "tools/ff.exe".into(), ..Default::default() };
        assert_eq!(ffmpeg_path(&custom), ffmpeg_path(&default));
        let custom = FFmpegOptions { OverrideCLIPath: true, CLIPath: "tools/ff.exe".into(), ..Default::default() };
        assert_eq!(ffmpeg_path(&custom), absolute_path("tools/ff.exe"));
        let empty = FFmpegOptions { OverrideCLIPath: true, ..Default::default() };
        assert_eq!(ffmpeg_path(&empty), ffmpeg_path(&default));
    }

    #[test]
    fn missing_is_sharex_error() {
        let o = FFmpegOptions { OverrideCLIPath: true, CLIPath: "no/such/ffmpeg.exe".into(), ..Default::default() };
        assert_eq!(check_ffmpeg(&o), Err(FFmpegCheckError::Missing(absolute_path("no/such/ffmpeg.exe"))));
        // A custom path is never replaced by a fallback.
        assert!(resolve_ffmpeg(&o).is_err());
    }

    #[test]
    fn falls_back_to_path() {
        let dir = std::env::temp_dir().join(format!("sxr-ffmpeg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join(FFMPEG_FILE_NAME);
        std::fs::write(&exe, b"").unwrap();
        let path = std::env::join_paths([dir.clone()]).unwrap();
        assert!(fallback_locations(&path).contains(&exe));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn file_name_per_platform() {
        // ShareX's rule is this name beside the executable: where the
        // Windows zip and installer, the .app (Contents/MacOS) and the Linux
        // packages (/usr/lib/sharexrs) put it.
        assert_eq!(ffmpeg_file_name("windows"), "ffmpeg.exe");
        assert_eq!(ffmpeg_file_name("macos"), "ffmpeg");
        assert_eq!(ffmpeg_file_name("linux"), "ffmpeg");
        assert_eq!(FFMPEG_FILE_NAME, ffmpeg_file_name(std::env::consts::OS));
    }

    #[test]
    fn macos_bundle_locations() {
        let exe_dir = Path::new("/Applications/ShareXrs.app/Contents/MacOS");
        let v = fallback_locations_for("macos", exe_dir, std::ffi::OsStr::new(""));
        assert_eq!(v[0], exe_dir.join("ffmpeg"));
        assert_eq!(v[1], Path::new("/Applications/ShareXrs.app/Contents/Resources/ffmpeg"));
        assert!(v.contains(&PathBuf::from("/opt/homebrew/bin/ffmpeg")));
        // Outside a bundle there's no Resources.
        let v = fallback_locations_for("macos", Path::new("/usr/local/bin"), std::ffi::OsStr::new(""));
        assert!(!v.iter().any(|p| p.ends_with("Resources/ffmpeg")));
    }

    #[test]
    fn linux_and_windows_locations() {
        let exe_dir = Path::new("/usr/lib/sharexrs");
        let v = fallback_locations_for("linux", exe_dir, std::ffi::OsStr::new(""));
        assert_eq!(v, vec![PathBuf::from("/usr/bin/ffmpeg"), PathBuf::from("/usr/local/bin/ffmpeg")]);
        // Windows: PATH only (ShareX's rule already looked beside the exe).
        let v = fallback_locations_for("windows", Path::new("C:/ShareXrs"), std::ffi::OsStr::new(""));
        assert!(v.is_empty());
    }
}
