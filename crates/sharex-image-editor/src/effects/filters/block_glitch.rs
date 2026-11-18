//! `BlockGlitchImageEffect` (`Core/ImageEffects/Filters/BlockGlitchImageEffect.cs`).

use crate::effects::prelude::*;

/// `BlockGlitchImageEffect`.
#[derive(Clone, Debug)]
pub struct BlockGlitchImageEffect {
    pub block_count: i32,
    pub min_block_width: i32,
    pub max_block_width: i32,
    pub min_block_height: i32,
    pub max_block_height: i32,
    pub max_displacement: i32,
    pub channel_shift: i32,
    pub noise_amount: f32,
    pub seed: i32,
}

impl Default for BlockGlitchImageEffect {
    fn default() -> Self {
        BlockGlitchImageEffect {
            block_count: 36,
            min_block_width: 24,
            max_block_width: 200,
            min_block_height: 6,
            max_block_height: 50,
            max_displacement: 50,
            channel_shift: 4,
            noise_amount: 10.0,
            seed: 1945,
        }
    }
}

impl EffectImpl for BlockGlitchImageEffect {
    fn info() -> Info {
        Info::new("block_glitch", "Block glitch / Databending", EffectCategory::Filters, "image-effect/cpu")
            .desc("Simulates digital glitch artifacts with displaced color blocks.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("block_count", "Block count", 1, 240, 36, |e: &mut Self, v| e.block_count = v),
            int_slider::<Self>("min_block_width", "Min block width", 4, 400, 24, |e: &mut Self, v| e.min_block_width = v),
            int_slider::<Self>("max_block_width", "Max block width", 4, 900, 200, |e: &mut Self, v| e.max_block_width = v),
            int_slider::<Self>("min_block_height", "Min block height", 2, 200, 6, |e: &mut Self, v| e.min_block_height = v),
            int_slider::<Self>("max_block_height", "Max block height", 2, 500, 50, |e: &mut Self, v| e.max_block_height = v),
            int_slider::<Self>("max_displacement", "Max displacement", 0, 500, 50, |e: &mut Self, v| e.max_displacement = v),
            int_slider::<Self>("channel_shift", "Channel shift", 0, 64, 4, |e: &mut Self, v| e.channel_shift = v),
            float_slider::<Self>("noise_amount", "Noise amount", 0.0, 100.0, 10.0, |e: &mut Self, v| e.noise_amount = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        let block_count = self.block_count.clamp(1, 240);
        let min_w = self.min_block_width.clamp(4, 400);
        let max_w = self.max_block_width.clamp(min_w, 900);
        let min_h = self.min_block_height.clamp(2, 200);
        let max_h = self.max_block_height.clamp(min_h, 500);
        let max_shift = self.max_displacement.clamp(0, 500);
        let channel_shift = self.channel_shift.clamp(0, 64);
        let noise = clampf(self.noise_amount, 0.0, 100.0) / 100.0;
        let src = pixels(source);
        let mut dst = src.clone();
        let mut rnd = random::Compat::new(self.seed);
        for _ in 0..block_count {
            let bw = rnd.next_range(min_w, max_w + 1);
            let bh = rnd.next_range(min_h, max_h + 1);
            let x = rnd.next_range(0, 1.max(width - 1));
            let y = rnd.next_range(0, 1.max(height - 1));
            let right = width.min(x + bw);
            let bottom = height.min(y + bh);
            if right <= x || bottom <= y {
                continue;
            }
            let dx = rnd.next_range(-max_shift, max_shift + 1);
            let dy = rnd.next_range(-(1.max(max_shift / 6)), 2.max((max_shift / 6) + 1));
            let red_shift = rnd.next_range(-channel_shift, channel_shift + 1);
            let blue_shift = rnd.next_range(-channel_shift, channel_shift + 1);
            for py in y..bottom {
                for px in x..right {
                    let sx = clamp_int(px + dx, 0, width - 1);
                    let sy = clamp_int(py + dy, 0, height - 1);
                    let sr_x = clamp_int(sx + red_shift, 0, width - 1);
                    let sb_x = clamp_int(sx - blue_shift, 0, width - 1);
                    let base = src[(sy * width + sx) as usize];
                    let rc = src[(sy * width + sr_x) as usize];
                    let bc = src[(sy * width + sb_x) as usize];
                    let mut n = || {
                        if noise > 0.0 {
                            ((rnd.next_double() as f32) * 2.0 - 1.0) * noise * 48.0
                        } else {
                            0.0
                        }
                    };
                    let nr = n();
                    let ng = n();
                    let nb = n();
                    dst[(py * width + px) as usize] = SkColor::rgba(
                        clamp_to_byte(rc.r as f32 + nr),
                        clamp_to_byte(base.g as f32 + ng),
                        clamp_to_byte(bc.b as f32 + nb),
                        base.a,
                    );
                }
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
