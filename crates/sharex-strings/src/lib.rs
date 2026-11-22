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

/// One language's strings (or every key's name), each key's in key order:
/// `text` holds them back to back and `ends` where each ends, as
/// little-endian `u32`s with [`MISSING`] set for one the language lacks.
/// (Generated files, so each language's strings sit together in the
/// executable and only the language in use, and English, gets read.)
struct Blob {
    text: &'static str,
    ends: &'static [u8],
}

/// Set on an end offset for a key the language doesn't translate.
const MISSING: u32 = 1 << 31;

impl Blob {
    fn end(&self, key: usize) -> u32 {
        u32::from_le_bytes(self.ends[key * 4..key * 4 + 4].try_into().unwrap())
    }

    /// The string for key number `key`, if this language has one.
    fn get(&self, key: usize) -> Option<&'static str> {
        let end = self.end(key);
        if end & MISSING != 0 {
            return None;
        }
        let start = if key == 0 { 0 } else { self.end(key - 1) & !MISSING };
        Some(&self.text[start as usize..end as usize])
    }
}

/// Key number `key` in the current language, or in English (which has
/// every key) when that language doesn't translate it.
fn get(key: usize) -> &'static str {
    pick(&STRINGS[CURRENT.load(Ordering::Relaxed)], &STRINGS[0], key)
}

fn pick(language: &Blob, english: &Blob, key: usize) -> &'static str {
    language.get(key).or_else(|| english.get(key)).unwrap_or("")
}

/// The string named `name` among keys `keys` (one module's, sorted).
fn lookup(keys: std::ops::Range<usize>, name: &str) -> Option<&'static str> {
    let (mut low, mut high) = (keys.start, keys.end);
    while low < high {
        let mid = low + (high - low) / 2;
        match KEYS.get(mid).unwrap_or("").cmp(name) {
            std::cmp::Ordering::Less => low = mid + 1,
            std::cmp::Ordering::Greater => high = mid,
            std::cmp::Ordering::Equal => return Some(get(mid)),
        }
    }
    None
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

    #[test]
    fn culture_falls_back_to_neutral_then_english() {
        let _lock = lock();
        assert_eq!(set_language("de"), "de");
        assert_eq!(set_language("es-mx"), "es-MX");
        assert_eq!(set_language("es-AR"), "es");
        // No neutral `pt` or `ja`: English, as before.
        assert_eq!(set_language("pt"), "");
        assert_eq!(set_language("ja"), "");
        assert_eq!(set_language("ja-JP"), "ja-JP");
        assert_eq!(language(), "ja-JP");
        assert_eq!(set_language("xx-YY"), "");
        assert_eq!(sharex::MainWindow_ShareXHotkeys(), "ShareX hotkeys");
        set_language("");
    }

    /// A key a language lacks shows English's; an empty translation stays
    /// empty. (ShareX's files have every key in every language today, so
    /// this is a made-up pair of blobs.)
    #[test]
    fn missing_string_falls_back_to_english() {
        const M: u8 = 0x80;
        static ENGLISH: Blob = Blob { text: "OneTwoThree", ends: &[3, 0, 0, 0, 6, 0, 0, 0, 11, 0, 0, 0] };
        static OTHER: Blob = Blob { text: "Eins", ends: &[4, 0, 0, 0, 4, 0, 0, M, 4, 0, 0, 0] };
        assert_eq!(OTHER.get(1), None);
        assert_eq!([0, 1, 2].map(|k| pick(&OTHER, &ENGLISH, k)), ["Eins", "Two", ""]);
        assert_eq!([0, 1, 2].map(|k| pick(&ENGLISH, &ENGLISH, k)), ["One", "Two", "Three"]);
    }

    /// Every string of every module in every language, through `lookup` and
    /// (checked equal) the accessors: `module \0 key \0 language \0 value \0`.
    /// Its length and FNV-1a hash were taken from the build that kept every
    /// language's strings side by side per key (before 2026-11-19's split
    /// by language; the photo and pixel editors' modules, unused, left out),
    /// so the layout can change but no string can. `SHAREX_STRINGS_DUMP=<file>`
    /// writes it out, to diff two builds. When the `.resx` themselves change
    /// (a ShareX update), diff the dumps from before and after and take the
    /// new length and hash.
    #[test]
    fn every_string_unchanged() {
        let _lock = lock();
        let mut out = Vec::new();
        for lang in LANGUAGES {
            assert_eq!(set_language(lang), lang);
            for (module, lookup, keys) in MODULES {
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
        // ShareX 30d1682 added four keys in every language (the dumps' only
        // difference): RegionCaptureAction_CaptureSelection,
        // EditorBuiltInToolbars_MoveToolbar, ApplicationSettingsWindow_
        // DescriptionCantBeEmpty and _FormatCantBeEmpty. ShareX 93145cc's
        // toolbar customization (#9278) added ten ToolbarCustomizationDialogView_
        // keys (AllItemsOnToolbar, AvailableItems, BlankSpacer, DragHint,
        // MoveLeft, MoveRight, ShowSpacerLine, Spacer, SpacerToolTip,
        // SpacerWidth) and removed Moveup and Movedown, in every language;
        // no other string changed. ShareX ca3f01f's slimmer item panel
        // (#9324) removed Begingroup, MoveLeft, MoveRight, ShowHide,
        // ShowSpacerLine and SpacerWidth from ToolbarCustomizationDialogView_
        // in every language (174 records); nothing else changed. Its region
        // capture toolbar item (#9326) renamed ScreenCaptureLib's
        // RegionCaptureWindow_RectangleRegionTab to _RectangleRegion, the
        // same text without " (Tab)", in every language (29 records).
        // ShareX 099bcb6's spacer width box (#9327) added
        // ToolbarCustomizationDialogView_SpacerWidth back in every language
        // (29 records); nothing else changed.
        assert_eq!((out.len(), hash), (9_078_817, 1_643_134_991_618_301_900));
    }
}
