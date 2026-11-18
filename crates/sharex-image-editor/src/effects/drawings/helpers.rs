//! `Core/ImageEffects/Drawings/DrawingEffectHelpers.cs`.

#![allow(dead_code)]

use super::enums::{DrawingImageRotateFlipType, DrawingInterpolationMode, DrawingPlacement};
use crate::effects::prelude::*;

/// `GetPosition`: where an object of `object` size goes on `background`.
pub(crate) fn get_position(placement: DrawingPlacement, offset: PointI, background: SizeI, object: SizeI) -> PointI {
    let mid_x = (background.width as f32 / 2.0 - object.width as f32 / 2.0).round_ties_even() as i32;
    let mid_y = (background.height as f32 / 2.0 - object.height as f32 / 2.0).round_ties_even() as i32;
    let right = background.width - object.width;
    let bottom = background.height - object.height;
    match placement {
        DrawingPlacement::TopCenter => PointI::new(mid_x, offset.y),
        DrawingPlacement::TopRight => PointI::new(right - offset.x, offset.y),
        DrawingPlacement::MiddleLeft => PointI::new(offset.x, mid_y),
        DrawingPlacement::MiddleCenter => PointI::new(mid_x, mid_y),
        DrawingPlacement::MiddleRight => PointI::new(right - offset.x, mid_y),
        DrawingPlacement::BottomLeft => PointI::new(offset.x, bottom - offset.y),
        DrawingPlacement::BottomCenter => PointI::new(mid_x, bottom - offset.y),
        DrawingPlacement::BottomRight => PointI::new(right - offset.x, bottom - offset.y),
        DrawingPlacement::TopLeft => offset,
    }
}

/// `ApplyAspectRatio`: a 0 dimension follows the bitmap's aspect ratio.
pub(crate) fn apply_aspect_ratio(width: i32, height: i32, bitmap: &Bitmap) -> SizeI {
    let (bw, bh) = (bitmap.width() as f32, bitmap.height() as f32);
    if width == 0 {
        SizeI::new((height as f32 / bh * bw).round_ties_even() as i32, height)
    } else if height == 0 {
        SizeI::new(width, (width as f32 / bw * bh).round_ties_even() as i32)
    } else {
        SizeI::new(width, height)
    }
}

/// `GetSamplingOptions`. Catmull-Rom is the default (`HighQualityBicubic`).
pub(crate) fn sampling(mode: DrawingInterpolationMode) -> Sampling {
    match mode {
        DrawingInterpolationMode::Bicubic | DrawingInterpolationMode::HighQualityBilinear => Sampling::Mitchell,
        DrawingInterpolationMode::Bilinear => Sampling::Linear,
        DrawingInterpolationMode::NearestNeighbor => Sampling::Nearest,
        DrawingInterpolationMode::HighQualityBicubic => Sampling::CatmullRom,
    }
}

/// `RotateFlip`.
pub(crate) fn rotate_flip(source: &Bitmap, mode: DrawingImageRotateFlipType) -> Bitmap {
    use DrawingImageRotateFlipType as R;
    match mode {
        R::Rotate90 => rotate(source, 90),
        R::Rotate180 => rotate(source, 180),
        R::Rotate270 => rotate(source, 270),
        R::FlipX => flip(source, true),
        R::FlipY => flip(source, false),
        R::Rotate90FlipX => flip(&rotate(source, 90), true),
        R::Rotate90FlipY => flip(&rotate(source, 90), false),
        R::None => source.clone(),
    }
}

fn rotate(source: &Bitmap, degrees: i32) -> Bitmap {
    let (w, h) = (source.width(), source.height());
    let (nw, nh, tx, ty) = match degrees {
        90 => (h, w, h as f32, 0.0),
        180 => (w, h, w as f32, h as f32),
        _ => (h, w, 0.0, w as f32),
    };
    let mut out = Bitmap::new(nw, nh);
    let mut canvas = Canvas::new(&mut out);
    canvas.translate(tx, ty);
    canvas.rotate(degrees as f32);
    canvas.draw_bitmap(source, 0.0, 0.0, None);
    drop(canvas);
    out
}

fn flip(source: &Bitmap, horizontal: bool) -> Bitmap {
    let (w, h) = (source.width(), source.height());
    let mut out = Bitmap::new(w, h);
    let mut canvas = Canvas::new(&mut out);
    if horizontal {
        canvas.translate(w as f32, 0.0);
        canvas.scale(-1.0, 1.0);
    } else {
        canvas.translate(0.0, h as f32);
        canvas.scale(1.0, -1.0);
    }
    canvas.draw_bitmap(source, 0.0, 0.0, None);
    drop(canvas);
    out
}
