//! `RopeBorderImageEffect` (`Core/ImageEffects/Filters/RopeBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `RopeBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct RopeBorderImageEffect {
    pub border_size: i32,
    pub rope_color: SkColor,
    pub bg_color: SkColor,
    pub twist_density: f32,
    pub fiber_detail: f32,
}

impl Default for RopeBorderImageEffect {
    fn default() -> Self {
        RopeBorderImageEffect {
            border_size: 40,
            rope_color: SkColor::argb(0xFFB4915A),
            bg_color: SkColor::argb(0xFF3C2D1E),
            twist_density: 50.0,
            fiber_detail: 55.0,
        }
    }
}

impl EffectImpl for RopeBorderImageEffect {
    fn info() -> Info {
        Info::new(
            "rope_border",
            "Rope border",
            EffectCategory::Filters,
            "app/link",
        )
        .desc("Adds a twisted rope/cord border with braided fiber detail and shading.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                12,
                200,
                40,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "rope_color",
                "Rope color",
                SkColor::argb(0xFFB4915A),
                |e: &mut Self, v| e.rope_color = v,
            ),
            color_param::<Self>(
                "bg_color",
                "Background color",
                SkColor::argb(0xFF3C2D1E),
                |e: &mut Self, v| e.bg_color = v,
            ),
            float_slider::<Self>(
                "twist_density",
                "Twist density",
                10.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.twist_density = v,
            ),
            float_slider::<Self>(
                "fiber_detail",
                "Fiber detail",
                0.0,
                100.0,
                55.0,
                |e: &mut Self, v| e.fiber_detail = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const SEED: i32 = 8271;
        let border = self.border_size.clamp(12, 200);
        let twist = clampf(self.twist_density, 10.0, 100.0) / 1000.0;
        let fiber = clampf(self.fiber_detail, 0.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let rc = self.rope_color;
        let (rr, rg, rb) = (
            rc.r as f32 / 255.0,
            rc.g as f32 / 255.0,
            rc.b as f32 / 255.0,
        );
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        let radius = border as f32 * 0.42;
        let pi = std::f32::consts::PI;
        for y in 0..nh {
            let top = y < border;
            let bottom = y >= border + ih;
            for x in 0..nw {
                let left = x < border;
                let right = x >= border + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let center = border as f32 * 0.5;
                let (along, across, seed) = if top {
                    (x as f32, y as f32 - center, SEED ^ 0x1A)
                } else if bottom {
                    (
                        (nw - x) as f32,
                        -(((y - (nh - border)) as f32) - (border as f32 - center)),
                        SEED ^ 0x2B,
                    )
                } else if left {
                    ((nh - y) as f32, x as f32 - center, SEED ^ 0x3C)
                } else {
                    (
                        y as f32,
                        -(((x - (nw - border)) as f32) - (border as f32 - center)),
                        SEED ^ 0x4D,
                    )
                };
                let mut best = f32::MAX;
                let mut best_shade = 0f32;
                for s in 0..3 {
                    let phase = s as f32 * (pi * 2.0 / 3.0);
                    let off = (along * twist * pi * 2.0 + phase).sin() * radius * 0.45;
                    let d = (across - off).abs();
                    let sr = radius * 0.38;
                    if d < sr && d < best {
                        best = d;
                        let n = d / sr;
                        best_shade = 1.0 - n * n;
                    }
                }
                let i = (y * nw + x) as usize;
                if best < radius * 0.38 {
                    let fn_ = if fiber > 0.0 {
                        (hash01((along * 1.5) as i32, (across * 3.0) as i32, seed) * 2.0 - 1.0)
                            * fiber
                            * 0.18
                    } else {
                        0.0
                    };
                    let th = (along * twist * pi * 2.0).cos() * 0.08;
                    let shade = best_shade * 0.35 + th + fn_;
                    dst[i] = SkColor::rgba(
                        clamp_to_byte((rr + shade) * 255.0),
                        clamp_to_byte((rg + shade * 0.8) * 255.0),
                        clamp_to_byte((rb + shade * 0.5) * 255.0),
                        255,
                    );
                } else {
                    dst[i] = self.bg_color;
                }
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        draw_bitmap(&mut out, source, border as f32, border as f32);
        Ok(out)
    }
}
