//! `CrystalPrismImageEffect` (`Core/ImageEffects/Filters/CrystalPrismImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrystalPrismImageEffect`.
#[derive(Clone, Debug)]
pub struct CrystalPrismImageEffect {
    pub facet_size: i32,
    pub refraction: f32,
    pub dispersion: f32,
    pub sparkle: f32,
    pub seed: i32,
}

impl Default for CrystalPrismImageEffect {
    fn default() -> Self {
        CrystalPrismImageEffect {
            facet_size: 24,
            refraction: 8.0,
            dispersion: 4.0,
            sparkle: 25.0,
            seed: 4771,
        }
    }
}

impl EffectImpl for CrystalPrismImageEffect {
    fn info() -> Info {
        Info::new("crystal_prism", "Crystal prism", EffectCategory::Filters, "image-effect/gem")
            .desc("Refracts and disperses light through crystal-like facets.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("facetSize", "Facet size", 6, 96, 24, |e: &mut Self, v| e.facet_size = v),
            float_slider::<Self>("refraction", "Refraction", 0.0, 30.0, 8.0, |e: &mut Self, v| e.refraction = v),
            float_slider::<Self>("dispersion", "Dispersion", 0.0, 20.0, 4.0, |e: &mut Self, v| e.dispersion = v),
            float_slider::<Self>("sparkle", "Sparkle", 0.0, 100.0, 25.0, |e: &mut Self, v| e.sparkle = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let facet = clamp_int(self.facet_size, 6, 96);
        let refraction = clampf(self.refraction, 0.0, 30.0);
        let dispersion = clampf(self.dispersion, 0.0, 20.0);
        let sparkle = clampf(self.sparkle, 0.0, 100.0) / 100.0;
        if refraction <= 0.0 && dispersion <= 0.0 && sparkle <= 0.0 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let two_pi = std::f32::consts::PI * 2.0;
        let blend_strength = 0.58 + (0.25 * (refraction / 30.0));
        let seed = self.seed;
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let (cell_x, cell_y) = (x / facet, y / facet);
                let h0 = hash01(cell_x, cell_y, seed);
                let h1 = hash01(cell_x, cell_y, seed ^ 149);
                let h2 = hash01(cell_x, cell_y, seed ^ 313);
                let h3 = hash01(cell_x, cell_y, seed ^ 341);
                let cx = (cell_x as f32 + 0.20 + (0.60 * h0)) * facet as f32;
                let cy = (cell_y as f32 + 0.20 + (0.60 * h1)) * facet as f32;
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = ((dx * dx) + (dy * dy)).sqrt();
                let radial = 1.0 - clamp01(dist / (facet as f32 * 0.85));
                let angle = h2 * two_pi;
                let (dir_x, dir_y) = (angle.cos(), angle.sin());
                let off = refraction * (0.30 + (0.70 * radial));
                let sx = x as f32 + (dir_x * off);
                let sy = y as f32 + (dir_y * off);
                let disp = dispersion * (0.25 + (0.75 * radial));
                let cr = bilinear_sample(&src, width, height, sx + (dir_y * disp), sy - (dir_x * disp));
                let cg = bilinear_sample(&src, width, height, sx, sy);
                let cb = bilinear_sample(&src, width, height, sx - (dir_y * disp), sy + (dir_x * disp));
                let shade = 0.88 + ((h3 - 0.5) * 0.30);
                let mut pr = (cr.r as f32 / 255.0) * shade;
                let mut pg = (cg.g as f32 / 255.0) * shade;
                let mut pb = (cb.b as f32 / 255.0) * shade;
                let fx = (x % facet) as f32 / facet as f32;
                let fy = (y % facet) as f32 / facet as f32;
                let border = fx.min(1.0 - fx).min(fy.min(1.0 - fy));
                let edge = 1.0 - smooth_step(0.04, 0.28, border);
                let sm = edge * edge * sparkle;
                pr += sm * 0.32;
                pg += sm * 0.34;
                pb += sm * 0.38;
                let o = src[(row + x) as usize];
                let r = lerp(o.r as f32 / 255.0, pr, blend_strength);
                let g = lerp(o.g as f32 / 255.0, pg, blend_strength);
                let b = lerp(o.b as f32 / 255.0, pb, blend_strength);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(r * 255.0), clamp_to_byte(g * 255.0), clamp_to_byte(b * 255.0), o.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
