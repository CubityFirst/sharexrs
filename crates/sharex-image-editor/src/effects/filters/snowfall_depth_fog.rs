//! `SnowfallDepthFogImageEffect` (`Core/ImageEffects/Filters/SnowfallDepthFogImageEffect.cs`).

use crate::effects::prelude::*;

/// `SnowfallDepthFogImageEffect`.
#[derive(Clone, Debug)]
pub struct SnowfallDepthFogImageEffect {
    pub snow_amount: f32,
    pub flake_size: f32,
    pub wind: f32,
    pub fog_amount: f32,
    pub fog_height: f32,
}

impl Default for SnowfallDepthFogImageEffect {
    fn default() -> Self {
        SnowfallDepthFogImageEffect {
            snow_amount: 55.0,
            flake_size: 5.0,
            wind: 20.0,
            fog_amount: 40.0,
            fog_height: 65.0,
        }
    }
}

impl EffectImpl for SnowfallDepthFogImageEffect {
    fn info() -> Info {
        Info::new("snowfall_depth_fog", "Snowfall + depth fog", EffectCategory::Filters, "image-effect/cloud-snow")
            .desc("Simulates snowfall with depth-based fog.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("snow_amount", "Snow amount", 0.0, 100.0, 55.0, |e: &mut Self, v| e.snow_amount = v),
            float_slider::<Self>("flake_size", "Flake size", 1.0, 14.0, 5.0, |e: &mut Self, v| e.flake_size = v),
            float_slider::<Self>("wind", "Wind", -100.0, 100.0, 20.0, |e: &mut Self, v| e.wind = v),
            float_slider::<Self>("fog_amount", "Fog amount", 0.0, 100.0, 40.0, |e: &mut Self, v| e.fog_amount = v),
            float_slider::<Self>("fog_height", "Fog height", 10.0, 100.0, 65.0, |e: &mut Self, v| e.fog_height = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let snow_amount = clampf(self.snow_amount, 0.0, 100.0) / 100.0;
        let flake = clampf(self.flake_size, 1.0, 14.0);
        let wind = clampf(self.wind, -100.0, 100.0) / 100.0;
        let fog_amount = clampf(self.fog_amount, 0.0, 100.0) / 100.0;
        let fog_height = clampf(self.fog_height, 10.0, 100.0) / 100.0;
        if snow_amount <= 0.0001 && fog_amount <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let bottom = height - 1;
        let seed = random::shared(|r| r.next_range(1, i32::MAX));
        let inv_h = 1.0 / 1.max(bottom) as f32;
        let c1 = clampf(22.0 - (flake * 0.9), 8.0, 24.0);
        let c2 = 6f32.max(c1 * 0.58);
        let r1 = 0.9 + (flake * 0.22);
        let r2 = 0.55 + (flake * 0.14);
        let d1 = 0.015 + (snow_amount * 0.22);
        let d2 = 0.04 + (snow_amount * 0.30);
        let fog_start = 1.0 - fog_height;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            let ny = y as f32 * inv_h;
            let depth = 1.0 - ny;
            let perspective = 0.55 + (ny * 0.45);
            let wind_off = wind * y as f32 * 0.38;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (mut r, mut g, mut b, a) = (s.r as f32, s.g as f32, s.b as f32, s.a as f32);
                if fog_amount > 0.0001 {
                    let fm = smooth_step(fog_start, 1.0, depth);
                    if fm > 0.0001 {
                        let coarse = hash01((x as f32 * 0.06) as i32, (y as f32 * 0.06) as i32, seed ^ 191);
                        let broad = hash01((x as f32 * 0.018) as i32, (y as f32 * 0.018) as i32, seed ^ 379);
                        let fn_ = (coarse * 0.65) + (broad * 0.35);
                        let fog = fog_amount * fm * (0.75 + (0.25 * fn_)) * (0.62 + (0.38 * depth));
                        r = lerp(r, 226.0, fog);
                        g = lerp(g, 236.0, fog);
                        b = lerp(b, 248.0, fog);
                    }
                }
                if snow_amount > 0.0001 {
                    let l1 = flake_layer(x, y, c1, r1, d1, wind_off, seed ^ 911);
                    let l2 = flake_layer(x, y, c2, r2, d2, wind_off * 1.3, seed ^ 3001);
                    let sparkle = hash01(x, y, seed ^ 4451).powf(20.0) * snow_amount * 0.9;
                    let snow = clampf(((l1 * 0.72) + (l2 * 0.48) + sparkle) * perspective, 0.0, 1.0);
                    if snow > 0.0001 {
                        r = lerp(r, 255.0, snow);
                        g = lerp(g, 255.0, snow);
                        b = lerp(b, 255.0, snow);
                    }
                }
                dst[(row + x) as usize] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), clamp_to_byte(a));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn flake_layer(x: i32, y: i32, cell: f32, radius: f32, density: f32, wind: f32, seed: i32) -> f32 {
    let px = x as f32 + wind;
    let py = y as f32;
    let cell_x = (px / cell).floor() as i32;
    let cell_y = (py / cell).floor() as i32;
    let mut strongest = 0f32;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (nx, ny) = (cell_x + ox, cell_y + oy);
            if hash01(nx, ny, seed) > density {
                continue;
            }
            let cx = (nx as f32 + hash01(nx, ny, seed ^ 1777)) * cell;
            let cy = (ny as f32 + hash01(nx, ny, seed ^ 3907)) * cell;
            let dx = px - cx;
            let dy = py - cy;
            let d2 = (dx * dx) + (dy * dy);
            let fr = radius * (0.6 + (hash01(nx, ny, seed ^ 727) * 0.9));
            if d2 >= fr * fr {
                continue;
            }
            let alpha = 1.0 - smooth_step(fr * 0.3, fr, d2.sqrt());
            if alpha > strongest {
                strongest = alpha;
            }
        }
    }
    strongest
}
