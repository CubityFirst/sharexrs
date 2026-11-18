//! `ResizeImageEffect` (`Core/ImageEffects/Manipulations/ResizeImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeImageEffectAspectRatioAnchor {
    LargestDimension,
    Width,
    Height,
}

/// `ResizeImageEffect`.
#[derive(Clone, Debug)]
pub struct ResizeImageEffect {
    pub width: i32,
    pub height: i32,
    pub maintain_aspect_ratio: bool,
    pub aspect_ratio_anchor: ResizeImageEffectAspectRatioAnchor,
}

impl Default for ResizeImageEffect {
    fn default() -> Self {
        ResizeImageEffect {
            width: 0,
            height: 0,
            maintain_aspect_ratio: false,
            aspect_ratio_anchor: ResizeImageEffectAspectRatioAnchor::LargestDimension,
        }
    }
}

impl EffectImpl for ResizeImageEffect {
    fn info() -> Info {
        Info::new(
            "resize_image",
            "Resize image",
            EffectCategory::Manipulations,
            "app/resize",
        )
        .desc("Resizes the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("width", "Width", 0, 10000, 0, |e: &mut Self, v| e.width = v),
            int_numeric::<Self>("height", "Height", 0, 10000, 0, |e: &mut Self, v| {
                e.height = v
            }),
            bool_param::<Self>(
                "maintain_aspect_ratio",
                "Maintain aspect ratio",
                false,
                |e: &mut Self, v| e.maintain_aspect_ratio = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (w, h) = self.target_size(source.width() as i32, source.height() as i32);
        Ok(resize_catmull_rom(source, w as u32, h as u32))
    }
}

impl ResizeImageEffect {
    /// `GetTargetSize`.
    pub fn target_size(&self, source_width: i32, source_height: i32) -> (i32, i32) {
        resolve_target_size(
            source_width,
            source_height,
            self.width,
            self.height,
            self.maintain_aspect_ratio,
            self.aspect_ratio_anchor,
        )
    }
}

/// `ResizeImageEffect.ResolveTargetSize`.
pub fn resolve_target_size(
    source_width: i32,
    source_height: i32,
    requested_width: i32,
    requested_height: i32,
    maintain_aspect_ratio: bool,
    anchor: ResizeImageEffectAspectRatioAnchor,
) -> (i32, i32) {
    let mut width = requested_width.max(0);
    let mut height = requested_height.max(0);
    if !maintain_aspect_ratio {
        width = if width > 0 { width } else { source_width };
        height = if height > 0 { height } else { source_height };
        return (width, height);
    }
    if width <= 0 && height <= 0 {
        return (source_width, source_height);
    }
    let (sw, sh) = (source_width as f64, source_height as f64);
    let r = |v: f64| 1.max(v.round_ties_even() as i32);
    if width <= 0 {
        return (r(height as f64 / sh * sw), height);
    }
    if height <= 0 {
        return (width, r(width as f64 / sw * sh));
    }
    match anchor {
        ResizeImageEffectAspectRatioAnchor::Width => (width, r(width as f64 / sw * sh)),
        ResizeImageEffectAspectRatioAnchor::Height => (r(height as f64 / sh * sw), height),
        ResizeImageEffectAspectRatioAnchor::LargestDimension => {
            let scale = (width as f64 / sw).max(height as f64 / sh);
            (r(sw * scale), r(sh * scale))
        }
    }
}

/// `SKBitmap.Resize(info, CatmullRom)`: `SkPixmap::scalePixels` (the image
/// shader, clamped, drawn over the whole result with `kSrc`).
pub fn resize_catmull_rom(source: &Bitmap, width: u32, height: u32) -> Bitmap {
    let mut out = Bitmap::new(width, height);
    if width == source.width() && height == source.height() {
        // scalePixels copies when the size is unchanged.
        return source.clone();
    }
    {
        let mut canvas = Canvas::new(&mut out);
        let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
        paint.blend = Blend::Src;
        canvas.draw_image_rect(
            source,
            None,
            RRect::xywh(0.0, 0.0, width as f32, height as f32),
            Sampling::CatmullRom,
            Some(&paint),
        );
    }
    out
}
