//! `DrawImageEffect` (`Core/ImageEffects/Drawings/DrawImageEffect.cs`).

use crate::effects::drawings::enums::*;
use crate::effects::prelude::*;

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
        Info::new(
            "draw_image",
            "Image",
            EffectCategory::Drawings,
            "app/image-add",
        )
        .desc("Draws an image overlay on the source image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            file_param::<Self>("image_location", "Image location", "", |e: &mut Self, v| {
                e.image_location = v
            }),
            enum_param::<Self, DrawingPlacement>(
                "placement",
                "Placement",
                DrawingPlacement::TopLeft,
                |e: &mut Self, v| e.placement = v,
                &[
                    ("Top left", DrawingPlacement::TopLeft),
                    ("Top center", DrawingPlacement::TopCenter),
                    ("Top right", DrawingPlacement::TopRight),
                    ("Middle left", DrawingPlacement::MiddleLeft),
                    ("Middle center", DrawingPlacement::MiddleCenter),
                    ("Middle right", DrawingPlacement::MiddleRight),
                    ("Bottom left", DrawingPlacement::BottomLeft),
                    ("Bottom center", DrawingPlacement::BottomCenter),
                    ("Bottom right", DrawingPlacement::BottomRight),
                ],
            ),
            int_numeric::<Self>(
                "offset_x",
                "Offset X",
                -10000,
                10000,
                0,
                |e: &mut Self, v| e.offset.x = v,
            ),
            int_numeric::<Self>(
                "offset_y",
                "Offset Y",
                -10000,
                10000,
                0,
                |e: &mut Self, v| e.offset.y = v,
            ),
            enum_param::<Self, DrawingImageSizeMode>(
                "size_mode",
                "Size mode",
                DrawingImageSizeMode::DontResize,
                |e: &mut Self, v| e.size_mode = v,
                &[
                    ("Don't resize", DrawingImageSizeMode::DontResize),
                    ("Absolute size", DrawingImageSizeMode::AbsoluteSize),
                    (
                        "Percentage of watermark",
                        DrawingImageSizeMode::PercentageOfWatermark,
                    ),
                    (
                        "Percentage of canvas",
                        DrawingImageSizeMode::PercentageOfCanvas,
                    ),
                ],
            ),
            int_numeric::<Self>(
                "size_width",
                "Size width",
                -1,
                10000,
                0,
                |e: &mut Self, v| e.size.width = v,
            ),
            int_numeric::<Self>(
                "size_height",
                "Size height",
                -1,
                10000,
                0,
                |e: &mut Self, v| e.size.height = v,
            ),
            enum_param::<Self, DrawingImageRotateFlipType>(
                "rotate_flip",
                "Rotate / flip",
                DrawingImageRotateFlipType::None,
                |e: &mut Self, v| e.rotate_flip = v,
                &[
                    ("None", DrawingImageRotateFlipType::None),
                    ("Rotate 90", DrawingImageRotateFlipType::Rotate90),
                    ("Rotate 180", DrawingImageRotateFlipType::Rotate180),
                    ("Rotate 270", DrawingImageRotateFlipType::Rotate270),
                    ("Flip X", DrawingImageRotateFlipType::FlipX),
                    (
                        "Rotate 90 Flip X",
                        DrawingImageRotateFlipType::Rotate90FlipX,
                    ),
                    ("Flip Y", DrawingImageRotateFlipType::FlipY),
                    (
                        "Rotate 90 Flip Y",
                        DrawingImageRotateFlipType::Rotate90FlipY,
                    ),
                ],
            ),
            bool_param::<Self>("tile", "Tile", false, |e: &mut Self, v| e.tile = v),
            bool_param::<Self>("auto_hide", "Auto hide", false, |e: &mut Self, v| {
                e.auto_hide = v
            }),
            enum_param::<Self, DrawingInterpolationMode>(
                "interpolation_mode",
                "Interpolation mode",
                DrawingInterpolationMode::HighQualityBicubic,
                |e: &mut Self, v| e.interpolation_mode = v,
                &[
                    (
                        "High quality bicubic",
                        DrawingInterpolationMode::HighQualityBicubic,
                    ),
                    ("Bicubic", DrawingInterpolationMode::Bicubic),
                    (
                        "High quality bilinear",
                        DrawingInterpolationMode::HighQualityBilinear,
                    ),
                    ("Bilinear", DrawingInterpolationMode::Bilinear),
                    (
                        "Nearest neighbor",
                        DrawingInterpolationMode::NearestNeighbor,
                    ),
                ],
            ),
            enum_param::<Self, DrawingCompositingMode>(
                "compositing_mode",
                "Compositing mode",
                DrawingCompositingMode::SourceOver,
                |e: &mut Self, v| e.compositing_mode = v,
                &[
                    ("Source over", DrawingCompositingMode::SourceOver),
                    ("Source copy", DrawingCompositingMode::SourceCopy),
                ],
            ),
            int_slider::<Self>("opacity", "Opacity", 0, 100, 100, |e: &mut Self, v| {
                e.opacity = v.clamp(0, 100)
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        use super::helpers as dh;
        if self.opacity < 1
            || (self.size_mode != DrawingImageSizeMode::DontResize
                && self.size.width <= 0
                && self.size.height <= 0)
        {
            return Ok(source.clone());
        }
        let path = expand_environment_variables(self.image_location.trim());
        if is_blank(&path) || !file_exists(&path) {
            return Ok(source.clone());
        }
        let Some(decoded) = decode_file(&path) else {
            return Ok(source.clone());
        };
        if decoded.width() == 0 || decoded.height() == 0 {
            return Ok(source.clone());
        }
        let watermark = dh::rotate_flip(&decoded, self.rotate_flip);
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        let (ww, wh) = (watermark.width() as f32, watermark.height() as f32);
        let pct = |v: i32, full: f32| (v as f32 / 100.0 * full).round_ties_even() as i32;
        let size = match self.size_mode {
            DrawingImageSizeMode::AbsoluteSize => dh::apply_aspect_ratio(
                if self.size.width == -1 {
                    sw
                } else {
                    self.size.width
                },
                if self.size.height == -1 {
                    sh
                } else {
                    self.size.height
                },
                &watermark,
            ),
            DrawingImageSizeMode::PercentageOfWatermark => dh::apply_aspect_ratio(
                pct(self.size.width, ww),
                pct(self.size.height, wh),
                &watermark,
            ),
            DrawingImageSizeMode::PercentageOfCanvas => dh::apply_aspect_ratio(
                pct(self.size.width, sw as f32),
                pct(self.size.height, sh as f32),
                &watermark,
            ),
            DrawingImageSizeMode::DontResize => {
                SizeI::new(watermark.width() as i32, watermark.height() as i32)
            }
        };
        if size.width <= 0 || size.height <= 0 {
            return Ok(source.clone());
        }
        let pos = dh::get_position(self.placement, self.offset, SizeI::new(sw, sh), size);
        let (l, t, r, b) = (pos.x, pos.y, pos.x + size.width, pos.y + size.height);
        if self.auto_hide && !(l >= 0 && t >= 0 && r <= sw && b <= sh) {
            return Ok(source.clone());
        }
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let sampling = dh::sampling(self.interpolation_mode);
        let blend = if self.compositing_mode == DrawingCompositingMode::SourceCopy {
            Blend::Src
        } else {
            Blend::SrcOver
        };
        let modulate = (self.opacity < 100).then(|| {
            [
                255,
                255,
                255,
                (255.0 * (self.opacity as f32 / 100.0)).round_ties_even() as u8,
            ]
        });
        if self.tile {
            let mut paint = Paint::shader(Shader::Image {
                image: &watermark,
                matrix: Matrix::IDENTITY,
                sampling,
                tile_x: TileMode::Repeat,
                tile_y: TileMode::Repeat,
            });
            paint.anti_alias = true;
            paint.blend = blend;
            paint.modulate = modulate;
            let saved = canvas.matrix;
            canvas.translate(l as f32, t as f32);
            sk::draw_rect_xywh(
                &mut canvas,
                0.0,
                0.0,
                (r - l) as f32,
                (b - t) as f32,
                &paint,
                None,
            );
            canvas.matrix = saved;
        } else {
            let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
            paint.anti_alias = true;
            paint.blend = blend;
            paint.modulate = modulate;
            canvas.draw_image_rect(
                &watermark,
                None,
                RRect::ltrb(l as f32, t as f32, r as f32, b as f32),
                sampling,
                Some(&paint),
            );
        }
        drop(canvas);
        Ok(out)
    }
}
