//! `CrystalizeShardsImageEffect` (`Core/ImageEffects/Filters/CrystalizeShardsImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrystalizeShardsImageEffect`.
#[derive(Clone, Debug)]
pub struct CrystalizeShardsImageEffect {
    pub shard_size: i32,
    pub jitter: f32,
    pub edge_strength: f32,
    pub shine: f32,
    pub seed: i32,
}

impl Default for CrystalizeShardsImageEffect {
    fn default() -> Self {
        CrystalizeShardsImageEffect {
            shard_size: 22,
            jitter: 65.0,
            edge_strength: 75.0,
            shine: 30.0,
            seed: 4242,
        }
    }
}

impl EffectImpl for CrystalizeShardsImageEffect {
    fn info() -> Info {
        Info::new(
            "crystalize_shards",
            "Crystalize shards",
            EffectCategory::Filters,
            "image-effect/diamond",
        )
        .desc("Breaks the image into crystalline Voronoi shards with edge highlights.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("shardSize", "Shard size", 6, 80, 22, |e: &mut Self, v| {
                e.shard_size = v
            }),
            float_slider::<Self>("jitter", "Jitter", 0.0, 100.0, 65.0, |e: &mut Self, v| {
                e.jitter = v
            }),
            float_slider::<Self>(
                "edgeStrength",
                "Edge strength",
                0.0,
                100.0,
                75.0,
                |e: &mut Self, v| e.edge_strength = v,
            ),
            float_slider::<Self>("shine", "Shine", 0.0, 100.0, 30.0, |e: &mut Self, v| {
                e.shine = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let cell = self.shard_size.clamp(6, 80) as f32;
        let inv_cell = 1.0 / cell;
        let jitter = clampf(self.jitter, 0.0, 100.0) / 100.0;
        let edge_strength = clampf(self.edge_strength, 0.0, 100.0) / 100.0;
        let shine = clampf(self.shine, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let seed = self.seed;
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let (xf, yf) = (x as f32, y as f32);
                let cx = (xf * inv_cell).floor() as i32;
                let cy = (yf * inv_cell).floor() as i32;
                let (mut nd, mut sd) = (f32::MAX, f32::MAX);
                let (mut nsx, mut nsy) = (xf, yf);
                let (mut ncx, mut ncy) = (cx, cy);
                for oy in -1..=1 {
                    for ox in -1..=1 {
                        let (cell_x, cell_y) = (cx + ox, cy + oy);
                        let jx = ((hash01(cell_x, cell_y, seed) * 2.0) - 1.0) * 0.45 * jitter;
                        let jy = ((hash01(cell_x, cell_y, seed ^ 911) * 2.0) - 1.0) * 0.45 * jitter;
                        let sx = (cell_x as f32 + 0.5 + jx) * cell;
                        let sy = (cell_y as f32 + 0.5 + jy) * cell;
                        let dx = sx - xf;
                        let dy = sy - yf;
                        let d = (dx * dx) + (dy * dy);
                        if d < nd {
                            sd = nd;
                            nd = d;
                            nsx = sx;
                            nsy = sy;
                            ncx = cell_x;
                            ncy = cell_y;
                        } else if d < sd {
                            sd = d;
                        }
                    }
                }
                let s = bilinear_sample(&src, width, height, nsx, nsy);
                let (mut r, mut g, mut b, a) = (s.r as f32, s.g as f32, s.b as f32, s.a as f32);
                let nearest = 0f32.max(nd).sqrt();
                let second = 0f32.max(sd).sqrt();
                let mut edge = 1.0 - smooth_step(0.0, cell * 0.12, second - nearest);
                edge *= edge_strength;
                let facet_noise = hash01(ncx, ncy, seed ^ 1777);
                let center = 1.0 - 1f32.min(nearest / 1f32.max(cell));
                let shade = 1.0 + ((facet_noise - 0.5) * 0.25) + (center * shine * 0.45);
                r *= shade;
                g *= shade;
                b *= shade;
                if edge > 0.001 {
                    let ed = edge * 0.78;
                    r = lerp(r, r * 0.22, ed);
                    g = lerp(g, g * 0.26, ed);
                    b = lerp(b, b * 0.34, ed);
                    let glint = edge * (0.20 + (facet_noise * 0.25));
                    r += 38.0 * glint;
                    g += 52.0 * glint;
                    b += 80.0 * glint;
                }
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(r),
                    clamp_to_byte(g),
                    clamp_to_byte(b),
                    clamp_to_byte(a),
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
