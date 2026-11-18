//! `ConfettiImageEffect` (`Core/ImageEffects/Drawings/ConfettiImageEffect.cs`).

use crate::effects::prelude::*;

/// `ConfettiImageEffect`.
#[derive(Clone, Debug)]
pub struct ConfettiImageEffect {
    pub count: i32,
    pub min_size: i32,
    pub max_size: i32,
    pub opacity: f32,
    pub background: bool,
}

impl Default for ConfettiImageEffect {
    fn default() -> Self {
        ConfettiImageEffect {
            count: 150,
            min_size: 4,
            max_size: 12,
            opacity: 90.0,
            background: false,
        }
    }
}

impl EffectImpl for ConfettiImageEffect {
    fn info() -> Info {
        Info::new("confetti", "Confetti", EffectCategory::Drawings, "image-effect/party-popper")
            .desc("Scatters colorful confetti pieces over the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("count", "Count", 10, 1000, 150, |e: &mut Self, v| e.count = v),
            int_slider::<Self>("min_size", "Min size", 2, 40, 4, |e: &mut Self, v| e.min_size = v),
            int_slider::<Self>("max_size", "Max size", 4, 80, 12, |e: &mut Self, v| e.max_size = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 90.0, |e: &mut Self, v| e.opacity = v),
            bool_param::<Self>("background", "Background", false, |e: &mut Self, v| e.background = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
