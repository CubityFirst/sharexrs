//! `FakeHairImageEffect` (`Core/ImageEffects/Filters/FakeHairImageEffect.cs`).

use crate::effects::prelude::*;

/// `FakeHairImageEffect`.
#[derive(Clone, Debug)]
pub struct FakeHairImageEffect {
    pub count: i32,
    pub length: f32,
    pub thickness: f32,
    pub curliness: f32,
    pub hair_color: SkColor,
}

impl Default for FakeHairImageEffect {
    fn default() -> Self {
        FakeHairImageEffect {
            count: 1,
            length: 180.0,
            thickness: 1.5,
            curliness: 30.0,
            hair_color: SkColor::argb(0xDC140F0A),
        }
    }
}

impl EffectImpl for FakeHairImageEffect {
    fn info() -> Info {
        Info::new("fake_hair", "Fake Hair", EffectCategory::Drawings, "app/scissors")
            .desc("Scatters realistic hair strands over the image to annoy your friends.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("count", "Hair Count", 1, 100, 1, |e: &mut Self, v| e.count = v),
            float_slider::<Self>("length", "Length", 20.0, 600.0, 180.0, |e: &mut Self, v| e.length = v),
            float_slider::<Self>("thickness", "Thickness", 0.5, 4.0, 1.0, |e: &mut Self, v| e.thickness = v).tick(0.1).snap(false).format("{}{0:0.0}"),
            float_slider::<Self>("curliness", "Curliness", 0.0, 100.0, 30.0, |e: &mut Self, v| e.curliness = v),
            color_param::<Self>("hair_color", "Hair Color", SkColor::argb(0xDC140F0A), |e: &mut Self, v| e.hair_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
