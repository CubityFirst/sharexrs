//! `MinecraftGrassImageEffect` (`Core/ImageEffects/Filters/MinecraftGrassImageEffect.cs`).

use crate::effects::prelude::*;

/// `MinecraftGrassImageEffect`.
#[derive(Clone, Debug)]
pub struct MinecraftGrassImageEffect {
    pub block_size: i32,
    pub grass_rows: i32,
    pub dirt_rows: i32,
    pub top_edge: bool,
    pub overlap: i32,
    pub grass_color: SkColor,
    pub dirt_color: SkColor,
}

impl Default for MinecraftGrassImageEffect {
    fn default() -> Self {
        MinecraftGrassImageEffect {
            block_size: 16,
            grass_rows: 2,
            dirt_rows: 4,
            top_edge: false,
            overlap: 1,
            grass_color: SkColor::argb(0xFF59B030),
            dirt_color: SkColor::argb(0xFF866043),
        }
    }
}

impl EffectImpl for MinecraftGrassImageEffect {
    fn info() -> Info {
        Info::new("minecraft_grass", "Minecraft grass", EffectCategory::Filters, "image-effect/grass")
            .desc("Adds a Minecraft-style pixelated grass border to the top or bottom of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("block_size", "Block size", 4, 64, 16, |e: &mut Self, v| e.block_size = v),
            int_slider::<Self>("grass_rows", "Grass rows", 1, 8, 2, |e: &mut Self, v| e.grass_rows = v),
            int_slider::<Self>("dirt_rows", "Dirt rows", 1, 16, 4, |e: &mut Self, v| e.dirt_rows = v),
            bool_param::<Self>("top_edge", "Top edge", false, |e: &mut Self, v| e.top_edge = v),
            int_slider::<Self>("overlap", "Overlap", 0, 8, 1, |e: &mut Self, v| e.overlap = v),
            color_param::<Self>("grass_color", "Grass color", SkColor::argb(0xFF59B030), |e: &mut Self, v| e.grass_color = v),
            color_param::<Self>("dirt_color", "Dirt color", SkColor::argb(0xFF866043), |e: &mut Self, v| e.dirt_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let bs = self.block_size.clamp(4, 64);
        let grass_rows = self.grass_rows.clamp(1, 8);
        let dirt_rows = self.dirt_rows.clamp(1, 16);
        let total = grass_rows + dirt_rows;
        let bh = total * bs;
        let overlap = self.overlap.clamp(0, 8) * bs;
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let nh = sh + bh - overlap;
        let mut out = Bitmap::new(sw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let image_y = if self.top_edge { bh - overlap } else { 0 };
        canvas.draw_bitmap(source, 0.0, image_y as f32, None);
        let border_y = if self.top_edge { 0 } else { sh - overlap };
        let cols = (sw + bs - 1) / bs;
        let fill = |canvas: &mut Canvas, c: SkColor, x: f32, y: f32, w: f32, h: f32| {
            let mut p = Paint::fill(c.rgba_array());
            p.anti_alias = false;
            canvas.draw_rect(RRect::xywh(x, y, w, h), &p);
        };
        for col in 0..cols {
            let jagged = hash01(col, 0, 42) > 0.5;
            for row in 0..total {
                let x = col * bs;
                let y = if self.top_edge { bh - (row + 1) * bs } else { border_y + row * bs };
                let from_top = if self.top_edge { total - 1 - row } else { row };
                if from_top == 0 && jagged {
                    continue;
                }
                let variation = hash01(col, row, 137);
                let g = self.grass_color;
                let d = self.dirt_color;
                let cl = |v: i32| v.clamp(0, 255) as u8;
                let base = if from_top < grass_rows {
                    let rv = (variation * 20.0 - 10.0) as i32;
                    let gv = (variation * 30.0 - 15.0) as i32;
                    SkColor::rgba(cl(g.r as i32 + rv), cl(g.g as i32 + gv), cl(g.b as i32 + rv), 255)
                } else {
                    let dv = (variation * 24.0 - 12.0) as i32;
                    SkColor::rgba(cl(d.r as i32 + dv), cl(d.g as i32 + dv), cl(d.b as i32 + dv + 4), 255)
                };
                let draw_w = bs.min(sw - x);
                if draw_w <= 0 {
                    continue;
                }
                fill(&mut canvas, base, x as f32, y as f32, draw_w as f32, bs as f32);
                if hash01(col, row + 100, 271) > 0.6 {
                    let ss = 1.max(bs / 4);
                    let sx = x as f32 + hash01(col, row, 311) * (bs - ss) as f32;
                    let sy = y as f32 + hash01(col, row, 419) * (bs - ss) as f32;
                    let dark = SkColor::rgba(cl(base.r as i32 - 25), cl(base.g as i32 - 20), cl(base.b as i32 - 20), 255);
                    fill(&mut canvas, dark, sx, sy, ss as f32, ss as f32);
                }
            }
        }
        drop(canvas);
        Ok(out)
    }
}
