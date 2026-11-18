//! `ChalkDrawingImageEffect` (`Core/ImageEffects/Filters/ChalkDrawingImageEffect.cs`).

use crate::effects::prelude::*;

/// `ChalkDrawingImageEffect`.
#[derive(Clone, Debug)]
pub struct ChalkDrawingImageEffect {
    pub detail: f32,
    pub roughness: f32,
    pub chalk_color: SkColor,
    pub board_color: SkColor,
}

impl Default for ChalkDrawingImageEffect {
    fn default() -> Self {
        ChalkDrawingImageEffect {
            detail: 50.0,
            roughness: 40.0,
            chalk_color: SkColor::argb(0xFFFFFFFF),
            board_color: SkColor::argb(0xFF1E3228),
        }
    }
}

impl EffectImpl for ChalkDrawingImageEffect {
    fn info() -> Info {
        Info::new(
            "chalk_drawing",
            "Chalk Drawing",
            EffectCategory::Filters,
            "image-effect/chalk-brush",
        )
        .desc("Simulates a chalk drawing on a dark surface.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("detail", "Detail", 0.0, 100.0, 50.0, |e: &mut Self, v| {
                e.detail = v
            }),
            float_slider::<Self>(
                "roughness",
                "Roughness",
                0.0,
                100.0,
                40.0,
                |e: &mut Self, v| e.roughness = v,
            ),
            color_param::<Self>(
                "chalk_color",
                "Chalk Color",
                SkColor::argb(0xFFFFFFFF),
                |e: &mut Self, v| e.chalk_color = v,
            ),
            color_param::<Self>(
                "board_color",
                "Board Color",
                SkColor::argb(0xFF1E3228),
                |e: &mut Self, v| e.board_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let detail = clampf(self.detail, 0.0, 100.0) / 100.0;
        let roughness = clampf(self.roughness, 0.0, 100.0) / 100.0;
        let src = pixels(source);
        let lum: Vec<f32> = src
            .iter()
            .map(|c| (0.299 * c.r as f32 + 0.587 * c.g as f32 + 0.114 * c.b as f32) / 255.0)
            .collect();
        let mut edge = vec![0f32; src.len()];
        let mut max_edge = 0f32;
        let l = |x: i32, y: i32| lum[(y * width + x) as usize];
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let gx = -l(x - 1, y - 1) + l(x + 1, y - 1) - 2.0 * l(x - 1, y) + 2.0 * l(x + 1, y)
                    - l(x - 1, y + 1)
                    + l(x + 1, y + 1);
                let gy = -l(x - 1, y - 1) - 2.0 * l(x, y - 1) - l(x + 1, y - 1)
                    + l(x - 1, y + 1)
                    + 2.0 * l(x, y + 1)
                    + l(x + 1, y + 1);
                let m = (gx * gx + gy * gy).sqrt();
                edge[(y * width + x) as usize] = m;
                if m > max_edge {
                    max_edge = m;
                }
            }
        }
        if max_edge > 0.0 {
            let inv = 1.0 / max_edge;
            for e in edge.iter_mut() {
                *e *= inv;
            }
        }
        let mut noise = vec![0f32; src.len()];
        let mut rng: i32 = 12345;
        for n in noise.iter_mut() {
            rng ^= rng.wrapping_shl(13);
            rng ^= rng >> 17;
            rng ^= rng.wrapping_shl(5);
            *n = (rng & 0x7FFF_FFFF) as f32 / i32::MAX as f32;
        }
        let (br, bg, bb) = (
            self.board_color.r as f32 / 255.0,
            self.board_color.g as f32 / 255.0,
            self.board_color.b as f32 / 255.0,
        );
        let (cr, cg, cb) = (
            self.chalk_color.r as f32 / 255.0,
            self.chalk_color.g as f32 / 255.0,
            self.chalk_color.b as f32 / 255.0,
        );
        let edge_weight = 0.3 + detail * 0.7;
        let lum_weight = 1.0 - edge_weight * 0.5;
        let to = |v: f32| (clampf(v, 0.0, 1.0) * 255.0).round_ties_even() as u8;
        let dst: Vec<SkColor> = (0..src.len())
            .map(|i| {
                let mut ci = edge[i] * edge_weight + lum[i] * lum_weight;
                ci = clampf(ci, 0.0, 1.0);
                let tf = 1.0 - roughness * (1.0 - noise[i]) * 0.6;
                ci *= tf;
                ci *= lum[i].powf(0.5);
                ci = clampf(ci, 0.0, 1.0);
                SkColor::rgba(
                    to(br + (cr - br) * ci),
                    to(bg + (cg - bg) * ci),
                    to(bb + (cb - bb) * ci),
                    src[i].a,
                )
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
