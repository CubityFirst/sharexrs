//! The .NET string comparisons the history windows use. ShareX runs with
//! `System.Globalization.UseNls` (ShareX.runtimeconfig.json), so its
//! culture-aware comparisons are Windows NLS calls; on Windows these are
//! the same calls with the flags .NET passes (`CompareInfo.Nls.cs`:
//! `NORM_LINGUISTIC_CASING`, plus `LINGUISTIC_IGNORECASE` to ignore case).
//! Elsewhere (ShareX doesn't run there) a simple-case-folding stand-in.

use std::cmp::Ordering;

/// `StringComparer.OrdinalIgnoreCase`'s key: each character upper-cased by
/// its simple mapping (one character to one).
pub fn ordinal_upper(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut up = c.to_uppercase();
            match (up.next(), up.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        })
        .collect()
}

/// `StringComparer.OrdinalIgnoreCase.Equals`.
pub fn ordinal_ignore_case_eq(a: &str, b: &str) -> bool {
    ordinal_upper(a) == ordinal_upper(b)
}

/// `StringComparer.OrdinalIgnoreCase.Compare`: upper-cased UTF-16 code units.
pub fn ordinal_ignore_case_cmp(a: &str, b: &str) -> Ordering {
    ordinal_upper(a).encode_utf16().cmp(ordinal_upper(b).encode_utf16())
}

#[cfg(windows)]
mod nls {
    use std::cmp::Ordering;
    use windows_sys::Win32::Globalization::{
        CSTR_EQUAL, CSTR_GREATER_THAN, CSTR_LESS_THAN, CompareStringEx, FIND_FROMSTART, FindNLSStringEx,
        LINGUISTIC_IGNORECASE, NORM_LINGUISTIC_CASING,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// `LOCALE_NAME_USER_DEFAULT` is a null pointer; the invariant locale is "".
    fn locale(invariant: bool) -> Vec<u16> {
        if invariant { vec![0] } else { Vec::new() }
    }

    pub fn compare(a: &str, b: &str, invariant: bool, ignore_case: bool) -> Option<Ordering> {
        let (a, b, loc) = (wide(a), wide(b), locale(invariant));
        let flags = NORM_LINGUISTIC_CASING | if ignore_case { LINGUISTIC_IGNORECASE } else { 0 };
        let r = unsafe {
            CompareStringEx(
                if loc.is_empty() { std::ptr::null() } else { loc.as_ptr() },
                flags,
                a.as_ptr(),
                a.len() as i32,
                b.as_ptr(),
                b.len() as i32,
                std::ptr::null(),
                std::ptr::null(),
                0,
            )
        };
        match r {
            CSTR_LESS_THAN => Some(Ordering::Less),
            CSTR_EQUAL => Some(Ordering::Equal),
            CSTR_GREATER_THAN => Some(Ordering::Greater),
            _ => None,
        }
    }

    pub fn contains_invariant_ignore_case(source: &str, value: &str) -> Option<bool> {
        if value.is_empty() {
            return Some(true);
        }
        let (s, v, loc) = (wide(source), wide(value), locale(true));
        let r = unsafe {
            FindNLSStringEx(
                loc.as_ptr(),
                FIND_FROMSTART | NORM_LINGUISTIC_CASING | LINGUISTIC_IGNORECASE,
                s.as_ptr(),
                s.len() as i32,
                v.as_ptr(),
                v.len() as i32,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                0,
            )
        };
        Some(r >= 0)
    }
}

fn fold(s: &str) -> String {
    s.chars().flat_map(char::to_lowercase).collect()
}

/// `Comparer<string>.Default` (`string.CompareTo`, the current culture).
pub fn current_culture_cmp(a: &str, b: &str) -> Ordering {
    #[cfg(windows)]
    if let Some(o) = nls::compare(a, b, false, false) {
        return o;
    }
    fold(a).cmp(&fold(b)).then_with(|| b.cmp(a))
}

/// `a.Equals(b, StringComparison.InvariantCultureIgnoreCase)`.
pub fn invariant_ignore_case_eq(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    if let Some(o) = nls::compare(a, b, true, true) {
        return o == Ordering::Equal;
    }
    fold(a) == fold(b)
}

/// `source.Contains(value, StringComparison.InvariantCultureIgnoreCase)`.
pub fn invariant_ignore_case_contains(source: &str, value: &str) -> bool {
    #[cfg(windows)]
    if let Some(found) = nls::contains_invariant_ignore_case(source, value) {
        return found;
    }
    fold(source).contains(&fold(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparisons() {
        assert!(ordinal_ignore_case_eq("Chrome", "CHROME"));
        assert_eq!(ordinal_ignore_case_cmp("a", "B"), Ordering::Less);
        assert_eq!(current_culture_cmp("a", "B"), Ordering::Less);
        assert!(invariant_ignore_case_eq("Image", "IMAGE"));
        assert!(invariant_ignore_case_contains("Amazon S3", "s3"));
        assert!(invariant_ignore_case_contains("x", ""));
        assert!(!invariant_ignore_case_contains("Imgur", "s3"));
    }
}
