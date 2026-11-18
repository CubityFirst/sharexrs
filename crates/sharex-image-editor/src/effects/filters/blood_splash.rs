//! `BloodSplashImageEffect` (`Core/ImageEffects/Filters/BloodSplashImageEffect.cs`).

use crate::effects::prelude::*;

/// `BloodSplashImageEffect`.
#[derive(Clone, Debug)]
pub struct BloodSplashImageEffect {
    pub splash_amount: f32,
    pub drip_length: f32,
    pub spread: f32,
    pub darkness: f32,
    pub wet_shine: f32,
}

impl Default for BloodSplashImageEffect {
    fn default() -> Self {
        BloodSplashImageEffect {
            splash_amount: 52.0,
            drip_length: 48.0,
            spread: 42.0,
            darkness: 58.0,
            wet_shine: 30.0,
        }
    }
}

impl EffectImpl for BloodSplashImageEffect {
    fn info() -> Info {
        Info::new("blood_splash", "Blood splash", EffectCategory::Filters, "image-effect/droplet")
            .desc("Overlays procedural blood splash and drip effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("splash_amount", "Splash amount", 0.0, 100.0, 52.0, |e: &mut Self, v| e.splash_amount = v),
            float_slider::<Self>("drip_length", "Drip length", 0.0, 100.0, 48.0, |e: &mut Self, v| e.drip_length = v),
            float_slider::<Self>("spread", "Spread", 0.0, 100.0, 42.0, |e: &mut Self, v| e.spread = v),
            float_slider::<Self>("darkness", "Darkness", 0.0, 100.0, 58.0, |e: &mut Self, v| e.darkness = v),
            float_slider::<Self>("wet_shine", "Wet shine", 0.0, 100.0, 30.0, |e: &mut Self, v| e.wet_shine = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let splash_amount = clampf(self.splash_amount, 0.0, 100.0) / 100.0;
        let drip_length = clampf(self.drip_length, 0.0, 100.0) / 100.0;
        let spread = clampf(self.spread, 0.0, 100.0) / 100.0;
        let darkness = clampf(self.darkness, 0.0, 100.0) / 100.0;
        let wet_shine = clampf(self.wet_shine, 0.0, 100.0) / 100.0;
        if splash_amount <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let seed = random::shared(|r| r.next_range(1, i32::MAX));
        let splash_cell = 18.0 + ((1.0 - spread) * 20.0);
        let splash_radius = 5.5 + (spread * 15.5);
        let splash_density = 0.025 + (splash_amount * 0.16);
        let spray_cell = 10.0 + ((1.0 - spread) * 10.0);
        let spray_radius = 0.85 + (spread * 2.5);
        let spray_density = 0.035 + (splash_amount * 0.22);
        let drip_cell = 16.0 + ((1.0 - spread) * 18.0);
        let drip_probability = 0.08 + (splash_amount * 0.26);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (mut r, mut g, mut b, a) = (s.r as f32, s.g as f32, s.b as f32, s.a as f32);
                let splash = splash_mask(x, y, splash_cell, splash_radius, splash_density, seed ^ 991);
                let spray = droplet_field(x, y, spray_cell, spray_radius, spray_density, seed ^ 2081);
                let drips = drip_mask(x, y, height, drip_cell, drip_probability, drip_length, spread, seed ^ 4201);
                let mist = hash01(x, y, seed ^ 6311).powf(24.0) * splash_amount * (0.18 + (spread * 0.45));
                let mask = clampf((splash * 0.98) + (spray * 0.58) + (drips * 0.88) + (mist * 0.32), 0.0, 1.0)
                    * (0.52 + (splash_amount * 0.48));
                if mask > 0.0001 {
                    let clot = 0f32.max(splash - 0.32) * (0.85 + (darkness * 0.35));
                    let pooled = clot.max(drips * 0.28);
                    let dark_mix = clampf((darkness * 0.72) + (pooled * 0.55), 0.0, 1.0);
                    let tr = lerp(198.0, 78.0, dark_mix);
                    let tg = lerp(24.0, 6.0, dark_mix);
                    let tb = lerp(26.0, 10.0, dark_mix);
                    let coverage = clampf(mask + (splash * 0.12), 0.0, 1.0);
                    r = lerp(r, tr, coverage);
                    g = lerp(g, tg, coverage);
                    b = lerp(b, tb, coverage);
                    let absorb = clampf((pooled * 0.24) + (darkness * mask * 0.16), 0.0, 0.42);
                    r *= 1.0 - (absorb * 0.18);
                    g *= 1.0 - (absorb * 0.34);
                    b *= 1.0 - (absorb * 0.38);
                    if wet_shine > 0.0001 {
                        let sn = hash01((x as f32 * 0.28) as i32, (y as f32 * 0.28) as i32, seed ^ 7607);
                        let sm = smooth_step(0.76, 0.985, sn) * smooth_step(0.22, 0.82, splash.max(drips));
                        let shine = sm * wet_shine * (0.25 + (0.75 * splash.max(drips)));
                        if shine > 0.0001 {
                            r = lerp(r, 255.0, shine * 0.24);
                            g = lerp(g, 132.0, shine * 0.20);
                            b = lerp(b, 122.0, shine * 0.14);
                        }
                    }
                }
                dst[(row + x) as usize] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), clamp_to_byte(a));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

const TAU: f32 = std::f32::consts::TAU;

fn splash_mask(x: i32, y: i32, cell: f32, radius: f32, density: f32, seed: i32) -> f32 {
    let cell_x = (x as f32 / cell).floor() as i32;
    let cell_y = (y as f32 / cell).floor() as i32;
    let mut strongest = 0f32;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (nx, ny) = (cell_x + ox, cell_y + oy);
            if hash01(nx, ny, seed) > density {
                continue;
            }
            let cx = (nx as f32 + hash01(nx, ny, seed ^ 157)) * cell;
            let cy = (ny as f32 + hash01(nx, ny, seed ^ 389)) * cell;
            let local = radius * (0.48 + (hash01(nx, ny, seed ^ 701) * 0.92));
            strongest = strongest.max(droplet(x as f32, y as f32, cx, cy, local, seed ^ 911));
            for i in 0..3 {
                let chance = hash01((nx * 17) + i, (ny * 19) - i, seed ^ 1237);
                if chance > 0.82 {
                    continue;
                }
                let angle = hash01((nx * 23) + i, (ny * 29) + i, seed ^ 1601) * TAU;
                let orbit = local * (0.32 + (hash01((nx * 31) + i, (ny * 37) + i, seed ^ 1907) * 1.12));
                let sx = cx + (angle.cos() * orbit);
                let sy = cy + (angle.sin() * orbit * (0.72 + (chance * 0.55)));
                let sr = local * (0.16 + (chance * 0.32));
                strongest = strongest.max(droplet(x as f32, y as f32, sx, sy, sr, seed ^ (2401 + i)));
            }
        }
    }
    strongest
}

