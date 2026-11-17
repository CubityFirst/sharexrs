//! Text for the Text watermark and Text effects: `ImageFont.CreateFont` (a system
//! typeface by family, bold and italic), `SkiaDrawing.MeasureText`, `DrawText` and
//! `TextPath`. The same font files ShareX's Skia finds (DirectWrite on Windows) are
//! found through fontdb; metrics are the ones Skia reports there (hhea ascent, descent
//! and line gap, `post` underline, OS/2 strikeout); glyphs are mapped through the
//! `cmap` without shaping or kerning, as `SKCanvas.DrawText` does, and advanced by
//! their unhinted advance widths.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::dotnet::{ImageFont, font_style};

/// Font files by family and style, from the system fonts.
/// A font file's bytes and the face's index in it.
type FaceData = Arc<(Vec<u8>, u32)>;

#[derive(Default)]
pub struct Fonts {
    cache: HashMap<(String, bool, bool), Option<FaceData>>,
}

fn database() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

impl Fonts {
    fn face_data(&mut self, family: &str, bold: bool, italic: bool) -> Option<FaceData> {
        let key = (family.to_ascii_lowercase(), bold, italic);
        if let Some(v) = self.cache.get(&key) {
            return v.clone();
        }
        let db = database();
        let find = |name: &str| {
            let families = [fontdb::Family::Name(name)];
            let query = fontdb::Query {
                families: &families,
                weight: if bold {
                    fontdb::Weight::BOLD
                } else {
                    fontdb::Weight::NORMAL
                },
                stretch: fontdb::Stretch::Normal,
                style: if italic {
                    fontdb::Style::Italic
                } else {
                    fontdb::Style::Normal
                },
            };
            db.query(&query)
        };
        // SKTypeface.FromFamilyName, falling back to SKTypeface.Default (Segoe UI under
        // DirectWrite).
        let id = find(family)
            .or_else(|| find("Segoe UI"))
            .or_else(|| find("Arial"))
            .or_else(|| db.faces().next().map(|f| f.id));
        let data =
            id.and_then(|id| db.with_face_data(id, |data, index| Arc::new((data.to_vec(), index))));
        self.cache.insert(key, data.clone());
        data
    }

    /// `ImageFont.CreateFont()`.
    pub fn font(&mut self, f: &ImageFont) -> Option<SkFont> {
        let data = self.face_data(
            &f.name,
            f.style & font_style::BOLD != 0,
            f.style & font_style::ITALIC != 0,
        )?;
        ttf_parser::Face::parse(&data.0, data.1).ok()?;
        Some(SkFont {
            data,
            size: f.pixel_size(),
            style: f.style,
        })
    }
}

/// `SKFont`: a typeface at a pixel size.
pub struct SkFont {
    data: FaceData,
    pub size: f32,
    style: u8,
}

/// `SKFontMetrics`, as far as ShareX reads them.
#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    pub ascent: f32,
    pub descent: f32,
    pub leading: f32,
    pub underline_position: f32,
    pub underline_thickness: f32,
    pub strikeout_position: f32,
    pub strikeout_thickness: f32,
}

struct OutlineToPath<'a> {
    pb: &'a mut tiny_skia::PathBuilder,
    x: f32,
    y: f32,
    scale: f32,
}

