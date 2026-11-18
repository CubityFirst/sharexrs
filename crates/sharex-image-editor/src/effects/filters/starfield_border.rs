//! `StarfieldBorderImageEffect` (`Core/ImageEffects/Filters/StarfieldBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `StarfieldBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct StarfieldBorderImageEffect {
    pub border_size: i32,
    pub bg_color: SkColor,
    pub nebula_color: SkColor,
    pub star_density: f32,
    pub nebula_intensity: f32,
    pub bright_stars: bool,
}

impl Default for StarfieldBorderImageEffect {
    fn default() -> Self {
        StarfieldBorderImageEffect {
            border_size: 60,
            bg_color: SkColor::argb(0xFF050512),
            nebula_color: SkColor::argb(0xFF280A50),
            star_density: 65.0,
            nebula_intensity: 45.0,
            bright_stars: true,
        }
    }
}

impl EffectImpl for StarfieldBorderImageEffect {
    fn info() -> Info {
        Info::new("starfield_border", "Starfield border", EffectCategory::Filters, "image-effect/orbit")
            .desc("Adds a cosmic starfield border with scattered stars, nebula wisps, and subtle color tints.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                12,
                300,
                60,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "bg_color",
                "Background color",
                SkColor::argb(0xFF050512),
                |e: &mut Self, v| e.bg_color = v,
            ),
            color_param::<Self>(
                "nebula_color",
                "Nebula tint",
                SkColor::argb(0xFF280A50),
                |e: &mut Self, v| e.nebula_color = v,
            ),
            float_slider::<Self>(
                "star_density",
                "Star density",
                10.0,
                100.0,
                65.0,
                |e: &mut Self, v| e.star_density = v,
            ),
            float_slider::<Self>(
                "nebula_intensity",
                "Nebula intensity",
                0.0,
                100.0,
                45.0,
                |e: &mut Self, v| e.nebula_intensity = v,
            ),
            bool_param::<Self>(
                "bright_stars",
                "Bright star glow",
                true,
                |e: &mut Self, v| e.bright_stars = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const SEED: i32 = 4207;
        let border = self.border_size.clamp(12, 300);
        let star_dens = clampf(self.star_density, 10.0, 100.0) / 100.0;
        let neb_str = clampf(self.nebula_intensity, 0.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        let scale = 0.012f32;
        let (bg, nc) = (self.bg_color, self.nebula_color);
        for y in 0..nh {
            let top = y < border;
            let bottom = y >= border + ih;
            for x in 0..nw {
                let left = x < border;
                let right = x >= border + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let mut r = bg.r as f32 / 255.0;
                let mut g = bg.g as f32 / 255.0;
                let mut b = bg.b as f32 / 255.0;
                let neb1 = fractal_noise(x as f32 * scale, y as f32 * scale, 4, 2.0, 0.5, SEED);
                let neb2 = fractal_noise(
                    x as f32 * scale * 0.7 + 300.0,
                    y as f32 * scale * 0.7 + 500.0,
                    3,
                    2.0,
                    0.5,
                    SEED ^ 0x33,
                );
                let m1 = clamp01((neb1 + 0.3) * 0.7) * neb_str;
                let m2 = clamp01((neb2 + 0.2) * 0.5) * neb_str * 0.5;
                r += (nc.r as f32 / 255.0) * m1 + (0.1 * m2);
                g += (nc.g as f32 / 255.0) * m1 + (0.15 * m2);
                b += (nc.b as f32 / 255.0) * m1 + (0.25 * m2);
                let sh = hash01(x, y, SEED ^ 0xAA);
                let thr = 1.0 - star_dens * 0.015;
                if sh > thr {
                    let mut sb = (sh - thr) / (1.0 - thr);
                    sb *= sb;
                    let hue = hash01(x ^ 0x55, y ^ 0x33, SEED ^ 0xBB);
                    r += sb * (0.7 + hue * 0.3);
                    g += sb * (0.7 + (1.0 - hue) * 0.3);
                    b += sb * 0.9;
                }
                dst[(y * nw + x) as usize] = SkColor::rgba(
                    clamp_to_byte(r * 255.0),
                    clamp_to_byte(g * 255.0),
                    clamp_to_byte(b * 255.0),
                    255,
                );
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        if self.bright_stars {
            let count = ((nw + nh) as f32 * 0.15 * star_dens) as i32;
            let bf = border as f32;
            for i in 0..count {
                let sx = hash01(i, 0, SEED ^ 0xDE) * nw as f32;
                let sy = hash01(0, i, SEED ^ 0xEF) * nh as f32;
                let in_border =
                    sx < bf || sx >= (border + iw) as f32 || sy < bf || sy >= (border + ih) as f32;
                if !in_border {
                    continue;
                }
                let br = hash01(i, i, SEED ^ 0xF1);
                let size = 0.8 + br * 2.5;
                let temp = hash01(i ^ 0x77, i ^ 0x99, SEED ^ 0xF2);
                let sr = clamp_to_byte(200.0 + 55.0 * br);
                let sg = clamp_to_byte(190.0 + 55.0 * br * (0.9 + temp * 0.1));
                let sbl = clamp_to_byte(180.0 + 75.0 * br * (0.5 + temp * 0.5));
                let alpha = clamp_to_byte(160.0 + 95.0 * br);
                sk::draw_circle_blur(
                    &mut canvas,
                    sx,
                    sy,
                    size * 2.5,
                    &solid(SkColor::rgba(sr, sg, sbl, alpha / 2), true),
                    None,
                    3.0,
                );
                sk::draw_circle(
                    &mut canvas,
                    sx,
                    sy,
                    size,
                    &solid(SkColor::rgba(sr, sg, sbl, alpha), true),
                    None,
                );
            }
        }
        canvas.draw_bitmap(source, border as f32, border as f32, None);
        drop(canvas);
        Ok(out)
    }
}