fn droplet_field(x: i32, y: i32, cell: f32, radius: f32, density: f32, seed: i32) -> f32 {
    let cell_x = (x as f32 / cell).floor() as i32;
    let cell_y = (y as f32 / cell).floor() as i32;
    let mut strongest = 0f32;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (nx, ny) = (cell_x + ox, cell_y + oy);
            if hash01(nx, ny, seed) > density {
                continue;
            }
            let cx = (nx as f32 + hash01(nx, ny, seed ^ 223)) * cell;
            let cy = (ny as f32 + hash01(nx, ny, seed ^ 521)) * cell;
            let local = radius * (0.65 + (hash01(nx, ny, seed ^ 883) * 0.9));
            strongest = strongest.max(droplet(x as f32, y as f32, cx, cy, local, seed ^ 1291));
        }
    }
    strongest
}

fn droplet(x: f32, y: f32, cx: f32, cy: f32, radius: f32, seed: i32) -> f32 {
    let dx = x - cx;
    let dy = y - cy;
    let dist = ((dx * dx) + (dy * dy)).sqrt();
    let edge = hash01((cx * 0.25) as i32, (cy * 0.25) as i32, seed);
    let irregular = radius * (0.78 + (edge * 0.34));
    1.0 - smooth_step(irregular * 0.22, irregular, dist)
}

#[allow(clippy::too_many_arguments)]
fn drip_mask(x: i32, y: i32, height: i32, cell_w: f32, probability: f32, drip_length: f32, spread: f32, seed: i32) -> f32 {
    let column = (x as f32 / cell_w).floor() as i32;
    let mut strongest = 0f32;
    let (xf, yf) = (x as f32, y as f32);
    for c in column - 2..=column + 2 {
        if hash01(c, 0, seed) > probability {
            continue;
        }
        let base_x = (c as f32 + hash01(c, 1, seed ^ 191)) * cell_w;
        let start_y = hash01(c, 2, seed ^ 487) * height as f32 * 0.58;
        let length = (0.12 + (drip_length * 0.9)) * (0.36 + (hash01(c, 3, seed ^ 761) * 0.88)) * height as f32;
        let thickness = (1.15 + (spread * 3.8)) * (0.68 + (hash01(c, 4, seed ^ 1033) * 0.85));
        let dy = yf - start_y;
        if dy < 0.0 || dy > length {
            continue;
        }
        let sway = ((hash01(c, 5, seed ^ 1327) * 2.0) - 1.0) * spread * 5.5;
        let frequency = 0.012 + (hash01(c, 6, seed ^ 1607) * 0.035);
        let phase = hash01(c, 7, seed ^ 1931) * TAU;
        let lane_x = base_x + ((dy * frequency) + phase).sin() * sway + (dy * sway * 0.018);
        let dx = (xf - lane_x).abs();
        let line = 1.0 - smooth_step(thickness * 0.45, thickness, dx);
        if line <= 0.0 {
            continue;
        }
        let fade = 1.0 - smooth_step(0.0, length, dy);
        let drip = line * (0.48 + (0.52 * fade));
        let tip_y = start_y + length;
        let tdx = xf - lane_x;
        let tdy = yf - tip_y;
        let tip_dist = ((tdx * tdx) + ((tdy * tdy) * 0.52)).sqrt();
        let tip = 1.0 - smooth_step(thickness * 0.24, thickness * 1.75, tip_dist);
        let sdx = xf - base_x;
        let sdy = yf - start_y;
        let sdist = ((sdx * sdx) + ((sdy * sdy) * 0.7)).sqrt();
        let shoulder = 1.0 - smooth_step(thickness * 0.20, thickness * 1.9, sdist);
        strongest = strongest.max(drip.max((tip * 0.92).max(shoulder * 0.38)));
    }
    strongest
}
