//! `GlitchShiftImageEffect` (`Core/ImageEffects/Filters/GlitchShiftImageEffect.cs`).

use crate::effects::prelude::*;

/// `GlitchShiftImageEffect`.
#[derive(Clone, Debug)]
pub struct GlitchShiftImageEffect {
    pub intensity: i32,
    pub bands: i32,
    pub seed: i32,
}

impl Default for GlitchShiftImageEffect {
    fn default() -> Self {
        GlitchShiftImageEffect {
            intensity: 30,
            bands: 12,
            seed: 42,
        }
    }
}

impl EffectImpl for GlitchShiftImageEffect {
    fn info() -> Info {
        Info::new("glitch_shift", "Glitch shift", EffectCategory::Filters, "app/zap")
            .desc("Randomly shifts horizontal bands of the image for a digital glitch look.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("intensity", "Intensity", 1, 100, 30, |e: &mut Self, v| e.intensity = v),
            int_slider::<Self>("bands", "Band count", 2, 50, 12, |e: &mut Self, v| e.bands = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.intensity <= 0 {
            return Ok(source.clone());
        }
        let (w, h) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = src.clone();
        let mut rng = random::Compat::new(self.seed);
        let max_shift = w * self.intensity / 100;
        let bands = 2.max(self.bands);
        for _ in 0..bands {
            let start = rng.next_max(h);
            let bh = rng.next_range(2, 3.max(h / bands));
            let shift = rng.next_range(-max_shift, max_shift + 1);
            for y in start..(start + bh).min(h) {
                for x in 0..w {
                    let sx = x - shift;
                    dst[(y * w + x) as usize] = if sx >= 0 && sx < w { src[(y * w + sx) as usize] } else { color::BLACK };
                }
            }
        }
        Ok(from_pixels(w as u32, h as u32, &dst))
    }
}
