//! `MirrorTilesImageEffect` (`Core/ImageEffects/Manipulations/MirrorTilesImageEffect.cs`).

use crate::effects::prelude::*;

/// `MirrorTilesImageEffect`.
#[derive(Clone, Debug)]
pub struct MirrorTilesImageEffect {
    pub columns: i32,
    pub rows: i32,
    pub mirror_alternate_columns: bool,
    pub mirror_alternate_rows: bool,
}

impl Default for MirrorTilesImageEffect {
    fn default() -> Self {
        MirrorTilesImageEffect {
            columns: 3,
            rows: 3,
            mirror_alternate_columns: true,
            mirror_alternate_rows: true,
        }
    }
}

impl EffectImpl for MirrorTilesImageEffect {
    fn info() -> Info {
        Info::new(
            "mirror_tiles",
            "Mirror tiles",
            EffectCategory::Manipulations,
            "image-effect/flip-horizontal",
        )
        .desc("Tiles the image with alternating mirrored reflections.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("columns", "Columns", 1, 16, 3, |e: &mut Self, v| {
                e.columns = v
            }),
            int_slider::<Self>("rows", "Rows", 1, 16, 3, |e: &mut Self, v| e.rows = v),
            bool_param::<Self>(
                "mirror_alternate_columns",
                "Mirror alternate columns",
                true,
                |e: &mut Self, v| e.mirror_alternate_columns = v,
            ),
            bool_param::<Self>(
                "mirror_alternate_rows",
                "Mirror alternate rows",
                true,
                |e: &mut Self, v| e.mirror_alternate_rows = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let columns = self.columns.clamp(1, 16);
        let rows = self.rows.clamp(1, 16);
        if columns == 1
            && rows == 1
            && !self.mirror_alternate_columns
            && !self.mirror_alternate_rows
        {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            let tiled_y = ((y as f32 + 0.5) / height as f32) * rows as f32;
            let tile_y = (rows - 1).min(0.max(tiled_y.floor() as i32));
            let mut local_y = tiled_y - tile_y as f32;
            if tile_y == rows - 1 {
                local_y = 1f32.min(local_y);
            }
            if self.mirror_alternate_rows && (tile_y & 1) == 1 {
                local_y = 1.0 - local_y;
            }
            let sample_y = local_y * 0.max(height - 1) as f32;
            for x in 0..width {
                let tiled_x = ((x as f32 + 0.5) / width as f32) * columns as f32;
                let tile_x = (columns - 1).min(0.max(tiled_x.floor() as i32));
                let mut local_x = tiled_x - tile_x as f32;
                if tile_x == columns - 1 {
                    local_x = 1f32.min(local_x);
                }
                if self.mirror_alternate_columns && (tile_x & 1) == 1 {
                    local_x = 1.0 - local_x;
                }
                let sample_x = local_x * 0.max(width - 1) as f32;
                dst[(row + x) as usize] = bilinear_sample(&src, width, height, sample_x, sample_y);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
