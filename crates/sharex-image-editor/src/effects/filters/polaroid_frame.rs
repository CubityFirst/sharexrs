//! `PolaroidFrameImageEffect` (`Core/ImageEffects/Filters/PolaroidFrameImageEffect.cs`).

use crate::effects::prelude::*;

/// `PolaroidFrameImageEffect`.
#[derive(Clone, Debug)]
pub struct PolaroidFrameImageEffect {
    pub border_size: i32,
    pub bottom_size: i32,
    pub rotation: f32,
    pub frame_color: SkColor,
    pub caption: String,
    pub shadow_size: i32,
}

impl Default for PolaroidFrameImageEffect {
    fn default() -> Self {
        PolaroidFrameImageEffect {
            border_size: 30,
            bottom_size: 80,
            rotation: 0.0,
            frame_color: SkColor::argb(0xFFFAF9F5),
            caption: "".to_string(),
            shadow_size: 15,
        }
    }
}

impl EffectImpl for PolaroidFrameImageEffect {
    fn info() -> Info {
        Info::new("polaroid_frame", "Polaroid frame", EffectCategory::Filters, "app/camera")
            .desc("Wraps the image in a Polaroid-style instant photo frame.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("border_size", "Border size", 5, 100, 30, |e: &mut Self, v| e.border_size = v),
            int_slider::<Self>("bottom_size", "Bottom size", 20, 200, 80, |e: &mut Self, v| e.bottom_size = v),
            float_slider::<Self>("rotation", "Rotation", -30.0, 30.0, 0.0, |e: &mut Self, v| e.rotation = v),
            color_param::<Self>("frame_color", "Frame color", SkColor::argb(0xFFFAF9F5), |e: &mut Self, v| e.frame_color = v),
            text_param::<Self>("caption", "Caption", "", |e: &mut Self, v| e.caption = v),
            int_slider::<Self>("shadow_size", "Shadow size", 0, 60, 15, |e: &mut Self, v| e.shadow_size = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
