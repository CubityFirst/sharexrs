//! `InkSplatterDripsImageEffect` (`Core/ImageEffects/Filters/InkSplatterDripsImageEffect.cs`).

use crate::effects::prelude::*;

/// `InkSplatterDripsImageEffect`.
#[derive(Clone, Debug)]
pub struct InkSplatterDripsImageEffect {
    pub ink_amount: f32,
    pub drip_length: f32,
    pub spread: f32,
    pub paper_fade: f32,
}

impl Default for InkSplatterDripsImageEffect {
    fn default() -> Self {
        InkSplatterDripsImageEffect {
            ink_amount: 55.0,
            drip_length: 45.0,
            spread: 35.0,
            paper_fade: 20.0,
        }
    }
}

impl EffectImpl for InkSplatterDripsImageEffect {
    fn info() -> Info {
        Info::new("ink_splatter_drips", "Ink splatter + drips", EffectCategory::Filters, "image-effect/paint-brush")
            .desc("Adds ink splatter blobs and vertical drip marks.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("ink_amount", "Ink amount", 0.0, 100.0, 55.0, |e: &mut Self, v| e.ink_amount = v),
            float_slider::<Self>("drip_length", "Drip length", 0.0, 100.0, 45.0, |e: &mut Self, v| e.drip_length = v),
            float_slider::<Self>("spread", "Spread", 0.0, 100.0, 35.0, |e: &mut Self, v| e.spread = v),
            float_slider::<Self>("paper_fade", "Paper fade", 0.0, 100.0, 20.0, |e: &mut Self, v| e.paper_fade = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let ink_amount = clampf(self.ink_amount, 0.0, 100.0) / 100.0;
        let drip_length = clampf(self.drip_length, 0.0, 100.0) / 100.0;
        let spread = clampf(self.spread, 0.0, 100.0) / 100.0;
        let paper_fade = clampf(self.paper_fade, 0.0, 100.0) / 100.0;
        if ink_amount <= 0.0001 && paper_fade <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let seed = random::shared(|r| r.next_range(1, i32::MAX));
        let splat_cell = 14.0 + ((1.0 - spread) * 16.0);
        let splat_radius = 3.6 + (spread * 11.5);
        let splat_density = 0.04 + (ink_amount * 0.25);
        let drip_cell = 12.0 + ((1.0 - spread) * 24.0);
        let drip_probability = 0.12 + (ink_amount * 0.42);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (mut r, mut g, mut b, a) = (s.r as f32, s.g as f32, s.b as f32, s.a as f32);
                if paper_fade > 0.0001 {
                    let gray = (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
                    let pn = ((hash01(x, y, seed ^ 131) * 2.0) - 1.0) * paper_fade * 8.0;
                    let target = gray + (9.0 * paper_fade) + pn;
                    let mix = paper_fade * 0.48;
                    r = lerp(r, target, mix);
                    g = lerp(g, target, mix);
                    b = lerp(b, target + 4.0, mix);
                }
                let splatter = splat_mask(x, y, splat_cell, splat_radius, splat_density, seed ^ 991);
                let drips = drip_mask(x, y, height, drip_cell, drip_probability, drip_length, spread, seed ^ 3701);
                let speckle = hash01(x, y, seed ^ 5239).powf(26.0) * ink_amount * 0.85;
                let mask = clampf((splatter * 0.9) + (drips * 0.85) + speckle, 0.0, 1.0) * ink_amount;
                if mask > 0.0001 {
                    let bleed = 0f32.max(splatter - 0.25) * 0.3;
                    r = lerp(r, 11.0 + (bleed * 10.0), mask);
                    g = lerp(g, 11.0 + (bleed * 8.0), mask);
                    b = lerp(b, 15.0 + (bleed * 12.0), mask);
                }
                dst[(row + x) as usize] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), clamp_to_byte(a));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn splat_mask(x: i32, y: i32, cell: f32, radius: f32, density: f32, seed: i32) -> f32 {
    let cell_x = (x as f32 / cell).floor() as i32;
    let cell_y = (y as f32 / cell).floor() as i32;
    let mut best = 0f32;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (nx, ny) = (cell_x + ox, cell_y + oy);
            if hash01(nx, ny, seed) > density {
                continue;
            }
            let cx = (nx as f32 + hash01(nx, ny, seed ^ 177)) * cell;
            let cy = (ny as f32 + hash01(nx, ny, seed ^ 571)) * cell;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = ((dx * dx) + (dy * dy)).sqrt();
            let local = radius * (0.42 + (hash01(nx, ny, seed ^ 887) * 0.95));
            let core = 1.0 - smooth_step(local * 0.28, local, dist);
            if core > best {
                best = core;
            }
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
fn drip_mask(x: i32, y: i32, height: i32, cell_w: f32, probability: f32, drip_length: f32, spread: f32, seed: i32) -> f32 {
    let column = (x as f32 / cell_w).floor() as i32;
    let mut strongest = 0f32;
    let (xf, yf) = (x as f32, y as f32);
    for c in column - 1..=column + 1 {
        if hash01(c, 0, seed) > probability {
            continue;
        }
        let cx = (c as f32 + hash01(c, 1, seed ^ 211)) * cell_w;
        let start_y = hash01(c, 2, seed ^ 419) * height as f32 * 0.75;
        let len = (0.08 + (drip_length * 0.85)) * (0.4 + (hash01(c, 3, seed ^ 677) * 0.9)) * height as f32;
        let thickness = (0.9 + (spread * 3.2)) * (0.65 + (hash01(c, 4, seed ^ 991) * 0.95));
        let dy = yf - start_y;
        if dy < 0.0 || dy > len {
            continue;
        }
        let dx = (xf - cx).abs();
        let line = 1.0 - smooth_step(thickness * 0.45, thickness, dx);
        if line <= 0.0 {
            continue;
        }
        let tail = 1.0 - smooth_step(0.0, len, dy);
        let mut drip = line * (0.55 + (0.45 * tail));
        let tdx = xf - cx;
        let tdy = yf - (start_y + len);
        let td = ((tdx * tdx) + ((tdy * tdy) * 0.55)).sqrt();
        let tip = 1.0 - smooth_step(thickness * 0.2, thickness * 1.6, td);
        drip = drip.max(tip * 0.9);
        if drip > strongest {
            strongest = drip;
        }
    }
    strongest
}
