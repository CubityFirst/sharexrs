//! `RoundedCornersImageEffect` (`Core/ImageEffects/Manipulations/RoundedCornersImageEffect.cs`).

use crate::effects::prelude::*;

/// `RoundedCornersImageEffect`.
#[derive(Clone, Debug)]
pub struct RoundedCornersImageEffect {
    pub corner_radius: i32,
}

impl Default for RoundedCornersImageEffect {
    fn default() -> Self {
        RoundedCornersImageEffect {
            corner_radius: 20,
        }
    }
}

impl EffectImpl for RoundedCornersImageEffect {
    fn info() -> Info {
        Info::new("rounded_corners", "Rounded Corners", EffectCategory::Manipulations, "image-effect/rounded-corner")
            .desc("Rounds the corners of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("corner_radius", "Corner radius", 0, 500, 20, |e: &mut Self, v| e.corner_radius = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.corner_radius <= 0 {
            return Ok(source.clone());
        }
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        let mut clip = SkPath::new();
        let r = self.corner_radius as f32;
        clip.add_round_rect(
            sharex_skia::Rect::from_ltrb(0.0, 0.0, source.width() as f32, source.height() as f32),
            r,
            r,
            sharex_skia::Direction::Cw,
        );
        sk::with_clip_path_aa(&mut canvas, &clip, |canvas| canvas.draw_bitmap(source, 0.0, 0.0, None));
        drop(canvas);
        Ok(out)
    }
}