impl ttf_parser::OutlineBuilder for OutlineToPath<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb
            .move_to(self.x + x * self.scale, self.y - y * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb
            .line_to(self.x + x * self.scale, self.y - y * self.scale);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(
            self.x + x1 * self.scale,
            self.y - y1 * self.scale,
            self.x + x * self.scale,
            self.y - y * self.scale,
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let s = self.scale;
        self.pb.cubic_to(
            self.x + x1 * s,
            self.y - y1 * s,
            self.x + x2 * s,
            self.y - y2 * s,
            self.x + x * s,
            self.y - y * s,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

impl SkFont {
    fn face(&self) -> ttf_parser::Face<'_> {
        ttf_parser::Face::parse(&self.data.0, self.data.1).expect("checked when loaded")
    }

    fn scale(&self, face: &ttf_parser::Face) -> f32 {
        self.size / face.units_per_em() as f32
    }

    pub fn metrics(&self) -> Metrics {
        let face = self.face();
        let s = self.scale(&face);
        let (up, ut) = face
            .underline_metrics()
            .map(|m| (-(m.position as f32) * s, m.thickness as f32 * s))
            .unwrap_or((self.size / 10.0, self.size / 16.0));
        let (sp, st) = face
            .strikeout_metrics()
            .map(|m| (-(m.position as f32) * s, m.thickness as f32 * s))
            .unwrap_or((-self.size / 3.0, self.size / 16.0));
        Metrics {
            ascent: -(face.ascender() as f32) * s,
            descent: -(face.descender() as f32) * s,
            leading: face.line_gap() as f32 * s,
            underline_position: up,
            underline_thickness: ut,
            strikeout_position: sp,
            strikeout_thickness: st,
        }
    }

    /// `SKFont.Spacing`: descent - ascent + leading.
    pub fn spacing(&self) -> f32 {
        let m = self.metrics();
        m.descent - m.ascent + m.leading
    }

    fn glyphs(&self, face: &ttf_parser::Face, text: &str) -> Vec<(ttf_parser::GlyphId, f32)> {
        let s = self.scale(face);
        text.chars()
            .map(|c| {
                let g = face.glyph_index(c).unwrap_or(ttf_parser::GlyphId(0));
                (g, face.glyph_hor_advance(g).unwrap_or(0) as f32 * s)
            })
            .collect()
    }

    /// `SKFont.MeasureText(text)`: the advance width.
    pub fn measure(&self, text: &str) -> f32 {
        let face = self.face();
        self.glyphs(&face, text).iter().map(|g| g.1).sum()
    }

    /// The glyph outlines of one line from `(x, baseline)`, plus underline and
    /// strikeout (`SkiaDrawing.TextDecorations`). `round_origins` snaps each glyph's
    /// origin to whole pixels, as non-subpixel glyph drawing does.
    pub fn line_path(
        &self,
        pb: &mut tiny_skia::PathBuilder,
        text: &str,
        x: f32,
        baseline: f32,
        round_origins: bool,
    ) {
        let face = self.face();
        let s = self.scale(&face);
        let mut pen = x;
        for (g, adv) in self.glyphs(&face, text) {
            let (ox, oy) = if round_origins {
                ((pen + 0.5).floor(), (baseline + 0.5).floor())
            } else {
                (pen, baseline)
            };
            let mut sink = OutlineToPath {
                pb,
                x: ox,
                y: oy,
                scale: s,
            };
            face.outline_glyph(g, &mut sink);
            pen += adv;
        }
        let width = pen - x;
        let m = self.metrics();
        let mut rect = |top: f32, h: f32| {
            if let Some(r) = tiny_skia::Rect::from_xywh(x, top, width, h) {
                pb.push_rect(r);
            }
        };
        if self.style & font_style::UNDERLINE != 0 {
            rect(baseline + m.underline_position, m.underline_thickness);
        }
        if self.style & font_style::STRIKEOUT != 0 {
            rect(baseline + m.strikeout_position, m.strikeout_thickness);
        }
    }
}

fn lines(text: &str) -> Vec<String> {
    text.replace('\r', "")
        .split('\n')
        .map(str::to_string)
        .collect()
}

/// `SkiaDrawing.MeasureText`: whole pixels, rounded up.
pub fn measure_text(font: &SkFont, text: &str) -> (i32, i32) {
    let ls = lines(text);
    let w = ls.iter().map(|l| font.measure(l)).fold(f32::MIN, f32::max);
    (
        w.ceil() as i32,
        (ls.len() as f32 * font.spacing()).ceil() as i32,
    )
}

/// The path `SkiaDrawing.DrawText` fills (glyph origins snapped to pixels).
pub fn draw_text_path(font: &SkFont, text: &str, x: f32, y: f32) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    let mut baseline = y - font.metrics().ascent;
    for line in lines(text) {
        font.line_path(&mut pb, &line, x, baseline, true);
        baseline += font.spacing();
    }
    pb.finish()
}

/// `SkiaDrawing.TextPath`.
pub fn text_path(font: &SkFont, text: &str) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    let mut baseline = -font.metrics().ascent;
    for line in lines(text) {
        font.line_path(&mut pb, &line, 0.0, baseline, false);
        baseline += font.spacing();
    }
    pb.finish()
}
