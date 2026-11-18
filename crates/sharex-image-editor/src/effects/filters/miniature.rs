//! `MiniatureImageEffect` (`Core/ImageEffects/Filters/MiniatureImageEffect.cs`).

use crate::effects::prelude::*;

/// `MiniatureImageEffect`.
#[derive(Clone, Debug)]
pub struct MiniatureImageEffect {
    pub blur_radius: i32,
    pub focus_position: f32,
    pub focus_band: f32,
    pub saturation_boost: f32,
}

impl Default for MiniatureImageEffect {
    fn default() -> Self {
        MiniatureImageEffect {
            blur_radius: 15,
            focus_position: 50.0,
            focus_band: 20.0,
            saturation_boost: 20.0,
        }
    }
}

impl EffectImpl for MiniatureImageEffect {
    fn info() -> Info {
        Info::new(
            "miniature",
            "Miniature",
            EffectCategory::Filters,
            "image-effect/cube",
        )
        .desc("Creates a toy miniature look by blurring top and bottom with boosted saturation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>(
                "blur_radius",
                "Blur radius",
                2,
                40,
                15,
                |e: &mut Self, v| e.blur_radius = v,
            ),
            float_slider::<Self>(
                "focus_position",
                "Focus position %",
                10.0,
                90.0,
                50.0,
                |e: &mut Self, v| e.focus_position = v,
            ),
            float_slider::<Self>(
                "focus_band",
                "Focus band %",
                5.0,
                50.0,
                20.0,
                |e: &mut Self, v| e.focus_band = v,
            ),
            float_slider::<Self>(
                "saturation",
                "Saturation boost",
                0.0,
                50.0,
                20.0,
                |e: &mut Self, v| e.saturation_boost = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let radius = 1.max(self.blur_radius);
        let (w, h) = (source.width() as i32, source.height() as i32);
        let blurred = blur(source, radius as f32, radius as f32);
        let hf = h as f32;
        let focus_y = hf * self.focus_position / 100.0;
        let band_half = hf * self.focus_band / 200.0;
        let top = focus_y - band_half;
        let bottom = focus_y + band_half;
        let sharp = pixels(source);
        let bl = pixels(&blurred);
        let mut dst = vec![SkColor::default(); sharp.len()];
        let boost = 1.0 + self.saturation_boost / 100.0;
        for y in 0..h {
            let yf = y as f32;
            let t = if yf < top {
                1.0 - clampf((top - yf) / (top + 1.0), 0.0, 1.0)
            } else if yf > bottom {
                1.0 - clampf((yf - bottom) / (hf - bottom + 1.0), 0.0, 1.0)
            } else {
                1.0
            };
            let sw = t * t * (3.0 - 2.0 * t);
            for x in 0..w {
                let i = (y * w + x) as usize;
                let (s, b) = (sharp[i], bl[i]);
                let mix = |a: u8, c: u8| (a as f32 * sw + c as f32 * (1.0 - sw)) as u8;
                let m = SkColor::rgba(mix(s.r, b.r), mix(s.g, b.g), mix(s.b, b.b), s.a);
                let (hue, sat, lum) = m.to_hsl();
                dst[i] = SkColor::from_hsl(hue, clampf(sat * boost, 0.0, 100.0), lum, s.a);
            }
        }
        Ok(from_pixels(w as u32, h as u32, &dst))
    }
}
