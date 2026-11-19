//! `ShareX.FontFamily.Default` is "Segoe UI, Inter" and
//! `ShareX.FontFamily.Monospace` is "Cascadia Mono, Consolas, Courier New,
//! Monospace" (`ShareXTheme.axaml`). On Windows that's the installed Segoe
//! UI; elsewhere ShareX falls back to the Inter it bundles, so ShareXrs
//! bundles the same Inter (`assets/fonts`, OFL): Inter 3.019, the faces
//! inside `Avalonia.Fonts.Inter.dll` 12.1.6, not a newer release (Inter 4
//! has other metrics).
//!
//! Inter is also Avalonia's default family in ShareX (`.WithInterFont()` in
//! `AvaloniaBootstrapper`), which shows wherever text isn't a `TextBlock`
//! styled by ShareX: a `MenuItem`'s or `Button`'s plain string content, drawn
//! by its `ContentPresenter`. [`inter`] is that family, on every platform.
//!
//! egui has no font weights, so each weight ShareX uses is its own family:
//! [`FontFamily::Proportional`] is Normal, and [`semibold`] / [`bold`] the
//! others. Faces missing a glyph fall back to egui's defaults (emoji).

use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// Inter Regular, for drawing text outside egui (the progress tray icon's
/// fallback when Arial is missing).
pub const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
const INTER_BOLD: &[u8] = include_bytes!("../assets/fonts/Inter-Bold.ttf");

/// `FontWeight="SemiBold"`.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// `FontWeight="Bold"`.
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// `ShareX.FontFamily.Monospace` at SemiBold, as ShareX's global
/// `TextBlock` style draws a monospace `TextBlock`.
pub fn mono_semibold() -> FontFamily {
    FontFamily::Name("mono-semibold".into())
}

/// Avalonia's default family in ShareX (Inter) at a XAML weight.
pub fn inter(w: u16) -> FontFamily {
    FontFamily::Name(
        match w {
            ..=500 => "inter",
            501..=650 => "inter-semibold",
            _ => "inter-bold",
        }
        .into(),
    )
}

/// The family for a XAML weight (400 normal, 600 semibold, 700 bold).
pub fn weight(w: u16) -> FontFamily {
    match w {
        ..=500 => FontFamily::Proportional,
        501..=650 => semibold(),
        _ => bold(),
    }
}

fn system_font(names: &[&str]) -> Option<&'static [u8]> {
    let dirs: &[&str] = if cfg!(windows) {
        &["C:\\Windows\\Fonts"]
    } else if cfg!(target_os = "macos") {
        &["/System/Library/Fonts/Supplemental", "/Library/Fonts", "/System/Library/Fonts"]
    } else {
        &["/usr/share/fonts/truetype/msttcorefonts", "/usr/share/fonts/TTF", "/usr/share/fonts/truetype"]
    };
    let windir = std::env::var("WINDIR").ok().map(|w| format!("{w}\\Fonts"));
    let local = std::env::var("LOCALAPPDATA").ok().map(|l| format!("{l}\\Microsoft\\Windows\\Fonts"));
    let all = windir.iter().chain(local.iter()).map(String::as_str).chain(dirs.iter().copied());
    for dir in all {
        for name in names {
            if let Some(data) = font_file(&std::path::Path::new(dir).join(name)) {
                return Some(data);
            }
        }
    }
    None
}

/// A font file's bytes, once per process however many windows (egui
/// contexts) and faces use it: egui copies a font it owns into every
/// context, and borrows a `'static` one. On Windows the file is mapped
/// read-only, so its pages are the file's (shared, and only those read
/// are resident) rather than a private copy.
fn font_file(path: &std::path::Path) -> Option<&'static [u8]> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static FILES: Mutex<Option<HashMap<std::path::PathBuf, Option<&'static [u8]>>>> = Mutex::new(None);
    let mut files = FILES.lock().unwrap_or_else(|e| e.into_inner());
    *files.get_or_insert_with(HashMap::new).entry(path.to_path_buf()).or_insert_with(|| {
        map_file(path).or_else(|| std::fs::read(path).ok().map(|d| &*Box::leak(d.into_boxed_slice())))
    })
}

