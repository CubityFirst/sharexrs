//! `DrawImageEffect` (`Core/ImageEffects/Drawings/DrawImageEffect.cs`).

use crate::effects::prelude::*;
use crate::effects::drawings::enums::*;

/// `DrawImageEffect`.
#[derive(Clone, Debug)]
pub struct DrawImageEffect {
    /// `Math.Clamp(value, 0, 100)` when set.
    pub opacity: i32,
    pub image_location: String,
    pub placement: DrawingPlacement,
    pub offset: PointI,
    pub size_mode: DrawingImageSizeMode,
    pub size: SizeI,
    pub rotate_flip: DrawingImageRotateFlipType,
    pub tile: bool,
    pub auto_hide: bool,
    pub interpolation_mode: DrawingInterpolationMode,
    pub compositing_mode: DrawingCompositingMode,
}

impl Default for DrawImageEffect {
    fn default() -> Self {
        DrawImageEffect {
            opacity: 100,
            image_location: "".to_string(),
            placement: DrawingPlacement::TopLeft,
            offset: PointI::new(0, 0),
            size_mode: DrawingImageSizeMode::DontResize,
            size: SizeI::new(0, 0),
            rotate_flip: DrawingImageRotateFlipType::None,
            tile: false,
            auto_hide: false,
            interpolation_mode: DrawingInterpolationMode::HighQualityBicubic,
            compositing_mode: DrawingCompositingMode::SourceOver,
        }
    }
}

impl EffectImpl for DrawImageEffect {
    fn info() -> Info {
        Info::new("draw_image", "Image", EffectCategory::Drawings, "app/image-add")
            .desc("Draws an image overlay on the source image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            file_param::<Self>("image_location", "Image location", "", |e: &mut Self, v| e.image_location = v),
            enum_param::<Self, DrawingPlacement>("placement", "Placement", DrawingPlacement::TopLeft, |e: &mut Self, v| e.placement = v, &[("Top left", DrawingPlacement::TopLeft), ("Top center", DrawingPlacement::TopCenter), ("Top right", DrawingPlacement::TopRight), ("Middle left", DrawingPlacement::MiddleLeft), ("Middle center", DrawingPlacement::MiddleCenter), ("Middle right", DrawingPlacement::MiddleRight), ("Bottom left", DrawingPlacement::BottomLeft), ("Bottom center", DrawingPlacement::BottomCenter), ("Bottom right", DrawingPlacement::BottomRight)]),
            int_numeric::<Self>("offset_x", "Offset X", -10000, 10000, 0, |e: &mut Self, v| e.offset.x = v),
            int_numeric::<Self>("offset_y", "Offset Y", -10000, 10000, 0, |e: &mut Self, v| e.offset.y = v),
            enum_param::<Self, DrawingImageSizeMode>("size_mode", "Size mode", DrawingImageSizeMode::DontResize, |e: &mut Self, v| e.size_mode = v, &[("Don't resize", DrawingImageSizeMode::DontResize), ("Absolute size", DrawingImageSizeMode::AbsoluteSize), ("Percentage of watermark", DrawingImageSizeMode::PercentageOfWatermark), ("Percentage of canvas", DrawingImageSizeMode::PercentageOfCanvas)]),
            int_numeric::<Self>("size_width", "Size width", -1, 10000, 0, |e: &mut Self, v| e.size.width = v),
            int_numeric::<Self>("size_height", "Size height", -1, 10000, 0, |e: &mut Self, v| e.size.height = v),
            enum_param::<Self, DrawingImageRotateFlipType>("rotate_flip", "Rotate / flip", DrawingImageRotateFlipType::None, |e: &mut Self, v| e.rotate_flip = v, &[("None", DrawingImageRotateFlipType::None), ("Rotate 90", DrawingImageRotateFlipType::Rotate90), ("Rotate 180", DrawingImageRotateFlipType::Rotate180), ("Rotate 270", DrawingImageRotateFlipType::Rotate270), ("Flip X", DrawingImageRotateFlipType::FlipX), ("Rotate 90 Flip X", DrawingImageRotateFlipType::Rotate90FlipX), ("Flip Y", DrawingImageRotateFlipType::FlipY), ("Rotate 90 Flip Y", DrawingImageRotateFlipType::Rotate90FlipY)]),
            bool_param::<Self>("tile", "Tile", false, |e: &mut Self, v| e.tile = v),
            bool_param::<Self>("auto_hide", "Auto hide", false, |e: &mut Self, v| e.auto_hide = v),
            enum_param::<Self, DrawingInterpolationMode>("interpolation_mode", "Interpolation mode", DrawingInterpolationMode::HighQualityBicubic, |e: &mut Self, v| e.interpolation_mode = v, &[("High quality bicubic", DrawingInterpolationMode::HighQualityBicubic), ("Bicubic", DrawingInterpolationMode::Bicubic), ("High quality bilinear", DrawingInterpolationMode::HighQualityBilinear), ("Bilinear", DrawingInterpolationMode::Bilinear), ("Nearest neighbor", DrawingInterpolationMode::NearestNeighbor)]),
            enum_param::<Self, DrawingCompositingMode>("compositing_mode", "Compositing mode", DrawingCompositingMode::SourceOver, |e: &mut Self, v| e.compositing_mode = v, &[("Source over", DrawingCompositingMode::SourceOver), ("Source copy", DrawingCompositingMode::SourceCopy)]),
            int_slider::<Self>("opacity", "Opacity", 0, 100, 100, |e: &mut Self, v| e.opacity = v.clamp(0, 100)),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let _ = source;
        not_ported()
    }
}
