//! `PixelSortingImageEffect` (`Core/ImageEffects/Filters/PixelSortingImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelSortDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelSortMetric {
    Brightness,
    Hue,
}

/// `PixelSortingImageEffect`.
#[derive(Clone, Debug)]
pub struct PixelSortingImageEffect {
    pub direction: PixelSortDirection,
    pub metric: PixelSortMetric,
    pub threshold_low: f32,
    pub threshold_high: f32,
    pub min_span_length: i32,
    pub max_span_length: i32,
    pub sort_probability: f32,
    pub seed: i32,
}

impl Default for PixelSortingImageEffect {
    fn default() -> Self {
        PixelSortingImageEffect {
            direction: PixelSortDirection::Vertical,
            metric: PixelSortMetric::Brightness,
            threshold_low: 12.0,
            threshold_high: 85.0,
            min_span_length: 8,
            max_span_length: 120,
            sort_probability: 85.0,
            seed: 3110,
        }
    }
}

impl EffectImpl for PixelSortingImageEffect {
    fn info() -> Info {
        Info::new("pixel_sorting", "Pixel sorting", EffectCategory::Filters, "image-effect/sort-descending")
            .desc("Sorts pixels along rows or columns based on brightness or hue thresholds.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, PixelSortDirection>("direction", "Direction", PixelSortDirection::Vertical, |e: &mut Self, v| e.direction = v, &[("Horizontal", PixelSortDirection::Horizontal), ("Vertical", PixelSortDirection::Vertical)]),
            enum_param::<Self, PixelSortMetric>("metric", "Metric", PixelSortMetric::Brightness, |e: &mut Self, v| e.metric = v, &[("Brightness", PixelSortMetric::Brightness), ("Hue", PixelSortMetric::Hue)]),
            float_slider::<Self>("threshold_low", "Threshold low", 0.0, 100.0, 12.0, |e: &mut Self, v| e.threshold_low = v),
            float_slider::<Self>("threshold_high", "Threshold high", 0.0, 100.0, 85.0, |e: &mut Self, v| e.threshold_high = v),
            int_slider::<Self>("min_span_length", "Min span length", 2, 256, 8, |e: &mut Self, v| e.min_span_length = v),
            int_slider::<Self>("max_span_length", "Max span length", 2, 512, 120, |e: &mut Self, v| e.max_span_length = v),
            float_slider::<Self>("sort_probability", "Sort probability", 0.0, 100.0, 85.0, |e: &mut Self, v| e.sort_probability = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
