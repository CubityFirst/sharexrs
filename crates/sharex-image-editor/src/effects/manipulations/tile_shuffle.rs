//! `TileShuffleImageEffect` (`Core/ImageEffects/Manipulations/TileShuffleImageEffect.cs`).

use crate::effects::prelude::*;

/// `TileShuffleImageEffect`.
#[derive(Clone, Debug)]
pub struct TileShuffleImageEffect {
    pub columns: i32,
    pub rows: i32,
    pub seed: i32,
    pub gap: f32,
    pub gap_color: SkColor,
}

impl Default for TileShuffleImageEffect {
    fn default() -> Self {
        TileShuffleImageEffect {
            columns: 4,
            rows: 4,
            seed: 42,
            gap: 0.0,
            gap_color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for TileShuffleImageEffect {
    fn info() -> Info {
        Info::new(
            "tile_shuffle",
            "Tile Shuffle",
            EffectCategory::Manipulations,
            "image-effect/shuffle",
        )
        .desc("Divides the image into a grid and randomly rearranges the tiles.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("columns", "Columns", 2, 20, 4, |e: &mut Self, v| {
                e.columns = v
            }),
            int_slider::<Self>("rows", "Rows", 2, 20, 4, |e: &mut Self, v| e.rows = v),
            float_slider::<Self>("gap", "Gap", 0.0, 10.0, 0.0, |e: &mut Self, v| e.gap = v),
            color_param::<Self>(
                "gap_color",
                "Gap Color",
                SkColor::argb(0xFF000000),
                |e: &mut Self, v| e.gap_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let cols = self.columns.clamp(1, 100);
        let rows = self.rows.clamp(1, 100);
        let (width, height) = (source.width() as i32, source.height() as i32);
        let gap = clampf(self.gap, 0.0, 50.0);
        let total = (cols * rows) as usize;
        let mut indices: Vec<i32> = (0..total as i32).collect();
        let mut rng: i32 = if self.seed == 0 { 1 } else { self.seed };
        for i in (1..total).rev() {
            rng ^= rng.wrapping_shl(13);
            rng ^= rng >> 17;
            rng ^= rng.wrapping_shl(5);
            let j = ((rng & 0x7FFF_FFFF) as u32 % (i as u32 + 1)) as usize;
            indices.swap(i, j);
        }
        let tile_w = width as f32 / cols as f32;
        let tile_h = height as f32 / rows as f32;
        let mut out = Bitmap::new(width as u32, height as u32);
        {
            let mut canvas = Canvas::new(&mut out);
            canvas.clear(self.gap_color.rgba_array());
            for (dest, &src_idx) in indices.iter().enumerate() {
                let dest = dest as i32;
                let (src_col, src_row) = (src_idx % cols, src_idx / cols);
                let (dst_col, dst_row) = (dest % cols, dest / cols);
                let src_rect = RRect::ltrb(
                    src_col as f32 * tile_w,
                    src_row as f32 * tile_h,
                    (src_col + 1) as f32 * tile_w,
                    (src_row + 1) as f32 * tile_h,
                );
                let dst_rect = RRect::ltrb(
                    dst_col as f32 * tile_w + gap * 0.5,
                    dst_row as f32 * tile_h + gap * 0.5,
                    (dst_col + 1) as f32 * tile_w - gap * 0.5,
                    (dst_row + 1) as f32 * tile_h - gap * 0.5,
                );
                if dst_rect.width() <= 0.0 || dst_rect.height() <= 0.0 {
                    continue;
                }
                canvas.draw_image_rect(source, Some(src_rect), dst_rect, Sampling::Nearest, None);
            }
        }
        Ok(out)
    }
}
