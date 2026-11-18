//! `GlowImageEffect` (`Core/ImageEffects/Filters/GlowImageEffect.cs`).

use crate::effects::prelude::*;

/// `GlowImageEffect`.
#[derive(Clone, Debug)]
pub struct GlowImageEffect {
    pub size: i32,
    pub strength: f32,
    pub color: SkColor,
    pub offset_x: i32,
    pub offset_y: i32,
    pub auto_resize: bool,
}

impl Default for GlowImageEffect {
    fn default() -> Self {
        GlowImageEffect {
            size: 20,
            strength: 80.0,
            color: SkColor::argb(0xFFFFFFFF),
            offset_x: 0,
            offset_y: 0,
            auto_resize: true,
        }
    }
}

impl EffectImpl for GlowImageEffect {
    fn info() -> Info {
        Info::new("glow", "Glow", EffectCategory::Filters, "app/lightbulb")
            .desc("Applies a glowing effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 1, 100, 20, |e: &mut Self, v| e.size = v).snap(false),
            float_slider::<Self>("strength", "Strength", 1.0, 100.0, 80.0, |e: &mut Self, v| e.strength = v).snap(false).format("{}{0:0}%"),
            int_slider::<Self>("offset_x", "Offset X", -100, 100, 0, |e: &mut Self, v| e.offset_x = v).snap(false),
            int_slider::<Self>("offset_y", "Offset Y", -100, 100, 0, |e: &mut Self, v| e.offset_y = v).snap(false),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.color = v),
            bool_param::<Self>("auto_resize", "Auto resize", true, |e: &mut Self, v| e.auto_resize = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let pad = if self.auto_resize { self.size } else { 0 };
        let (l, r, t, b) = if self.auto_resize {
            (0.max(-self.offset_x) + pad, 0.max(self.offset_x) + pad, 0.max(-self.offset_y) + pad, 0.max(self.offset_y) + pad)
        } else {
            (0, 0, 0, 0)
        };
        let nw = source.width() as i32 + l + r;
        let nh = source.height() as i32 + t + b;
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let (gx, gy) = (l + self.offset_x, t + self.offset_y);
        let glow_color = self.color.with_alpha((255.0 * self.strength / 100.0) as u8);
        let sigma = self.size as f32;
        draw_src_in_blurred(&mut out, source, gx, gy, glow_color, sigma);
        draw_bitmap(&mut out, source, l as f32, t as f32);
        Ok(out)
    }
}
