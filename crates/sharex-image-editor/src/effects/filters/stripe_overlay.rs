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
        Info::new(
            "stripe_overlay",
            "Stripe overlay",
            EffectCategory::Filters,
            "image-effect/blinds",
        )
        .desc("Overlays diagonal stripes on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("width", "Stripe width", 2, 50, 10, |e: &mut Self, v| {
                e.stripe_width = v
            }),
            int_slider::<Self>("gap", "Gap width", 2, 50, 10, |e: &mut Self, v| {
                e.gap_width = v
            }),
            int_slider::<Self>("angle", "Angle", 0, 180, 45, |e: &mut Self, v| e.angle = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 40.0, |e: &mut Self, v| {
                e.opacity = v
            }),
            color_param::<Self>(
                "color",
                "Color",
                SkColor::argb(0xFF000000),
                |e: &mut Self, v| e.color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let alpha = clampf(self.opacity / 100.0, 0.0, 1.0);
        if alpha <= 0.0 {
            return Ok(source.clone());
        }
        let period = 4.max(self.stripe_width.wrapping_add(self.gap_width));
        let (w, h) = (source.width() as i32, source.height() as i32);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let a = (self.color.a as f32 * alpha) as u8;
        let paint = solid(self.color.with_alpha(a), true);
        let diagonal = ((w * w + h * h) as f32).sqrt();
        let count = (diagonal / period as f32) as i32 + 2;
        sk::rotate_about(
            &mut canvas,
            self.angle as f32,
            w as f32 / 2.0,
            h as f32 / 2.0,
        );
        let start_x = -diagonal / 2.0 + w as f32 / 2.0;
        let start_y = -diagonal / 2.0 + h as f32 / 2.0;
        for i in 0..count {
            let x = start_x + (i * period) as f32;
            sk::draw_rect_xywh(
                &mut canvas,
                x,
                start_y,
                self.stripe_width as f32,
                diagonal * 2.0,
                &paint,
                None,
            );
        }
        drop(canvas);
        Ok(out)
    }
}
