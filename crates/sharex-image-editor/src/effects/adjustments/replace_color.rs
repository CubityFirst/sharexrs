//! `ReplaceColorImageEffect` (`Core/ImageEffects/Adjustments/ReplaceColorImageEffect.cs`).

use crate::effects::prelude::*;

/// `ReplaceColorImageEffect`.
#[derive(Clone, Debug)]
pub struct ReplaceColorImageEffect {
    pub target_color: SkColor,
    pub replace_color: SkColor,
    pub tolerance: f32,
}

impl Default for ReplaceColorImageEffect {
    fn default() -> Self {
        ReplaceColorImageEffect {
            target_color: SkColor::argb(0x00000000),
            replace_color: SkColor::argb(0x00000000),
            tolerance: 0.0,
        }
    }
}

impl EffectImpl for ReplaceColorImageEffect {
    fn info() -> Info {
        Info::new("replace_color", "Replace Color", EffectCategory::Adjustments, "image-effect/replace")
            .desc("Replaces a specific color.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>("target_color", "Target color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.target_color = v),
            color_param::<Self>("replace_color", "Replace color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.replace_color = v),
            float_slider::<Self>("tolerance", "Tolerance", 0.0, 255.0, 40.0, |e: &mut Self, v| e.tolerance = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let tol = (self.tolerance * 2.55) as i32;
        let (target, replace) = (self.target_color, self.replace_color);
        Ok(apply_pixel_operation(source, |c| {
            let m = |a: u8, b: u8| (a as i32 - b as i32).abs() <= tol;
            if m(c.r, target.r) && m(c.g, target.g) && m(c.b, target.b) && m(c.a, target.a) {
                replace
            } else {
                c
            }
        }))
    }
}
