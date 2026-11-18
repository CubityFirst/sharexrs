//! `FilmEmulationImageEffect` (`Core/ImageEffects/Adjustments/FilmEmulationImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilmEmulationPreset {
    Classic,
    Warm,
    Cool,
    Faded,
    CrossProcessed,
}

/// `FilmEmulationImageEffect`.
#[derive(Clone, Debug)]
pub struct FilmEmulationImageEffect {
    pub preset: FilmEmulationPreset,
    pub tone_strength: f32,
    pub grain_amount: f32,
    pub fade_amount: f32,
    pub contrast_amount: f32,
    pub seed: i32,
}

impl Default for FilmEmulationImageEffect {
    fn default() -> Self {
        FilmEmulationImageEffect {
            preset: FilmEmulationPreset::Classic,
            tone_strength: 65.0,
            grain_amount: 12.0,
            fade_amount: 10.0,
            contrast_amount: 110.0,
            seed: 1977,
        }
    }
}

impl EffectImpl for FilmEmulationImageEffect {
    fn info() -> Info {
        Info::new(
            "film_emulation",
            "Film emulation",
            EffectCategory::Adjustments,
            "app/film",
        )
        .desc("Applies cinematic analog film looks with grain and fade.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, FilmEmulationPreset>(
                "preset",
                "Preset",
                FilmEmulationPreset::Classic,
                |e: &mut Self, v| e.preset = v,
                &[
                    ("Classic", FilmEmulationPreset::Classic),
                    ("Warm", FilmEmulationPreset::Warm),
                    ("Cool", FilmEmulationPreset::Cool),
                    ("Faded", FilmEmulationPreset::Faded),
                    ("Cross processed", FilmEmulationPreset::CrossProcessed),
                ],
            ),
            float_slider::<Self>(
                "tone_strength",
                "Tone strength",
                0.0,
                100.0,
                65.0,
                |e: &mut Self, v| e.tone_strength = v,
            ),
            float_slider::<Self>(
                "grain_amount",
                "Grain amount",
                0.0,
                100.0,
                12.0,
                |e: &mut Self, v| e.grain_amount = v,
            ),
            float_slider::<Self>(
                "fade_amount",
                "Fade amount",
                0.0,
                100.0,
                10.0,
                |e: &mut Self, v| e.fade_amount = v,
            ),
            float_slider::<Self>(
                "contrast_amount",
                "Contrast amount",
                50.0,
                150.0,
                110.0,
                |e: &mut Self, v| e.contrast_amount = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let tone = clampf(self.tone_strength, 0.0, 100.0) / 100.0;
        let grain = clampf(self.grain_amount, 0.0, 100.0) / 100.0;
        let fade = clampf(self.fade_amount, 0.0, 100.0) / 100.0;
        let contrast = clampf(self.contrast_amount, 50.0, 150.0) / 100.0;
        if tone <= 0.0 && grain <= 0.0 && fade <= 0.0 && (contrast - 1.0).abs() < 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let mut r = s.r as f32 / 255.0;
                let mut g = s.g as f32 / 255.0;
                let mut b = s.b as f32 / 255.0;
                let (tr, tg, tb) = apply_preset(self.preset, r, g, b);
                r = lerp(r, tr, tone);
                g = lerp(g, tg, tone);
                b = lerp(b, tb, tone);
                r = ((r - 0.5) * contrast) + 0.5;
                g = ((g - 0.5) * contrast) + 0.5;
                b = ((b - 0.5) * contrast) + 0.5;
                let fade_scale = 1.0 - (fade * 0.35);
                let black_lift = fade * 0.06;
                r = (r * fade_scale) + black_lift;
                g = (g * fade_scale) + black_lift;
                b = (b * fade_scale) + black_lift;
                if grain > 0.0 {
                    let n0 = (hash01(x, y, self.seed) * 2.0) - 1.0;
                    let n1 = (hash01(x, y, self.seed ^ 117) * 2.0) - 1.0;
                    let n2 = (hash01(x, y, self.seed ^ 911) * 2.0) - 1.0;
                    let amp = grain * 0.10;
                    r += n0 * amp;
                    g += n1 * amp;
                    b += n2 * amp;
                }
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(clamp01(r) * 255.0),
                    clamp_to_byte(clamp01(g) * 255.0),
                    clamp_to_byte(clamp01(b) * 255.0),
                    s.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn apply_preset(preset: FilmEmulationPreset, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let (tr, tg, tb) = match preset {
        FilmEmulationPreset::Warm => ((r * 1.10) + (g * 0.015), g * 1.04, (b * 0.88) + (r * 0.01)),
        FilmEmulationPreset::Cool => (
            (r * 0.94) + (b * 0.02),
            (g * 1.01) + (b * 0.01),
            (b * 1.10) + (g * 0.01),
        ),
        FilmEmulationPreset::Faded => {
            let lum = (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
            (
                lerp(r, lum + 0.04, 0.22),
                lerp(g, lum + 0.03, 0.18),
                lerp(b, lum - 0.01, 0.26),
            )
        }
        FilmEmulationPreset::CrossProcessed => (
            r.max(0.0001).powf(0.88) * 1.08,
            g.max(0.0001).powf(1.05) * 0.98,
            b.max(0.0001).powf(0.82) * 1.12,
        ),
        FilmEmulationPreset::Classic => (
            (r * 1.05) + (g * 0.02),
            (g * 1.02) + (r * 0.01),
            (b * 0.94) + (g * 0.02),
        ),
    };
    (clamp01(tr), clamp01(tg), clamp01(tb))
}
