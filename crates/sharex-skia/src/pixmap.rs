//! A premultiplied N32 pixmap, as Skia's `kN32_SkColorType` on a
//! little-endian machine (BGRA bytes): each pixel a `u32` of
//! `a << 24 | r << 16 | g << 8 | b`.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixmap {
    width: u32,
    height: u32,
    data: Vec<u32>,
}

#[inline]
pub fn get_a(c: u32) -> u32 {
    c >> 24
}

#[inline]
pub fn pack(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

impl Pixmap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width * height) as usize],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u32] {
        &self.data
    }

    pub fn pixels_mut(&mut self) -> &mut [u32] {
        &mut self.data
    }

    pub fn pixel(&self, x: i32, y: i32) -> u32 {
        self.data[(y as u32 * self.width + x as u32) as usize]
    }

    pub fn pixel_mut(&mut self, x: i32, y: i32) -> &mut u32 {
        &mut self.data[(y as u32 * self.width + x as u32) as usize]
    }

    /// Pixels `x0..x1` of row `y`.
    pub fn row_span(&mut self, y: i32, x0: i32, x1: i32) -> &mut [u32] {
        let start = (y as u32 * self.width) as usize;
        &mut self.data[start + x0 as usize..start + x1 as usize]
    }

    /// From straight (unpremultiplied) RGBA bytes, premultiplying as
    /// `SkPremultiplyARGBInline` does.
    pub fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> Self {
        let data = rgba
            .chunks_exact(4)
            .map(|p| {
                crate::blitter::premultiply_color(
                    p[3] as u32,
                    p[0] as u32,
                    p[1] as u32,
                    p[2] as u32,
                )
            })
            .collect();
        Self {
            width,
            height,
            data,
        }
    }

    /// From premultiplied RGBA bytes.
    pub fn from_premul_rgba(width: u32, height: u32, rgba: &[u8]) -> Self {
        let data = rgba
            .chunks_exact(4)
            .map(|p| pack(p[3] as u32, p[0] as u32, p[1] as u32, p[2] as u32))
            .collect();
        Self {
            width,
            height,
            data,
        }
    }

    /// Premultiplied RGBA bytes.
    pub fn to_premul_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len() * 4);
        for &c in &self.data {
            out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]);
        }
        out
    }

    /// Straight RGBA bytes, unpremultiplied as Skia's raster pipeline
    /// does when encoding a PNG (`unpremul`: `c * (1/a)` in floats,
    /// rounded).
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len() * 4);
        for &c in &self.data {
            let a = c >> 24;
            let (r, g, b) = ((c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF);
            if a == 255 || a == 0 {
                out.extend_from_slice(&[r as u8, g as u8, b as u8, a as u8]);
            } else {
                let inv = 255.0 / a as f32;
                let u = |v: u32| ((v as f32 * inv + 0.5) as u32).min(255) as u8;
                out.extend_from_slice(&[u(r), u(g), u(b), a as u8]);
            }
        }
        out
    }
}
