//! `WatercolorKuwaharaImageEffect` (`Core/ImageEffects/Filters/WatercolorKuwaharaImageEffect.cs`).

use crate::effects::prelude::*;

/// `WatercolorKuwaharaImageEffect`.
#[derive(Clone, Debug)]
pub struct WatercolorKuwaharaImageEffect {
    pub radius: i32,
    pub saturation_boost: f32,
    pub detail_blend: f32,
}

impl Default for WatercolorKuwaharaImageEffect {
    fn default() -> Self {
        WatercolorKuwaharaImageEffect {
            radius: 4,
            saturation_boost: 20.0,
            detail_blend: 18.0,
        }
    }
}

impl EffectImpl for WatercolorKuwaharaImageEffect {
    fn info() -> Info {
        Info::new(
            "watercolor_kuwahara",
            "Watercolor / Kuwahara",
            EffectCategory::Filters,
            "image-effect/paint-brush",
        )
        .desc("Applies a Kuwahara filter for a watercolor painting effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("radius", "Radius", 2, 10, 4, |e: &mut Self, v| e.radius = v),
            float_slider::<Self>(
                "saturation_boost",
                "Saturation boost",
                0.0,
                100.0,
                20.0,
                |e: &mut Self, v| e.saturation_boost = v,
            ),
            float_slider::<Self>(
                "detail_blend",
                "Detail blend",
                0.0,
                100.0,
                18.0,
                |e: &mut Self, v| e.detail_blend = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let radius = self.radius.clamp(2, 10);
        let sat = clampf(self.saturation_boost, 0.0, 100.0) / 100.0;
        let detail = clampf(self.detail_blend, 0.0, 100.0) / 100.0;
        let src = pixels(source);
        let stride = width + 1;
        let size = (stride * (height + 1)) as usize;
        let mut ir = vec![0f64; size];
        let mut ig = vec![0f64; size];
        let mut ib = vec![0f64; size];
        let mut il = vec![0f64; size];
        let mut il2 = vec![0f64; size];
        for y in 1..=height {
            let (mut rr, mut rg, mut rb, mut rl, mut rl2) = (0f64, 0f64, 0f64, 0f64, 0f64);
            for x in 1..=width {
                let c = src[((y - 1) * width + (x - 1)) as usize];
                let l =
                    ((0.2126 * c.r as f64) + (0.7152 * c.g as f64) + (0.0722 * c.b as f64)) / 255.0;
                rr += c.r as f64;
                rg += c.g as f64;
                rb += c.b as f64;
                rl += l;
                rl2 += l * l;
                let i = (y * stride + x) as usize;
                let p = ((y - 1) * stride + x) as usize;
                ir[i] = ir[p] + rr;
                ig[i] = ig[p] + rg;
                ib[i] = ib[p] + rb;
                il[i] = il[p] + rl;
                il2[i] = il2[p] + rl2;
            }
        }
        let rect = |t: &[f64], x0: i32, y0: i32, x1: i32, y1: i32| {
            t[(y1 * stride + x1) as usize]
                - t[(y0 * stride + x1) as usize]
                - t[(y1 * stride + x0) as usize]
                + t[(y0 * stride + x0) as usize]
        };
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            for x in 0..width {
                let mut best = (0f64, 0f64, 0f64);
                let mut best_var = f64::MAX;
                let quads = [
                    (x - radius, y - radius, x + 1, y + 1),
                    (x, y - radius, x + radius + 1, y + 1),
                    (x - radius, y, x + 1, y + radius + 1),
                    (x, y, x + radius + 1, y + radius + 1),
                ];
                for (x0, y0, x1, y1) in quads {
                    let (l, t, r, b) = (
                        x0.clamp(0, width),
                        y0.clamp(0, height),
                        x1.clamp(0, width),
                        y1.clamp(0, height),
                    );
                    if r <= l || b <= t {
                        continue;
                    }
                    let count = (r - l) * (b - t);
                    if count <= 0 {
                        continue;
                    }
                    let inv = 1.0 / count as f64;
                    let mean_l = rect(&il, l, t, r, b) * inv;
                    let var = (rect(&il2, l, t, r, b) * inv) - (mean_l * mean_l);
                    if var < best_var {
                        best_var = var;
                        best = (
                            rect(&ir, l, t, r, b) * inv,
                            rect(&ig, l, t, r, b) * inv,
                            rect(&ib, l, t, r, b) * inv,
                        );
                    }
                }
                let i = (y * width + x) as usize;
                let mut f = SkColor::rgba(
                    clamp_to_byte(best.0 as f32),
                    clamp_to_byte(best.1 as f32),
                    clamp_to_byte(best.2 as f32),
                    src[i].a,
                );
                if sat > 0.0 {
                    let (r, g, b) = (f.r as f32, f.g as f32, f.b as f32);
                    let gray = (r + g + b) / 3.0;
                    let factor = 1.0 + sat;
                    f = SkColor::rgba(
                        clamp_to_byte(gray + ((r - gray) * factor)),
                        clamp_to_byte(gray + ((g - gray) * factor)),
                        clamp_to_byte(gray + ((b - gray) * factor)),
                        f.a,
                    );
                }
                if detail > 0.0 {
                    f = lerp_color(f, src[i], detail * 0.55);
                }
                dst[i] = f;
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
