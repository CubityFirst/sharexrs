//! `WoodenFrameImageEffect` (`Core/ImageEffects/Filters/WoodenFrameImageEffect.cs`).

use crate::effects::prelude::*;

/// `WoodenFrameImageEffect`.
#[derive(Clone, Debug)]
pub struct WoodenFrameImageEffect {
    pub frame_width: i32,
    pub grain_strength: f32,
    pub bevel_strength: f32,
    pub wood_color: SkColor,
    pub seed: i32,
}

impl Default for WoodenFrameImageEffect {
    fn default() -> Self {
        WoodenFrameImageEffect {
            frame_width: 48,
            grain_strength: 60.0,
            bevel_strength: 65.0,
            wood_color: SkColor::argb(0xFF8B5E3C),
            seed: 1649,
        }
    }
}

impl EffectImpl for WoodenFrameImageEffect {
    fn info() -> Info {
        Info::new("wooden_frame", "Wooden frame", EffectCategory::Filters, "image-effect/frame")
            .desc("Adds a procedural wooden frame around the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("frame_width", "Frame width", 2, 300, 48, |e: &mut Self, v| e.frame_width = v),
            float_slider::<Self>("grain_strength", "Grain strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.grain_strength = v),
            float_slider::<Self>("bevel_strength", "Bevel strength", 0.0, 100.0, 65.0, |e: &mut Self, v| e.bevel_strength = v),
            color_param::<Self>("wood_color", "Wood color", SkColor::argb(0xFF8B5E3C), |e: &mut Self, v| e.wood_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let fw = self.frame_width.clamp(2, 300);
        let grain_s = clampf(self.grain_strength, 0.0, 100.0) / 100.0;
        let bevel_s = clampf(self.bevel_strength, 0.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + fw * 2, ih + fw * 2);
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        let seed = self.seed;
        let wc = self.wood_color;
        for y in 0..nh {
            let top = y < fw;
            let bottom = y >= fw + ih;
            for x in 0..nw {
                let left = x < fw;
                let right = x >= fw + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let (side, along, across, outer, inner, bseed) = if top {
                    (0, x as f32, y as f32, y as f32, ((fw - 1) - y) as f32, seed ^ 0x1C3)
                } else if bottom {
                    (1, x as f32, (y - (fw + ih)) as f32, ((nh - 1) - y) as f32, (y - (fw + ih)) as f32, seed ^ 0x29B)
                } else if left {
                    (2, y as f32, x as f32, x as f32, ((fw - 1) - x) as f32, seed ^ 0x35F)
                } else {
                    (3, y as f32, (x - (fw + iw)) as f32, ((nw - 1) - x) as f32, (x - (fw + iw)) as f32, seed ^ 0x41D)
                };
                let span = 1f32.max(fw as f32 - 1.0);
                let ratio = across / span;
                let variation = ((hash01(bseed, fw, seed) * 2.0) - 1.0) * 0.06;
                let grain = wood_grain(along, across, grain_s, bseed);
                let mut shade = variation + bevel_shade(side, ratio, outer, inner, fw, bevel_s);
                shade -= miter_seam(x, y, nw, nh, fw);
                let red = wc.r as f32 * (1.0 + grain + shade + 0.03);
                let green = wc.g as f32 * (1.0 + (grain * 0.82) + shade);
                let blue = wc.b as f32 * (1.0 + (grain * 0.48) + shade - 0.03);
                dst[(y * nw + x) as usize] = SkColor::rgba(clamp_to_byte(red), clamp_to_byte(green), clamp_to_byte(blue), 255);
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        draw_bitmap(&mut out, source, fw as f32, fw as f32);
        Ok(out)
    }
}

// ---- port ----

/// `MathF.Sign` (0 for 0).
fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn wood_grain(along: f32, across: f32, strength: f32, seed: i32) -> f32 {
    let coarse = (hash01((along * 0.20) as i32, (across * 0.08) as i32, seed) * 2.0) - 1.0;
    let medium = (hash01((along * 0.55) as i32, (across * 0.16) as i32, seed ^ 0x57) * 2.0) - 1.0;
    let fine = (hash01((along * 1.30) as i32, (across * 0.42) as i32, seed ^ 0xA1) * 2.0) - 1.0;
    let w1 = ((along * 0.075) + (coarse * 5.6) + (across * 0.028)).sin();
    let w2 = ((along * 0.19) - (medium * 3.1) + (across * 0.012)).sin();
    let ridge = sign(w1) * w1.abs().powf(1.75);
    let pore = fine * 0.12;
    let tint = ((across * 0.55) + (coarse * 2.2)).sin() * 0.04;
    ((ridge * 0.20) + (w2 * 0.08) + pore + tint) * strength
}

fn bevel_shade(side: i32, ratio: f32, outer: f32, inner: f32, fw: i32, strength: f32) -> f32 {
    let span = 2f32.max(fw as f32 * (0.16 + (0.10 * strength)));
    let om = 1.0 - smooth_step(0.0, span, outer);
    let im = 1.0 - smooth_step(0.0, span, inner);
    let slope = 0.5 - ratio;
    let shade = match side {
        0 => (om * 0.24) - (im * 0.28) + (slope * 0.12),
        2 => (om * 0.18) - (im * 0.22) + (slope * 0.10),
        3 => -(om * 0.20) + (im * 0.08) - (slope * 0.08),
        _ => -(om * 0.26) + (im * 0.10) - (slope * 0.12),
    };
    shade * strength
}

fn miter_seam(x: i32, y: i32, width: i32, height: i32, fw: i32) -> f32 {
    let seam = 1.5f32.max(fw as f32 * 0.08);
    let f = |d: f32| (1.0 - smooth_step(0.0, seam, d.abs())) * 0.10;
    let fw1 = fw as f32 - 1.0;
    if x < fw && y < fw {
        return f((x + y) as f32 - fw1);
    }
    if x >= width - fw && y < fw {
        return f(((width - 1 - x) + y) as f32 - fw1);
    }
    if x < fw && y >= height - fw {
        return f((x + (height - 1 - y)) as f32 - fw1);
    }
    if x >= width - fw && y >= height - fw {
        return f(((width - 1 - x) + (height - 1 - y)) as f32 - fw1);
    }
    0.0
}
