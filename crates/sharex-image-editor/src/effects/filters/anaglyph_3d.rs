//! `Anaglyph3DImageEffect` (`Core/ImageEffects/Filters/Anaglyph3DImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnaglyphMode {
    RedCyan,
    AmberBlue,
    GreenMagenta,
}

/// `Anaglyph3DImageEffect`.
#[derive(Clone, Debug)]
pub struct Anaglyph3DImageEffect {
    pub mode: AnaglyphMode,
    pub separation_x: i32,
    pub separation_y: i32,
    pub ghost_reduction: f32,
}

impl Default for Anaglyph3DImageEffect {
    fn default() -> Self {
        Anaglyph3DImageEffect {
            mode: AnaglyphMode::RedCyan,
            separation_x: 10,
            separation_y: 0,
            ghost_reduction: 45.0,
        }
    }
}

impl EffectImpl for Anaglyph3DImageEffect {
    fn info() -> Info {
        Info::new("anaglyph_3d", "Anaglyph 3D", EffectCategory::Filters, "image-effect/glasses")
            .desc("Simulates a retro anaglyph 3D stereoscopic effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, AnaglyphMode>("mode", "Mode", AnaglyphMode::RedCyan, |e: &mut Self, v| e.mode = v, &[("Red / Cyan", AnaglyphMode::RedCyan), ("Amber / Blue", AnaglyphMode::AmberBlue), ("Green / Magenta", AnaglyphMode::GreenMagenta)]),
            int_slider::<Self>("separation_x", "Separation X", -100, 100, 10, |e: &mut Self, v| e.separation_x = v),
            int_slider::<Self>("separation_y", "Separation Y", -100, 100, 0, |e: &mut Self, v| e.separation_y = v),
            float_slider::<Self>("ghost_reduction", "Ghost reduction", 0.0, 100.0, 45.0, |e: &mut Self, v| e.ghost_reduction = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
