//! `RippleRefractionImageEffect` (`Core/ImageEffects/Manipulations/RippleRefractionImageEffect.cs`).

use crate::effects::prelude::*;

/// `RippleRefractionImageEffect`.
#[derive(Clone, Debug)]
pub struct RippleRefractionImageEffect {
    pub amplitude: f32,
    pub wavelength: f32,
    pub phase: f32,
    pub refraction: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for RippleRefractionImageEffect {
    fn default() -> Self {
        RippleRefractionImageEffect {
            amplitude: 12.0,
            wavelength: 32.0,
            phase: 0.0,
            refraction: 35.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for RippleRefractionImageEffect {
    fn info() -> Info {
        Info::new(
            "ripple_refraction",
            "Ripple refraction",
            EffectCategory::Manipulations,
            "image-effect/waves",
        )
        .desc("Applies a ripple refraction distortion effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "amplitude",
                "Amplitude",
                0.0,
                80.0,
                12.0,
                |e: &mut Self, v| e.amplitude = v,
            ),
            float_slider::<Self>(
                "wavelength",
                "Wavelength",
                4.0,
                400.0,
                32.0,
                |e: &mut Self, v| e.wavelength = v,
            ),
            float_slider::<Self>("phase", "Phase", -360.0, 360.0, 0.0, |e: &mut Self, v| {
                e.phase = v
            }),
            float_slider::<Self>(
                "refraction",
                "Refraction",
                0.0,
                100.0,
                35.0,
                |e: &mut Self, v| e.refraction = v,
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
        let amplitude = clampf(self.amplitude, 0.0, 80.0);
        if amplitude <= 0.01 {
            return Ok(source.clone());
        }
        let wavelength = clampf(self.wavelength, 4.0, 400.0);
        let phase = self.phase * (std::f32::consts::PI / 180.0);
        let refraction01 = clampf(self.refraction, 0.0, 100.0) / 100.0;
        let cx = percentage_to_x(source.width() as i32, self.center_x_percentage);
        let cy = percentage_to_y(source.height() as i32, self.center_y_percentage);
        let wave_scale = (std::f32::consts::PI * 2.0) / wavelength;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let i = (row + x) as usize;
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let distance = ((dx * dx) + (dy * dy)).sqrt();
                if distance <= 0.0001 {
                    dst[i] = src[i];
                    continue;
                }
                let wave_phase = (distance * wave_scale) + phase;
                let radial = wave_phase.sin() * amplitude;
                let refracted = wave_phase.cos() * amplitude * refraction01 * 0.35;
                let sample_distance = distance + radial + refracted;
                let scale = sample_distance / distance;
                dst[i] = bilinear_sample(&src, width, height, cx + (dx * scale), cy + (dy * scale));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
