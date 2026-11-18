//! `ColorMatrixImageEffect` (`Core/ImageEffects/Adjustments/ColorMatrixImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorMatrixImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorMatrixImageEffect {
    pub rr: f32,
    pub rg: f32,
    pub rb: f32,
    pub ra: f32,
    pub ro: f32,
    pub gr: f32,
    pub gg: f32,
    pub gb: f32,
    pub ga: f32,
    pub go: f32,
    pub br: f32,
    pub bg: f32,
    pub bb: f32,
    pub ba: f32,
    pub bo: f32,
    pub ar: f32,
    pub ag: f32,
    pub ab: f32,
    pub aa: f32,
    pub ao: f32,
}

impl Default for ColorMatrixImageEffect {
    fn default() -> Self {
        ColorMatrixImageEffect {
            rr: 1.0,
            rg: 0.0,
            rb: 0.0,
            ra: 0.0,
            ro: 0.0,
            gr: 0.0,
            gg: 1.0,
            gb: 0.0,
            ga: 0.0,
            go: 0.0,
            br: 0.0,
            bg: 0.0,
            bb: 1.0,
            ba: 0.0,
            bo: 0.0,
            ar: 0.0,
            ag: 0.0,
            ab: 0.0,
            aa: 1.0,
            ao: 0.0,
        }
    }
}

impl EffectImpl for ColorMatrixImageEffect {
    fn info() -> Info {
        Info::new(
            "color_matrix",
            "Color matrix",
            EffectCategory::Adjustments,
            "app/grid",
        )
        .desc("Applies a color matrix transformation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            double_numeric::<Self>("rr", "Rr", -5.0, 5.0, 1.0, |e: &mut Self, v| {
                e.rr = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("rg", "Rg", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.rg = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("rb", "Rb", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.rb = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("ra", "Ra", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.ra = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("ro", "Ro", -255.0, 255.0, 0.0, |e: &mut Self, v| {
                e.ro = v as f32
            })
            .format("0"),
            double_numeric::<Self>("gr", "Gr", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.gr = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("gg", "Gg", -5.0, 5.0, 1.0, |e: &mut Self, v| {
                e.gg = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("gb", "Gb", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.gb = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("ga", "Ga", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.ga = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("go", "Go", -255.0, 255.0, 0.0, |e: &mut Self, v| {
                e.go = v as f32
            })
            .format("0"),
            double_numeric::<Self>("br", "Br", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.br = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("bg", "Bg", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.bg = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("bb", "Bb", -5.0, 5.0, 1.0, |e: &mut Self, v| {
                e.bb = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("ba", "Ba", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.ba = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("bo", "Bo", -255.0, 255.0, 0.0, |e: &mut Self, v| {
                e.bo = v as f32
            })
            .format("0"),
            double_numeric::<Self>("ar", "Ar", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.ar = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("ag", "Ag", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.ag = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("ab", "Ab", -5.0, 5.0, 0.0, |e: &mut Self, v| {
                e.ab = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("aa", "Aa", -5.0, 5.0, 1.0, |e: &mut Self, v| {
                e.aa = v as f32
            })
            .increment(0.1),
            double_numeric::<Self>("ao", "Ao", -255.0, 255.0, 0.0, |e: &mut Self, v| {
                e.ao = v as f32
            })
            .format("0"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        #[rustfmt::skip]
        let m = [
            self.rr, self.rg, self.rb, self.ra, self.ro,
            self.gr, self.gg, self.gb, self.ga, self.go,
            self.br, self.bg, self.bb, self.ba, self.bo,
            self.ar, self.ag, self.ab, self.aa, self.ao,
        ];
        Ok(apply_color_matrix(source, &m))
    }
}
