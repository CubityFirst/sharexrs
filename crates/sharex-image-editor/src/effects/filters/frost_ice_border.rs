//! `FrostIceBorderImageEffect` (`Core/ImageEffects/Filters/FrostIceBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `FrostIceBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct FrostIceBorderImageEffect {
    pub border_size: i32,
    pub ice_color: SkColor,
    pub crystallization: f32,
    pub transparency: f32,
    pub sparkle: bool,
}

impl Default for FrostIceBorderImageEffect {
    fn default() -> Self {
        FrostIceBorderImageEffect {
            border_size: 55,
            ice_color: SkColor::argb(0xFFC8E1FF),
            crystallization: 65.0,
            transparency: 25.0,
            sparkle: true,
        }
    }
}

impl EffectImpl for FrostIceBorderImageEffect {
    fn info() -> Info {
        Info::new("frost_ice_border", "Frost ice border", EffectCategory::Filters, "image-effect/snowflake")
            .desc("Adds a frosted ice crystal border with rime buildup and icy blue highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 300, 55, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("ice_color", "Ice color", SkColor::argb(0xFFC8E1FF), |e: &mut Self, v| e.ice_color = v),
            float_slider::<Self>("crystallization", "Crystallization", 0.0, 100.0, 65.0, |e: &mut Self, v| e.crystallization = v),
            float_slider::<Self>("transparency", "Transparency", 0.0, 80.0, 25.0, |e: &mut Self, v| e.transparency = v),
            bool_param::<Self>("sparkle", "Ice sparkle", true, |e: &mut Self, v| e.sparkle = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const SEED: i32 = 4519;
        let border = self.border_size.clamp(10, 300);
        let crystal = clampf(self.crystallization, 0.0, 100.0) / 100.0;
        let transparency = clampf(self.transparency, 0.0, 80.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let ic = self.ice_color;
        let (ir, ig, ib) = (ic.r as f32 / 255.0, ic.g as f32 / 255.0, ic.b as f32 / 255.0);
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        for y in 0..nh {
            let top = y < border;
            let bottom = y >= border + ih;
            for x in 0..nw {
                let left = x < border;
                let right = x >= border + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let cross = cross_ratio(x, y, border, iw, ih);
                let thick = smooth_step(0.0, 0.5, cross);
                let coarse = fractal_noise(x as f32 * 0.02, y as f32 * 0.02, 4, 2.5, 0.45, SEED);
                let fine = fractal_noise(x as f32 * 0.08, y as f32 * 0.08, 3, 2.0, 0.5, SEED ^ 0x3F);
                let pattern = coarse + fine * 0.5;
                let cz = if crystal > 0.0 { clampf(pattern, 0.0, 1.0).powf(1.0 - crystal * 0.7) } else { pattern };
                let hl = smooth_step(0.55, 0.75, cz);
                let mut r = lerp(ir, 1.0, hl * 0.6);
                let mut g = lerp(ig, 1.0, hl * 0.5);
                let b = lerp(ib, 1.0, hl * 0.35);
                let vein = smooth_step(0.2, 0.35, pattern);
                r -= vein * 0.12;
                g -= vein * 0.06;
                let mut alpha = lerp(1.0 - transparency, 1.0, thick);
                alpha *= 0.85 + cz * 0.15;
                alpha = clampf(alpha, 0.0, 1.0);
                dst[(y * nw + x) as usize] = SkColor::rgba(
                    clamp_to_byte(r * 255.0),
                    clamp_to_byte(g * 255.0),
                    clamp_to_byte(b * 255.0),
                    clamp_to_byte(alpha * 255.0),
                );
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        canvas.draw_bitmap(source, border as f32, border as f32, None);
        if self.sparkle {
            let count = (nw + nh) / 12;
            for i in 0..count {
                let hx = hash01(i, SEED ^ 0xAA, SEED);
                let hy = hash01(i, SEED ^ 0xBB, SEED ^ 0x11);
                let hs = hash01(i, SEED ^ 0xCC, SEED ^ 0x22);
                let sx = (hx * nw as f32) as i32;
                let sy = (hy * nh as f32) as i32;
                let in_border = sx < border || sx >= border + iw || sy < border || sy >= border + ih;
                if !in_border {
                    continue;
                }
                let radius = 0.8 + hs * 2.2;
                let paint = solid(SkColor::rgba(255, 255, 255, (140.0 + hs * 115.0) as u8), true);
                let (fx, fy) = (sx as f32, sy as f32);
                sk::draw_circle(&mut canvas, fx, fy, radius, &paint, None);
                if hs > 0.5 {
                    let len = radius * 2.5;
                    let star = solid(SkColor::rgba(255, 255, 255, (80.0 + hs * 80.0) as u8), true);
                    let pen = Pen::new(0.5);
                    sk::draw_line(&mut canvas, fx - len, fy, fx + len, fy, &pen, &star);
                    sk::draw_line(&mut canvas, fx, fy - len, fx, fy + len, &pen, &star);
                }
            }
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn cross_ratio(x: i32, y: i32, border: i32, sw: i32, sh: i32) -> f32 {
    let (l, t) = (border as f32, border as f32);
    let (r, b) = ((border + sw) as f32, (border + sh) as f32);
    let cx = clampf(x as f32, l, r - 1.0);
    let cy = clampf(y as f32, t, b - 1.0);
    let (dx, dy) = (x as f32 - cx, y as f32 - cy);
    clampf((dx * dx + dy * dy).sqrt() / border as f32, 0.0, 1.0)
}