/// `path` mapped read-only for the rest of the process (never unmapped).
#[cfg(windows)]
fn map_file(path: &std::path::Path) -> Option<&'static [u8]> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, GetFileSizeEx, OPEN_EXISTING};
    use windows_sys::Win32::System::Memory::{CreateFileMappingW, FILE_MAP_READ, MapViewOfFile, PAGE_READONLY};
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: a NUL-terminated path; every handle is checked and closed (the
    // view keeps the mapping alive), and the view is never unmapped, so the
    // slice stays valid for the process.
    unsafe {
        let file = CreateFileW(wide.as_ptr(), GENERIC_READ, FILE_SHARE_READ, std::ptr::null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        if file == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut size = 0i64;
        let ok = GetFileSizeEx(file, &mut size) != 0;
        let mapping = if ok && size > 0 { CreateFileMappingW(file, std::ptr::null(), PAGE_READONLY, 0, 0, std::ptr::null()) } else { std::ptr::null_mut() };
        CloseHandle(file);
        if mapping.is_null() {
            return None;
        }
        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        CloseHandle(mapping);
        if view.Value.is_null() {
            return None;
        }
        Some(std::slice::from_raw_parts(view.Value as *const u8, size as usize))
    }
}

#[cfg(not(windows))]
fn map_file(_path: &std::path::Path) -> Option<&'static [u8]> {
    None
}

/// A font as Avalonia finds it on Windows: through DirectWrite's system
/// collection, which also has fonts that aren't in `C:\Windows\Fonts`
/// (Cascadia Mono comes with Windows Terminal's package, for one).
#[cfg(windows)]
fn directwrite(family: &str, weight: u16) -> Option<FontData> {
    use dwrote::{FontCollection, FontStretch, FontStyle, FontWeight};
    let family = FontCollection::system().font_family_by_name(family).ok()??;
    let font = family
        .first_matching_font(FontWeight::from_u32(weight as u32), FontStretch::Normal, FontStyle::Normal)
        .ok()?;
    let face = font.create_font_face();
    let file = face.files().ok()?.into_iter().next()?;
    let bytes = match file.font_file_path().ok().and_then(|p| font_file(&p)) {
        Some(b) => b,
        // Not a file (a font in memory): its bytes, once.
        None => Box::leak(file.font_file_bytes().ok()?.into_boxed_slice()),
    };
    let mut data = FontData::from_static(bytes);
    data.index = face.get_index();
    if face.has_variations() {
        // A variable font: pick the weight asked for.
        data.tweak.coords = egui::epaint::text::VariationCoords::new([(b"wght", weight as f32)]);
    }
    Some(data)
}

#[cfg(not(windows))]
fn directwrite(_family: &str, _weight: u16) -> Option<FontData> {
    None
}

/// The first of `families` this system has, as ShareX's font fallback lists
/// pick them.
fn first_of(families: &[&str], files: &[&str], weight: u16) -> Option<FontData> {
    families
        .iter()
        .find_map(|f| directwrite(f, weight))
        .or_else(|| system_font(files).map(FontData::from_static))
}

/// The fonts ShareX would pick on this system: `(regular, semibold, bold,
/// monospace)`.
fn faces() -> (FontData, FontData, FontData, Option<FontData>, Option<FontData>) {
    let segoe = |w, file| first_of(&["Segoe UI"], &[file], w);
    let (regular, semibold, bold) = match (segoe(400, "segoeui.ttf"), segoe(600, "seguisb.ttf"), segoe(700, "segoeuib.ttf")) {
        (Some(r), Some(s), Some(b)) if cfg!(windows) => (r, s, b),
        _ => (
            FontData::from_static(INTER_REGULAR),
            FontData::from_static(INTER_SEMIBOLD),
            FontData::from_static(INTER_BOLD),
        ),
    };
    let mono = first_of(
        &["Cascadia Mono", "Consolas", "Courier New"],
        &["CascadiaMono.ttf", "consola.ttf", "Courier New.ttf", "cour.ttf"],
        400,
    );
    let mono_semibold = first_of(
        &["Cascadia Mono", "Consolas", "Courier New"],
        &["CascadiaMono.ttf", "consola.ttf", "Courier New.ttf", "cour.ttf"],
        600,
    );
    (regular, semibold, bold, mono, mono_semibold)
}

/// Installs ShareX's fonts on `ctx`.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}

/// ShareX's fonts for an egui context: found once per process (a window
/// opening doesn't ask DirectWrite again), their data shared by every
/// context.
pub fn definitions() -> FontDefinitions {
    static DEFS: std::sync::OnceLock<FontDefinitions> = std::sync::OnceLock::new();
    DEFS.get_or_init(build_definitions).clone()
}

