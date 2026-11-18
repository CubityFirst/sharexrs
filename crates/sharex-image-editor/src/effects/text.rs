//! Text for the effects that draw it: `SKTypeface.FromFamilyName`, `SKFont`
//! metrics and `MeasureText` as Skia's DirectWrite backend reports them
//! (win ascent and descent, design advances), `GetTextPath`, and
//! `SKCanvas.DrawText`.
//!
//! ShareX's Skia rasterises `DrawText` glyphs through DirectWrite
//! (`SkScalerContext_DW`), which can't be reproduced here: glyphs are drawn
//! as their filled outlines from pixel-snapped origins, so text pixels differ
//! from ShareX's (listed in tests/data/effects/known.json).

use std::sync::{Arc, OnceLock};

use sharex_skia::raster::{Canvas, Paint};
use sharex_skia::{FillType, Path};

/// An `SKTypeface`: a font file's bytes and the face's index in it.
#[derive(Clone)]
pub(crate) struct Typeface {
    data: Arc<Vec<u8>>,
    index: u32,
}

fn database() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

fn query(family: &str, weight: u16, italic: bool) -> Option<Typeface> {
    let db = database();
    let families = [fontdb::Family::Name(family)];
    let q = fontdb::Query {
        families: &families,
        weight: fontdb::Weight(weight),
        stretch: fontdb::Stretch::Normal,
        style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
    };
    let id = db.query(&q)?;
    let (_, index) = db.face_source(id)?;
    let data = db.with_face_data(id, |bytes, _| bytes.to_vec())?;
    Some(Typeface { data: Arc::new(data), index })
}

impl Typeface {
    /// `SKTypeface.FromFamilyName(family, weight, Normal, slant)`: DirectWrite's
    /// `legacyMakeTypeface` falls back to the default family for a name it
    /// doesn't know.
    pub fn from_family(family: &str, weight: u16, italic: bool) -> Typeface {
        query(family, weight, italic).unwrap_or_else(|| Typeface::default_with(weight, italic))
    }

    /// `SKTypeface.Default` (Segoe UI under DirectWrite).
    pub fn default_with(weight: u16, italic: bool) -> Typeface {
        query("Segoe UI", weight, italic)
            .or_else(|| query("Arial", weight, italic))
            .or_else(|| {
                let db = database();
                let id = db.faces().next()?.id;
                let (_, index) = db.face_source(id)?;
                let data = db.with_face_data(id, |bytes, _| bytes.to_vec())?;
                Some(Typeface { data: Arc::new(data), index })
            })
            .unwrap_or(Typeface { data: Arc::new(Vec::new()), index: 0 })
    }

    fn face(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.data, self.index).ok()
    }
}

/// `SKFontMetrics`, as far as the effects read them.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Metrics {
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
}

/// `SKTextAlign`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Left,
    Center,
}

/// An `SKFont`.
pub(crate) struct Font {
    pub typeface: Typeface,
    pub size: f32,
}

impl Font {
    pub fn new(typeface: Typeface, size: f32) -> Font {
        Font { typeface, size }
    }

    /// DWRITE_FONT_METRICS: win ascent and descent, and the hhea line's
    /// leftover as the line gap.
    pub fn metrics(&self) -> Metrics {
        let Some(face) = self.typeface.face() else {
            return Metrics::default();
        };
        let s = self.size / face.units_per_em() as f32;
        let (asc, desc) = match face.tables().os2 {
            Some(os2) => (os2.windows_ascender() as f32, os2.windows_descender().unsigned_abs() as f32),
            None => (face.ascender() as f32, -(face.descender() as f32)),
        };
        let hhea = face.ascender() as f32 - face.descender() as f32 + face.line_gap() as f32;
        let gap = (hhea - (asc + desc)).max(0.0);
        Metrics {
            ascent: -asc * s,
            descent: desc * s,
            leading: gap * s,
        }
    }

    fn glyphs(&self, face: &ttf_parser::Face, text: &str) -> Vec<(u16, f32)> {
        let s = self.size / face.units_per_em() as f32;
        text.chars()
            .map(|c| {
                let g = face.glyph_index(c).unwrap_or(ttf_parser::GlyphId(0));
                (g.0, face.glyph_hor_advance(g).unwrap_or(0) as f32 * s)
            })
            .collect()
    }

    /// `SKFont.MeasureText(text)`: the advance width.
    pub fn measure(&self, text: &str) -> f32 {
        let Some(face) = self.typeface.face() else {
            return 0.0;
        };
        self.glyphs(&face, text).iter().map(|g| g.1).sum()
    }

    /// The glyph outlines of `text` from `(x, baseline)` (origins snapped to
    /// whole pixels when `snap`).
    fn path(&self, text: &str, x: f32, baseline: f32, snap: bool) -> Path {
        let mut out = Path::new();
        out.fill_type = FillType::Winding;
        let Some(face) = self.typeface.face() else {
            return out;
        };
        let s = self.size / face.units_per_em() as f32;
        let mut pen = x;
        for (g, adv) in self.glyphs(&face, text) {
            let (ox, oy) = if snap { ((pen + 0.5).floor(), (baseline + 0.5).floor()) } else { (pen, baseline) };
            let mut b = Outline {
                p: &mut out,
                s,
                ox,
                oy,
            };
            face.outline_glyph(ttf_parser::GlyphId(g), &mut b);
            pen += adv;
        }
        out
    }

    /// `SKFont.GetTextPath(text, origin)`.
    pub fn text_path(&self, text: &str, x: f32, y: f32) -> Path {
        self.path(text, x, y, false)
    }

    /// `SKCanvas.DrawText(text, x, y, align, font, paint)`.
    pub fn draw(&self, canvas: &mut Canvas, text: &str, x: f32, y: f32, align: Align, paint: &Paint) {
        let x = match align {
            Align::Left => x,
            Align::Center => x - self.measure(text) / 2.0,
        };
        let path = self.path(text, x, y, true);
        if !path.is_empty() {
            canvas.fill_sk_path(&path, paint);
        }
    }

    /// `SKCanvas.DrawText` with a blur mask filter.
    pub fn draw_blurred(&self, canvas: &mut Canvas, text: &str, x: f32, y: f32, paint: &Paint, sigma: f32) {
        let path = self.path(text, x, y, true);
        if !path.is_empty() {
            canvas.draw_path_mask_blur(&path, None, sigma, paint);
        }
    }
}

struct Outline<'a> {
    p: &'a mut Path,
    s: f32,
    ox: f32,
    oy: f32,
}

impl ttf_parser::OutlineBuilder for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.p.move_to(self.ox + x * self.s, self.oy - y * self.s);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.p.line_to(self.ox + x * self.s, self.oy - y * self.s);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (s, ox, oy) = (self.s, self.ox, self.oy);
        self.p.quad_to(ox + x1 * s, oy - y1 * s, ox + x * s, oy - y * s);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (s, ox, oy) = (self.s, self.ox, self.oy);
        self.p.cubic_to(ox + x1 * s, oy - y1 * s, ox + x2 * s, oy - y2 * s, ox + x * s, oy - y * s);
    }
    fn close(&mut self) {
        self.p.close();
    }
}
