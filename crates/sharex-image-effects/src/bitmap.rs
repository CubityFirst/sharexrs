//! The image the effects work on: ShareX's `SKBitmap` (`SkiaImageHelpers.CreateBitmap`,
//! `Bgra8888`, premultiplied). Kept as premultiplied RGBA bytes: every operation ShareX
//! does treats the colour channels alike, so the channel order doesn't change a result.

use crate::skia;

/// A premultiplied RGBA8 image, possibly empty (ShareX's effects can make a 0 px wide one).
#[derive(Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl std::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bitmap({}x{})", self.width, self.height)
    }
}

impl Bitmap {
    /// `SkiaImageHelpers.CreateBitmap`: transparent.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; width as usize * height as usize * 4],
        }
    }

    /// From premultiplied RGBA bytes.
    pub fn from_premultiplied(width: u32, height: u32, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            width as usize * height as usize * 4,
            "premultiplied RGBA size"
        );
        Self {
            width,
            height,
            data,
        }
    }

    /// From straight (unpremultiplied) RGBA bytes, premultiplied as Skia does when it
    /// decodes or converts (`SkOpts::RGBA_to_rgbA`: `(c * a + 127) / 255`).
    pub fn from_straight(width: u32, height: u32, rgba: &[u8]) -> Self {
        assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "straight RGBA size"
        );
        let mut data = rgba.to_vec();
        for p in data.chunks_exact_mut(4) {
            let a = p[3];
            p[0] = skia::premul_byte(p[0], a);
            p[1] = skia::premul_byte(p[1], a);
            p[2] = skia::premul_byte(p[2], a);
        }
        Self {
            width,
            height,
            data,
        }
    }

    /// Straight RGBA bytes, unpremultiplied as Skia's `ReadPixels` does (for saving).
    pub fn to_straight(&self) -> Vec<u8> {
        let mut out = self.data.clone();
        for p in out.chunks_exact_mut(4) {
            let c = skia::unpremul([p[0], p[1], p[2], p[3]]);
            p.copy_from_slice(&c);
        }
        out
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Premultiplied RGBA bytes, row by row.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    /// The premultiplied pixel at (x, y).
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, p: [u8; 4]) {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        self.data[i..i + 4].copy_from_slice(&p);
    }

    pub(crate) fn px(&self, i: usize) -> [u8; 4] {
        let i = i * 4;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    pub(crate) fn set_px(&mut self, i: usize, p: [u8; 4]) {
        let i = i * 4;
        self.data[i..i + 4].copy_from_slice(&p);
    }

    /// `bitmap.CreateEmptyBitmap(widthOffset, heightOffset)`.
    pub(crate) fn empty_like(&self, width_offset: i32, height_offset: i32) -> Bitmap {
        Bitmap::new(
            (self.width as i32 + width_offset).max(0) as u32,
            (self.height as i32 + height_offset).max(0) as u32,
        )
    }

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
}
