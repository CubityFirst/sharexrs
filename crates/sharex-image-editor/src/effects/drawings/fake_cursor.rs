//! `FakeCursorImageEffect` (`Core/ImageEffects/Drawings/FakeCursorImageEffect.cs`).

use crate::effects::prelude::*;

/// `FakeCursorImageEffect`.
#[derive(Clone, Debug)]
pub struct FakeCursorImageEffect {
    pub count: i32,
    pub scale: f32,
    pub random_size: bool,
    pub random_size_min: f32,
    pub random_size_max: f32,
    pub random_angle: bool,
    pub random_angle_min: i32,
    pub random_angle_max: i32,
    pub fill_color: SkColor,
    pub border_color: SkColor,
}

impl Default for FakeCursorImageEffect {
    fn default() -> Self {
        FakeCursorImageEffect {
            count: 1,
            scale: 1.0,
            random_size: false,
            random_size_min: 0.5,
            random_size_max: 2.0,
            random_angle: false,
            random_angle_min: 0,
            random_angle_max: 360,
            fill_color: SkColor::argb(0xFFFFFFFF),
            border_color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for FakeCursorImageEffect {
    fn info() -> Info {
        Info::new("fake_cursor", "Fake cursor", EffectCategory::Drawings, "image-editor/cursor")
            .desc("Draws fake mouse cursors at random positions on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("count", "Count", 1, 200, 1, |e: &mut Self, v| e.count = v),
            float_slider::<Self>("scale", "Scale", 0.5, 5.0, 1.0, |e: &mut Self, v| e.scale = v).tick(0.5),
            bool_param::<Self>("random_size", "Random size", false, |e: &mut Self, v| e.random_size = v),
            float_slider::<Self>("random_size_min", "Random size min", 0.5, 5.0, 0.5, |e: &mut Self, v| e.random_size_min = v).tick(0.5),
            float_slider::<Self>("random_size_max", "Random size max", 0.5, 5.0, 2.0, |e: &mut Self, v| e.random_size_max = v).tick(0.5),
            bool_param::<Self>("random_angle", "Random angle", false, |e: &mut Self, v| e.random_angle = v),
            int_numeric::<Self>("random_angle_min", "Random angle min", 0, 360, 0, |e: &mut Self, v| e.random_angle_min = v),
            int_numeric::<Self>("random_angle_max", "Random angle max", 0, 360, 360, |e: &mut Self, v| e.random_angle_max = v),
            color_param::<Self>("fill_color", "Fill color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.fill_color = v),
            color_param::<Self>("border_color", "Border color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.border_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
