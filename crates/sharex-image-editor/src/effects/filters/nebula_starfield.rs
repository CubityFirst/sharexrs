//! `NebulaStarfieldImageEffect` (`Core/ImageEffects/Filters/NebulaStarfieldImageEffect.cs`).

use crate::effects::prelude::*;

/// `NebulaStarfieldImageEffect`.
#[derive(Clone, Debug)]
pub struct NebulaStarfieldImageEffect {
    pub intensity: f32,
    pub scale: f32,
    pub hue_shift: f32,
    pub star_density: f32,
    pub star_size: f32,
    pub twinkle: f32,
    pub vignette_strength: f32,
    pub seed: i32,
}

impl Default for NebulaStarfieldImageEffect {
    fn default() -> Self {
        NebulaStarfieldImageEffect {
            intensity: 70.0,
            scale: 80.0,
            hue_shift: -15.0,
            star_density: 55.0,
            star_size: 10.0,
            twinkle: 40.0,
            vignette_strength: 18.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for NebulaStarfieldImageEffect {
    fn info() -> Info {
        Info::new(
            "nebula_starfield",
            "Nebula starfield",
            EffectCategory::Filters,
            "app/sparkles",
        )
        .desc("Generates a procedural nebula and starfield overlay.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "intensity",
                "Intensity",
                0.0,
                100.0,
                70.0,
                |e: &mut Self, v| e.intensity = v,
            ),
            float_slider::<Self>("scale", "Scale", 0.0, 100.0, 80.0, |e: &mut Self, v| {
                e.scale = v
            }),
            float_slider::<Self>(
                "hue_shift",
                "Hue shift",
                -180.0,
                180.0,
                -15.0,
                |e: &mut Self, v| e.hue_shift = v,
            ),
            float_slider::<Self>(
                "star_density",
                "Star density",
                0.0,
                100.0,
                55.0,
                |e: &mut Self, v| e.star_density = v,
            ),
            float_slider::<Self>(
                "star_size",
                "Star size",
                0.0,
                200.0,
                10.0,
                |e: &mut Self, v| e.star_size = v,
            ),
            float_slider::<Self>("twinkle", "Twinkle", 0.0, 100.0, 40.0, |e: &mut Self, v| {
                e.twinkle = v
            }),
            float_slider::<Self>(
                "vignette_strength",
                "Vignette strength",
                0.0,
                100.0,
                18.0,
                |e: &mut Self, v| e.vignette_strength = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let intensity = clampf(self.intensity, 0.0, 100.0) / 100.0;
        if intensity <= 0.0 {
            return Ok(source.clone());
        }
        let scale = clampf(self.scale, 0.0, 100.0) / 100.0;
        let hue_shift = clampf(self.hue_shift, -180.0, 180.0);
        let density = clampf(self.star_density, 0.0, 100.0) / 100.0;
        let size = clampf(self.star_size, 0.0, 200.0) / 200.0;
        let twinkle = clampf(self.twinkle, 0.0, 100.0) / 100.0;
        let vignette = clampf(self.vignette_strength, 0.0, 100.0) / 100.0;
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let seed = self.seed;
        let cx = hash01(seed, 1, seed ^ 31) * (width - 1) as f32;
        let cy = hash01(seed, 2, seed ^ 47) * (height - 1) as f32;
        let max_dist = 1f32.max(
            (((width - 1) as f32 * (width - 1) as f32)
                + ((height - 1) as f32 * (height - 1) as f32))
                .sqrt(),
        );
        let base_hue = (hue_shift + 180.0) * (1.0 / 360.0);
        let (neb_r, neb_g, neb_b) = nebula_hsv(base_hue, 0.85, 1.0);
        let cell = ((10.0 - (size * 6.0)).round_ties_even() as i32).clamp(3, 16);
        let star_threshold = 1.0 - density;
        let radius_base = 0.18 + (0.55 * size);
        let radius_feather = 0.05 + (0.10 * size);
        let freq = 0.25 + (scale * 2.25);
        let phase = (seed % 1000) as f32 * 0.013;
        let pi = std::f32::consts::PI;
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let idx = (row + x) as usize;
                let s = src[idx];
                let (xf, yf) = (x as f32, y as f32);
                let nx = (xf - cx) / width as f32;
                let ny = (yf - cy) / height as f32;
                let dist = ((nx * nx) + (ny * ny)).sqrt();
                let u = xf / 1f32.max((width - 1) as f32);
                let v = yf / 1f32.max((height - 1) as f32);
                let wa = ((u * freq * 8.0) + (v * freq * 5.0) + phase).sin();
                let wb = ((u * freq * 13.0) - (v * freq * 9.0) + (phase * 1.37)).sin();
                let wc = (((u + v) * freq * 7.0) + (phase * 0.77)).sin();
                let mut noise = (wa * 0.45) + (wb * 0.35) + (wc * 0.20);
                noise = (noise * 0.5) + 0.5;
                let falloff = (-(dist * dist) * (2.2 + (scale * 3.2))).exp();
                let neb = intensity * (0.10 + (0.90 * noise)) * falloff;
                let (cell_x, cell_y) = (x / cell, y / cell);
                let spawn = hash01(cell_x, cell_y, seed ^ 0x3A17);
                let mut star = 0f32;
                if spawn > star_threshold {
                    let ox = hash01(cell_x, cell_y, seed ^ 0xBEEF);
                    let oy = hash01(cell_x, cell_y, seed ^ 0xCAFE);
                    let scx = (cell_x * cell) as f32 + (ox * cell as f32);
                    let scy = (cell_y * cell) as f32 + (oy * cell as f32);
                    let dx = xf - scx;
                    let dy = yf - scy;
                    let mut d01 = ((dx * dx) + (dy * dy)).sqrt() / (cell as f32 * 0.5);
                    let ts = hash01(cell_x, cell_y, seed ^ 0x51A7);
                    let tw = 0.5
                        + (0.5
                            * ((ts * pi * 10.0)
                                + phase
                                + (cell_x as f32 * 0.91)
                                + (cell_y as f32 * 1.37))
                                .sin());
                    let tr = lerp(1.0, 0.82 + (tw * 0.70), twinkle);
                    let ti = lerp(1.0, 0.35 + (tw * 1.65), twinkle);
                    d01 /= 0.35f32.max(tr);
                    let r = radius_base;
                    let w = radius_feather * lerp(1.0, 1.35, twinkle);
                    star = 1.0 - smooth_step(r, r + w, d01);
                    if twinkle > 0.001 {
                        let ss = 0.08 + (size * 0.20);
                        let ax = dx.abs() / 0.0001f32.max(cell as f32 * ss);
                        let ay = dy.abs() / 0.0001f32.max(cell as f32 * ss);
                        let sx = 1.0 - smooth_step(0.2, 1.45, ax);
                        let sy = 1.0 - smooth_step(0.2, 1.45, ay);
                        let halo = 1.0 - smooth_step(r * 0.7, r + (1.25 + size), d01);
                        let glint = sx.max(sy) * halo;
                        star = (star * ti) + (glint * twinkle * 0f32.max(ti - 1.0) * 0.45);
                    }
                }
                let star_strength = intensity * star * (0.25 + (0.75 * size));
                let rc = (((xf - cx) * (xf - cx)) + ((yf - cy) * (yf - cy))).sqrt() / max_dist;
                let vign = clamp01(1.0 - (vignette * rc * rc));
                let add = 1f32.min(neb + star_strength);
                let rem = 1.0 - add;
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let or = ((sr * rem) + (neb_r * neb) + (neb_r * star_strength)) * vign;
                let og = ((sg * rem) + (neb_g * neb) + (neb_g * star_strength)) * vign;
                let ob = ((sb * rem) + (neb_b * neb) + (neb_b * star_strength)) * vign;
                dst[idx] = SkColor::rgba(
                    clamp_to_byte(or * 255.0),
                    clamp_to_byte(og * 255.0),
                    clamp_to_byte(ob * 255.0),
                    s.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn nebula_hsv(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = h - h.floor();
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
