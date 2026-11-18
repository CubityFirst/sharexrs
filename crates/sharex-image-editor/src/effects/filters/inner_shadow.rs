//! `InnerShadowImageEffect` (`Core/ImageEffects/Filters/InnerShadowImageEffect.cs`).

use crate::effects::prelude::*;

/// `InnerShadowImageEffect`.
#[derive(Clone, Debug)]
pub struct InnerShadowImageEffect {
    pub opacity: f32,
    pub size: i32,
    pub color: SkColor,
    pub offset_x: i32,
    pub offset_y: i32,
}

impl Default for InnerShadowImageEffect {
    fn default() -> Self {
        InnerShadowImageEffect {
            opacity: 70.0,
            size: 20,
            color: SkColor::argb(0xFF000000),
            offset_x: 0,
            offset_y: 0,
        }
    }
}

impl EffectImpl for InnerShadowImageEffect {
    fn info() -> Info {
        Info::new("inner_shadow", "Inner shadow", EffectCategory::Filters, "image-effect/square-dashed")
            .desc("Adds an inner shadow along the edges of opaque regions.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 70.0, |e: &mut Self, v| e.opacity = v),
            int_slider::<Self>("size", "Size", 0, 300, 20, |e: &mut Self, v| e.size = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.color = v),
            int_numeric::<Self>("offset_x", "Offset X", -300, 300, 0, |e: &mut Self, v| e.offset_x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -300, 300, 0, |e: &mut Self, v| e.offset_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let opacity = clampf(self.opacity, 0.0, 100.0) / 100.0;
        let size = self.size.clamp(0, 300);
        if opacity <= 0.0 || (size <= 0 && self.offset_x == 0 && self.offset_y == 0) || self.color.a == 0 {
            return Ok(source.clone());
        }
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (w, h) = (source.width(), source.height());
        let src = pixels(source);
        let mask: Vec<SkColor> = src.iter().map(|c| SkColor::rgba(c.a, c.a, c.a, c.a)).collect();
        let mask_bmp = from_pixels(w, h, &mask);
        let mut shifted = Bitmap::new(w, h);
        draw_bitmap(&mut shifted, &mask_bmp, -self.offset_x as f32, -self.offset_y as f32);
        let blurred = if size > 0 {
            let radius = size as f32 / 2.0;
            if radius <= 0.01 {
                shifted.clone()
            } else {
                let padding = 2.max((radius * 2.0).ceil() as i32);
                blur_transparent_padded(&shifted, padding, 0.001f32.max(radius / 3.0))
            }
        } else {
            shifted.clone()
        };
        let bp = pixels(&blurred);
        let ca = self.color.a as f32 / 255.0;
        let c = self.color;
        let dst: Vec<SkColor> = src
            .iter()
            .zip(&bp)
            .map(|(s, b)| {
                let sa = s.a as f32 / 255.0;
                if sa <= 0.0 {
                    return *s;
                }
                let inner = clamp01(sa - b.a as f32 / 255.0);
                let mix = inner * opacity * ca;
                SkColor::rgba(
                    clamp_to_byte(lerp(s.r as f32, c.r as f32, mix)),
                    clamp_to_byte(lerp(s.g as f32, c.g as f32, mix)),
                    clamp_to_byte(lerp(s.b as f32, c.b as f32, mix)),
                    s.a,
                )
            })
            .collect();
        Ok(from_pixels(w, h, &dst))
    }
}
