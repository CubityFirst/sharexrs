//! `MotionBlurImageEffect` (`Core/ImageEffects/Filters/MotionBlurImageEffect.cs`).

use crate::effects::prelude::*;

/// `MotionBlurImageEffect`.
#[derive(Clone, Debug)]
pub struct MotionBlurImageEffect {
    pub distance: i32,
    pub angle: f32,
}

impl Default for MotionBlurImageEffect {
    fn default() -> Self {
        MotionBlurImageEffect {
            distance: 12,
            angle: 0.0,
        }
    }
}

impl EffectImpl for MotionBlurImageEffect {
    fn info() -> Info {
        Info::new("motion_blur", "Motion blur", EffectCategory::Filters, "app/move-horizontal")
            .desc("Applies a directional motion blur at a specified angle.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("distance", "Distance", 1, 200, 12, |e: &mut Self, v| e.distance = v),
            float_slider::<Self>("angle", "Angle", 0.0, 360.0, 0.0, |e: &mut Self, v| e.angle = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let distance = self.distance.clamp(1, 200);
        if distance <= 1 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let radians = self.angle * (std::f32::consts::PI / 180.0);
        let (dx, dy) = (radians.cos(), radians.sin());
        let start = -((distance - 1) / 2);
        let end = distance / 2;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            for x in 0..width {
                let (mut sr, mut sg, mut sb, mut sa, mut n) = (0i32, 0i32, 0i32, 0i32, 0i32);
                for i in start..=end {
                    let sx = clamp_int((x as f32 + (i as f32 * dx)).round_ties_even() as i32, 0, right);
                    let sy = clamp_int((y as f32 + (i as f32 * dy)).round_ties_even() as i32, 0, bottom);
                    let c = src[(sy * width + sx) as usize];
                    sr += c.r as i32;
                    sg += c.g as i32;
                    sb += c.b as i32;
                    sa += c.a as i32;
                    n += 1;
                }
                dst[(y * width + x) as usize] = SkColor::rgba((sr / n) as u8, (sg / n) as u8, (sb / n) as u8, (sa / n) as u8);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
