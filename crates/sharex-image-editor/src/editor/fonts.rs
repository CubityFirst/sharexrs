//! `MainViewModel.BuildAvailableFontFamilies`: the system's font families,
//! distinct (ignoring case) and sorted, for the font family picker.

use std::sync::OnceLock;

use sharex_skia::Pixmap;

use crate::model::{Annotation, Kind, Point, TextHorizontalAlignment};

/// The families, sorted ignoring case (`StringComparer.CurrentCultureIgnoreCase`).
pub fn system_font_families() -> &'static [String] {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let mut names: Vec<String> = Vec::new();
        for face in db.faces() {
            let Some(name) = family_name(&db, face) else { continue };
            if !name.trim().is_empty() && !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                names.push(name);
            }
        }
        names.sort_by_key(|n| n.to_lowercase());
        if names.is_empty() {
            names.push("Segoe UI".into());
        }
        names
    })
}

/// The family DirectWrite (and so Avalonia's `SystemFonts`) files a face
/// under: its typographic family (name 16), else its family (name 1)
/// without trailing weight and width words ("Arial Rounded MT Bold" is
/// "Arial Rounded MT", "Berlin Sans FB Demi" "Berlin Sans FB").
fn family_name(db: &fontdb::Database, face: &fontdb::FaceInfo) -> Option<String> {
    let legacy = face.families.first().map(|(n, _)| n.clone())?;
    let typographic = db
        .with_face_data(face.id, |data, index| {
            let f = ttf_parser::Face::parse(data, index).ok()?;
            let names = f.names();
            let pick = |id: u16| {
                names.into_iter().filter(|n| n.name_id == id && n.is_unicode()).find_map(|n| {
                    (n.language() == ttf_parser::Language::English_UnitedStates).then(|| n.to_string()).flatten()
                })
            };
            pick(16)
        })
        .flatten();
    if let Some(t) = typographic {
        return Some(t);
    }
    const STYLE_WORDS: [&str; 16] = [
        "Bold", "Demi", "Demibold", "Semibold", "SemiBold", "Light", "Semilight", "Black", "Heavy", "Medium", "Thin",
        "Condensed", "Extended", "Narrow", "Italic", "Oblique",
    ];
    let mut words: Vec<&str> = legacy.split(' ').collect();
    while words.len() > 1 && words.last().is_some_and(|w| STYLE_WORDS.contains(w)) {
        words.pop();
    }
    Some(words.join(" "))
}

/// `name` drawn in its own family at `size`, `argb` (straight), as the
/// picker's items show it: a transparent image as wide and high as the
/// text's layout.
pub fn render_font_name(name: &str, size: f32, argb: u32) -> Option<Pixmap> {
    let font = crate::render::text::load_face(name, false, false)?;
    let lay = crate::render::text::layout(&font, name, size as f64, crate::render::text::Align::Left, f64::INFINITY)?;
    let (w, h) = (lay.width().ceil().max(1.0) as u32, lay.height().ceil().max(1.0) as u32);
    let mut a = Annotation::new("Text")?;
    // `OutlinedTextControl` pads by 8 and the stroke: start 8px up-left.
    a.common.start_point = Point::new(-8.0, -8.0);
    a.common.end_point = Point::new(w as f32 + 8.0, h as f32 + 8.0);
    a.common.stroke_width = 0.0;
    a.common.stroke_color = "#00000000".into();
    if let Kind::Text(t) = &mut a.kind {
        t.text = name.to_string();
        t.font_family = name.to_string();
        t.font_size = size;
        t.is_bold = false;
        t.text_color = format!("#{argb:08X}");
        t.horizontal_alignment = TextHorizontalAlignment::Left;
    }
    Some(crate::render::render(&Pixmap::new(w, h), &[a]))
}
