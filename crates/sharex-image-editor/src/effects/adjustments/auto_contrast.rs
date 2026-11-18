//! `AutoContrastImageEffect` (`Core/ImageEffects/Adjustments/AutoContrastImageEffect.cs`).

use crate::effects::prelude::*;

/// `AutoContrastImageEffect`.
#[derive(Clone, Debug)]
pub struct AutoContrastImageEffect {
    pub clip_percent: f32,
}

impl Default for AutoContrastImageEffect {
    fn default() -> Self {
        AutoContrastImageEffect {
            clip_percent: 0.5,
        }
    }
}

impl EffectImpl for AutoContrastImageEffect {
    fn info() -> Info {
        Info::new("auto_contrast", "Auto contrast", EffectCategory::Adjustments, "app/magic-wand")
            .desc("Automatically adjusts the contrast.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("clip_percent", "Clip percent", 0.0, 20.0, 0.5, |e: &mut Self, v| e.clip_percent = v).tick(0.1).snap(false).format("{}{0:0.0}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let clip_percent = clampf(self.clip_percent, 0.0, 20.0);
        let src = pixels(source);
        let total = src.len();
        if total == 0 {
            return Ok(source.clone());
        }
        let clip_count = (total as i32 as f32 * (clip_percent / 100.0)).round_ties_even() as i32;
        let mut hist = [[0i32; 256]; 3];
        for c in &src {
            hist[0][c.r as usize] += 1;
            hist[1][c.g as usize] += 1;
            hist[2][c.b as usize] += 1;
        }
        let (min_r, max_r) = find_range(&hist[0], clip_count);
        let (min_g, max_g) = find_range(&hist[1], clip_count);
        let (min_b, max_b) = find_range(&hist[2], clip_count);
        if max_r <= min_r && max_g <= min_g && max_b <= min_b {
            return Ok(source.clone());
        }
        let dst: Vec<SkColor> = src
            .iter()
            .map(|c| {
                SkColor::rgba(
                    stretch(c.r, min_r, max_r),
                    stretch(c.g, min_g, max_g),
                    stretch(c.b, min_b, max_b),
                    c.a,
                )
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}

// ---- port ----

fn find_range(histogram: &[i32; 256], clip_count: i32) -> (i32, i32) {
    let mut min = 0;
    let mut sum = 0;
    for (i, v) in histogram.iter().enumerate() {
        sum += v;
        if sum > clip_count {
            min = i as i32;
            break;
        }
    }
    let mut max = 255;
    sum = 0;
    for i in (0..256).rev() {
        sum += histogram[i];
        if sum > clip_count {
            max = i as i32;
            break;
        }
    }
    (min, max)
}

fn stretch(value: u8, min: i32, max: i32) -> u8 {
    if max <= min {
        return value;
    }
    let v = value as i32;
    if v <= min {
        return 0;
    }
    if v >= max {
        return 255;
    }
    let mapped = (v - min) as f32 * 255.0 / (max - min) as f32;
    mapped.round_ties_even() as u8
}
