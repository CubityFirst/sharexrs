//! `OilSlickInterferenceImageEffect` (`Core/ImageEffects/Filters/OilSlickInterferenceImageEffect.cs`).

use crate::effects::prelude::*;

/// `OilSlickInterferenceImageEffect`.
#[derive(Clone, Debug)]
pub struct OilSlickInterferenceImageEffect {
    pub intensity: f32,
    pub scale: f32,
    pub darkness: f32,
    pub gloss: f32,
    pub shift: f32,
}

impl Default for OilSlickInterferenceImageEffect {
    fn default() -> Self {
        OilSlickInterferenceImageEffect {
            intensity: 58.0,
            scale: 70.0,
            darkness: 45.0,
            gloss: 60.0,
            shift: 0.0,
        }
    }
}

impl EffectImpl for OilSlickInterferenceImageEffect {
    fn info() -> Info {
        Info::new("oil_slick_interference", "Oil slick interference", EffectCategory::Filters, "image-effect/rainbow")
            .desc("Simulates iridescent oil slick interference patterns.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 58.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("scale", "Scale", 0.0, 100.0, 70.0, |e: &mut Self, v| e.scale = v),
            float_slider::<Self>("darkness", "Darkness", 0.0, 100.0, 45.0, |e: &mut Self, v| e.darkness = v),
            float_slider::<Self>("gloss", "Gloss", 0.0, 100.0, 60.0, |e: &mut Self, v| e.gloss = v),
            float_slider::<Self>("shift", "Shift", 0.0, 360.0, 0.0, |e: &mut Self, v| e.shift = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let intensity = clampf(self.intensity, 0.0, 100.0) / 100.0;
        let scale = 24.0 + (clampf(self.scale, 0.0, 100.0) / 100.0 * 180.0);
        let inv_scale = 1.0 / scale;
        let darkness = clampf(self.darkness, 0.0, 100.0) / 100.0;
        let gloss = clampf(self.gloss, 0.0, 100.0) / 100.0;
        let shift = clampf(self.shift, 0.0, 360.0) / 360.0;
        if intensity <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let l01 = |c: SkColor| ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0;
        let p = |x: i32, y: i32| src[(y * width + x) as usize];
        let scr = |s: f32, o: f32| 1.0 - ((1.0 - s) * (1.0 - clamp01(o)));
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let lum = (0.2126 * sr) + (0.7152 * sg) + (0.0722 * sb);
                let edge = clamp01(
                    ((l01(p((width - 1).min(x + 1), y)) - l01(p(0.max(x - 1), y))).abs()
                        + (l01(p(x, (height - 1).min(y + 1))) - l01(p(x, 0.max(y - 1)))).abs())
                        * 1.2,
                );
                let u = x as f32 * inv_scale;
                let v = y as f32 * inv_scale;
                let wa = ((u * 2.7) + (v * 1.6) + 0.7).sin();
                let wb = ((u * 4.4) - (v * 2.8) - 1.1).cos();
                let wc = (((u + v) * 3.6) + 1.9).sin();
                let mut bands = (wa * 0.40) + (wb * 0.34) + (wc * 0.26);
                bands = (bands * 0.5) + 0.5;
                let mut hue = shift + (bands * 0.32) + (u * 0.06) + (v * 0.03);
                hue -= hue.floor();
                let mask = clamp01(intensity * ((1.0 - lum) * 0.45 + (edge * 0.30) + (bands * 0.25)));
                let (ir, ig, ib) = slick_hsv(hue, 0.84, 0.92 + (gloss * 0.08));
                let dr = sr * (1.0 - darkness * 0.58);
                let dg = sg * (1.0 - darkness * 0.60);
                let db = sb * (1.0 - darkness * 0.66);
                let gm = gloss * smooth_step(0.58, 0.96, bands);
                let or = (ir * mask) + (gm * 0.24);
                let og = (ig * mask) + (gm * 0.20);
                let ob = (ib * mask) + (gm * 0.28);
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(scr(dr, or) * 255.0),
                    clamp_to_byte(scr(dg, og) * 255.0),
                    clamp_to_byte(scr(db, ob) * 255.0),
                    s.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn slick_hsv(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
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
