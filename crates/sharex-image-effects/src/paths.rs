//! `ImageEffectPathHelpers`: the files the drawing effects load must be local (no UNC,
//! device or remote paths, no reparse points along the way); relative paths are
//! relative to the application folder; `%ShareXImageEffects%`-style folder variables
//! and environment variables are expanded (`FileHelpers.ExpandFolderVariables`).

use std::path::{Path, PathBuf};

use crate::effects::Context;

fn replace_ignore_case(text: &str, from: &str, to: &str) -> String {
    let lower = text.to_lowercase();
    let from_l = from.to_lowercase();
    let mut out = String::new();
    let mut i = 0;
    while let Some(pos) = lower[i..].find(&from_l) {
        out.push_str(&text[i..i + pos]);
        out.push_str(to);
        i += pos + from.len();
    }
    out.push_str(&text[i..]);
    out
}

/// `FileHelpers.ExpandFolderVariables(path, true)`.
pub fn expand_folder_variables(path: &str, special: &[(String, PathBuf)]) -> String {
    let mut p = path.to_string();
    for (name, folder) in special {
        p = replace_ignore_case(&p, &format!("%{name}%"), &folder.to_string_lossy());
    }
    // Environment.ExpandEnvironmentVariables: %NAME% for names that exist.
    let mut out = String::new();
    let mut rest = p.as_str();
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) if !name.is_empty() => {
                        out.push_str(&rest[..start]);
                        out.push_str(&v);
                        rest = &after[end + 1..];
                    }
                    _ => {
                        out.push_str(&rest[..start + 1]);
                        rest = after;
                    }
                }
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

fn is_remote(path: &str) -> bool {
    let p = path.trim();
    p.is_empty() || p.starts_with("\\\\") || p.starts_with("//")
}

#[cfg(windows)]
fn is_reparse_point(p: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    std::fs::symlink_metadata(p)
        .map(|m| m.file_attributes() & 0x400 != 0)
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_reparse_point(p: &Path) -> bool {
    std::fs::symlink_metadata(p)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

fn has_reparse_point(path: &Path, is_folder: bool) -> bool {
    let mut current = if is_folder { Some(path) } else { path.parent() };
    while let Some(dir) = current {
        if dir.is_dir() && is_reparse_point(dir) {
            return true;
        }
        current = dir.parent();
    }
    !is_folder && path.is_file() && is_reparse_point(path)
}

/// `ImageEffectPathHelpers.TryGetSafeLocalFilePath`/`FolderPath`.
pub fn safe_local_path(path: &str, is_folder: bool, ctx: &Context) -> Option<PathBuf> {
    if path.trim().is_empty() {
        return None;
    }
    let mut p = expand_folder_variables(path, &ctx.special_folders);
    if let Some(rest) = p.strip_prefix("file:///") {
        p = rest.replace('/', std::path::MAIN_SEPARATOR_STR);
    } else if p.contains("://") {
        return None;
    }
    if is_remote(&p) {
        return None;
    }
    let mut full = PathBuf::from(&p);
    if !full.is_absolute() {
        full = ctx.base_dir.join(full);
    }
    let full = std::path::absolute(&full).ok()?;
    if is_remote(&full.to_string_lossy()) || has_reparse_point(&full, is_folder) {
        return None;
    }
    Some(full)
}

/// `ImageEffectPathHelpers.IsPathInFolder`.
pub fn is_path_in_folder(path: &str, folder: &Path, ctx: &Context) -> bool {
    let Some(full) = safe_local_path(path, false, ctx) else {
        return false;
    };
    let Some(dir) = safe_local_path(&folder.to_string_lossy(), true, ctx) else {
        return false;
    };
    let full = full
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase();
    let dir = dir
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase();
    full == dir || full.starts_with(&format!("{dir}{}", std::path::MAIN_SEPARATOR))
}

/// `FileHelpers.GetFileNameSafe`, for summaries.
pub fn file_name(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    Some(name.to_string())
}
