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
    use std::sync::{Mutex, MutexGuard};

    /// The language is global: tests that switch it take turns.
    fn lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn english_and_fallback() {
        let _lock = lock();
        assert_eq!(sharex::MainWindow_ShareXHotkeys(), "ShareX hotkeys");
        assert_eq!(sharex::lookup("MainWindow_ShareXHotkeys"), Some("ShareX hotkeys"));
        assert_eq!(sharex::lookup("NoSuchKey"), None);
        assert_eq!(set_language("de-AT"), "de");
        assert_ne!(sharex::MainWindow_ShareXHotkeys(), "");
        set_language("en");
        assert_eq!(language(), "");
    }

    /// Every string of every module in every language, through `lookup` and
    /// (checked equal) the accessors: `module \0 key \0 language \0 value \0`.
    /// Its length and FNV-1a hash were taken from the build that kept every
    /// language's strings side by side per key (before 2026-11-19's split
    /// by language), so the layout can change but no string can.
    /// `SHAREX_STRINGS_DUMP=<file>` writes it out, to diff two builds.
    #[test]
    fn every_string_unchanged() {
        let _lock = lock();
        let mut out = Vec::new();
        for lang in LANGUAGES {
            assert_eq!(set_language(lang), lang);
            for (module, lookup, keys) in MODULES {
                // The photo and pixel editors' strings aren't ported (yet).
                if matches!(*module, "photo_editor" | "pixel_editor") {
                    continue;
                }
                for (key, accessor) in *keys {
                    let value = lookup(key).unwrap();
                    if let Some(accessor) = accessor {
                        assert_eq!(accessor(), value, "{module}::{key} in {lang:?}");
                    }
                    for part in [*module, key, lang, value] {
                        out.extend_from_slice(part.as_bytes());
                        out.push(0);
                    }
                }
            }
        }
        set_language("");
        if let Ok(path) = std::env::var("SHAREX_STRINGS_DUMP") {
            std::fs::write(path, &out).unwrap();
        }
        let hash = out.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        assert_eq!((out.len(), hash), (9_051_597, 18_317_942_704_018_421_494));
    }
}
