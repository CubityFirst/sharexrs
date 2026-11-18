//! `AnimeSpeedLinesImageEffect` (`Core/ImageEffects/Filters/AnimeSpeedLinesImageEffect.cs`).

use crate::effects::prelude::*;

/// `AnimeSpeedLinesImageEffect`.
#[derive(Clone, Debug)]
pub struct AnimeSpeedLinesImageEffect {
    pub density: f32,
    pub strength: f32,
    pub focus_radius: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub contrast: f32,
    pub seed: i32,
}

impl Default for AnimeSpeedLinesImageEffect {
    fn default() -> Self {
        AnimeSpeedLinesImageEffect {
            density: 70.0,
            strength: 65.0,
            focus_radius: 18.0,
            center_x: 50.0,
            center_y: 50.0,
            contrast: 35.0,
            seed: 9911,
        }
    }
}

impl EffectImpl for AnimeSpeedLinesImageEffect {
    fn info() -> Info {
        Info::new(
            "anime_speed_lines",
            "Anime speed lines",
            EffectCategory::Filters,
            "app/scan-line",
        )
        .desc("Overlays radial speed lines in a manga/anime style.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "density",
                "Density",
                10.0,
                100.0,
                70.0,
                |e: &mut Self, v| e.density = v,
            ),
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                65.0,
                |e: &mut Self, v| e.strength = v,
            ),
            float_slider::<Self>(
                "focus_radius",
                "Focus radius",
                0.0,
                80.0,
                18.0,
                |e: &mut Self, v| e.focus_radius = v,
            ),
            float_slider::<Self>(
                "center_x",
                "Center X",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_x = v,
            ),
            float_slider::<Self>(
                "center_y",
                "Center Y",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_y = v,
            ),
            float_slider::<Self>(
                "contrast",
                "Contrast",
                0.0,
                100.0,
                35.0,
                |e: &mut Self, v| e.contrast = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let density = clampf(self.density, 10.0, 100.0) / 100.0;
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        let focus = clampf(self.focus_radius, 0.0, 80.0) / 100.0;
        let contrast = clampf(self.contrast, 0.0, 100.0) / 100.0;
        if strength <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let cx = (clampf(self.center_x, 0.0, 100.0) / 100.0) * (width - 1) as f32;
        let cy = (clampf(self.center_y, 0.0, 100.0) / 100.0) * (height - 1) as f32;
        let min_dim = (width as f32).min(height as f32);
        let focus_px = min_dim * focus;
        let feather_px = 8f32.max(min_dim * 0.22);
        let freq = 40.0 + (density * 180.0);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (mut r, mut g, mut b, a) = (s.r as f32, s.g as f32, s.b as f32, s.a as f32);
                let gray = (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
                let desat = 0.25 + (contrast * 0.45);
                r = lerp(r, gray, desat);
                g = lerp(g, gray, desat);
                b = lerp(b, gray, desat);
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = ((dx * dx) + (dy * dy)).sqrt();
                let radial = smooth_step(focus_px, focus_px + feather_px, dist);
                let theta = dy.atan2(dx);
                let band = (dist * 0.018)
                    + (hash01((dist * 0.12) as i32, (theta * 160.0) as i32, self.seed) * 0.85);
                let stripe = 0.5 + (0.5 * ((theta * freq) + band).sin());
                let bright = stripe.powf(36.0);
                let dark = (1.0 - stripe).powf(30.0);
                let line = radial * strength;
                let bm = bright * line * 0.92;
                let dm = dark * line * 0.58;
                r = lerp(r, 255.0, bm);
                g = lerp(g, 255.0, bm);
                b = lerp(b, 255.0, bm);
                r = lerp(r, 0.0, dm);
                g = lerp(g, 0.0, dm);
                b = lerp(b, 0.0, dm);
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(r),
                    clamp_to_byte(g),
                    clamp_to_byte(b),
                    clamp_to_byte(a),
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
