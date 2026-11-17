//! ShareX's UI text, in every language ShareX ships, generated at build time
//! from the `.resx` files in `resx/` (see `build.rs`).
//!
//! ```
//! use sharex_strings::sharex;
//! assert_eq!(sharex::MainWindow_ShareXHotkeys(), "ShareX hotkeys");
//! ```

use std::sync::atomic::{AtomicUsize, Ordering};

include!(concat!(env!("OUT_DIR"), "/strings.rs"));

static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// Switches every string to `culture` (a .NET culture name such as `de` or
/// `pt-BR`), falling back from `pt-XX` to `pt` as .NET does, and to English
/// when ShareX has no such translation. Returns the language now in use.
pub fn set_language(culture: &str) -> &'static str {
    let index = LANGUAGES
        .iter()
        .position(|l| l.eq_ignore_ascii_case(culture))
        .or_else(|| {
            let neutral = culture.split('-').next()?;
            LANGUAGES.iter().position(|l| l.eq_ignore_ascii_case(neutral))
        })
        .unwrap_or(0);
    CURRENT.store(index, Ordering::Relaxed);
    LANGUAGES[index]
}

/// The language strings are in, as given to [`set_language`].
pub fn language() -> &'static str {
    LANGUAGES[CURRENT.load(Ordering::Relaxed)]
}

fn pick(values: &[Option<&'static str>]) -> &'static str {
    values[CURRENT.load(Ordering::Relaxed)].or(values[0]).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_and_fallback() {
        assert_eq!(sharex::MainWindow_ShareXHotkeys(), "ShareX hotkeys");
        assert_eq!(sharex::lookup("MainWindow_ShareXHotkeys"), Some("ShareX hotkeys"));
        assert_eq!(sharex::lookup("NoSuchKey"), None);
        assert_eq!(set_language("de-AT"), "de");
        assert_ne!(sharex::MainWindow_ShareXHotkeys(), "");
        set_language("en");
        assert_eq!(language(), "");
    }
}
