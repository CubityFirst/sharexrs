//! `ShareX/WatchFolderSettings.cs`, and how .NET's `FileSystemWatcher`
//! (which ShareX's `WatchFolder` uses) matches its `Filter` against a new
//! file's name. The watching itself is the app's (`sharexrs::watch_folder`).

use crate::settings;

settings! {
    /// `ShareX.WatchFolderSettings`.
    pub struct WatchFolderSettings {
        FolderPath: Option<String> = None,
        Filter: Option<String> = None,
        IncludeSubdirectories: bool = false,
        MoveFilesToScreenshotsFolder: bool = false,
    }
}

impl WatchFolderSettings {
    /// Whether a file named `name` (no folder) passes the filter, as
    /// `FileSystemWatcher.MatchPattern` decides: an empty filter (ShareX
    /// leaves the watcher's `*.*`) and `*.*` match every name, as `*` does;
    /// names compare ignoring case on Windows and macOS.
    pub fn matches(&self, name: &str) -> bool {
        filter_matches(self.Filter.as_deref(), name, !cfg!(target_os = "linux"))
    }
}

/// `FileSystemWatcher.MatchPattern` for one filter: `NormalizedFilterCollection`
/// turns an empty filter and `*.*` into `*`, then
/// `FileSystemName.MatchesSimpleExpression`.
pub fn filter_matches(filter: Option<&str>, name: &str, ignore_case: bool) -> bool {
    if name.is_empty() {
        return false;
    }
    let filter = match filter {
        None | Some("") | Some("*.*") => "*",
        Some(f) => f,
    };
    matches_simple_expression(filter, name, ignore_case)
}

/// `FileSystemName.MatchesSimpleExpression`: `*` matches any run of
/// characters (none included), `?` exactly one, and `\` makes the next
/// character literal.
pub fn matches_simple_expression(expression: &str, name: &str, ignore_case: bool) -> bool {
    if expression.is_empty() || name.is_empty() {
        return false;
    }
    // The expression as tokens.
    enum T {
        Any,
        One,
        Char(char),
    }
    let mut tokens = Vec::new();
    let mut chars = expression.chars();
    while let Some(c) = chars.next() {
        tokens.push(match c {
            '*' => T::Any,
            '?' => T::One,
            '\\' => match chars.next() {
                Some(n) => T::Char(n),
                // A trailing escape matches nothing more.
                None => break,
            },
            c => T::Char(c),
        });
    }
    let fold = |c: char| if ignore_case { simple_upper(c) } else { c };
    let name: Vec<char> = name.chars().map(fold).collect();
    // Which name positions each token prefix can reach.
    let mut reach = vec![false; name.len() + 1];
    reach[0] = true;
    for t in &tokens {
        let mut next = vec![false; name.len() + 1];
        match t {
            T::Any => {
                let mut on = false;
                for i in 0..=name.len() {
                    on |= reach[i];
                    next[i] = on;
                }
            }
            T::One => {
                next[1..].copy_from_slice(&reach[..name.len()]);
            }
            T::Char(c) => {
                let c = fold(*c);
                for i in 0..name.len() {
                    next[i + 1] = reach[i] && name[i] == c;
                }
            }
        }
        reach = next;
    }
    reach[name.len()]
}

fn simple_upper(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) => x,
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json() {
        let w = WatchFolderSettings { FolderPath: Some("C:\\in".into()), Filter: Some("*.png".into()), IncludeSubdirectories: true, ..Default::default() };
        assert_eq!(
            serde_json::to_string(&w).unwrap(),
            r#"{"FolderPath":"C:\\in","Filter":"*.png","IncludeSubdirectories":true,"MoveFilesToScreenshotsFolder":false}"#
        );
    }

    #[test]
    fn filters() {
        let m = |f: Option<&str>, n: &str| filter_matches(f, n, true);
        assert!(m(None, "a"));
        assert!(m(Some(""), "a.png"));
        assert!(m(Some("*.*"), "noext"));
        assert!(m(Some("*.png"), "A.PNG"));
        assert!(!m(Some("*.png"), "a.png.txt"));
        assert!(m(Some("a?c.*"), "abc.txt"));
        assert!(!m(Some("a?c.*"), "ac.txt"));
        assert!(m(Some("*"), ".hidden"));
        assert!(m(Some("shot*"), "shot"));
        assert!(m(Some("a\\*b"), "a*b"));
        assert!(!m(Some("a\\*b"), "axb"));
        assert!(!filter_matches(Some("*.png"), "A.PNG", false));
    }
}
