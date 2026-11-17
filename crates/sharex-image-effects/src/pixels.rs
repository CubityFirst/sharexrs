//! `SkiaPixelBuffer` (HelpersLib/SkiaPixelBuffer.cs): the straight-alpha pixels ShareX's
//! own pixel loops work on. Locking converts premultiplied to straight as Skia's
//! `ReadPixels` does, unlocking converts back (`(c * a + 127) / 255`), so a loop that
//! leaves a pixel alone can still change it by a rounding step, exactly as in ShareX.

use crate::{Bitmap, skia};

/// Straight RGBA pixels (ShareX's `ColorBgra`, channel order aside).
pub struct PixelBuffer {
    pub width: usize,
    pub height: usize,
    pub px: Vec<[u8; 4]>,
}

impl PixelBuffer {
    /// `new SkiaPixelBuffer(bmp, true, ReadOnly/ReadWrite)`.
    pub fn read(bmp: &Bitmap) -> Self {
        let px = bmp
            .data()
            .chunks_exact(4)
            .map(|p| skia::unpremul([p[0], p[1], p[2], p[3]]))
            .collect();
        PixelBuffer {
            width: bmp.width() as usize,
            height: bmp.height() as usize,
            px,
        }
    }

    /// `WriteOnly`: starts transparent.
    pub fn blank(width: usize, height: usize) -> Self {
        PixelBuffer {
            width,
            height,
            px: vec![[0; 4]; width * height],
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> [u8; 4] {
        self.px[y * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, c: [u8; 4]) {
        self.px[y * self.width + x] = c;
    }

    /// `Unlock`: writes the pixels back, premultiplied.
    pub fn write(&self, bmp: &mut Bitmap) {
        for (i, p) in self.px.iter().enumerate() {
            bmp.set_px(i, skia::premul(*p));
        }
    }

    pub fn into_bitmap(self) -> Bitmap {
        let mut bmp = Bitmap::new(self.width as u32, self.height as u32);
        self.write(&mut bmp);
        bmp
    }
}
