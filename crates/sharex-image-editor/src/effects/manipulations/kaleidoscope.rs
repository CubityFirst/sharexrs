//! `KaleidoscopeImageEffect` (`Core/ImageEffects/Manipulations/KaleidoscopeImageEffect.cs`).

use crate::effects::prelude::*;

/// `KaleidoscopeImageEffect`.
#[derive(Clone, Debug)]
pub struct KaleidoscopeImageEffect {
    pub segments: i32,
    pub rotation: f32,
    pub zoom: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for KaleidoscopeImageEffect {
    fn default() -> Self {
        KaleidoscopeImageEffect {
            segments: 8,
            rotation: 0.0,
            zoom: 100.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for KaleidoscopeImageEffect {
    fn info() -> Info {
        Info::new("kaleidoscope", "Kaleidoscope", EffectCategory::Manipulations, "image-effect/pinwheel")
            .desc("Creates a kaleidoscope mirror pattern.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("segments", "Segments", 2, 24, 8, |e: &mut Self, v| e.segments = v),
            float_slider::<Self>("rotation", "Rotation", 0.0, 360.0, 0.0, |e: &mut Self, v| e.rotation = v),
            float_slider::<Self>("zoom", "Zoom", 20.0, 300.0, 100.0, |e: &mut Self, v| e.zoom = v),
            float_slider::<Self>("center_x_percentage", "Center X percentage", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x_percentage = v),
            float_slider::<Self>("center_y_percentage", "Center Y percentage", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y_percentage = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let segment_count = self.segments.clamp(2, 24);
        let zoom = clampf(self.zoom, 20.0, 300.0) / 100.0;
        let rotation = self.rotation * (std::f32::consts::PI / 180.0);
        let cx = percentage_to_x(source.width() as i32, self.center_x_percentage);
        let cy = percentage_to_y(source.height() as i32, self.center_y_percentage);
        let segment_angle = (std::f32::consts::PI * 2.0) / segment_count as f32;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let dx = (x as f32 - cx) / zoom;
                let dy = (y as f32 - cy) / zoom;
                let distance = ((dx * dx) + (dy * dy)).sqrt();
                let angle = wrap_angle(dy.atan2(dx) - rotation);
                let segment_index = (angle / segment_angle).floor() as i32;
                let mut local = angle - (segment_index as f32 * segment_angle);
                if (segment_index & 1) == 1 {
                    local = segment_angle - local;
                }
                let sample_angle = local + rotation;
                let sx = cx + (sample_angle.cos() * distance);
                let sy = cy + (sample_angle.sin() * distance);
                dst[(row + x) as usize] = bilinear_sample(&src, width, height, sx, sy);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
