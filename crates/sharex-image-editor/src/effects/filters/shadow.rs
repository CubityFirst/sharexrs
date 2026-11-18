//! `ShadowImageEffect` (`Core/ImageEffects/Filters/ShadowImageEffect.cs`).

use crate::effects::prelude::*;

/// `ShadowImageEffect`.
#[derive(Clone, Debug)]
pub struct ShadowImageEffect {
    pub opacity: f32,
    pub size: i32,
    pub color: SkColor,
    pub offset_x: i32,
    pub offset_y: i32,
    pub auto_resize: bool,
}

impl Default for ShadowImageEffect {
    fn default() -> Self {
        ShadowImageEffect {
            opacity: 80.0,
            size: 20,
            color: SkColor::argb(0xFF000000),
            offset_x: 5,
            offset_y: 5,
            auto_resize: true,
        }
    }
}

impl EffectImpl for ShadowImageEffect {
    fn info() -> Info {
        Info::new("shadow", "Shadow", EffectCategory::Filters, "image-effect/cloud-moon")
            .desc("Adds a drop shadow to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 80.0, |e: &mut Self, v| e.opacity = v).snap(false).format("{}{0:0}%"),
            int_slider::<Self>("size", "Size", 0, 100, 20, |e: &mut Self, v| e.size = v).snap(false),
            int_numeric::<Self>("offset_x", "Offset X", -1000, 1000, 5, |e: &mut Self, v| e.offset_x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -1000, 1000, 5, |e: &mut Self, v| e.offset_y = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.color = v),
            bool_param::<Self>("auto_resize", "Auto resize", true, |e: &mut Self, v| e.auto_resize = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let pad = self.size;
        let (l, r, t, b) = if self.auto_resize {
            (0.max(-self.offset_x) + pad, 0.max(self.offset_x) + pad, 0.max(-self.offset_y) + pad, 0.max(self.offset_y) + pad)
        } else {
            (0, 0, 0, 0)
        };
        let nw = source.width() as i32 + l + r;
        let nh = source.height() as i32 + t + b;
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let (sx, sy) = (l + self.offset_x, t + self.offset_y);
        let c = self.color;
        let shadow_color = SkColor::rgba(c.r, c.g, c.b, (255.0 * self.opacity / 100.0) as u8);
        let sigma = self.size as f32 / 2.0;
        draw_src_in_blurred(&mut out, source, sx, sy, shadow_color, sigma);
        draw_bitmap(&mut out, source, l as f32, t as f32);
        Ok(out)
    }
}
