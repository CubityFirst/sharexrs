//! `ColorSplashImageEffect` (`Core/ImageEffects/Filters/ColorSplashImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorSplashImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorSplashImageEffect {
    pub target_hue: i32,
    pub tolerance: i32,
}

impl Default for ColorSplashImageEffect {
    fn default() -> Self {
        ColorSplashImageEffect {
            target_hue: 0,
            tolerance: 30,
        }
    }
}

impl EffectImpl for ColorSplashImageEffect {
    fn info() -> Info {
        Info::new("color_splash", "Color splash", EffectCategory::Filters, "image-effect/droplets")
            .desc("Keeps one color and desaturates the rest of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("hue", "Target hue", 0, 360, 0, |e: &mut Self, v| e.target_hue = v),
            int_slider::<Self>("tolerance", "Tolerance", 1, 180, 30, |e: &mut Self, v| e.tolerance = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let target = self.target_hue as f32;
        let tol = self.tolerance as f32;
        let dst: Vec<SkColor> = pixels(source)
            .iter()
            .map(|&c| {
                let (hue, sat, _) = c.to_hsl();
                let mut diff = (hue - target).abs();
                if diff > 180.0 {
                    diff = 360.0 - diff;
                }
                if diff <= tol && sat > 5.0 {
                    c
                } else {
                    let gray = (0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32) as u8;
                    SkColor::rgba(gray, gray, gray, c.a)
                }
            })
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
