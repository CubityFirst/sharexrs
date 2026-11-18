//! `VhsTapeDamageImageEffect` (`Core/ImageEffects/Filters/VhsTapeDamageImageEffect.cs`).

use crate::effects::prelude::*;

/// `VhsTapeDamageImageEffect`.
#[derive(Clone, Debug)]
pub struct VhsTapeDamageImageEffect {
    pub distortion: f32,
    pub noise: f32,
    pub color_bleed: f32,
    pub tracking: f32,
    pub scanlines: f32,
}

impl Default for VhsTapeDamageImageEffect {
    fn default() -> Self {
        VhsTapeDamageImageEffect {
            distortion: 55.0,
            noise: 28.0,
            color_bleed: 30.0,
            tracking: 24.0,
            scanlines: 48.0,
        }
    }
}

impl EffectImpl for VhsTapeDamageImageEffect {
    fn info() -> Info {
        Info::new("vhs_tape_damage", "VHS tape damage", EffectCategory::Filters, "image-effect/videotape")
            .desc("Simulates VHS tape degradation with distortion, noise, and color bleeding.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("distortion", "Distortion", 0.0, 100.0, 55.0, |e: &mut Self, v| e.distortion = v),
            float_slider::<Self>("noise", "Noise", 0.0, 100.0, 28.0, |e: &mut Self, v| e.noise = v),
            float_slider::<Self>("color_bleed", "Color bleed", 0.0, 100.0, 30.0, |e: &mut Self, v| e.color_bleed = v),
            float_slider::<Self>("tracking", "Tracking", 0.0, 100.0, 24.0, |e: &mut Self, v| e.tracking = v),
            float_slider::<Self>("scanlines", "Scanlines", 0.0, 100.0, 48.0, |e: &mut Self, v| e.scanlines = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let distortion = clampf(self.distortion, 0.0, 100.0) / 100.0;
        let noise = clampf(self.noise, 0.0, 100.0) / 100.0;
        let bleed = clampf(self.color_bleed, 0.0, 100.0) / 100.0;
        let tracking = clampf(self.tracking, 0.0, 100.0) / 100.0;
        let scanlines = clampf(self.scanlines, 0.0, 100.0) / 100.0;
        if distortion <= 0.0001 && noise <= 0.0001 && bleed <= 0.0001 && tracking <= 0.0001 && scanlines <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            let yf = y as f32;
            let rn = (hash01(y, 0, 1703) * 2.0) - 1.0;
            let shift = (((yf * 0.085) + 0.7).sin() * 0.55 + ((yf * 0.024) - 1.6).cos() * 0.45 + (rn * 0.65)) * distortion * 10.0;
            let mut dropout = 0f32;
            if hash01(y / 3, 4, 4201) > 0.987 {
                dropout = 0.35 + (tracking * 0.45);
            }
            let tw = 0f32.max(((yf * 0.022) + 1.1).sin()).powf(10.0) * tracking;
            let scan = 1.0 - (scanlines * 0.18 * (0.5 + (0.5 * (yf * std::f32::consts::PI).sin())));
            for x in 0..width {
                let bpx = 0.5 + (bleed * 4.5);
                let sx = x as f32 + shift;
                let rs = bilinear_sample(&src, width, height, sx + bpx, yf);
                let gs = bilinear_sample(&src, width, height, sx, yf);
                let bs = bilinear_sample(&src, width, height, sx - bpx, yf);
                let (mut r, mut g, mut b, a) = (rs.r as f32, gs.g as f32, bs.b as f32, gs.a as f32);
                let ln = ((hash01(x, y, 6113) * 2.0) - 1.0) * noise * 28.0;
                r += ln * 1.05;
                g += ln;
                b += ln * 0.95;
                if tw > 0.0001 {
                    let lift = tw * 42.0;
                    r = lerp(r, 255.0, tw * 0.18);
                    g += lift * 0.22;
                    b += lift * 0.30;
                }
                if dropout > 0.0001 && hash01(x / 4, y, 7001) > 0.35 {
                    r *= 1.0 - dropout;
                    g *= 1.0 - (dropout * 0.95);
                    b *= 1.0 - (dropout * 0.90);
                }
                r *= scan;
                g *= scan;
                b *= scan;
                dst[(row + x) as usize] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), clamp_to_byte(a));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
