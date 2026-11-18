//! `TiltShiftImageEffect` (`Core/ImageEffects/Filters/TiltShiftImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TiltShiftMode {
    Linear,
    Radial,
}

/// `TiltShiftImageEffect`.
#[derive(Clone, Debug)]
pub struct TiltShiftImageEffect {
    pub mode: TiltShiftMode,
    pub blur_radius: f32,
    pub focus_size: f32,
    pub focus_position_x: f32,
    pub focus_position_y: f32,
    pub falloff: f32,
    pub saturation_boost: f32,
}

impl Default for TiltShiftImageEffect {
    fn default() -> Self {
        TiltShiftImageEffect {
            mode: TiltShiftMode::Linear,
            blur_radius: 12.0,
            focus_size: 30.0,
            focus_position_x: 50.0,
            focus_position_y: 50.0,
            falloff: 24.0,
            saturation_boost: 35.0,
        }
    }
}

impl EffectImpl for TiltShiftImageEffect {
    fn info() -> Info {
        Info::new("tilt_shift", "Tilt-shift", EffectCategory::Filters, "image-editor/blur")
            .desc("Simulates a tilt-shift lens with selective focus.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, TiltShiftMode>("mode", "Mode", TiltShiftMode::Linear, |e: &mut Self, v| e.mode = v, &[("Linear", TiltShiftMode::Linear), ("Radial", TiltShiftMode::Radial)]),
            float_slider::<Self>("blur_radius", "Blur radius", 0.0, 30.0, 12.0, |e: &mut Self, v| e.blur_radius = v),
            float_slider::<Self>("focus_size", "Focus size", 5.0, 90.0, 30.0, |e: &mut Self, v| e.focus_size = v),
            float_slider::<Self>("focus_position_x", "Focus position X", 0.0, 100.0, 50.0, |e: &mut Self, v| e.focus_position_x = v),
            float_slider::<Self>("focus_position_y", "Focus position Y", 0.0, 100.0, 50.0, |e: &mut Self, v| e.focus_position_y = v),
            float_slider::<Self>("falloff", "Falloff", 1.0, 60.0, 24.0, |e: &mut Self, v| e.falloff = v),
            float_slider::<Self>("saturation_boost", "Saturation boost", 0.0, 100.0, 35.0, |e: &mut Self, v| e.saturation_boost = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let blur_r = clampf(self.blur_radius, 0.0, 30.0);
        let focus = clampf(self.focus_size, 5.0, 90.0) / 100.0;
        let fx = clampf(self.focus_position_x, 0.0, 100.0) / 100.0;
        let fy = clampf(self.focus_position_y, 0.0, 100.0) / 100.0;
        let falloff = clampf(self.falloff, 1.0, 60.0) / 100.0;
        let sat = clampf(self.saturation_boost, 0.0, 100.0) / 100.0;
        let blurred = if blur_r > 0.01 { blur(source, blur_r, blur_r) } else { source.clone() };
        let src = pixels(source);
        let bl = pixels(&blurred);
        let mut dst = vec![SkColor::default(); src.len()];
        let cx = fx * (width - 1) as f32;
        let cy = fy * (height - 1) as f32;
        let min_dim = width.min(height) as f32;
        let half_lin = (height as f32 * focus) * 0.5;
        let rad = (min_dim * focus) * 0.5;
        let fall_lin = 1f32.max(height as f32 * falloff);
        let fall_rad = 1f32.max(min_dim * falloff);
        for y in 0..height {
            for x in 0..width {
                let mask = if self.mode == TiltShiftMode::Radial {
                    let dx = x as f32 - cx;
                    let dy = y as f32 - cy;
                    smooth_step(rad, rad + fall_rad, ((dx * dx) + (dy * dy)).sqrt())
                } else {
                    smooth_step(half_lin, half_lin + fall_lin, (y as f32 - cy).abs())
                };
                let i = (y * width + x) as usize;
                let (s, b) = (src[i], bl[i]);
                let mut r = lerp(s.r as f32, b.r as f32, mask);
                let mut g = lerp(s.g as f32, b.g as f32, mask);
                let mut bb = lerp(s.b as f32, b.b as f32, mask);
                let amount = sat * (1.0 - (mask * 0.7));
                if amount > 0.0 {
                    let gray = (r + g + bb) / 3.0;
                    let f = 1.0 + amount;
                    r = gray + ((r - gray) * f);
                    g = gray + ((g - gray) * f);
                    bb = gray + ((bb - gray) * f);
                }
                dst[i] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(bb), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
