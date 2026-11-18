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
        Info::new("wave_distortion", "Wave distortion", EffectCategory::Manipulations, "image-effect/waves")
            .desc("Applies a sine wave distortion to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("amplitude", "Amplitude", 1, 100, 20, |e: &mut Self, v| e.amplitude = v),
            int_slider::<Self>("wavelength", "Wavelength", 10, 500, 100, |e: &mut Self, v| e.wavelength = v),
            enum_param::<Self, WaveDirection>("direction", "Direction", WaveDirection::Horizontal, |e: &mut Self, v| e.direction = v, &[("Horizontal", WaveDirection::Horizontal), ("Vertical", WaveDirection::Vertical), ("Both", WaveDirection::Both)]),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
