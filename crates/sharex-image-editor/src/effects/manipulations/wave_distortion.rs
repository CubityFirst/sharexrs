//! `WaveDistortionImageEffect` (`Core/ImageEffects/Manipulations/WaveDistortionImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveDirection {
    Horizontal,
    Vertical,
    Both,
}

/// `WaveDistortionImageEffect`.
#[derive(Clone, Debug)]
pub struct WaveDistortionImageEffect {
    pub amplitude: i32,
    pub wavelength: i32,
    pub direction: WaveDirection,
}

impl Default for WaveDistortionImageEffect {
    fn default() -> Self {
        WaveDistortionImageEffect {
            amplitude: 20,
            wavelength: 100,
            direction: WaveDirection::Horizontal,
        }
    }
}

impl EffectImpl for WaveDistortionImageEffect {
    fn info() -> Info {
        Info::new(
            "wave_distortion",
            "Wave distortion",
            EffectCategory::Manipulations,
            "image-effect/waves",
        )
        .desc("Applies a sine wave distortion to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("amplitude", "Amplitude", 1, 100, 20, |e: &mut Self, v| {
                e.amplitude = v
            }),
            int_slider::<Self>(
                "wavelength",
                "Wavelength",
                10,
                500,
                100,
                |e: &mut Self, v| e.wavelength = v,
            ),
            enum_param::<Self, WaveDirection>(
                "direction",
                "Direction",
                WaveDirection::Horizontal,
                |e: &mut Self, v| e.direction = v,
                &[
                    ("Horizontal", WaveDirection::Horizontal),
                    ("Vertical", WaveDirection::Vertical),
                    ("Both", WaveDirection::Both),
                ],
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.amplitude <= 0 {
            return Ok(source.clone());
        }
        let (w, h) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let freq = std::f32::consts::PI * 2.0 / 10.max(self.wavelength) as f32;
        let horizontal = matches!(
            self.direction,
            WaveDirection::Horizontal | WaveDirection::Both
        );
        let vertical = matches!(
            self.direction,
            WaveDirection::Vertical | WaveDirection::Both
        );
        for y in 0..h {
            for x in 0..w {
                let (mut sx, mut sy) = (x, y);
                if horizontal {
                    sx = x + (self.amplitude as f32 * (y as f32 * freq).sin()) as i32;
                }
                if vertical {
                    sy = y + (self.amplitude as f32 * (x as f32 * freq).sin()) as i32;
                }
                dst[(y * w + x) as usize] = if sx >= 0 && sx < w && sy >= 0 && sy < h {
                    src[(sy * w + sx) as usize]
                } else {
                    color::TRANSPARENT
                };
            }
        }
        Ok(from_pixels(w as u32, h as u32, &dst))
    }
}
