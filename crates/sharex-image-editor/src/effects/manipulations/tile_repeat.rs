//! `TileRepeatImageEffect` (`Core/ImageEffects/Manipulations/TileRepeatImageEffect.cs`).

use crate::effects::prelude::*;

/// `TileRepeatImageEffect`.
#[derive(Clone, Debug)]
pub struct TileRepeatImageEffect {
    pub columns: i32,
    pub rows: i32,
}

impl Default for TileRepeatImageEffect {
    fn default() -> Self {
        TileRepeatImageEffect {
            columns: 3,
            rows: 3,
        }
    }
}

impl EffectImpl for TileRepeatImageEffect {
    fn info() -> Info {
        Info::new("tile_repeat", "Tile repeat", EffectCategory::Manipulations, "app/split")
            .desc("Tiles the image into a grid of smaller copies.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("columns", "Columns", 2, 10, 3, |e: &mut Self, v| e.columns = v),
            int_slider::<Self>("rows", "Rows", 2, 10, 3, |e: &mut Self, v| e.rows = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let cols = 1.max(self.columns);
        let rows = 1.max(self.rows);
        let (w, h) = (source.width() as i32, source.height() as i32);
        let tile_w = w as f32 / cols as f32;
        let tile_h = h as f32 / rows as f32;
        let mut out = Bitmap::new(w as u32, h as u32);
        {
            let mut canvas = Canvas::new(&mut out);
            let full = RRect::ltrb(0.0, 0.0, w as f32, h as f32);
            for r in 0..rows {
                for c in 0..cols {
                    let dst = RRect::ltrb(
                        c as f32 * tile_w,
                        r as f32 * tile_h,
                        (c + 1) as f32 * tile_w,
                        (r + 1) as f32 * tile_h,
                    );
                    canvas.draw_image_rect(source, Some(full), dst, Sampling::Nearest, None);
                }
            }
        }
        Ok(out)
    }
}
