//! `MatrixDigitalRainImageEffect` (`Core/ImageEffects/Filters/MatrixDigitalRainImageEffect.cs`).

use crate::effects::prelude::*;

/// `MatrixDigitalRainImageEffect`.
#[derive(Clone, Debug)]
pub struct MatrixDigitalRainImageEffect {
    pub cell_size: i32,
    pub density: f32,
    pub trail_length: i32,
    pub glow_amount: f32,
    pub source_blend: f32,
    pub rain_offset: f32,
    pub luminance_influence: f32,
    pub character_set: String,
    pub seed: i32,
}

impl Default for MatrixDigitalRainImageEffect {
    fn default() -> Self {
        MatrixDigitalRainImageEffect {
            cell_size: 12,
            density: 85.0,
            trail_length: 12,
            glow_amount: 40.0,
            source_blend: 22.0,
            rain_offset: 0.0,
            luminance_influence: 65.0,
            character_set: "01<>[]{}*+-/\\=#$%&".to_string(),
            seed: 1337,
        }
    }
}

impl EffectImpl for MatrixDigitalRainImageEffect {
    fn info() -> Info {
        Info::new(
            "matrix_digital_rain",
            "Matrix digital rain",
            EffectCategory::Filters,
            "app/terminal",
        )
        .desc("Overlays a Matrix-style digital rain effect using source luminance.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell size", 6, 24, 12, |e: &mut Self, v| {
                e.cell_size = v
            }),
            float_slider::<Self>("density", "Density", 0.0, 100.0, 85.0, |e: &mut Self, v| {
                e.density = v
            }),
            int_slider::<Self>(
                "trail_length",
                "Trail length",
                3,
                50,
                12,
                |e: &mut Self, v| e.trail_length = v,
            ),
            float_slider::<Self>(
                "glow_amount",
                "Glow amount",
                0.0,
                100.0,
                40.0,
                |e: &mut Self, v| e.glow_amount = v,
            ),
            float_slider::<Self>(
                "source_blend",
                "Source blend",
                0.0,
                100.0,
                22.0,
                |e: &mut Self, v| e.source_blend = v,
            ),
            float_slider::<Self>(
                "rain_offset",
                "Rain offset",
                0.0,
                100.0,
                0.0,
                |e: &mut Self, v| e.rain_offset = v,
            ),
            float_slider::<Self>(
                "luminance_influence",
                "Luminance influence",
                0.0,
                100.0,
                65.0,
                |e: &mut Self, v| e.luminance_influence = v,
            ),
            text_param::<Self>(
                "character_set",
                "Character set",
                "01<>[]{}*+-/\\=#$%&",
                |e: &mut Self, v| e.character_set = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        let cell = self.cell_size.clamp(6, 24);
        let density = clampf(self.density, 0.0, 100.0) / 100.0;
        let trail = self.trail_length.clamp(3, 50);
        let glow = clampf(self.glow_amount, 0.0, 100.0) / 100.0;
        let source_blend = clampf(self.source_blend, 0.0, 100.0) / 100.0;
        let rain_offset = clampf(self.rain_offset, 0.0, 100.0) / 100.0;
        let lum_inf = clampf(self.luminance_influence, 0.0, 100.0) / 100.0;
        let chars = normalize_chars(&self.character_set);
        let columns = ((width as f32 / cell as f32) as f64).ceil() as i32;
        let rows = ((height as f32 / cell as f32) as f64).ceil() as i32;
        let src = pixels(source);
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        let mut bg = solid(SkColor::rgb(0, 12, 0), false);
        bg.anti_alias = false;
        sk::draw_rect(
            &mut canvas,
            RRect::ltrb(0.0, 0.0, width as f32, height as f32),
            &bg,
            None,
        );
        if source_blend > 0.0 {
            let mut p = Paint::shader(Shader::Color([255, 255, 255, 255]));
            p.alpha = clamp_to_byte(source_blend * 255.0);
            canvas.draw_bitmap(source, 0.0, 0.0, Some(&p));
        }
        let font = Font::new(
            Typeface::from_family("Consolas", 400, false),
            cell as f32 * 1.02,
        );
        let sigma = 2.2 + (glow * 3.4);
        let baseline = cell as f32 * 0.88;
        let seed = self.seed;
        for col in 0..columns {
            let head = ((hash01(col, seed, seed ^ 9) + rain_offset) % 1.0) * rows as f32;
            let head2 =
                ((hash01(col, seed ^ 37, seed ^ 101) + (rain_offset * 0.63)) % 1.0) * rows as f32;
            for row in 0..rows {
                let (x0, y0) = (col * cell, row * cell);
                if x0 >= width || y0 >= height {
                    continue;
                }
                let (x1, y1) = (width.min(x0 + cell), height.min(y0 + cell));
                let (mut sl, mut sa, mut n) = (0f32, 0f32, 0i32);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let c = src[(y * width + x) as usize];
                        sl +=
                            ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32))
                                / 255.0;
                        sa += c.a as f32;
                        n += 1;
                    }
                }
                let (lum, alpha) = if n == 0 {
                    (0.0, 255)
                } else {
                    (sl * (1.0 / n as f32), clamp_to_byte(sa * (1.0 / n as f32)))
                };
                let spawn = hash01(col, row, seed ^ 503);
                let local = density * (0.35 + (0.65 * (lum_inf * lum + (1.0 - lum_inf))));
                if spawn > local {
                    continue;
                }
                let t1 = trail_factor(row, head, rows, trail as f32);
                let t2 = trail_factor(row, head2, rows, trail as f32 * 0.75);
                let tf = t1.max(t2);
                let base_i = (lum * lum_inf) + (0.35 * (1.0 - lum_inf));
                let intensity = clampf((base_i * 0.55) + (tf * 0.9), 0.0, 1.2);
                if intensity <= 0.03 {
                    continue;
                }
                let n_chars = chars.len() as f32;
                let mut ci = (((lum * n_chars) + (row as f32 * 0.61) + (col as f32 * 0.17))
                    % n_chars)
                    .floor()
                    .abs() as i32;
                if ci >= chars.len() as i32 {
                    ci = chars.len() as i32 - 1;
                }
                let glyph = chars[ci as usize].to_string();
                let gs = glow * 0f32.max(intensity - 0.35);
                let (x, y) = (x0 as f32, y0 as f32 + baseline);
                if gs > 0.0 {
                    let c = SkColor::rgba(
                        clamp_to_byte(56.0 + (intensity * 72.0)),
                        clamp_to_byte(180.0 + (intensity * 60.0)),
                        clamp_to_byte(56.0 + (intensity * 70.0)),
                        clamp_to_byte(gs * 180.0),
                    );
                    if glow > 0.0 {
                        font.draw_blurred(&mut canvas, &glyph, x, y, &solid(c, true), sigma);
                    } else {
                        font.draw(&mut canvas, &glyph, x, y, Align::Left, &solid(c, true));
                    }
                }
                let c = SkColor::rgba(
                    clamp_to_byte(28.0 + (intensity * 118.0)),
                    clamp_to_byte(120.0 + (intensity * 135.0)),
                    clamp_to_byte(28.0 + (intensity * 118.0)),
                    alpha,
                );
                font.draw(&mut canvas, &glyph, x, y, Align::Left, &solid(c, true));
            }
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn normalize_chars(input: &str) -> Vec<char> {
    if is_blank(input) {
        return "01<>[]{}*+-/\\=#$%&".chars().collect();
    }
    let t = input.trim();
    if t.chars().count() == 1 {
        return format!("{t}0").chars().collect();
    }
    t.chars().collect()
}

fn trail_factor(row: i32, head: f32, rows: i32, length: f32) -> f32 {
    let mut d = row as f32 - head;
    if d < 0.0 {
        d += rows as f32;
    }
    if d < 0.0 || d > length {
        return 0.0;
    }
    1.0 - (d / 1f32.max(length))
}
