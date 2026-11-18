//! `BrickWallBorderImageEffect` (`Core/ImageEffects/Filters/BrickWallBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `BrickWallBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct BrickWallBorderImageEffect {
    pub border_size: i32,
    pub brick_color: SkColor,
    pub mortar_color: SkColor,
    pub mortar_thickness: f32,
    pub color_variation: f32,
    pub bevel_strength: f32,
}

impl Default for BrickWallBorderImageEffect {
    fn default() -> Self {
        BrickWallBorderImageEffect {
            border_size: 56,
            brick_color: SkColor::argb(0xFFB25A3E),
            mortar_color: SkColor::argb(0xFFC3BCAF),
            mortar_thickness: 4.0,
            color_variation: 40.0,
            bevel_strength: 50.0,
        }
    }
}

impl EffectImpl for BrickWallBorderImageEffect {
    fn info() -> Info {
        Info::new("brick_wall_border", "Brick wall border", EffectCategory::Filters, "image-effect/brick-wall")
            .desc("Adds a procedural brick wall texture border with mortar lines, shading, and color variation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                10,
                300,
                56,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "brick_color",
                "Brick color",
                SkColor::argb(0xFFB25A3E),
                |e: &mut Self, v| e.brick_color = v,
            ),
            color_param::<Self>(
                "mortar_color",
                "Mortar color",
                SkColor::argb(0xFFC3BCAF),
                |e: &mut Self, v| e.mortar_color = v,
            ),
            float_slider::<Self>(
                "mortar_thickness",
                "Mortar thickness",
                1.0,
                20.0,
                4.0,
                |e: &mut Self, v| e.mortar_thickness = v,
            ),
            float_slider::<Self>(
                "color_variation",
                "Color variation",
                0.0,
                100.0,
                40.0,
                |e: &mut Self, v| e.color_variation = v,
            ),
            float_slider::<Self>(
                "bevel_strength",
                "Bevel strength",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.bevel_strength = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const SEED: i32 = 3271;
        let border = self.border_size.clamp(10, 300);
        let mortar_t = clampf(self.mortar_thickness, 1.0, 20.0);
        let color_var = clampf(self.color_variation, 0.0, 100.0) / 100.0;
        let bevel = clampf(self.bevel_strength, 0.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut brick_h = border as f32 * 0.45;
        brick_h = brick_h.max(mortar_t * 2.0 + 4.0);
        let brick_w = brick_h * 2.4;
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        let bc = self.brick_color;
        for py in 0..nh {
            let top = py < border;
            let bottom = py >= border + ih;
            for px in 0..nw {
                let left = px < border;
                let right = px >= border + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let (lx, ly) = if top {
                    (px as f32, py as f32)
                } else if bottom {
                    ((nw - 1 - px) as f32, (nh - 1 - py) as f32)
                } else if left {
                    (py as f32, px as f32)
                } else {
                    ((nh - 1 - py) as f32, (nw - 1 - px) as f32)
                };
                let row = (ly / brick_h) as i32;
                let row_frac = (ly % brick_h) / brick_h;
                let offset = if row % 2 == 0 { 0.0 } else { brick_w * 0.5 };
                let col = ((lx + offset) / brick_w) as i32;
                let col_frac = ((lx + offset) % brick_w) / brick_w;
                let mfh = mortar_t / brick_h;
                let mfw = mortar_t / brick_w;
                let i = (py * nw + px) as usize;
                if row_frac < mfh || col_frac < mfw {
                    dst[i] = self.mortar_color;
                    continue;
                }
                let vh = hash01(col, row ^ (row.wrapping_mul(31).wrapping_add(SEED)), SEED);
                let vh2 = hash01(col ^ col.wrapping_mul(13), row, SEED ^ 0x5A);
                let red_var = (vh - 0.5) * color_var * 0.5;
                let green_var = (vh2 - 0.5) * color_var * 0.25;
                let bevel_frac = (row_frac - mfh)
                    .min(1.0 - row_frac)
                    .min((col_frac - mfw).min(1.0 - col_frac));
                let bevel_shade = if bevel > 0.0 {
                    (1.0 - smooth_step(0.0, 0.18, bevel_frac)) * bevel * 0.35
                } else {
                    0.0
                };
                let noise = hash01((lx * 2.0) as i32, (ly * 2.0) as i32, SEED ^ 0x99) * 0.08 - 0.04;
                let r = bc.r as f32 / 255.0 + red_var + noise - bevel_shade;
                let g = bc.g as f32 / 255.0 + green_var * 0.5 + noise * 0.4 - bevel_shade;
                let b = bc.b as f32 / 255.0 - red_var * 0.2 - bevel_shade;
                dst[i] = SkColor::rgba(
                    clamp_to_byte(r * 255.0),
                    clamp_to_byte(g * 255.0),
                    clamp_to_byte(b * 255.0),
                    255,
                );
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        canvas.draw_bitmap(source, border as f32, border as f32, None);
        let mut paint = Paint::fill(self.mortar_color.rgba_array());
        paint.anti_alias = false;
        let b = border as f32 - mortar_t * 0.5;
        canvas.stroke_rect(
            RRect::xywh(b, b, iw as f32 + mortar_t, ih as f32 + mortar_t),
            mortar_t,
            &paint,
        );
        Ok(out)
    }
}
