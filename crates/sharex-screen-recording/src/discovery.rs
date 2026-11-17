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

/// The places [`resolve_ffmpeg`] tries after ShareX's, in order, for a
/// `PATH` value.
pub fn fallback_locations(path_var: &std::ffi::OsStr) -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "macos")]
    {
        let exe_dir = sharex_settings::paths::base_directory();
        v.push(exe_dir.join(FFMPEG_FILE_NAME));
        // X.app/Contents/MacOS/X -> X.app/Contents/Resources/ffmpeg
        if exe_dir.file_name().is_some_and(|n| n == "MacOS") {
            v.push(exe_dir.with_file_name("Resources").join(FFMPEG_FILE_NAME));
        }
        for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"] {
            v.push(Path::new(dir).join(FFMPEG_FILE_NAME));
        }
    }
    v.extend(std::env::split_paths(path_var).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join(FFMPEG_FILE_NAME)));
    #[cfg(all(unix, not(target_os = "macos")))]
    for dir in ["/usr/bin", "/usr/local/bin"] {
        v.push(Path::new(dir).join(FFMPEG_FILE_NAME));
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
}
