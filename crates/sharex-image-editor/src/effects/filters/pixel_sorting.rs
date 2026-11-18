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
        Info::new(
            "pixel_sorting",
            "Pixel sorting",
            EffectCategory::Filters,
            "image-effect/sort-descending",
        )
        .desc("Sorts pixels along rows or columns based on brightness or hue thresholds.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, PixelSortDirection>(
                "direction",
                "Direction",
                PixelSortDirection::Vertical,
                |e: &mut Self, v| e.direction = v,
                &[
                    ("Horizontal", PixelSortDirection::Horizontal),
                    ("Vertical", PixelSortDirection::Vertical),
                ],
            ),
            enum_param::<Self, PixelSortMetric>(
                "metric",
                "Metric",
                PixelSortMetric::Brightness,
                |e: &mut Self, v| e.metric = v,
                &[
                    ("Brightness", PixelSortMetric::Brightness),
                    ("Hue", PixelSortMetric::Hue),
                ],
            ),
            float_slider::<Self>(
                "threshold_low",
                "Threshold low",
                0.0,
                100.0,
                12.0,
                |e: &mut Self, v| e.threshold_low = v,
            ),
            float_slider::<Self>(
                "threshold_high",
                "Threshold high",
                0.0,
                100.0,
                85.0,
                |e: &mut Self, v| e.threshold_high = v,
            ),
            int_slider::<Self>(
                "min_span_length",
                "Min span length",
                2,
                256,
                8,
                |e: &mut Self, v| e.min_span_length = v,
            ),
            int_slider::<Self>(
                "max_span_length",
                "Max span length",
                2,
                512,
                120,
                |e: &mut Self, v| e.max_span_length = v,
            ),
            float_slider::<Self>(
                "sort_probability",
                "Sort probability",
                0.0,
                100.0,
                85.0,
                |e: &mut Self, v| e.sort_probability = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let mut low = clampf(self.threshold_low, 0.0, 100.0) / 100.0;
        let mut high = clampf(self.threshold_high, 0.0, 100.0) / 100.0;
        if high < low {
            std::mem::swap(&mut low, &mut high);
        }
        let min_span = self.min_span_length.clamp(2, 256);
        let max_span = self.max_span_length.clamp(min_span, 512);
        let probability = clampf(self.sort_probability, 0.0, 100.0) / 100.0;
        let mut px = pixels(source);
        let hue_metric = self.metric == PixelSortMetric::Hue;
        let metric = move |c: &SkColor| -> f32 {
            if hue_metric {
                sort_hue(*c)
            } else {
                ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0
            }
        };
        let horizontal = self.direction == PixelSortDirection::Horizontal;
        let (outer, inner) = if horizontal {
            (height, width)
        } else {
            (width, height)
        };
        let index = |o: i32, i: i32| {
            if horizontal {
                (o * width + i) as usize
            } else {
                (i * width + o) as usize
            }
        };
        for o in 0..outer {
            let mut i = 0;
            while i < inner {
                let m = metric(&px[index(o, i)]);
                if m < low || m > high {
                    i += 1;
                    continue;
                }
                let start = i;
                i += 1;
                while i < inner {
                    let m = metric(&px[index(o, i)]);
                    if m < low || m > high {
                        break;
                    }
                    i += 1;
                }
                if i - start < min_span {
                    continue;
                }
                let mut cursor = start;
                while cursor < i {
                    let length = (i - cursor).min(max_span);
                    if length < min_span {
                        break;
                    }
                    let selector = if horizontal {
                        hash01(cursor, o, self.seed ^ 1187)
                    } else {
                        hash01(o, cursor, self.seed ^ 7723)
                    };
                    if selector <= probability {
                        let mut seg: Vec<SkColor> =
                            (0..length).map(|k| px[index(o, cursor + k)]).collect();
                        dotnet_sort(&mut seg, &|a, b| float_compare(metric(a), metric(b)));
                        for (k, c) in seg.into_iter().enumerate() {
                            px[index(o, cursor + k as i32)] = c;
                        }
                    }
                    cursor += length;
                }
            }
        }
        Ok(from_pixels(width as u32, height as u32, &px))
    }
}

// ---- port ----

fn sort_hue(c: SkColor) -> f32 {
    let (r, g, b) = (c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let delta = max - min;
    if delta <= 0.0001 {
        return 0.0;
    }
    let mut hue = if max == r {
        ((g - b) / delta) % 6.0
    } else if max == g {
        ((b - r) / delta) + 2.0
    } else {
        ((r - g) / delta) + 4.0
    };
    hue /= 6.0;
    if hue < 0.0 {
        hue += 1.0;
    }
    hue
}
