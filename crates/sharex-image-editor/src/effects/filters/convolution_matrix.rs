//! `ConvolutionMatrixImageEffect` (`Core/ImageEffects/Filters/ConvolutionMatrixImageEffect.cs`).

use crate::effects::prelude::*;

/// `ConvolutionMatrixImageEffect`.
#[derive(Clone, Debug)]
pub struct ConvolutionMatrixImageEffect {
    pub x0_y0: i32,
    pub x1_y0: i32,
    pub x2_y0: i32,
    pub x0_y1: i32,
    pub x1_y1: i32,
    pub x2_y1: i32,
    pub x0_y2: i32,
    pub x1_y2: i32,
    pub x2_y2: i32,
    pub factor: f64,
    pub offset: i32,
}

impl Default for ConvolutionMatrixImageEffect {
    fn default() -> Self {
        ConvolutionMatrixImageEffect {
            x0_y0: 0,
            x1_y0: 0,
            x2_y0: 0,
            x0_y1: 0,
            x1_y1: 1,
            x2_y1: 0,
            x0_y2: 0,
            x1_y2: 0,
            x2_y2: 0,
            factor: 1.0,
            offset: 0,
        }
    }
}

impl EffectImpl for ConvolutionMatrixImageEffect {
    fn info() -> Info {
        Info::new("convolution_matrix", "Convolution matrix", EffectCategory::Filters, "app/grid")
            .desc("Applies a custom 3x3 convolution kernel to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("x0y0", "X0 Y0", -99, 99, 0, |e: &mut Self, v| e.x0_y0 = v),
            int_numeric::<Self>("x1y0", "X1 Y0", -99, 99, 0, |e: &mut Self, v| e.x1_y0 = v),
            int_numeric::<Self>("x2y0", "X2 Y0", -99, 99, 0, |e: &mut Self, v| e.x2_y0 = v),
            int_numeric::<Self>("x0y1", "X0 Y1", -99, 99, 0, |e: &mut Self, v| e.x0_y1 = v),
            int_numeric::<Self>("x1y1", "X1 Y1", -99, 99, 1, |e: &mut Self, v| e.x1_y1 = v),
            int_numeric::<Self>("x2y1", "X2 Y1", -99, 99, 0, |e: &mut Self, v| e.x2_y1 = v),
            int_numeric::<Self>("x0y2", "X0 Y2", -99, 99, 0, |e: &mut Self, v| e.x0_y2 = v),
            int_numeric::<Self>("x1y2", "X1 Y2", -99, 99, 0, |e: &mut Self, v| e.x1_y2 = v),
            int_numeric::<Self>("x2y2", "X2 Y2", -99, 99, 0, |e: &mut Self, v| e.x2_y2 = v),
            float_slider::<Self>("factor", "Factor", 0.009999999776482582, 10.0, 1.0, |e: &mut Self, v| e.factor = v as f64),
            int_numeric::<Self>("offset", "Offset", -255, 255, 0, |e: &mut Self, v| e.offset = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let factor = 0.01f64.max(self.factor) as f32;
        let gain = 1.0 / factor;
        let k = [
            self.x0_y0 as f32,
            self.x1_y0 as f32,
            self.x2_y0 as f32,
            self.x0_y1 as f32,
            self.x1_y1 as f32,
            self.x2_y1 as f32,
            self.x0_y2 as f32,
            self.x1_y2 as f32,
            self.x2_y2 as f32,
        ];
        Ok(matrix_convolution_3x3(source, &k, gain, self.offset as f32, false))
    }
}
