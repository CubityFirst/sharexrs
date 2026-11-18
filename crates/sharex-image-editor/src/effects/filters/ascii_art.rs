//! `ASCIIArtImageEffect` (`Core/ImageEffects/Filters/ASCIIArtImageEffect.cs`).

use crate::effects::prelude::*;

/// `ASCIIArtImageEffect`.
#[derive(Clone, Debug)]
pub struct ASCIIArtImageEffect {
    pub cell_size: i32,
    pub contrast: f32,
    pub character_set: String,
    pub invert: bool,
    pub dark_background: bool,
    pub use_source_color: bool,
}

impl Default for ASCIIArtImageEffect {
    fn default() -> Self {
        ASCIIArtImageEffect {
            cell_size: 8,
            contrast: 110.0,
            character_set: "@%#*+=-:. ".to_string(),
            invert: false,
            dark_background: true,
            use_source_color: true,
        }
    }
}

impl EffectImpl for ASCIIArtImageEffect {
    fn info() -> Info {
        Info::new("ascii_art", "ASCII art", EffectCategory::Filters, "app/terminal")
            .desc("Converts the image into ASCII character art.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell size", 4, 24, 8, |e: &mut Self, v| e.cell_size = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 110.0, |e: &mut Self, v| e.contrast = v),
            text_param::<Self>("character_set", "Character set", "@%#*+=-:. ", |e: &mut Self, v| e.character_set = v),
            bool_param::<Self>("invert", "Invert", false, |e: &mut Self, v| e.invert = v),
            bool_param::<Self>("dark_background", "Dark background", true, |e: &mut Self, v| e.dark_background = v),
            bool_param::<Self>("use_source_color", "Use source color", true, |e: &mut Self, v| e.use_source_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let cell = self.cell_size.clamp(4, 24);
        let contrast = clampf(self.contrast, 50.0, 200.0) / 100.0;
        let charset = normalize_charset(&self.character_set);
        let (width, height) = (source.width() as i32, source.height() as i32);
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(if self.dark_background { [12, 12, 12, 255] } else { [255, 255, 255, 255] });
        let font = Font::new(Typeface::from_family("Consolas", 400, false), cell as f32 * 1.02);
        let columns = ((width as f32 / cell as f32) as f64).ceil() as i32;
        let rows = ((height as f32 / cell as f32) as f64).ceil() as i32;
        let baseline = cell as f32 * 0.90;
        for row in 0..rows {
            let y0 = row * cell;
            let y1 = height.min(y0 + cell);
            for col in 0..columns {
                let x0 = col * cell;
                let x1 = width.min(x0 + cell);
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }
                let (mut sr, mut sg, mut sb, mut sa, mut n) = (0f32, 0f32, 0f32, 0f32, 0i32);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let c = src[(y * width + x) as usize];
                        sr += c.r as f32;
                        sg += c.g as f32;
                        sb += c.b as f32;
                        sa += c.a as f32;
                        n += 1;
                    }
                }
                let inv = 1.0 / n as f32;
                let (ar, ag, ab, aa) = (sr * inv, sg * inv, sb * inv, clamp_to_byte(sa * inv));
                let mut lum = ((0.2126 * ar) + (0.7152 * ag) + (0.0722 * ab)) / 255.0;
                lum = clampf(((lum - 0.5) * contrast) + 0.5, 0.0, 1.0);
                if self.invert {
                    lum = 1.0 - lum;
                }
                let idx = (lum * (charset.len() - 1) as f32).round_ties_even() as i32;
                let c = charset[idx.clamp(0, charset.len() as i32 - 1) as usize];
                if c.is_whitespace() {
                    continue;
                }
                let color = if self.use_source_color {
                    SkColor::rgba(clamp_to_byte(ar), clamp_to_byte(ag), clamp_to_byte(ab), aa)
                } else if self.dark_background {
                    let v = clamp_to_byte((0.80 + (0.20 * lum)) * 255.0);
                    SkColor::rgba(v, v, v, aa)
                } else {
                    let v = clamp_to_byte((0.10 + (0.55 * (1.0 - lum))) * 255.0);
                    SkColor::rgba(v, v, v, aa)
                };
                font.draw(&mut canvas, &c.to_string(), x0 as f32, y0 as f32 + baseline, Align::Left, &solid(color, true));
            }
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn normalize_charset(input: &str) -> Vec<char> {
    let default = || "@%#*+=-:. ".chars().collect();
    if input.is_empty() {
        return default();
    }
    let mut chars: Vec<char> = Vec::new();
    for c in input.chars() {
        if !chars.contains(&c) {
            chars.push(c);
        }
    }
    if chars.is_empty() {
        return default();
    }
    if chars.len() == 1 {
        chars.push(' ');
    }
    chars
}
