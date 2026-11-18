//! `WindStreaksImageEffect` (`Core/ImageEffects/Filters/WindStreaksImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindDirection {
    Left,
    Right,
}

/// `WindStreaksImageEffect`.
#[derive(Clone, Debug)]
pub struct WindStreaksImageEffect {
    pub length: i32,
    pub threshold: i32,
    pub opacity: f32,
    pub direction: WindDirection,
}

impl Default for WindStreaksImageEffect {
    fn default() -> Self {
        WindStreaksImageEffect {
            length: 30,
            threshold: 200,
            opacity: 40.0,
            direction: WindDirection::Right,
        }
    }
}

impl EffectImpl for WindStreaksImageEffect {
    fn info() -> Info {
        Info::new("wind_streaks", "Wind streaks", EffectCategory::Filters, "image-effect/wind")
            .desc("Adds directional wind streak trails to bright areas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("length", "Streak length", 5, 100, 30, |e: &mut Self, v| e.length = v),
            int_slider::<Self>("threshold", "Brightness threshold", 100, 254, 200, |e: &mut Self, v| e.threshold = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 40.0, |e: &mut Self, v| e.opacity = v),
            enum_param::<Self, WindDirection>("direction", "Direction", WindDirection::Right, |e: &mut Self, v| e.direction = v, &[("Left", WindDirection::Left), ("Right", WindDirection::Right)]),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
