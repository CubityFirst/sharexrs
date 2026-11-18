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
        let (width, height) = (source.width() as i32, source.height() as i32);
        let curl_size = 12f32.max(width.min(height) as f32 * clampf(self.curl_size, 5.0, 80.0) / 100.0);
        let depth01 = clampf(self.curl_depth, 0.0, 100.0) / 100.0;
        let shadow01 = clampf(self.shadow_strength, 0.0, 100.0) / 100.0;
        let fold_line = curl_size * (0.78 + (depth01 * 0.22));
        let shadow_width = 4f32.max(curl_size * (0.08 + (shadow01 * 0.20) + (depth01 * 0.10)));
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let corner = self.corner;
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let original = src[(row + x) as usize];
                let (lx, ly) = corner_local(corner, width, height, x as f32, y as f32);
                let out = if lx > curl_size + shadow_width || ly > curl_size + shadow_width {
                    original
                } else {
                    let diagonal = lx + ly;
                    if diagonal <= fold_line {
                        let (rx, ry) = (fold_line - ly, fold_line - lx);
                        let (sx, sy) = corner_local(corner, width, height, rx, ry);
                        let reflected = bilinear_sample(&src, width, height, sx, sy);
                        let curl_mix = 0.28 + (depth01 * 0.28) + ((1.0 - (diagonal / fold_line)) * 0.18);
                        let curled = blend(reflected, self.back_color, curl_mix);
                        let highlight = (0.05 + (depth01 * 0.09)) * smooth_step(fold_line * 0.55, 0.0, diagonal);
                        blend(curled, SkColor::rgba(255, 255, 255, curled.a), highlight)
                    } else if diagonal < fold_line + shadow_width {
                        let fade = 1.0 - ((diagonal - fold_line) / shadow_width);
                        let shade = 1.0 - (shadow01 * fade * 0.42);
                        multiply_rgb(original, shade)
                    } else {
                        original
                    }
                };
                dst[(row + x) as usize] = out;
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

/// `ToCornerLocal` / `FromCornerLocal` (the same mapping both ways).
fn corner_local(corner: PageCurlCorner, width: i32, height: i32, x: f32, y: f32) -> (f32, f32) {
    match corner {
        PageCurlCorner::TopLeft => (x, y),
        PageCurlCorner::TopRight => ((width - 1) as f32 - x, y),
        PageCurlCorner::BottomRight => ((width - 1) as f32 - x, (height - 1) as f32 - y),
        PageCurlCorner::BottomLeft => (x, (height - 1) as f32 - y),
    }
}
