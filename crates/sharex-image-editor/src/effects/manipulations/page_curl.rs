//! `PageCurlImageEffect` (`Core/ImageEffects/Manipulations/PageCurlImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageCurlCorner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

/// `PageCurlImageEffect`.
#[derive(Clone, Debug)]
pub struct PageCurlImageEffect {
    pub corner: PageCurlCorner,
    pub curl_size: f32,
    pub curl_depth: f32,
    pub shadow_strength: f32,
    pub back_color: SkColor,
}

impl Default for PageCurlImageEffect {
    fn default() -> Self {
        PageCurlImageEffect {
            corner: PageCurlCorner::BottomRight,
            curl_size: 28.0,
            curl_depth: 55.0,
            shadow_strength: 60.0,
            back_color: SkColor::argb(0xFFF8F4EC),
        }
    }
}

impl EffectImpl for PageCurlImageEffect {
    fn info() -> Info {
        Info::new("page_curl", "Page curl", EffectCategory::Manipulations, "image-effect/page-curl")
            .desc("Simulates a page curl at a corner of the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, PageCurlCorner>("corner", "Corner", PageCurlCorner::BottomRight, |e: &mut Self, v| e.corner = v, &[("Top left", PageCurlCorner::TopLeft), ("Top right", PageCurlCorner::TopRight), ("Bottom right", PageCurlCorner::BottomRight), ("Bottom left", PageCurlCorner::BottomLeft)]),
            float_slider::<Self>("curl_size", "Curl size", 5.0, 80.0, 28.0, |e: &mut Self, v| e.curl_size = v),
            float_slider::<Self>("curl_depth", "Curl depth", 0.0, 100.0, 55.0, |e: &mut Self, v| e.curl_depth = v),
            float_slider::<Self>("shadow_strength", "Shadow strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.shadow_strength = v),
            color_param::<Self>("back_color", "Back color", SkColor::argb(0xFFF8F4EC), |e: &mut Self, v| e.back_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
