//! `CandyCaneBorderImageEffect` (`Core/ImageEffects/Filters/CandyCaneBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `CandyCaneBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct CandyCaneBorderImageEffect {
    pub border_width: i32,
    pub stripe_width: f32,
    pub color_a: SkColor,
    pub color_b: SkColor,
    pub gloss: f32,
    pub rounded_corners: bool,
}

impl Default for CandyCaneBorderImageEffect {
    fn default() -> Self {
        CandyCaneBorderImageEffect {
            border_width: 32,
            stripe_width: 12.0,
            color_a: SkColor::argb(0xFFDC1414),
            color_b: SkColor::argb(0xFFFFFFFF),
            gloss: 50.0,
            rounded_corners: true,
        }
    }
}

impl EffectImpl for CandyCaneBorderImageEffect {
    fn info() -> Info {
        Info::new("candy_cane_border", "Candy Cane Border", EffectCategory::Drawings, "image-effect/candy-cane")
            .desc("Adds a candy cane striped border around the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("border_width", "Border Width", 1, 200, 10, |e: &mut Self, v| e.border_width = v),
            float_slider::<Self>("stripe_width", "Stripe Width", 2.0, 60.0, 12.0, |e: &mut Self, v| e.stripe_width = v),
            color_param::<Self>("color_a", "Color A", SkColor::argb(0xFFDC1414), |e: &mut Self, v| e.color_a = v),
            color_param::<Self>("color_b", "Color B", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.color_b = v),
            float_slider::<Self>("gloss", "Gloss", 0.0, 100.0, 50.0, |e: &mut Self, v| e.gloss = v),
            bool_param::<Self>("rounded_corners", "Rounded Corners", true, |e: &mut Self, v| e.rounded_corners = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
