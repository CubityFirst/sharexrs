//! `Anaglyph3DImageEffect` (`Core/ImageEffects/Filters/Anaglyph3DImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnaglyphMode {
    RedCyan,
    AmberBlue,
    GreenMagenta,
}

/// `Anaglyph3DImageEffect`.
#[derive(Clone, Debug)]
pub struct Anaglyph3DImageEffect {
    pub mode: AnaglyphMode,
    pub separation_x: i32,
    pub separation_y: i32,
    pub ghost_reduction: f32,
}

impl Default for Anaglyph3DImageEffect {
    fn default() -> Self {
        Anaglyph3DImageEffect {
            mode: AnaglyphMode::RedCyan,
            separation_x: 10,
            separation_y: 0,
            ghost_reduction: 45.0,
        }
    }
}

impl EffectImpl for Anaglyph3DImageEffect {
    fn info() -> Info {
        Info::new("anaglyph_3d", "Anaglyph 3D", EffectCategory::Filters, "image-effect/glasses")
            .desc("Simulates a retro anaglyph 3D stereoscopic effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, AnaglyphMode>("mode", "Mode", AnaglyphMode::RedCyan, |e: &mut Self, v| e.mode = v, &[("Red / Cyan", AnaglyphMode::RedCyan), ("Amber / Blue", AnaglyphMode::AmberBlue), ("Green / Magenta", AnaglyphMode::GreenMagenta)]),
            int_slider::<Self>("separation_x", "Separation X", -100, 100, 10, |e: &mut Self, v| e.separation_x = v),
            int_slider::<Self>("separation_y", "Separation Y", -100, 100, 0, |e: &mut Self, v| e.separation_y = v),
            float_slider::<Self>("ghost_reduction", "Ghost reduction", 0.0, 100.0, 45.0, |e: &mut Self, v| e.ghost_reduction = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let ghost = clampf(self.ghost_reduction, 0.0, 100.0) / 100.0;
        if self.separation_x == 0 && self.separation_y == 0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let half_x = self.separation_x as f32 * 0.5;
        let half_y = self.separation_y as f32 * 0.5;
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let left = bilinear_sample(&src, width, height, x as f32 - half_x, y as f32 - half_y);
                let right = bilinear_sample(&src, width, height, x as f32 + half_x, y as f32 + half_y);
                let lg = analog_luminance01(left);
                let rg = analog_luminance01(right);
                let (r, g, b) = match self.mode {
                    AnaglyphMode::AmberBlue => (
                        lerp(left.r as f32 / 255.0, lg, ghost * 0.85),
                        lerp(left.g as f32 / 255.0, lg, ghost * 0.50),
                        lerp(right.b as f32 / 255.0, rg, ghost),
                    ),
                    AnaglyphMode::GreenMagenta => (
                        lerp(right.r as f32 / 255.0, rg, ghost * 0.85),
                        lerp(left.g as f32 / 255.0, lg, ghost),
                        lerp(right.b as f32 / 255.0, rg, ghost * 0.85),
                    ),
                    AnaglyphMode::RedCyan => (
                        lerp(left.r as f32 / 255.0, lg, ghost),
                        lerp(right.g as f32 / 255.0, rg, ghost * 0.90),
                        lerp(right.b as f32 / 255.0, rg, ghost),
                    ),
                };
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(r * 255.0),
                    clamp_to_byte(g * 255.0),
                    clamp_to_byte(b * 255.0),
                    ((left.a as i32 + right.a as i32) / 2) as u8,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
