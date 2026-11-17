//! The `System.IO.Path` string functions the history windows use on stored
//! paths, with .NET's rules for this platform (on Windows both `\` and `/`
//! separate, and `C:` is a root).

fn is_sep(c: char) -> bool {
    if cfg!(windows) { c == '\\' || c == '/' } else { c == '/' }
}

/// `PathInternal.GetRootLength`.
fn root_length(p: &str) -> usize {
    let b: Vec<char> = p.chars().collect();
    if !cfg!(windows) {
        return usize::from(b.first() == Some(&'/'));
    }
    let bytes = |i: usize| b.get(i).copied();
    if bytes(0).is_some_and(is_sep) && bytes(1).is_some_and(is_sep) {
        // \\server\share
        let mut seps = 0;
        let mut i = 2;
        while i < b.len() {
            if is_sep(b[i]) {
                seps += 1;
                if seps == 2 {
                    break;
                }
            }
            i += 1;
        }
        return b[..i].iter().collect::<String>().len();
    }
    if bytes(0).is_some_and(is_sep) {
        return 1;
    }
    if bytes(0).is_some_and(|c| c.is_ascii_alphabetic()) && bytes(1) == Some(':') {
        return if bytes(2).is_some_and(is_sep) { 3 } else { 2 };
    }
    0
}

fn last_sep(p: &str) -> Option<usize> {
    p.char_indices().rev().find(|&(_, c)| is_sep(c)).map(|(i, _)| i)
}

/// `Path.GetFileName`.
pub fn file_name(p: &str) -> &str {
    let root = root_length(p);
    match last_sep(p) {
        Some(i) if i >= root => &p[i + 1..],
        _ => &p[root.min(p.len())..],
    }
}

/// `Path.GetExtension`: with the dot; empty when there's none or the name ends with one.
pub fn extension(p: &str) -> &str {
    let name = file_name(p);
    match name.rfind('.') {
        Some(i) if i + 1 < name.len() => &name[i..],
        _ => "",
    }
}

/// `Path.HasExtension`.
pub fn has_extension(p: &str) -> bool {
    !extension(p).is_empty()
}

/// `Path.GetFileNameWithoutExtension`.
pub fn file_name_without_extension(p: &str) -> &str {
    let name = file_name(p);
    match name.rfind('.') {
        Some(i) => &name[..i],
        None => name,
    }
}

/// `Path.GetDirectoryName`: `None` for an empty path or a root; separators
/// normalised (Windows: `/` becomes `\`, repeats collapse).
pub fn directory_name(p: &str) -> Option<String> {
    if p.is_empty() {
        return None;
    }
    let root = root_length(p);
    let chars: Vec<(usize, char)> = p.char_indices().collect();
    let mut end = chars.len();
    let root_chars = p[..root].chars().count();
    if end <= root_chars {
        return None;
    }
    while end > root_chars {
        end -= 1;
        if is_sep(chars[end].1) {
            break;
        }
    }
    while end > root_chars && is_sep(chars[end - 1].1) {
        end -= 1;
    }
    let slice: String = chars[..end].iter().map(|&(_, c)| c).collect();
    Some(normalize_separators(&slice))
}

fn normalize_separators(p: &str) -> String {
    if !cfg!(windows) {
        return p.to_string();
    }
    let mut out = String::with_capacity(p.len());
    let mut prev_sep = false;
    for (i, c) in p.chars().enumerate() {
        if is_sep(c) {
            // A leading pair (UNC) stays.
            if prev_sep && i > 1 {
                continue;
            }
            out.push('\\');
            prev_sep = true;
        } else {
            out.push(c);
            prev_sep = false;
        }
    }
    out
}

/// `Path.Combine(a, b)`.
pub fn combine(a: &str, b: &str) -> String {
    if a.is_empty() || root_length(b) > 0 {
        return b.to_string();
    }
    if a.ends_with(is_sep) || (cfg!(windows) && a.ends_with(':')) {
        format!("{a}{b}")
    } else {
        format!("{a}{}{b}", std::path::MAIN_SEPARATOR)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_paths() {
        assert_eq!(file_name(r"C:\a\b.png"), "b.png");
        assert_eq!(file_name("C:b.png"), "b.png");
        assert_eq!(file_name_without_extension(r"C:\a\b.tar.gz"), "b.tar");
        assert_eq!(extension(r"C:\a.b\c"), "");
        assert_eq!(extension("x."), "");
        assert_eq!(directory_name(r"C:\a/b//c.png").as_deref(), Some(r"C:\a\b"));
        assert_eq!(directory_name(r"C:\"), None);
        assert_eq!(directory_name("c.png").as_deref(), Some(""));
        assert_eq!(combine(r"C:\x", "y.png"), r"C:\x\y.png");
    }
}
