//! `WatercolorBorderImageEffect` (`Core/ImageEffects/Filters/WatercolorBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `WatercolorBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct WatercolorBorderImageEffect {
    pub border_size: i32,
    pub wash_color: SkColor,
    pub edge_roughness: f32,
    pub bleed_amount: f32,
    pub opacity: f32,
}

impl Default for WatercolorBorderImageEffect {
    fn default() -> Self {
        WatercolorBorderImageEffect {
            border_size: 60,
            wash_color: SkColor::argb(0xFFB9A082),
            edge_roughness: 65.0,
            bleed_amount: 40.0,
            opacity: 90.0,
        }
    }
}

impl EffectImpl for WatercolorBorderImageEffect {
    fn info() -> Info {
        Info::new("watercolor_border", "Watercolor border", EffectCategory::Filters, "image-effect/droplets")
            .desc("Adds a soft watercolor wash border with organic, painted edges that bleed into the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                10,
                300,
                60,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "wash_color",
                "Wash color",
                SkColor::argb(0xFFB9A082),
                |e: &mut Self, v| e.wash_color = v,
            ),
            float_slider::<Self>(
                "edge_roughness",
                "Edge roughness",
                0.0,
                100.0,
                65.0,
                |e: &mut Self, v| e.edge_roughness = v,
            ),
            float_slider::<Self>(
                "bleed_amount",
                "Bleed amount",
                0.0,
                100.0,
                40.0,
                |e: &mut Self, v| e.bleed_amount = v,
            ),
            float_slider::<Self>(
                "opacity",
                "Opacity",
                10.0,
                100.0,
                90.0,
                |e: &mut Self, v| e.opacity = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(10, 300);
        let roughness = clampf(self.edge_roughness, 0.0, 100.0) / 100.0;
        let bleed = clampf(self.bleed_amount, 0.0, 100.0) / 100.0;
        let opacity = clampf(self.opacity, 10.0, 100.0) / 100.0;
        let bleed_px = (border as f32 * bleed * 0.5) as i32;
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (sw + border * 2, sh + border * 2);
        let wc = self.wash_color;
        let (wr, wg, wb) = (
            wc.r as f32 / 255.0,
            wc.g as f32 / 255.0,
            wc.b as f32 / 255.0,
        );
        let mut dst = vec![SkColor::default(); (nw * nh) as usize];
        let src = pixels(source);
        for y in 0..nh {
            for x in 0..nw {
                let (sx, sy) = (x - border, y - border);
                let dl = (border - x + bleed_px) as f32;
                let dt = (border - y + bleed_px) as f32;
                let dr = (x - (border + sw - 1 - bleed_px)) as f32;
                let db = (y - (border + sh - 1 - bleed_px)) as f32;
                let max_d = dl.max(dt).max(dr.max(db));
                let i = (y * nw + x) as usize;
                if max_d <= 0.0 {
                    dst[i] = src[(sy * sw + sx) as usize];
                    continue;
                }
                let cross = clampf(max_d / (border + bleed_px) as f32, 0.0, 1.0);
                let noise = fractal_noise(x as f32 * 0.015, y as f32 * 0.015, 4, 2.0, 0.55, 37);
                let rough = roughness * (noise * 2.0 - 1.0) * 0.35;
                let t = clampf(cross + rough, 0.0, 1.0);
                let mut wa = smooth_step(0.0, 0.7, t) * opacity;
                let pn = fractal_noise(x as f32 * 0.04, y as f32 * 0.04, 3, 2.0, 0.4, 99);
                wa *= 0.85 + pn * 0.3;
                wa = clampf(wa, 0.0, 1.0);
                dst[i] = if wa >= 0.999 {
                    let shade = pn * 0.12 - 0.06;
                    SkColor::rgba(
                        clamp_to_byte((wr + shade) * 255.0),
                        clamp_to_byte((wg + shade) * 255.0),
                        clamp_to_byte((wb + shade) * 255.0),
                        255,
                    )
                } else if sx >= 0 && sx < sw && sy >= 0 && sy < sh {
                    let sc = src[(sy * sw + sx) as usize];
                    let sa = sc.a as f32 / 255.0 * (1.0 - wa);
                    let shade = pn * 0.10 - 0.05;
                    SkColor::rgba(
                        clamp_to_byte(lerp(sc.r as f32 / 255.0, wr + shade, wa) * 255.0),
                        clamp_to_byte(lerp(sc.g as f32 / 255.0, wg + shade, wa) * 255.0),
                        clamp_to_byte(lerp(sc.b as f32 / 255.0, wb + shade, wa) * 255.0),
                        clamp_to_byte((sa + wa) * 255.0),
                    )
                } else {
                    let shade = pn * 0.12 - 0.06;
                    SkColor::rgba(
                        clamp_to_byte((wr + shade) * 255.0),
                        clamp_to_byte((wg + shade) * 255.0),
                        clamp_to_byte((wb + shade) * 255.0),
                        clamp_to_byte(wa * 255.0),
                    )
                };
            }
        }
        Ok(from_pixels(nw as u32, nh as u32, &dst))
    }
}
