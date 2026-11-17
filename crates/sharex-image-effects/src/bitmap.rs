//! The image the effects work on: ShareX's `SKBitmap` (`SkiaImageHelpers.CreateBitmap`,
//! `Bgra8888`, premultiplied), [`Bitmap`] from the shared Skia port, and loading one
//! from a file as ShareX does.

pub use sharex_skia::raster::Bitmap;

/// Decodes an image file as `SkiaImageHelpers.LoadImage` does (premultiplied,
/// EXIF orientation applied); `None` where ShareX logs an error and gets null.
pub fn load(path: &std::path::Path) -> Option<Bitmap> {
    let reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = image::ImageDecoder::orientation(&mut decoder).ok();
    let mut img = image::DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let rgba = img.to_rgba8();
    Some(Bitmap::from_straight(
        rgba.width(),
        rgba.height(),
        rgba.as_raw(),
    ))
}
