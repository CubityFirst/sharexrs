//! `RainyWindowImageEffect` (`Core/ImageEffects/Filters/RainyWindowImageEffect.cs`).

use crate::effects::prelude::*;

/// `RainyWindowImageEffect`.
#[derive(Clone, Debug)]
pub struct RainyWindowImageEffect {
    pub distortion: f32,
    pub streak_density: f32,
    pub mist_amount: f32,
    pub droplet_amount: f32,
    pub seed: i32,
}

impl Default for RainyWindowImageEffect {
    fn default() -> Self {
        RainyWindowImageEffect {
            distortion: 8.0,
            streak_density: 45.0,
            mist_amount: 25.0,
            droplet_amount: 35.0,
            seed: 8723,
        }
    }
}

impl EffectImpl for RainyWindowImageEffect {
    fn info() -> Info {
        Info::new(
            "rainy_window",
            "Rainy window",
            EffectCategory::Filters,
            "image-effect/cloud-rain",
        )
        .desc("Simulates looking through a rain-streaked window with distortion and mist.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "distortion",
                "Distortion",
                0.0,
                30.0,
                8.0,
                |e: &mut Self, v| e.distortion = v,
            ),
            float_slider::<Self>(
                "streak_density",
                "Streak density",
                0.0,
                100.0,
                45.0,
                |e: &mut Self, v| e.streak_density = v,
            ),
            float_slider::<Self>(
                "mist_amount",
                "Mist amount",
                0.0,
                100.0,
                25.0,
                |e: &mut Self, v| e.mist_amount = v,
            ),
            float_slider::<Self>(
                "droplet_amount",
                "Droplet amount",
                0.0,
                100.0,
                35.0,
                |e: &mut Self, v| e.droplet_amount = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let dist_px = clampf(self.distortion, 0.0, 30.0);
        let streak_density = clampf(self.streak_density, 0.0, 100.0) / 100.0;
        let mist = clampf(self.mist_amount, 0.0, 100.0) / 100.0;
        let droplets = clampf(self.droplet_amount, 0.0, 100.0) / 100.0;
        if dist_px <= 0.0 && streak_density <= 0.0 && mist <= 0.0 && droplets <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let cell = 8f32.max(30.0 - (droplets * 22.0));
        let two_pi = std::f32::consts::PI * 2.0;
        let seed = self.seed;
        let blend = |a: SkColor, b: SkColor, t: f32| lerp_color(a, b, t);
        for y in 0..height {
            let row = y * width;
            let y_wave = y as f32 * 0.09;
            for x in 0..width {
                let (xf, yf) = (x as f32, y as f32);
                let phase = hash01(x / 8, 3, seed) * two_pi;
                let wa = ((yf * 0.075) + phase).sin();
                let wb = ((xf * 0.022) + (yf * 0.15) + (phase * 0.5)).sin();
                let ox = dist_px * ((0.38 * wa) + (0.18 * wb));
                let oy = dist_px * 0.08 * ((xf * 0.08) + (phase * 1.3)).sin();
                let (sx, sy) = (xf + ox, yf + oy);
                let mut base = bilinear_sample(&src, width, height, sx, sy);
                if mist > 0.0 {
                    let b0 = bilinear_sample(&src, width, height, sx - 1.25, sy);
                    let b1 = bilinear_sample(&src, width, height, sx + 1.25, sy);
                    let b2 = bilinear_sample(&src, width, height, sx, sy - 1.25);
                    let b3 = bilinear_sample(&src, width, height, sx, sy + 1.25);
                    let avg = |f: fn(SkColor) -> u8| {
                        ((f(b0) as i32 + f(b1) as i32 + f(b2) as i32 + f(b3) as i32) / 4) as u8
                    };
                    let blur =
                        SkColor::rgba(avg(|c| c.r), avg(|c| c.g), avg(|c| c.b), avg(|c| c.a));
                    base = blend(base, blur, mist * 0.45);
                }
                let col_noise = hash01(x / 6, 11, seed);
                let mut streak = 0f32;
                if col_noise > (1.0 - (0.72 * streak_density)) {
                    let signal = (y_wave + (col_noise * 18.0)).sin();
                    streak = 0f32.max(signal).powf(3.0) * streak_density;
                }
                let mut drop = 0f32;
                let cell_x = (xf / cell) as i32;
                let cell_y = (yf / cell) as i32;
                let cn = hash01(cell_x, cell_y, seed ^ 0x2A31);
                if cn > 0.93 - (0.55 * droplets) {
                    let cx = (cell_x as f32 + hash01(cell_x, cell_y, seed ^ 193)) * cell;
                    let cy = (cell_y as f32 + hash01(cell_x, cell_y, seed ^ 431)) * cell;
                    let radius = cell * (0.12 + (0.24 * hash01(cell_x, cell_y, seed ^ 997)));
                    let dx = xf - cx;
                    let dy = yf - cy;
                    let dist = ((dx * dx) + (dy * dy)).sqrt();
                    drop = 1.0 - smooth_step(0.2, 1.0, dist / radius.max(0.001));
                }
                let lum = ((0.2126 * base.r as f32)
                    + (0.7152 * base.g as f32)
                    + (0.0722 * base.b as f32))
                    / 255.0;
                let fog = SkColor::rgba(186, 196, 206, base.a);
                let mc = blend(base, fog, mist * (0.2 + ((1.0 - lum) * 0.8)));
                let wet = 1.0 - ((0.08 * mist) + (0.06 * streak));
                let hl = (drop * 0.50) + (streak * 0.20);
                let r = (mc.r as f32 * wet) + (hl * 88.0);
                let g = (mc.g as f32 * wet) + (hl * 96.0);
                let b = (mc.b as f32 * wet) + (hl * 116.0);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), mc.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