fn build_definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    let (regular, semibold_face, bold_face, mono, mono_semibold_face) = faces();
    let fallbacks = defs.families[&FontFamily::Proportional].clone();
    defs.font_data.insert("sharex-regular".into(), Arc::new(regular));
    defs.font_data.insert("sharex-semibold".into(), Arc::new(semibold_face));
    defs.font_data.insert("sharex-bold".into(), Arc::new(bold_face));
    for (family, face) in [
        (FontFamily::Proportional, "sharex-regular"),
        (semibold(), "sharex-semibold"),
        (bold(), "sharex-bold"),
    ] {
        let mut list = vec![face.to_string()];
        list.extend(fallbacks.iter().cloned());
        defs.families.insert(family, list);
    }
    for (family, face, data) in [
        ("inter", "inter-regular", INTER_REGULAR),
        ("inter-semibold", "inter-semibold", INTER_SEMIBOLD),
        ("inter-bold", "inter-bold", INTER_BOLD),
    ] {
        defs.font_data.insert(face.into(), Arc::new(FontData::from_static(data)));
        let mut list = vec![face.to_string()];
        list.extend(fallbacks.iter().cloned());
        defs.families.insert(FontFamily::Name(family.into()), list);
    }
    let mut mono_list = defs.families[&FontFamily::Monospace].clone();
    if let Some(mono) = mono {
        defs.font_data.insert("sharex-mono".into(), Arc::new(mono));
        defs.families.get_mut(&FontFamily::Monospace).unwrap().insert(0, "sharex-mono".into());
    }
    if let Some(face) = mono_semibold_face {
        defs.font_data.insert("sharex-mono-semibold".into(), Arc::new(face));
        mono_list.insert(0, "sharex-mono-semibold".into());
    }
    defs.families.insert(mono_semibold(), mono_list);
    // `FontFamily="Consolas"` (the FFmpeg command box): Consolas where the
    // system has it, else the monospace font.
    let mut consolas_list = defs.families[&FontFamily::Monospace].clone();
    if let Some(mut face) = first_of(&["Consolas"], &["consola.ttf"], 400) {
        // DirectWrite (Skia on Windows, so Avalonia) puts the baseline at
        // the font's Windows ascent (`usWinAscent`, 1884 of 2048 units for
        // Consolas) where egui puts it at `hhea`'s (1521): the glyphs go
        // that much lower, the line box stays the same height.
        face.tweak.y_offset_factor = win_ascent_shift(&face.font[..]);
        defs.font_data.insert("sharex-consolas".into(), Arc::new(face));
        consolas_list.insert(0, "sharex-consolas".into());
    }
    defs.families.insert(consolas(), consolas_list);
    defs
}

/// How much lower (a fraction of the font size) DirectWrite draws a
/// font's glyphs than its `hhea` ascent puts them: `(usWinAscent -
/// hhea.ascender) / unitsPerEm` for fonts that don't ask for their typo
/// metrics (`USE_TYPO_METRICS`), else 0.
pub fn win_ascent_shift(font: &[u8]) -> f32 {
    let be16 = |d: &[u8], at: usize| d.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let be32 = |d: &[u8], at: usize| d.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let table = |tag: &[u8; 4]| -> Option<usize> {
        let n = be16(font, 4)? as usize;
        (0..n).find_map(|i| {
            let rec = 12 + i * 16;
            (font.get(rec..rec + 4)? == tag).then(|| be32(font, rec + 8).map(|o| o as usize)).flatten()
        })
    };
    let shift = || -> Option<f32> {
        let (head, hhea, os2) = (table(b"head")?, table(b"hhea")?, table(b"OS/2")?);
        let units = be16(font, head + 18)? as f32;
        let ascender = be16(font, hhea + 4)? as i16 as f32;
        let fs_selection = be16(font, os2 + 62)?;
        let win_ascent = be16(font, os2 + 74)? as f32;
        (fs_selection & 0x80 == 0 && units > 0.0).then(|| (win_ascent - ascender) / units)
    };
    shift().unwrap_or(0.0)
}

/// `FontFamily="Consolas"`.
pub fn consolas() -> FontFamily {
    FontFamily::Name("consolas".into())
}
