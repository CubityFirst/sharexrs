//! `FisheyeLensImageEffect` (`Core/ImageEffects/Manipulations/FisheyeLensImageEffect.cs`).

use crate::effects::prelude::*;

/// `FisheyeLensImageEffect`.
#[derive(Clone, Debug)]
pub struct FisheyeLensImageEffect {
    pub strength: f32,
    pub radius_percentage: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for FisheyeLensImageEffect {
    fn default() -> Self {
        FisheyeLensImageEffect {
            strength: 58.0,
            radius_percentage: 100.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for FisheyeLensImageEffect {
    fn info() -> Info {
        Info::new(
            "fisheye_lens",
            "Fisheye lens",
            EffectCategory::Manipulations,
            "image-effect/gauge",
        )
        .desc("Applies a fisheye lens distortion effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                58.0,
                |e: &mut Self, v| e.strength = v,
            ),
            float_slider::<Self>(
                "radius_percentage",
                "Radius percentage",
                1.0,
                100.0,
                100.0,
                |e: &mut Self, v| e.radius_percentage = v,
            ),
            float_slider::<Self>(
                "center_x_percentage",
                "Center X percentage",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_x_percentage = v,
            ),
            float_slider::<Self>(
                "center_y_percentage",
                "Center Y percentage",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_y_percentage = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let strength01 = clampf(self.strength, 0.0, 100.0) / 100.0;
        if strength01 <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let radius =
            1f32.max(width.min(height) as f32 * clampf(self.radius_percentage, 1.0, 100.0) / 100.0);
        let cx = percentage_to_x(width, self.center_x_percentage);
        let cy = percentage_to_y(height, self.center_y_percentage);
        let exponent = 1.0 + (strength01 * 2.4);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let distance = ((dx * dx) + (dy * dy)).sqrt();
                let i = (row + x) as usize;
                if distance <= 0.0001 || distance >= radius {
                    dst[i] = src[i];
                    continue;
                }
                let normalized = distance / radius;
                let sample_distance = normalized.powf(exponent) * radius;
                let scale = sample_distance / distance;
                dst[i] = bilinear_sample(&src, width, height, cx + (dx * scale), cy + (dy * scale));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
