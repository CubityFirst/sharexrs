//! `HolographicFoilShimmerImageEffect` (`Core/ImageEffects/Filters/HolographicFoilShimmerImageEffect.cs`).

use crate::effects::prelude::*;

/// `HolographicFoilShimmerImageEffect`.
#[derive(Clone, Debug)]
pub struct HolographicFoilShimmerImageEffect {
    pub intensity: f32,
    pub scale: f32,
    pub shift: f32,
    pub specular: f32,
    pub grain: f32,
    pub seed: i32,
}

impl Default for HolographicFoilShimmerImageEffect {
    fn default() -> Self {
        HolographicFoilShimmerImageEffect {
            intensity: 65.0,
            scale: 120.0,
            shift: 0.0,
            specular: 45.0,
            grain: 20.0,
            seed: 2026,
        }
    }
}

impl EffectImpl for HolographicFoilShimmerImageEffect {
    fn info() -> Info {
        Info::new(
            "holographic_foil_shimmer",
            "Holographic foil shimmer",
            EffectCategory::Filters,
            "image-effect/rainbow",
        )
        .desc("Overlays a rainbow holographic foil shimmer effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "intensity",
                "Intensity",
                0.0,
                100.0,
                65.0,
                |e: &mut Self, v| e.intensity = v,
            ),
            float_slider::<Self>("scale", "Scale", 40.0, 300.0, 120.0, |e: &mut Self, v| {
                e.scale = v
            }),
            float_slider::<Self>("shift", "Shift", 0.0, 360.0, 0.0, |e: &mut Self, v| {
                e.shift = v
            }),
            float_slider::<Self>(
                "specular",
                "Specular",
                0.0,
                100.0,
                45.0,
                |e: &mut Self, v| e.specular = v,
            ),
            float_slider::<Self>("grain", "Grain", 0.0, 100.0, 20.0, |e: &mut Self, v| {
                e.grain = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let intensity = clampf(self.intensity, 0.0, 100.0) / 100.0;
        let scale = clampf(self.scale, 40.0, 300.0);
        let inv_scale = 1.0 / scale;
        let phase = clampf(self.shift, 0.0, 360.0) * (std::f32::consts::PI / 180.0);
        let specular = clampf(self.specular, 0.0, 100.0) / 100.0;
        let grain = clampf(self.grain, 0.0, 100.0) / 100.0;
        if intensity <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (sr, sg, sb, sa) = (
                    s.r as f32 / 255.0,
                    s.g as f32 / 255.0,
                    s.b as f32 / 255.0,
                    s.a as f32,
                );
                let lum = (0.2126 * sr) + (0.7152 * sg) + (0.0722 * sb);
                let u = x as f32 * inv_scale;
                let v = y as f32 * inv_scale;
                let wa = ((u * 2.7) + (v * 1.9) + phase).sin();
                let wb = ((u * 3.6) - (v * 2.2) - (phase * 0.7)).cos();
                let wc = (((u + v) * 4.1) + (phase * 1.3)).sin();
                let wave = (wa * 0.45) + (wb * 0.35) + (wc * 0.2);
                let mut hue = wave * 0.5 + 0.5 + (u * 0.08) + (v * 0.04);
                hue -= hue.floor();
                let sat = 0.65 + (0.25 * lum);
                let val = 0.80 + (0.20 * lum);
                let (hr, hg, hb) = foil_hsv(hue, sat, val);
                let scan = 0.5 + (0.5 * (((u + (v * 0.35)) * 35.0) + (phase * 2.0)).sin());
                let shimmer = intensity * ((scan * 0.55) + 0.45);
                let mut band = ((u * 8.0) - (v * 5.0) + (phase * 1.5)).sin();
                band = 0f32.max(band).powf(8.0) * specular;
                let mut or = (hr * shimmer) + band;
                let mut og = (hg * shimmer) + (band * 0.92);
                let mut ob = (hb * shimmer) + (band * 1.08);
                if grain > 0.001 {
                    let sparkle = hash01(x, y, self.seed).powf(22.0) * grain * 1.8;
                    or += sparkle;
                    og += sparkle * 0.95;
                    ob += sparkle * 1.1;
                }
                let blend = intensity * (0.42 + (0.58 * lum));
                let scr = |s: f32, o: f32| 1.0 - ((1.0 - s) * (1.0 - 1f32.min(o)));
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(lerp(sr, scr(sr, or), blend) * 255.0),
                    clamp_to_byte(lerp(sg, scr(sg, og), blend) * 255.0),
                    clamp_to_byte(lerp(sb, scr(sb, ob), blend) * 255.0),
                    clamp_to_byte(sa),
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn foil_hsv(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s <= 0.0001 {
        return (v, v, v);
    }
    let scaled = h * 6.0;
    let sector = scaled.floor() as i32;
    let frac = scaled - sector as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - (s * frac));
    let t = v * (1.0 - (s * (1.0 - frac)));
    match sector {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}
