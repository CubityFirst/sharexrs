//! `VintagePrintDamageImageEffect` (`Core/ImageEffects/Filters/VintagePrintDamageImageEffect.cs`).

use crate::effects::prelude::*;

/// `VintagePrintDamageImageEffect`.
#[derive(Clone, Debug)]
pub struct VintagePrintDamageImageEffect {
    pub sepia_amount: f32,
    pub fade_amount: f32,
    pub grain_amount: f32,
    pub scratch_amount: f32,
    pub dust_amount: f32,
    pub seed: i32,
}

impl Default for VintagePrintDamageImageEffect {
    fn default() -> Self {
        VintagePrintDamageImageEffect {
            sepia_amount: 65.0,
            fade_amount: 30.0,
            grain_amount: 25.0,
            scratch_amount: 25.0,
            dust_amount: 20.0,
            seed: 1947,
        }
    }
}

impl EffectImpl for VintagePrintDamageImageEffect {
    fn info() -> Info {
        Info::new("vintage_print_damage", "Vintage print damage", EffectCategory::Filters, "image-effect/newspaper")
            .desc("Simulates aged print damage with sepia toning, grain, scratches, and dust.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("sepia_amount", "Sepia amount", 0.0, 100.0, 65.0, |e: &mut Self, v| e.sepia_amount = v),
            float_slider::<Self>("fade_amount", "Fade amount", 0.0, 100.0, 30.0, |e: &mut Self, v| e.fade_amount = v),
            float_slider::<Self>("grain_amount", "Grain amount", 0.0, 100.0, 25.0, |e: &mut Self, v| e.grain_amount = v),
            float_slider::<Self>("scratch_amount", "Scratch amount", 0.0, 100.0, 25.0, |e: &mut Self, v| e.scratch_amount = v),
            float_slider::<Self>("dust_amount", "Dust amount", 0.0, 100.0, 20.0, |e: &mut Self, v| e.dust_amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let sepia = clampf(self.sepia_amount, 0.0, 100.0) / 100.0;
        let fade = clampf(self.fade_amount, 0.0, 100.0) / 100.0;
        let grain = clampf(self.grain_amount, 0.0, 100.0) / 100.0;
        let scratches = clampf(self.scratch_amount, 0.0, 100.0) / 100.0;
        let dust = clampf(self.dust_amount, 0.0, 100.0) / 100.0;
        if sepia <= 0.0 && fade <= 0.0 && grain <= 0.0 && scratches <= 0.0 && dust <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let cx = (width - 1) as f32 * 0.5;
        let cy = (height - 1) as f32 * 0.5;
        let inv_cx = if cx > 0.0 { 1.0 / cx } else { 1.0 };
        let inv_cy = if cy > 0.0 { 1.0 / cy } else { 1.0 };
        const INV_SQRT2: f32 = 0.70710678;
        let dust_cell = 8f32.max(26.0 - (dust * 18.0));
        let seed = self.seed;
        for y in 0..height {
            let row = y * width;
            let yf = y as f32;
            let dyn_ = (yf - cy) * inv_cy;
            for x in 0..width {
                let xf = x as f32;
                let s = src[(row + x) as usize];
                let (mut r, mut g, mut b) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let sr = (r * 0.393) + (g * 0.769) + (b * 0.189);
                let sg = (r * 0.349) + (g * 0.686) + (b * 0.168);
                let sb = (r * 0.272) + (g * 0.534) + (b * 0.131);
                r = lerp(r, sr, sepia);
                g = lerp(g, sg, sepia);
                b = lerp(b, sb, sepia);
                let contrast = 1.0 - (fade * 0.35);
                let lift = fade * 0.045;
                r = ((r - 0.5) * contrast) + 0.5 + lift;
                g = ((g - 0.5) * contrast) + 0.5 + lift;
                b = ((b - 0.5) * contrast) + 0.5 + lift;
                let pm = 0.08 + (fade * 0.10);
                r = lerp(r, 0.93, pm);
                g = lerp(g, 0.86, pm);
                b = lerp(b, 0.74, pm);
                if grain > 0.0 {
                    let n0 = (hash01(x, y, seed) * 2.0) - 1.0;
                    let n1 = (hash01(x, y, seed ^ 193) * 2.0) - 1.0;
                    let n2 = (hash01(x, y, seed ^ 761) * 2.0) - 1.0;
                    let amp = grain * 0.12;
                    r += n0 * amp;
                    g += n1 * amp;
                    b += n2 * amp;
                }
                if scratches > 0.0 {
                    let column = x / 2;
                    let chance = hash01(column, 0, seed ^ 71);
                    if chance > 0.985 - (scratches * 0.12) {
                        let center = (column as f32 * 2.0)
                            + ((hash01(column, 1, seed ^ 97) - 0.5) * 2.0)
                            + (((yf * 0.03) + (chance * 12.0)).sin() * (0.5 + (scratches * 2.0)));
                        let wpx = 0.5 + (hash01(column, 2, seed ^ 131) * 1.8);
                        let distance = (xf - center).abs() / wpx.max(0.001);
                        let li = (1.0 - smooth_step(0.1, 1.0, distance)) * scratches;
                        if hash01(column, 3, seed ^ 173) > 0.55 {
                            r += li * 0.25;
                            g += li * 0.23;
                            b += li * 0.20;
                        } else {
                            r -= li * 0.22;
                            g -= li * 0.22;
                            b -= li * 0.22;
                        }
                    }
                }
                if dust > 0.0 {
                    let cell_x = (xf / dust_cell) as i32;
                    let cell_y = (yf / dust_cell) as i32;
                    let chance = hash01(cell_x, cell_y, seed ^ 521);
                    if chance > 0.988 - (dust * 0.23) {
                        let sx = (cell_x as f32 + hash01(cell_x, cell_y, seed ^ 577)) * dust_cell;
                        let sy = (cell_y as f32 + hash01(cell_x, cell_y, seed ^ 653)) * dust_cell;
                        let radius = dust_cell * (0.10 + (0.24 * hash01(cell_x, cell_y, seed ^ 997)));
                        let dx = xf - sx;
                        let dy = yf - sy;
                        let dist = ((dx * dx) + (dy * dy)).sqrt();
                        let spot = (1.0 - smooth_step(0.15, 1.0, dist / radius.max(0.001))) * dust;
                        if hash01(cell_x, cell_y, seed ^ 733) > 0.60 {
                            r += spot * 0.18;
                            g += spot * 0.16;
                            b += spot * 0.14;
                        } else {
                            r -= spot * 0.20;
                            g -= spot * 0.19;
                            b -= spot * 0.18;
                        }
                    }
                }
                let dxn = (xf - cx) * inv_cx;
                let d01 = ((dxn * dxn) + (dyn_ * dyn_)).sqrt() * INV_SQRT2;
                let burn = smooth_step(0.62, 1.0, d01) * (0.10 + (fade * 0.32) + (scratches * 0.18));
                r *= 1.0 - burn;
                g *= 1.0 - burn;
                b *= 1.0 - burn;
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
