//! `WetPlateCollodionImageEffect` (`Core/ImageEffects/Filters/WetPlateCollodionImageEffect.cs`).

use crate::effects::prelude::*;

/// `WetPlateCollodionImageEffect`.
#[derive(Clone, Debug)]
pub struct WetPlateCollodionImageEffect {
    pub contrast: f32,
    pub plate_stains: f32,
    pub scratches: f32,
    pub silvering: f32,
    pub vignette: f32,
    pub seed: i32,
}

impl Default for WetPlateCollodionImageEffect {
    fn default() -> Self {
        WetPlateCollodionImageEffect {
            contrast: 132.0,
            plate_stains: 34.0,
            scratches: 24.0,
            silvering: 36.0,
            vignette: 58.0,
            seed: 1855,
        }
    }
}

impl EffectImpl for WetPlateCollodionImageEffect {
    fn info() -> Info {
        Info::new(
            "wet_plate_collodion",
            "Wet plate collodion",
            EffectCategory::Filters,
            "app/film",
        )
        .desc("Simulates the wet plate collodion photographic process with stains and silvering.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "contrast",
                "Contrast",
                50.0,
                200.0,
                132.0,
                |e: &mut Self, v| e.contrast = v,
            ),
            float_slider::<Self>(
                "plate_stains",
                "Plate stains",
                0.0,
                100.0,
                34.0,
                |e: &mut Self, v| e.plate_stains = v,
            ),
            float_slider::<Self>(
                "scratches",
                "Scratches",
                0.0,
                100.0,
                24.0,
                |e: &mut Self, v| e.scratches = v,
            ),
            float_slider::<Self>(
                "silvering",
                "Silvering",
                0.0,
                100.0,
                36.0,
                |e: &mut Self, v| e.silvering = v,
            ),
            float_slider::<Self>(
                "vignette",
                "Vignette",
                0.0,
                100.0,
                58.0,
                |e: &mut Self, v| e.vignette = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let contrast = clampf(self.contrast, 50.0, 200.0) / 100.0;
        let stains = clampf(self.plate_stains, 0.0, 100.0) / 100.0;
        let scratches = clampf(self.scratches, 0.0, 100.0) / 100.0;
        let silvering = clampf(self.silvering, 0.0, 100.0) / 100.0;
        let vignette = clampf(self.vignette, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (width - 1) as f32 * 0.5;
        let cy = (height - 1) as f32 * 0.5;
        let max_dist = 1f32.max(((cx * cx) + (cy * cy)).sqrt());
        let shadow = SkColor::rgb(20, 16, 18);
        let highlight = SkColor::rgb(220, 213, 202);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let seed = self.seed;
        for y in 0..height {
            for x in 0..width {
                let (xf, yf) = (x as f32, y as f32);
                let i = (y * width + x) as usize;
                let s = src[i];
                let lum = apply_contrast(analog_luminance01(s), contrast);
                let tone = lerp_color(shadow, highlight, lum.powf(0.88));
                let chem = fractal_noise(xf * 0.008, yf * 0.008, 5, 2.05, 0.52, seed);
                let streak = fractal_noise(xf * 0.024, yf * 0.0026, 4, 2.2, 0.48, seed ^ 337);
                let mut stain = smooth_step(0.54, 0.92, chem) * stains;
                stain += 0f32.max(streak - 0.72) * stains * 0.45;
                let edge_d = x.min(width - 1 - x).min(y.min(height - 1 - y)) as f32;
                let edge01 =
                    1.0 - clampf(edge_d / 1f32.max(width.min(height) as f32 * 0.16), 0.0, 1.0);
                let silver = silvering * edge01 * lum.max(0.02).powf(0.55);
                let radial = ((xf - cx).powf(2.0) + (yf - cy).powf(2.0)).sqrt() / max_dist;
                let vm = clamp01(1.0 - (vignette * radial.powf(1.95) * 0.78));
                let mut scratch = 0f32;
                if hash01(x / 3, y, seed ^ 173) > 0.9975 {
                    scratch += scratches * 0.55;
                }
                if hash01(x, y / 5, seed ^ 811) > 0.9984 {
                    scratch += scratches * 0.35;
                }
                let mut r = tone.r as f32 / 255.0;
                let mut g = tone.g as f32 / 255.0;
                let mut b = tone.b as f32 / 255.0;
                r = lerp(r, 0.88, silver * 0.34);
                g = lerp(g, 0.84, silver * 0.30);
                b = lerp(b, 0.92, silver * 0.38);
                r = clamp01((r - (stain * 0.10) + scratch) * vm);
                g = clamp01((g - (stain * 0.08) + scratch) * vm);
                b = clamp01((b - (stain * 0.06) + scratch) * vm);
                dst[i] = SkColor::rgba(
                    clamp_to_byte(r * 255.0),
                    clamp_to_byte(g * 255.0),
                    clamp_to_byte(b * 255.0),
                    s.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
