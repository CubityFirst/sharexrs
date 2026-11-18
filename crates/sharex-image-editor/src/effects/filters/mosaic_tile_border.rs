//! `MosaicTileBorderImageEffect` (`Core/ImageEffects/Filters/MosaicTileBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `MosaicTileBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct MosaicTileBorderImageEffect {
    pub border_size: i32,
    pub tile_size: f32,
    pub grout_color: SkColor,
    pub grout_width: f32,
    pub color_variety: f32,
    pub saturation: f32,
}

impl Default for MosaicTileBorderImageEffect {
    fn default() -> Self {
        MosaicTileBorderImageEffect {
            border_size: 48,
            tile_size: 14.0,
            grout_color: SkColor::argb(0xFFD2CDC3),
            grout_width: 2.0,
            color_variety: 70.0,
            saturation: 85.0,
        }
    }
}

impl EffectImpl for MosaicTileBorderImageEffect {
    fn info() -> Info {
        Info::new("mosaic_tile_border", "Mosaic tile border", EffectCategory::Filters, "app/grid")
            .desc("Adds a colorful mosaic tile border with individual square tiles, grout lines, and subtle per-tile variation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                12,
                300,
                48,
                |e: &mut Self, v| e.border_size = v,
            ),
            float_slider::<Self>(
                "tile_size",
                "Tile size",
                4.0,
                60.0,
                14.0,
                |e: &mut Self, v| e.tile_size = v,
            ),
            color_param::<Self>(
                "grout_color",
                "Grout color",
                SkColor::argb(0xFFD2CDC3),
                |e: &mut Self, v| e.grout_color = v,
            ),
            float_slider::<Self>(
                "grout_width",
                "Grout width",
                0.5,
                8.0,
                2.0,
                |e: &mut Self, v| e.grout_width = v,
            ),
            float_slider::<Self>(
                "color_variety",
                "Color variety",
                0.0,
                100.0,
                70.0,
                |e: &mut Self, v| e.color_variety = v,
            ),
            float_slider::<Self>(
                "saturation",
                "Saturation",
                0.0,
                100.0,
                85.0,
                |e: &mut Self, v| e.saturation = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const SEED: i32 = 5923;
        let border = self.border_size.clamp(12, 300);
        let tile = clampf(self.tile_size, 4.0, 60.0);
        let grout_w = clampf(self.grout_width, 0.5, 8.0);
        let variety = clampf(self.color_variety, 0.0, 100.0) / 100.0;
        let sat = clampf(self.saturation, 0.0, 100.0);
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        let half = grout_w * 0.5;
        for y in 0..nh {
            let top = y < border;
            let bottom = y >= border + ih;
            for x in 0..nw {
                let left = x < border;
                let right = x >= border + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let col = (x as f32 / tile).floor() as i32;
                let row = (y as f32 / tile).floor() as i32;
                let mut wx = x as f32 % tile;
                let mut wy = y as f32 % tile;
                if wx < 0.0 {
                    wx += tile;
                }
                if wy < 0.0 {
                    wy += tile;
                }
                let i = (y * nw + x) as usize;
                if wx < half || wx > tile - half || wy < half || wy > tile - half {
                    dst[i] = self.grout_color;
                    continue;
                }
                let hue = hash01(col, row, SEED) * 360.0 * variety;
                let bh = hash01(col ^ 0x55, row ^ 0xAA, SEED ^ 0x17);
                let tc = SkColor::from_hsv(hue, sat, 55.0 + bh * 40.0, 255);
                let dist = (wx - half)
                    .min(tile - half - wx)
                    .min((wy - half).min(tile - half - wy));
                let bevel = smooth_step(tile * 0.15, 0.0, dist) * 0.2;
                let noise = (hash01(x, y, SEED ^ 0x99) - 0.5) * 0.06;
                let f = |v: u8| clamp_to_byte((v as f32 / 255.0 - bevel + noise) * 255.0);
                dst[i] = SkColor::rgba(f(tc.r), f(tc.g), f(tc.b), 255);
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        canvas.draw_bitmap(source, border as f32, border as f32, None);
        let mut paint = Paint::fill(self.grout_color.rgba_array());
        paint.anti_alias = false;
        let b = border as f32 - grout_w * 0.5;
        canvas.stroke_rect(
            RRect::xywh(b, b, iw as f32 + grout_w, ih as f32 + grout_w),
            grout_w,
            &paint,
        );
        Ok(out)
    }
}
