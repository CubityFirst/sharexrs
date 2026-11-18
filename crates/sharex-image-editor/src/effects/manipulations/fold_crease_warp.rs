//! `FoldCreaseWarpImageEffect` (`Core/ImageEffects/Manipulations/FoldCreaseWarpImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoldCreaseOrientation {
    Vertical,
    Horizontal,
}

/// `FoldCreaseWarpImageEffect`.
#[derive(Clone, Debug)]
pub struct FoldCreaseWarpImageEffect {
    pub orientation: FoldCreaseOrientation,
    pub fold_count: i32,
    pub fold_depth: f32,
    pub shadow_strength: f32,
}

impl Default for FoldCreaseWarpImageEffect {
    fn default() -> Self {
        FoldCreaseWarpImageEffect {
            orientation: FoldCreaseOrientation::Vertical,
            fold_count: 3,
            fold_depth: 30.0,
            shadow_strength: 40.0,
        }
    }
}

impl EffectImpl for FoldCreaseWarpImageEffect {
    fn info() -> Info {
        Info::new(
            "fold_crease_warp",
            "Fold crease warp",
            EffectCategory::Manipulations,
            "image-effect/newspaper",
        )
        .desc("Simulates a folded paper crease distortion.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, FoldCreaseOrientation>(
                "orientation",
                "Orientation",
                FoldCreaseOrientation::Vertical,
                |e: &mut Self, v| e.orientation = v,
                &[
                    ("Vertical", FoldCreaseOrientation::Vertical),
                    ("Horizontal", FoldCreaseOrientation::Horizontal),
                ],
            ),
            int_slider::<Self>("fold_count", "Fold count", 1, 10, 3, |e: &mut Self, v| {
                e.fold_count = v
            }),
            float_slider::<Self>(
                "fold_depth",
                "Fold depth",
                0.0,
                100.0,
                30.0,
                |e: &mut Self, v| e.fold_depth = v,
            ),
            float_slider::<Self>(
                "shadow_strength",
                "Shadow strength",
                0.0,
                100.0,
                40.0,
                |e: &mut Self, v| e.shadow_strength = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let fold_count = self.fold_count.clamp(1, 10);
        let depth01 = clampf(self.fold_depth, 0.0, 100.0) / 100.0;
        if depth01 <= 0.0 {
            return Ok(source.clone());
        }
        let shadow01 = clampf(self.shadow_strength, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let vertical = self.orientation == FoldCreaseOrientation::Vertical;
        let span = (if vertical {
            1f32.max((width - 1) as f32)
        } else {
            1f32.max((height - 1) as f32)
        }) / fold_count as f32;
        let displacement = span * (0.06 + (depth01 * 0.22));
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let primary = if vertical { x as f32 } else { y as f32 };
                let phase = (primary / 1f32.max(span)) * std::f32::consts::PI;
                let offset = phase.sin() * displacement;
                let sx = if vertical {
                    x as f32 + offset
                } else {
                    x as f32
                };
                let sy = if vertical {
                    y as f32
                } else {
                    y as f32 + offset
                };
                let sampled = bilinear_sample(&src, width, height, sx, sy);
                let curvature = phase.sin().abs();
                let slope = phase.cos();
                let shade = 1.0 - (shadow01 * curvature * 0.22) + (shadow01 * slope * 0.08);
                dst[(row + x) as usize] = multiply_rgb(sampled, shade);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
