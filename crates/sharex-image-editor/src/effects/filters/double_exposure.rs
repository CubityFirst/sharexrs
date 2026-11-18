//! `DoubleExposureImageEffect` (`Core/ImageEffects/Filters/DoubleExposureImageEffect.cs`).

use crate::effects::prelude::*;

/// `DoubleExposureImageEffect`.
#[derive(Clone, Debug)]
pub struct DoubleExposureImageEffect {
    pub blend_amount: f32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub ghost_blur: f32,
    pub highlight_bias: f32,
    pub contrast: f32,
}

impl Default for DoubleExposureImageEffect {
    fn default() -> Self {
        DoubleExposureImageEffect {
            blend_amount: 62.0,
            offset_x: 22,
            offset_y: -14,
            ghost_blur: 8.0,
            highlight_bias: 55.0,
            contrast: 112.0,
        }
    }
}

impl EffectImpl for DoubleExposureImageEffect {
    fn info() -> Info {
        Info::new("double_exposure", "Double exposure", EffectCategory::Filters, "app/images")
            .desc("Blends a blurred ghost of the image over itself to simulate double exposure.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("blend_amount", "Blend amount", 0.0, 100.0, 62.0, |e: &mut Self, v| e.blend_amount = v),
            int_numeric::<Self>("offset_x", "Offset X", -500, 500, 22, |e: &mut Self, v| e.offset_x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -500, 500, -14, |e: &mut Self, v| e.offset_y = v),
            float_slider::<Self>("ghost_blur", "Ghost blur", 0.0, 30.0, 8.0, |e: &mut Self, v| e.ghost_blur = v),
            float_slider::<Self>("highlight_bias", "Highlight bias", 0.0, 100.0, 55.0, |e: &mut Self, v| e.highlight_bias = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 112.0, |e: &mut Self, v| e.contrast = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let blend = clampf(self.blend_amount, 0.0, 100.0) / 100.0;
        let blur = clampf(self.ghost_blur, 0.0, 30.0);
        let hb = clampf(self.highlight_bias, 0.0, 100.0) / 100.0;
        let contrast = clampf(self.contrast, 50.0, 200.0) / 100.0;
        if blend <= 0.0001 && blur <= 0.0001 && self.offset_x == 0 && self.offset_y == 0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let ghost_px = (blur > 0.01).then(|| pixels(&create_blurred_clamp(source, blur)));
        let gp = ghost_px.as_ref().unwrap_or(&src);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let ghost = bilinear_sample(gp, width, height, (x - self.offset_x) as f32, (y - self.offset_y) as f32);
                let (sr, sg, sb) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let gl = analog_luminance01(ghost);
                let mut gr = lerp(ghost.r as f32 / 255.0, gl * (1.12 + (hb * 0.48)), 0.68);
                let mut gg = lerp(ghost.g as f32 / 255.0, gl * 0.95, 0.78);
                let mut gb = lerp(ghost.b as f32 / 255.0, gl * 0.82, 0.84);
                gr = apply_contrast(gr, contrast);
                gg = apply_contrast(gg, contrast);
                gb = apply_contrast(gb, contrast);
                let lb = blend * (0.55 + (gl * 0.45));
                let r = lerp(sr, screen(sr, gr), lb);
                let g = lerp(sg, screen(sg, gg), lb * 0.92);
                let b = lerp(sb, screen(sb, gb), lb * 0.88);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
