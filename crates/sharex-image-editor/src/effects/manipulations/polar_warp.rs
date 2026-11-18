//! `PolarWarpImageEffect` (`Core/ImageEffects/Manipulations/PolarWarpImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolarWarpMode {
    CartesianToPolar,
    PolarToCartesian,
}

/// `PolarWarpImageEffect`.
#[derive(Clone, Debug)]
pub struct PolarWarpImageEffect {
    pub mode: PolarWarpMode,
    pub rotation: f32,
    pub radius_scale: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for PolarWarpImageEffect {
    fn default() -> Self {
        PolarWarpImageEffect {
            mode: PolarWarpMode::CartesianToPolar,
            rotation: 0.0,
            radius_scale: 100.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for PolarWarpImageEffect {
    fn info() -> Info {
        Info::new("polar_warp", "Polar warp", EffectCategory::Manipulations, "image-effect/orbit")
            .desc("Converts between Cartesian and polar coordinate systems.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, PolarWarpMode>("mode", "Mode", PolarWarpMode::CartesianToPolar, |e: &mut Self, v| e.mode = v, &[("Cartesian to polar", PolarWarpMode::CartesianToPolar), ("Polar to Cartesian", PolarWarpMode::PolarToCartesian)]),
            float_slider::<Self>("rotation", "Rotation", -360.0, 360.0, 0.0, |e: &mut Self, v| e.rotation = v),
            float_slider::<Self>("radius_scale", "Radius scale", 20.0, 200.0, 100.0, |e: &mut Self, v| e.radius_scale = v),
            float_slider::<Self>("center_x_percentage", "Center X %", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x_percentage = v),
            float_slider::<Self>("center_y_percentage", "Center Y %", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let radius_scale = clampf(self.radius_scale, 20.0, 200.0) / 100.0;
        let rotation = self.rotation * (std::f32::consts::PI / 180.0);
        let cx = percentage_to_x(width, self.center_x_percentage);
        let cy = percentage_to_y(height, self.center_y_percentage);
        let max_radius = ((width - 1).min(height - 1) as f32 * 0.5) * radius_scale;
        if max_radius <= 0.5 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let width_range = 1f32.max((width - 1) as f32);
        let height_range = 1f32.max((height - 1) as f32);
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let i = (row + x) as usize;
                if self.mode == PolarWarpMode::CartesianToPolar {
                    let polar_angle = ((x as f32 / width_range) * std::f32::consts::PI * 2.0) + rotation;
                    let radius = (y as f32 / height_range) * max_radius;
                    let sx = cx + (polar_angle.cos() * radius);
                    let sy = cy + (polar_angle.sin() * radius);
                    dst[i] = sample_transparent(&src, width, height, sx, sy);
                    continue;
                }
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let distance = ((dx * dx) + (dy * dy)).sqrt();
                if distance > max_radius {
                    dst[i] = color::TRANSPARENT;
                    continue;
                }
                let angle = wrap_angle(dy.atan2(dx) - rotation);
                let sx = (angle / (std::f32::consts::PI * 2.0)) * width_range;
                let sy = (distance / max_radius) * height_range;
                dst[i] = bilinear_sample(&src, width, height, sx, sy);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
