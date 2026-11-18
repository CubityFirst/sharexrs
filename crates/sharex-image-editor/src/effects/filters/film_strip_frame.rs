//! `FilmStripFrameImageEffect` (`Core/ImageEffects/Filters/FilmStripFrameImageEffect.cs`).

use crate::effects::prelude::*;

/// `FilmStripFrameImageEffect`.
#[derive(Clone, Debug)]
pub struct FilmStripFrameImageEffect {
    pub border_size: i32,
    pub hole_size: f32,
    pub frame_color: SkColor,
    pub side_strips: bool,
    pub scratches: bool,
}

impl Default for FilmStripFrameImageEffect {
    fn default() -> Self {
        FilmStripFrameImageEffect {
            border_size: 40,
            hole_size: 55.0,
            frame_color: SkColor::argb(0xFF0F0F0F),
            side_strips: false,
            scratches: true,
        }
    }
}

impl EffectImpl for FilmStripFrameImageEffect {
    fn info() -> Info {
        Info::new("film_strip_frame", "Film strip frame", EffectCategory::Filters, "app/film")
            .desc("Wraps the image in a cinema film strip frame with sprocket holes.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("border_size", "Border size", 10, 120, 40, |e: &mut Self, v| e.border_size = v),
            float_slider::<Self>("hole_size", "Hole size", 20.0, 80.0, 55.0, |e: &mut Self, v| e.hole_size = v),
            color_param::<Self>("frame_color", "Frame color", SkColor::argb(0xFF0F0F0F), |e: &mut Self, v| e.frame_color = v),
            bool_param::<Self>("side_strips", "Side strips", false, |e: &mut Self, v| e.side_strips = v),
            bool_param::<Self>("scratches", "Scratches", true, |e: &mut Self, v| e.scratches = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
