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

fn system_font(names: &[&str]) -> Option<Vec<u8>> {
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
            if let Ok(data) = std::fs::read(std::path::Path::new(dir).join(name)) {
                return Some(data);
            }
        }
    }
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
    let mut data = FontData::from_owned(file.font_file_bytes().ok()?);
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
        .or_else(|| system_font(files).map(FontData::from_owned))
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

pub fn definitions() -> FontDefinitions {
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
    defs
}
