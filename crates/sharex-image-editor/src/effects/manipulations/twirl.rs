//! `TwirlImageEffect` (`Core/ImageEffects/Manipulations/TwirlImageEffect.cs`).

use crate::effects::prelude::*;

/// `TwirlImageEffect`.
#[derive(Clone, Debug)]
pub struct TwirlImageEffect {
    pub angle: f32,
    pub radius_percentage: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for TwirlImageEffect {
    fn default() -> Self {
        TwirlImageEffect {
            angle: 90.0,
            radius_percentage: 50.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for TwirlImageEffect {
    fn info() -> Info {
        Info::new(
            "twirl",
            "Twirl",
            EffectCategory::Manipulations,
            "image-effect/pinwheel",
        )
        .desc("Applies a twirl distortion to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("angle", "Angle", -360.0, 360.0, 90.0, |e: &mut Self, v| {
                e.angle = v
            }),
            float_slider::<Self>(
                "radius_percentage",
                "Radius %",
                1.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.radius_percentage = v,
            ),
            float_slider::<Self>(
                "center_x_percentage",
                "Center X %",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_x_percentage = v,
            ),
            float_slider::<Self>(
                "center_y_percentage",
                "Center Y %",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_y_percentage = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let radius =
            1f32.max(width.min(height) as f32 * clampf(self.radius_percentage, 1.0, 100.0) / 100.0);
        let cx = clampf(self.center_x_percentage, 0.0, 100.0) / 100.0 * right as f32;
        let cy = clampf(self.center_y_percentage, 0.0, 100.0) / 100.0 * bottom as f32;
        let angle_rad = self.angle * std::f32::consts::PI / 180.0;
        if angle_rad.abs() < 0.0001 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let i = (row + x) as usize;
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist >= radius || dist <= 0.0001 {
                    dst[i] = src[i];
                    continue;
                }
                let normalized = dist / radius;
                let twirl = angle_rad * (1.0 - normalized);
                let theta = dy.atan2(dx) - twirl;
                let sx = clamp_int((cx + dist * theta.cos()).round_ties_even() as i32, 0, right);
                let sy = clamp_int(
                    (cy + dist * theta.sin()).round_ties_even() as i32,
                    0,
                    bottom,
                );
                dst[i] = src[(sy * width + sx) as usize];
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
