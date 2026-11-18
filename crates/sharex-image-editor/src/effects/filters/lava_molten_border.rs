//! `LavaMoltenBorderImageEffect` (`Core/ImageEffects/Filters/LavaMoltenBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `LavaMoltenBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct LavaMoltenBorderImageEffect {
    pub border_size: i32,
    pub rock_color: SkColor,
    pub lava_color: SkColor,
    pub crack_density: f32,
    pub glow_intensity: f32,
    pub ember_sparks: bool,
}

impl Default for LavaMoltenBorderImageEffect {
    fn default() -> Self {
        LavaMoltenBorderImageEffect {
            border_size: 56,
            rock_color: SkColor::argb(0xFF1E1916),
            lava_color: SkColor::argb(0xFFFF500A),
            crack_density: 60.0,
            glow_intensity: 70.0,
            ember_sparks: true,
        }
    }
}

impl EffectImpl for LavaMoltenBorderImageEffect {
    fn info() -> Info {
        Info::new("lava_molten_border", "Lava molten border", EffectCategory::Filters, "image-effect/flame")
            .desc("Adds a volcanic border with dark cracked rock and glowing molten lava seams.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 12, 300, 56, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("rock_color", "Rock color", SkColor::argb(0xFF1E1916), |e: &mut Self, v| e.rock_color = v),
            color_param::<Self>("lava_color", "Lava color", SkColor::argb(0xFFFF500A), |e: &mut Self, v| e.lava_color = v),
            float_slider::<Self>("crack_density", "Crack density", 10.0, 100.0, 60.0, |e: &mut Self, v| e.crack_density = v),
            float_slider::<Self>("glow_intensity", "Glow intensity", 10.0, 100.0, 70.0, |e: &mut Self, v| e.glow_intensity = v),
            bool_param::<Self>("ember_sparks", "Ember sparks", true, |e: &mut Self, v| e.ember_sparks = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
