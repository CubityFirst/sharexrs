//! `StripeOverlayImageEffect` (`Core/ImageEffects/Filters/StripeOverlayImageEffect.cs`).

use crate::effects::prelude::*;

/// `StripeOverlayImageEffect`.
#[derive(Clone, Debug)]
pub struct StripeOverlayImageEffect {
    pub stripe_width: i32,
    pub gap_width: i32,
    pub angle: i32,
    pub opacity: f32,
    pub color: SkColor,
}

impl Default for StripeOverlayImageEffect {
    fn default() -> Self {
        StripeOverlayImageEffect {
            stripe_width: 10,
            gap_width: 10,
            angle: 45,
            opacity: 40.0,
            color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for StripeOverlayImageEffect {
    fn info() -> Info {
        Info::new("stripe_overlay", "Stripe overlay", EffectCategory::Filters, "image-effect/blinds")
            .desc("Overlays diagonal stripes on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("width", "Stripe width", 2, 50, 10, |e: &mut Self, v| e.stripe_width = v),
            int_slider::<Self>("gap", "Gap width", 2, 50, 10, |e: &mut Self, v| e.gap_width = v),
            int_slider::<Self>("angle", "Angle", 0, 180, 45, |e: &mut Self, v| e.angle = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 40.0, |e: &mut Self, v| e.opacity = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
