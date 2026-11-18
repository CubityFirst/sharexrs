//! `RadialRainbowImageEffect` (`Core/ImageEffects/Filters/RadialRainbowImageEffect.cs`).

use crate::effects::prelude::*;

/// `RadialRainbowImageEffect`.
#[derive(Clone, Debug)]
pub struct RadialRainbowImageEffect {
    pub strength: f32,
    pub rotations: i32,
}

impl Default for RadialRainbowImageEffect {
    fn default() -> Self {
        RadialRainbowImageEffect {
            strength: 50.0,
            rotations: 1,
        }
    }
}

impl EffectImpl for RadialRainbowImageEffect {
    fn info() -> Info {
        Info::new(
            "radial_rainbow",
            "Radial rainbow",
            EffectCategory::Filters,
            "image-effect/rainbow",
        )
        .desc("Applies a rainbow hue shift based on angle from center.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.strength = v,
            ),
            int_slider::<Self>("rotations", "Rotations", 1, 5, 1, |e: &mut Self, v| {
                e.rotations = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let s = clampf(self.strength / 100.0, 0.0, 1.0);
        if s <= 0.0 {
            return Ok(source.clone());
        }
        let (w, h) = (source.width() as i32, source.height() as i32);
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let rots = 1.max(self.rotations);
        let pi = std::f32::consts::PI;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..h {
            for x in 0..w {
                let angle = (y as f32 - cy).atan2(x as f32 - cx);
                let shift = ((angle + pi) / (2.0 * pi)) * 360.0 * rots as f32;
                let c = src[(y * w + x) as usize];
                let (hue, sat, lum) = c.to_hsl();
                let mut nh = (hue + shift * s) % 360.0;
                if nh < 0.0 {
                    nh += 360.0;
                }
                dst[(y * w + x) as usize] = SkColor::from_hsl(nh, sat, lum, c.a);
            }
        }
        Ok(from_pixels(w as u32, h as u32, &dst))
    }
}
