//! `DatamoshSmearImageEffect` (`Core/ImageEffects/Filters/DatamoshSmearImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatamoshDirection {
    Horizontal,
    Vertical,
}

/// `DatamoshSmearImageEffect`.
#[derive(Clone, Debug)]
pub struct DatamoshSmearImageEffect {
    pub direction: DatamoshDirection,
    pub smear_amount: f32,
    pub corruption: f32,
    pub block_size: i32,
    pub drift: f32,
    pub channel_split: f32,
    pub seed: i32,
}

impl Default for DatamoshSmearImageEffect {
    fn default() -> Self {
        DatamoshSmearImageEffect {
            direction: DatamoshDirection::Horizontal,
            smear_amount: 58.0,
            corruption: 36.0,
            block_size: 12,
            drift: 24.0,
            channel_split: 25.0,
            seed: 9011,
        }
    }
}

impl EffectImpl for DatamoshSmearImageEffect {
    fn info() -> Info {
        Info::new(
            "datamosh_smear",
            "Datamosh smear",
            EffectCategory::Filters,
            "image-effect/cpu",
        )
        .desc("Simulates digital datamoshing with smeared pixel blocks and channel splitting.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, DatamoshDirection>(
                "direction",
                "Direction",
                DatamoshDirection::Horizontal,
                |e: &mut Self, v| e.direction = v,
                &[
                    ("Horizontal", DatamoshDirection::Horizontal),
                    ("Vertical", DatamoshDirection::Vertical),
                ],
            ),
            float_slider::<Self>(
                "smear_amount",
                "Smear amount",
                0.0,
                100.0,
                58.0,
                |e: &mut Self, v| e.smear_amount = v,
            ),
            float_slider::<Self>(
                "corruption",
                "Corruption",
                0.0,
                100.0,
                36.0,
                |e: &mut Self, v| e.corruption = v,
            ),
            int_slider::<Self>("block_size", "Block size", 4, 64, 12, |e: &mut Self, v| {
                e.block_size = v
            }),
            float_slider::<Self>("drift", "Drift", -100.0, 100.0, 24.0, |e: &mut Self, v| {
                e.drift = v
            }),
            float_slider::<Self>(
                "channel_split",
                "Channel split",
                0.0,
                100.0,
                25.0,
                |e: &mut Self, v| e.channel_split = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let smear = clampf(self.smear_amount, 0.0, 100.0) / 100.0;
        let corruption = clampf(self.corruption, 0.0, 100.0) / 100.0;
        let block = self.block_size.clamp(4, 64);
        let drift = clampf(self.drift, -100.0, 100.0);
        let split = clampf(self.channel_split, 0.0, 100.0) / 100.0 * 7.0;
        if smear <= 0.0001 && corruption <= 0.0001 && drift.abs() <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let distance = smear * block as f32 * 5.5;
        let seed = self.seed;
        let bmax = 1.max(block);
        if self.direction == DatamoshDirection::Horizontal {
            for y in 0..height {
                let row = y * width;
                let noise = (hash01(y / bmax, 7, seed) * 2.0) - 1.0;
                let base = (((y as f32 * 0.09) + 0.4).sin() + (noise * 1.15)) * drift * 0.34;
                let mut carry = 0f32;
                let mut bx = 0;
                while bx < width {
                    let bi = bx / block;
                    let trigger = hash01(bi, y / bmax, seed ^ 313);
                    if trigger > 1.0 - (corruption * 0.78) {
                        let sign = (hash01(bi, y, seed ^ 743) * 2.0) - 1.0;
                        carry += sign * distance * (0.35 + trigger);
                    } else {
                        carry *= 0.82;
                    }
                    for x in bx..width.min(bx + block) {
                        let fx = x as f32 - carry - base;
                        let sr = bilinear_sample(&src, width, height, fx + split, y as f32);
                        let sg = bilinear_sample(&src, width, height, fx, y as f32);
                        let sb = bilinear_sample(&src, width, height, fx - split, y as f32);
                        dst[(row + x) as usize] = SkColor::rgba(sr.r, sg.g, sb.b, sg.a);
                    }
                    bx += block;
                }
            }
        } else {
            for x in 0..width {
                let noise = (hash01(x / bmax, 11, seed) * 2.0) - 1.0;
                let base = (((x as f32 * 0.07) + 1.1).cos() + (noise * 1.20)) * drift * 0.34;
                let mut carry = 0f32;
                let mut by = 0;
                while by < height {
                    let bi = by / block;
                    let trigger = hash01(x / bmax, bi, seed ^ 919);
                    if trigger > 1.0 - (corruption * 0.78) {
                        let sign = (hash01(x, bi, seed ^ 1597) * 2.0) - 1.0;
                        carry += sign * distance * (0.35 + trigger);
                    } else {
                        carry *= 0.82;
                    }
                    for y in by..height.min(by + block) {
                        let fy = y as f32 - carry - base;
                        let sr = bilinear_sample(&src, width, height, x as f32, fy + split);
                        let sg = bilinear_sample(&src, width, height, x as f32, fy);
                        let sb = bilinear_sample(&src, width, height, x as f32, fy - split);
                        dst[(y * width + x) as usize] = SkColor::rgba(sr.r, sg.g, sb.b, sg.a);
                    }
                    by += block;
                }
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
