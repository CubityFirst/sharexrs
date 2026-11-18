//! `StainedGlassImageEffect` (`Core/ImageEffects/Filters/StainedGlassImageEffect.cs`).

use crate::effects::prelude::*;

/// `StainedGlassImageEffect`.
#[derive(Clone, Debug)]
pub struct StainedGlassImageEffect {
    pub tile_size: i32,
    pub irregularity: f32,
    pub lead_width: f32,
    pub lead_opacity: f32,
    pub color_boost: f32,
    pub seed: i32,
}

impl Default for StainedGlassImageEffect {
    fn default() -> Self {
        StainedGlassImageEffect {
            tile_size: 22,
            irregularity: 55.0,
            lead_width: 1.8,
            lead_opacity: 85.0,
            color_boost: 20.0,
            seed: 1337,
        }
    }
}

impl EffectImpl for StainedGlassImageEffect {
    fn info() -> Info {
        Info::new("stained_glass", "Stained glass", EffectCategory::Filters, "image-effect/hexagon")
            .desc("Creates a stained glass mosaic effect with lead borders.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("tile_size", "Tile size", 6, 120, 22, |e: &mut Self, v| e.tile_size = v),
            float_slider::<Self>("irregularity", "Irregularity", 0.0, 100.0, 55.0, |e: &mut Self, v| e.irregularity = v),
            float_slider::<Self>("lead_width", "Lead width", 0.0, 12.0, 1.7999999523162842, |e: &mut Self, v| e.lead_width = v),
            float_slider::<Self>("lead_opacity", "Lead opacity", 0.0, 100.0, 85.0, |e: &mut Self, v| e.lead_opacity = v),
            float_slider::<Self>("color_boost", "Color boost", 0.0, 100.0, 20.0, |e: &mut Self, v| e.color_boost = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let tile = self.tile_size.clamp(6, 120);
        let irregularity = clampf(self.irregularity, 0.0, 100.0) / 100.0;
        let lead_width = clampf(self.lead_width, 0.0, 12.0);
        let lead_opacity = clampf(self.lead_opacity, 0.0, 100.0) / 100.0;
        let boost = clampf(self.color_boost, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let grid_w = ((width as f32 / tile as f32) as f64).ceil() as i32 + 2;
        let grid_h = ((height as f32 / tile as f32) as f64).ceil() as i32 + 2;
        let jitter = tile as f32 * 0.5 * irregularity;
        let base_seed = (self.seed as u32).wrapping_mul(2246822519);
        let cb = |v: i32| v.clamp(0, 255) as u8;
        let mut grid = Vec::with_capacity((grid_w * grid_h) as usize);
        for gy in 0..grid_h {
            for gx in 0..grid_w {
                let bx = ((gx - 1) as f32 + 0.5) * tile as f32;
                let by = ((gy - 1) as f32 + 0.5) * tile as f32;
                let ox = (glass_hash01(gx, gy, base_seed ^ 0x9E37_79B9) * 2.0 - 1.0) * jitter;
                let oy = (glass_hash01(gx, gy, base_seed ^ 0x85EB_CA6B) * 2.0 - 1.0) * jitter;
                let cx = clampf(bx + ox, 0.0, right as f32);
                let cy = clampf(by + oy, 0.0, bottom as f32);
                let sx = clamp_int(cx.round_ties_even() as i32, 0, right);
                let sy = clamp_int(cy.round_ties_even() as i32, 0, bottom);
                let mut c = src[(sy * width + sx) as usize];
                if boost > 0.0 {
                    let (r, g, b) = (c.r as f32, c.g as f32, c.b as f32);
                    let gray = (r + g + b) / 3.0;
                    let f = 1.0 + boost;
                    let nr = (gray + ((r - gray) * f)).round_ties_even() as i32;
                    let ng = (gray + ((g - gray) * f)).round_ties_even() as i32;
                    let nb = (gray + ((b - gray) * f)).round_ties_even() as i32;
                    c = SkColor::rgba(cb(nr), cb(ng), cb(nb), c.a);
                }
                grid.push((cx, cy, c));
            }
        }
        for y in 0..height {
            let row = y * width;
            let gy = (y / tile) + 1;
            for x in 0..width {
                let gx = (x / tile) + 1;
                let (mut nearest, mut second) = (f32::MAX, f32::MAX);
                let mut color = src[(row + x) as usize];
                for ny in gy - 1..=gy + 1 {
                    for nx in gx - 1..=gx + 1 {
                        let (cx, cy, cc) = grid[(ny * grid_w + nx) as usize];
                        let dx = x as f32 - cx;
                        let dy = y as f32 - cy;
                        let d2 = (dx * dx) + (dy * dy);
                        if d2 < nearest {
                            second = nearest;
                            nearest = d2;
                            color = cc;
                        } else if d2 < second {
                            second = d2;
                        }
                    }
                }
                let mut out = color;
                if lead_width > 0.0 && second < f32::MAX {
                    let ed = (second.sqrt() - nearest.sqrt()) * 0.5;
                    if ed < lead_width {
                        let alpha = clampf(lead_opacity * (1.0 - (ed / lead_width)), 0.0, 1.0);
                        let keep = 1.0 - alpha;
                        out = SkColor::rgba(
                            cb((out.r as f32 * keep).round_ties_even() as i32),
                            cb((out.g as f32 * keep).round_ties_even() as i32),
                            cb((out.b as f32 * keep).round_ties_even() as i32),
                            out.a,
                        );
                    }
                }
                dst[(row + x) as usize] = out;
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn glass_hash01(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = x as u32;
    h = h.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263) as u32);
    h ^= seed;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h & 0xFF_FFFF) as f32 / 16777215.0
}
