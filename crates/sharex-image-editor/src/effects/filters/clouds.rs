//! `CloudsImageEffect` (`Core/ImageEffects/Filters/CloudsImageEffect.cs`).

use crate::effects::prelude::*;

/// `CloudsImageEffect`.
#[derive(Clone, Debug)]
pub struct CloudsImageEffect {
    pub coverage: f32,
    pub scale: f32,
    pub height_bias: f32,
    pub softness: f32,
    pub sunlight: f32,
}

impl Default for CloudsImageEffect {
    fn default() -> Self {
        CloudsImageEffect {
            coverage: 55.0,
            scale: 72.0,
            height_bias: 62.0,
            softness: 64.0,
            sunlight: 38.0,
        }
    }
}

impl EffectImpl for CloudsImageEffect {
    fn info() -> Info {
        Info::new("clouds", "Clouds", EffectCategory::Filters, "app/cloud")
            .desc("Overlays procedurally generated clouds onto the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "coverage",
                "Coverage",
                0.0,
                100.0,
                55.0,
                |e: &mut Self, v| e.coverage = v,
            ),
            float_slider::<Self>("scale", "Scale", 0.0, 100.0, 72.0, |e: &mut Self, v| {
                e.scale = v
            }),
            float_slider::<Self>(
                "heightBias",
                "Height bias",
                0.0,
                100.0,
                62.0,
                |e: &mut Self, v| e.height_bias = v,
            ),
            float_slider::<Self>(
                "softness",
                "Softness",
                0.0,
                100.0,
                64.0,
                |e: &mut Self, v| e.softness = v,
            ),
            float_slider::<Self>(
                "sunlight",
                "Sunlight",
                0.0,
                100.0,
                38.0,
                |e: &mut Self, v| e.sunlight = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let coverage = clampf(self.coverage, 0.0, 100.0) / 100.0;
        let scale = clampf(self.scale, 0.0, 100.0) / 100.0;
        let height_bias = clampf(self.height_bias, 0.0, 100.0) / 100.0;
        let softness = clampf(self.softness, 0.0, 100.0) / 100.0;
        let sunlight = clampf(self.sunlight, 0.0, 100.0) / 100.0;
        if coverage <= 0.0001 || source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let cell = 36.0 + (scale * 140.0);
        let detail_cell = 18f32.max(cell * 0.52);
        let warp_freq = 0.0045 + ((1.0 - scale) * 0.012);
        let warp_amount = 10.0 + (scale * 40.0);
        let erosion = 0.010 + ((1.0 - scale) * 0.020);
        let spawn = 0.82 - (coverage * 0.45);
        let inv_w = if width > 1 {
            1.0 / (width as f32 - 1.0)
        } else {
            0.0
        };
        let inv_h = if height > 1 {
            1.0 / (height as f32 - 1.0)
        } else {
            0.0
        };
        for y in 0..height {
            let row = y * width;
            let v = y as f32 * inv_h;
            let band_start = 0.20 + (height_bias * 0.55);
            let vmask = 1.0 - smooth_step(band_start, 1.0, v);
            let yf = y as f32;
            for x in 0..width {
                let xf = x as f32;
                let s = src[(row + x) as usize];
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let u = xf * inv_w;
                let warp_x = ((fractal_noise(
                    (xf * warp_freq) + 4.7,
                    (yf * warp_freq * 0.92) - 2.1,
                    3,
                    2.02,
                    0.56,
                    1411,
                ) * 2.0)
                    - 1.0)
                    * warp_amount;
                let warp_y = ((fractal_noise(
                    (xf * warp_freq * 0.88) - 6.4,
                    (yf * warp_freq) + 3.6,
                    3,
                    2.10,
                    0.54,
                    2311,
                ) * 2.0)
                    - 1.0)
                    * (warp_amount * 0.65);
                let sx = xf + warp_x;
                let sy = yf + warp_y;
                let (mf, ml, ms) = cloud_layer(sx, sy, cell, spawn, 3571);
                let (df, dl, ds) = cloud_layer(
                    sx + (cell * 0.24),
                    sy - (cell * 0.15),
                    detail_cell,
                    spawn + 0.08,
                    5171,
                );
                let field = 1.0 - ((1.0 - mf) * (1.0 - (df * 0.72)));
                let breakup = billow(
                    (sx * erosion) - 3.2,
                    (sy * erosion) + 5.9,
                    3,
                    2.00,
                    0.56,
                    6211,
                );
                let curls = fractal_noise(
                    (sx * erosion * 2.2) + 11.3,
                    (sy * erosion * 1.9) - 7.6,
                    2,
                    2.12,
                    0.58,
                    7351,
                );
                let bank = 0.5 + (0.5 * ((u * 5.6) + (v * 1.7) + 0.8).sin());
                let mut structure = field * (0.72 + (0.22 * breakup) + (0.10 * curls));
                structure = clamp01(structure + (bank * 0.07));
                let threshold = 0.30 - (coverage * 0.18) - (vmask * 0.08);
                let mut mask = smooth_step(threshold, 0.84, structure * (0.70 + (0.30 * vmask)));
                mask = mask.powf(1.45 - (softness * 0.95));
                let cl = ml.max(dl * 0.88);
                let cs = ms.max(ds * 0.92);
                let sun_bias = clamp01(0.68 + (((0.78 - u) * 0.42) + ((0.40 - v) * 0.74)));
                let highlight = sunlight * mask * (0.18 + (0.34 * cl) + (0.18 * sun_bias));
                let shade = mask * ((0.08 + (0.22 * cs)) * (0.90 - (sunlight * 0.20)));
                let body = 0.79 + (0.12 * structure) + (0.05 * breakup);
                let cr = clamp01(body - shade + (highlight * 1.06));
                let cg = clamp01((body + 0.03) - (shade * 0.94) + highlight);
                let cb = clamp01((body + 0.08) - (shade * 0.84) + (highlight * 0.82));
                let haze = coverage * vmask * (0.006 + (0.016 * structure));
                let hr = lerp(sr, 0.84, haze);
                let hg = lerp(sg, 0.89, haze);
                let hb = lerp(sb, 0.95, haze);
                let blend = mask * (0.52 + (coverage * 0.34));
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(lerp(hr, cr, blend) * 255.0),
                    clamp_to_byte(lerp(hg, cg, blend) * 255.0),
                    clamp_to_byte(lerp(hb, cb, blend) * 255.0),
                    s.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn cloud_layer(x: f32, y: f32, cell: f32, spawn_threshold: f32, seed: i32) -> (f32, f32, f32) {
    let (mut field, mut light, mut shadow) = (0f32, 0f32, 0f32);
    let cell_x = (x / cell).floor() as i32;
    let cell_y = (y / cell).floor() as i32;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (nx, ny) = (cell_x + ox, cell_y + oy);
            if hash01(nx, ny, seed) < spawn_threshold {
                continue;
            }
            let cx = (nx as f32 + 0.18 + (hash01(nx, ny, seed ^ 0x0713) * 0.64)) * cell;
            let cy = (ny as f32 + 0.22 + (hash01(nx, ny, seed ^ 0x1731) * 0.52)) * cell;
            let radius = cell * (0.28 + (hash01(nx, ny, seed ^ 0x2A51) * 0.20));
            puff(x, y, cx, cy, radius, &mut field, &mut light, &mut shadow);
            let chance = hash01(nx, ny, seed ^ 0x3C17);
            if chance > 0.32 {
                let angle = hash01(nx, ny, seed ^ 0x49D3) * (std::f32::consts::PI * 2.0);
                let offset = radius * (0.35 + (hash01(nx, ny, seed ^ 0x5123) * 0.40));
                let sr = radius * (0.42 + (hash01(nx, ny, seed ^ 0x6891) * 0.28));
                let sx = cx + (angle.cos() * offset);
                let sy = cy + (angle.sin() * offset * 0.72);
                puff(x, y, sx, sy, sr, &mut field, &mut light, &mut shadow);
            }
        }
    }
    (field, light, shadow)
}

#[allow(clippy::too_many_arguments)]
fn puff(
    x: f32,
    y: f32,
    cx: f32,
    cy: f32,
    radius: f32,
    field: &mut f32,
    light: &mut f32,
    shadow: &mut f32,
) {
    let nx = (x - cx) / (radius * 1.18);
    let dy = y - cy;
    let ny = if dy < 0.0 {
        dy / (radius * 0.96)
    } else {
        dy / (radius * 0.70)
    };
    let dist = ((nx * nx) + (ny * ny)).sqrt();
    if dist >= 1.0 {
        return;
    }
    let p = 1.0 - smooth_step(0.10, 1.0, dist);
    let local = clamp01(0.72 - (nx * 0.34) - (ny * 0.58));
    let underside = clamp01((ny * 0.90) + 0.12);
    *field = 1.0 - ((1.0 - *field) * (1.0 - p));
    *light = light.max(p * local);
    *shadow = shadow.max(p * (1.0 - local) * underside);
}

fn billow(x: f32, y: f32, octaves: i32, lacunarity: f32, gain: f32, seed: i32) -> f32 {
    let n = fractal_noise(x, y, octaves, lacunarity, gain, seed);
    1.0 - ((n * 2.0) - 1.0).abs()
}
