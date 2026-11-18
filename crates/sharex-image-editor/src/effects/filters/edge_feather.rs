//! `EdgeFeatherImageEffect` (`Core/ImageEffects/Filters/EdgeFeatherImageEffect.cs`).

use crate::effects::prelude::*;

/// `EdgeFeatherImageEffect`.
#[derive(Clone, Debug)]
pub struct EdgeFeatherImageEffect {
    pub radius: f32,
}

impl Default for EdgeFeatherImageEffect {
    fn default() -> Self {
        EdgeFeatherImageEffect { radius: 12.0 }
    }
}

impl EffectImpl for EdgeFeatherImageEffect {
    fn info() -> Info {
        Info::new(
            "edge_feather",
            "Edge feather",
            EffectCategory::Filters,
            "app/circle-dashed",
        )
        .desc("Softens the edges of non-transparent regions with a feathered alpha falloff.")
    }

    fn params() -> Vec<ParamDef> {
        vec![float_slider::<Self>(
            "radius",
            "Radius",
            0.0,
            100.0,
            12.0,
            |e: &mut Self, v| e.radius = v,
        )]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if source.is_empty() {
            return Ok(source.clone());
        }
        let radius = clampf(self.radius, 0.0, 100.0);
        if radius <= 0.01 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let mask: Vec<SkColor> = src
            .iter()
            .map(|c| {
                let a = if c.a > 16 { 255 } else { 0 };
                SkColor::rgba(a, a, a, a)
            })
            .collect();
        let mask_bmp = from_pixels(source.width(), source.height(), &mask);
        let padding = 2.max((radius * 2.0).ceil() as i32);
        let feathered = pixels(&blur_transparent_padded(
            &mask_bmp,
            padding,
            0.001f32.max(radius / 3.0),
        ));
        let dst: Vec<SkColor> = src
            .iter()
            .zip(&feathered)
            .map(|(s, f)| s.with_alpha(clamp_to_byte(s.a as f32 * (f.a as f32 / 255.0))))
            .collect();
        Ok(from_pixels(source.width(), source.height(), &dst))
    }
}
