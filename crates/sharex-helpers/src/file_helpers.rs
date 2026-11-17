//! `ShareX.HelpersLib/Helpers/FileHelpers.cs`: the parts used for naming and
//! saving files.

use std::path::{Path, PathBuf};

/// `Path.GetInvalidFileNameChars()` on this platform: on Windows `"<>|`,
/// the control characters, `:*?\/`; elsewhere NUL and `/`.
pub fn invalid_file_name_chars() -> Vec<char> {
    if cfg!(windows) {
        let mut chars = vec!['"', '<', '>', '|'];
        chars.extend((0u8..32).map(char::from));
        chars.extend([':', '*', '?', '\\', '/']);
        chars
    } else {
        vec!['\0', '/']
    }
}

/// `FileHelpers.SanitizeFileName`: trimmed, with every character that
/// can't be in a file name removed.
pub fn sanitize_file_name(name: &str) -> String {
    let invalid = invalid_file_name_chars();
    name.trim().chars().filter(|c| !invalid.contains(c)).collect()
}

/// `FileHelpers.SanitizePath`: as [`sanitize_file_name`], but keeping the
/// root (`C:\`, `/`) and directory separators.
pub fn sanitize_path(path: &str) -> String {
    let root = path_root(path);
    let rest = &path[root.len()..];
    let invalid: Vec<char> = invalid_file_name_chars()
        .into_iter()
        .filter(|c| *c != std::path::MAIN_SEPARATOR && *c != '/')
        .collect();
    let rest: String = rest.trim().chars().filter(|c| !invalid.contains(c)).collect();
    format!("{root}{rest}")
}

/// `FileHelpers.GetPathRoot`: `Path.GetPathRoot`, or empty.
fn path_root(path: &str) -> &str {
    if cfg!(windows) {
        let b = path.as_bytes();
        if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/') {
            return &path[..3];
        }
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            return &path[..2];
        }
        if path.starts_with("\\\\") || path.starts_with('\\') || path.starts_with('/') {
            return &path[..1];
        }
        ""
    } else if path.starts_with('/') {
        &path[..1]
    } else {
        ""
    }
}

/// `FileHelpers.GetUniqueFilePath`: `path`, or if a file is there already,
/// the first free `name (2).ext`, `name (3).ext`, ... (continuing from a
/// number the name already ends with).
pub fn unique_file_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let (mut name, mut number) = (stem.clone(), 1u64);
    // `^(.+) \((\d+)\)$`
    if let Some(open) = stem.rfind(" (")
        && stem.ends_with(')')
        && open > 0
    {
        let digits = &stem[open + 2..stem.len() - 1];
        if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
            name = stem[..open].to_string();
            number = digits.parse().unwrap_or(1);
        }
    }
    loop {
        number += 1;
        let candidate = folder.join(format!("{name} ({number}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_lose_invalid_characters() {
        assert_eq!(sanitize_file_name("  a/b  "), "ab");
        if cfg!(windows) {
            assert_eq!(sanitize_file_name("What? <yes>: \"no\""), "What yes no");
            assert_eq!(sanitize_path("C:\\Shots\\2026-11"), "C:\\Shots\\2026-11");
            assert_eq!(sanitize_path("2026-11?"), "2026-11");
        }
    }

    #[test]
    fn unique_paths_count_up() {
        let dir = std::env::temp_dir().join(format!("sharex-helpers-unique-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = dir.join("shot.png");
        assert_eq!(unique_file_path(&first), first);
        std::fs::write(&first, b"x").unwrap();
        assert_eq!(unique_file_path(&first), dir.join("shot (2).png"));
        std::fs::write(dir.join("shot (2).png"), b"x").unwrap();
        assert_eq!(unique_file_path(&dir.join("shot (2).png")), dir.join("shot (3).png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
