//! `InfraredFalseColorImageEffect` (`Core/ImageEffects/Filters/InfraredFalseColorImageEffect.cs`).

use crate::effects::prelude::*;

/// `InfraredFalseColorImageEffect`.
#[derive(Clone, Debug)]
pub struct InfraredFalseColorImageEffect {
    pub intensity: f32,
    pub foliage_shift: f32,
    pub sky_darkening: f32,
    pub glow: f32,
    pub contrast: f32,
}

impl Default for InfraredFalseColorImageEffect {
    fn default() -> Self {
        InfraredFalseColorImageEffect {
            intensity: 68.0,
            foliage_shift: 78.0,
            sky_darkening: 44.0,
            glow: 20.0,
            contrast: 118.0,
        }
    }
}

impl EffectImpl for InfraredFalseColorImageEffect {
    fn info() -> Info {
        Info::new("infrared_false_color", "Infrared false color", EffectCategory::Filters, "image-effect/flame")
            .desc("Simulates infrared false color photography.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("intensity", "Intensity", 0.0, 100.0, 68.0, |e: &mut Self, v| e.intensity = v),
            float_slider::<Self>("foliage_shift", "Foliage shift", 0.0, 100.0, 78.0, |e: &mut Self, v| e.foliage_shift = v),
            float_slider::<Self>("sky_darkening", "Sky darkening", 0.0, 100.0, 44.0, |e: &mut Self, v| e.sky_darkening = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 20.0, |e: &mut Self, v| e.glow = v),
            float_slider::<Self>("contrast", "Contrast", 50.0, 200.0, 118.0, |e: &mut Self, v| e.contrast = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let intensity = clampf(self.intensity, 0.0, 100.0) / 100.0;
        let foliage = clampf(self.foliage_shift, 0.0, 100.0) / 100.0;
        let sky = clampf(self.sky_darkening, 0.0, 100.0) / 100.0;
        let glow = clampf(self.glow, 0.0, 100.0) / 100.0;
        let contrast = clampf(self.contrast, 50.0, 200.0) / 100.0;
        if intensity <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let glow_px = (glow > 0.01).then(|| pixels(&create_blurred_clamp(source, 6.0 + (glow * 12.0))));
        let gp = glow_px.as_ref().unwrap_or(&src);
        let dst: Vec<SkColor> = (0..src.len())
            .map(|i| {
                let s = src[i];
                let (h, sa, l) = s.to_hsl();
                let (nh, ns, nl);
                let is_foliage = (45.0..=170.0).contains(&h) && sa >= 18.0;
                let is_sky = (170.0..=260.0).contains(&h) && sa >= 12.0;
                if is_foliage {
                    nh = lerp(h, 8.0 + (l * 0.16), foliage * (0.55 + (intensity * 0.35)));
                    ns = 100f32.min(sa * (1.0 + (0.18 * intensity) + (0.32 * foliage)));
                    nl = 100f32.min(l + (intensity * 14.0) + (foliage * 10.0));
                } else if is_sky {
                    nh = lerp(h, 196.0, 0.22 + (intensity * 0.28));
                    ns = 100f32.min(sa * (1.03 + (0.10 * intensity)));
                    nl = 0f32.max(l * (1.0 - (sky * 0.46)));
                } else {
                    nh = lerp(h, h + 6.0, intensity * 0.10);
                    ns = 100f32.min(sa * (1.0 + (intensity * 0.06)));
                    nl = clampf(l + (intensity * 3.0), 0.0, 100.0);
                }
                let m = analog_from_hsl(nh, ns, nl, s.a);
                let mut r = apply_contrast(m.r as f32 / 255.0, contrast);
                let mut g = apply_contrast(m.g as f32 / 255.0, contrast);
                let mut b = apply_contrast(m.b as f32 / 255.0, contrast);
                if glow > 0.0001 {
                    let mut gm = 0f32.max((analog_luminance01(gp[i]) - 0.52) / 0.48);
                    gm = gm.powf(1.55) * glow;
                    r = screen(r, gm * 0.80);
                    g = screen(g, gm * 0.18);
                    b = screen(b, gm * 0.42);
                }
                SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), s.a)
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
