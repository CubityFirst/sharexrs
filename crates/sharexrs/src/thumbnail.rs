//! Task thumbnails and the list view's image preview, made as ShareX's
//! Avalonia/Skia pipeline makes them, so they match pixel for pixel:
//!
//! - `ThumbnailItemViewModel.DecodeToFit`: the file decoded no larger than
//!   the thumbnail box (`Bitmap.DecodeToWidth/DecodeToHeight` with
//!   `HighQuality`, which in Avalonia's Skia backend is linear sampling
//!   with linear mipmaps when shrinking: Skia's `SkMipmap` levels and its
//!   level choice, `-log2(scale) - 0.5`, blended between two levels).
//! - The `Image` (`Stretch="Uniform"`) in its box: centred at a whole pixel,
//!   scaled up with bilinear sampling, its edges anti-aliased, clipped to
//!   the box's rounded corners (`CornerRadius="4" ClipToBounds="True"`).
//!
//! Measured against ShareX's thumbnails of the same files: identical but
//! for the odd pixel off by one.

use image::RgbaImage;

/// A premultiplied RGBA image in floats, one level of a mip chain.
struct Level {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Level {
    fn from_image(img: &RgbaImage) -> Self {
        let px = img
            .pixels()
            .map(|p| {
                let a = p[3] as f32 / 255.0;
                [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, p[3] as f32]
            })
            .collect();
        Self { w: img.width() as usize, h: img.height() as usize, px }
    }

    fn at(&self, x: isize, y: isize) -> [f32; 4] {
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        self.px[y * self.w + x]
    }

    /// `SkMipmap`'s next level: half the size (rounded down, at least 1),
    /// each pixel the 2-tap box (even dimension) or 1-2-1 (odd) of the
    /// pixels at 2x, 2x+1(, 2x+2), truncated as Skia's integer maths does.
    fn half(&self) -> Self {
        let (w, h) = ((self.w / 2).max(1), (self.h / 2).max(1));
        let taps = |n: usize| -> &'static [(isize, f32)] {
            if n == 1 {
                &[(0, 1.0)]
            } else if n % 2 == 1 {
                &[(0, 1.0), (1, 2.0), (2, 1.0)]
            } else {
                &[(0, 1.0), (1, 1.0)]
            }
        };
        let (tx, ty) = (taps(self.w), taps(self.h));
        let sum = |t: &[(isize, f32)]| t.iter().map(|(_, w)| w).sum::<f32>();
        let div = sum(tx) * sum(ty);
        let mut px = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for &(dy, wy) in ty {
                    for &(dx, wx) in tx {
                        let p = self.at(2 * x as isize + dx, 2 * y as isize + dy);
                        for c in 0..4 {
                            acc[c] += p[c] * wx * wy;
                        }
                    }
                }
                px.push(acc.map(|v| (v / div).floor()));
            }
        }
        Self { w, h, px }
    }

    /// Bilinear sampling at `(u, v)` in this level's pixels (pixel centres
    /// at .5), clamped at the edges.
    fn bilinear(&self, u: f32, v: f32) -> [f32; 4] {
        let (u, v) = (u - 0.5, v - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = (u - x0, v - y0);
        let (x0, y0) = (x0 as isize, y0 as isize);
        let (a, b, c, d) = (self.at(x0, y0), self.at(x0 + 1, y0), self.at(x0, y0 + 1), self.at(x0 + 1, y0 + 1));
        let mut out = [0.0; 4];
        for i in 0..4 {
            out[i] = (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy;
        }
        out
    }
}

/// An image ready to be sampled at any scale: its mip levels, built as
/// needed.
struct Sampler {
    levels: Vec<Level>,
}

impl Sampler {
    fn new(img: &RgbaImage) -> Self {
        Self { levels: vec![Level::from_image(img)] }
    }

    fn level(&mut self, i: usize) -> &Level {
        while self.levels.len() <= i {
            let last = self.levels.last().expect("base");
            if last.w == 1 && last.h == 1 {
                break;
            }
            let next = last.half();
            self.levels.push(next);
        }
        &self.levels[i.min(self.levels.len() - 1)]
    }

    /// The colour at `(u, v)` in base pixels, for drawing at `scale` (the
    /// smaller of the two axes' scales): `SkMipmap::ComputeLevel` and the
    /// accessor's blend of the two nearest levels.
    fn sample(&mut self, u: f32, v: f32, scale: f32) -> [f32; 4] {
        let lod = if scale < 1.0 { (-scale.log2() - 0.5).max(0.0) } else { 0.0 };
        let base_w = self.levels[0].w as f32;
        let base_h = self.levels[0].h as f32;
        let at = |l: &Level| l.bilinear(u * l.w as f32 / base_w, v * l.h as f32 / base_h);
        if lod <= 0.0 {
            return at(&self.levels[0]);
        }
        let i = lod.floor() as usize;
        let t = lod - lod.floor();
        let upper = at(self.level(i));
        if t == 0.0 {
            return upper;
        }
        let lower = at(self.level(i + 1));
        let mut out = [0.0; 4];
        for c in 0..4 {
            out[c] = upper[c] * (1.0 - t) + lower[c] * t;
        }
        out
    }
}

/// Premultiplied floats to an 8-bit straight-alpha pixel, rounded.
fn to_rgba(p: [f32; 4], coverage: f32) -> image::Rgba<u8> {
    let a = p[3] * coverage;
    if a <= 0.0 {
        return image::Rgba([0, 0, 0, 0]);
    }
    let un = |c: f32| (c * coverage / a * 255.0).round().clamp(0.0, 255.0) as u8;
    let a8 = a.round().clamp(0.0, 255.0) as u8;
    if p[3] >= 254.5 {
        // Opaque: no division, so colours stay exact.
        return image::Rgba([p[0].round() as u8, p[1].round() as u8, p[2].round() as u8, a8]);
    }
    image::Rgba([un(p[0]), un(p[1]), un(p[2]), a8])
}

/// `SKBitmap.Resize(width, height)` with Avalonia's `HighQuality`.
pub fn resize(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    if img.dimensions() == (width, height) {
        return img.clone();
    }
    let (sx, sy) = (width as f32 / img.width() as f32, height as f32 / img.height() as f32);
    let mut s = Sampler::new(img);
    RgbaImage::from_fn(width, height, |x, y| {
        let p = s.sample((x as f32 + 0.5) / sx, (y as f32 + 0.5) / sy, sx.min(sy));
        to_rgba(p, 1.0)
    })
}

/// `ThumbnailItemViewModel.DecodeToFit`: the image no larger than
/// `width` x `height`, by width when it's relatively wider, else by height
/// (`DecodeToWidth/Height`: the other side truncated), never enlarged.
pub fn decode_to_fit(img: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let (w, h) = (img.width() as u64, img.height() as u64);
    if w == 0 || h == 0 {
        return img.clone();
    }
    let (dw, dh) = if w * height as u64 > h * width as u64 {
        let dw = (width as u64).min(w);
        (dw, ((h as f64 / w as f64) * dw as f64) as u64)
    } else {
        let dh = (height as u64).min(h);
        (((w as f64 / h as f64) * dh as f64) as u64, dh)
    };
    resize(img, dw.max(1) as u32, dh.max(1) as u32)
}

/// `BitmapConversionHelpers.CreatePreview(source, maximumSize)`: the image
/// no larger than the box (`Math.Round`ed sides, never enlarged), resized
/// with Skia's Mitchell cubic (`SKBitmap.Resize`).
pub fn create_preview(img: &RgbaImage, max_w: u32, max_h: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || max_w == 0 || max_h == 0 {
        return img.clone();
    }
    let scale = (max_w as f64 / w as f64).min(max_h as f64 / h as f64).min(1.0);
    let dw = ((w as f64 * scale).round_ties_even() as u32).max(1);
    let dh = ((h as f64 * scale).round_ties_even() as u32).max(1);
    if (dw, dh) == (w, h) {
        return img.clone();
    }
    let bmp = sharex_tools::imaging::Bitmap::from_straight(w, h, img.as_raw());
    let out = sharex_tools::imaging::resize_image(bmp, dw as i32, dh as i32, sharex_tools::imaging::SamplingMode::HighQualityBicubic);
    crate::animation::straight(out)
}

/// How an `Image` with `Stretch="Uniform"` scales its bitmap in a box.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stretch {
    /// `StretchDirection="Both"` (the default): up or down.
    Uniform,
    /// `StretchDirection="DownOnly"`: never enlarged.
    UniformDownOnly,
}

/// Draws `img` into a transparent `box_w` x `box_h` box as Avalonia's
/// `Image` does: scaled uniformly, centred on whole pixels, sampled
/// bilinearly (with mipmaps when shrinking), anti-aliased at its edges and
/// clipped to the box's rounded corners (`radius`, 0 for none).
pub fn fit(img: &RgbaImage, box_w: u32, box_h: u32, stretch: Stretch, radius: f32) -> RgbaImage {
    fit_at(img, box_w, box_h, stretch, radius, 1.0)
}

/// [`fit`] into a box of physical pixels at `ppp` pixels per point: a 96 DPI
/// `Bitmap`'s natural size is its pixels in DIPs, so `DownOnly` lets it grow
/// to `ppp` times its pixels (at 125% a 100 px image shows 125 px wide).
pub fn fit_at(img: &RgbaImage, box_w: u32, box_h: u32, stretch: Stretch, radius: f32, ppp: f32) -> RgbaImage {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let mut out = RgbaImage::new(box_w, box_h);
    if w == 0.0 || h == 0.0 || box_w == 0 || box_h == 0 {
        return out;
    }
    let mut scale = (box_w as f32 / w).min(box_h as f32 / h);
    if stretch == Stretch::UniformDownOnly {
        scale = scale.min(ppp);
    }
    let (dw, dh) = (w * scale, h * scale);
    let ox = ((box_w as f32 - dw) / 2.0).round_ties_even();
    let oy = ((box_h as f32 - dh) / 2.0).round_ties_even();
    let mut s = Sampler::new(img);
    let cover = |p: f32, o: f32, d: f32| ((p + 1.0).min(o + d) - p.max(o)).clamp(0.0, 1.0);
    for y in 0..box_h {
        let cy = cover(y as f32, oy, dh);
        if cy <= 0.0 {
            continue;
        }
        for x in 0..box_w {
            let cx = cover(x as f32, ox, dw);
            if cx <= 0.0 {
                continue;
            }
            let u = (x as f32 + 0.5 - ox) / scale;
            let v = (y as f32 + 0.5 - oy) / scale;
            let p = s.sample(u, v, scale);
            let corner = corner_coverage(x, y, box_w, box_h, radius);
            out.put_pixel(x, y, to_rgba(p, cx * cy * corner));
        }
    }
    out
}

/// How much of pixel `(x, y)` is inside a `w` x `h` rectangle with corners
/// rounded to `radius`.
pub fn corner_coverage(x: u32, y: u32, w: u32, h: u32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 1.0;
    }
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let cx = if px < radius {
        radius
    } else if px > w as f32 - radius {
        w as f32 - radius
    } else {
        return 1.0;
    };
    let cy = if py < radius {
        radius
    } else if py > h as f32 - radius {
        h as f32 - radius
    } else {
        return 1.0;
    };
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    (radius - d + 0.5).clamp(0.0, 1.0)
}

/// An `RgbaImage` as an egui image (straight alpha in, premultiplied out).
pub fn to_color_image(img: &RgbaImage) -> egui::ColorImage {
    let size = [img.width() as usize, img.height() as usize];
    let pixels = img.pixels().map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])).collect();
    egui::ColorImage { size, source_size: egui::vec2(size[0] as f32, size[1] as f32), pixels }
}

/// Loads an image file for a thumbnail or preview.
pub fn load(path: &std::path::Path) -> Option<RgbaImage> {
    match image::open(path) {
        Ok(img) => Some(img.to_rgba8()),
        Err(e) => {
            eprintln!("couldn't load {}: {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_are_never_enlarged_and_round_to_even() {
        // `CreatePreview`: the box's scale, Math.Round'ed sides.
        let img = RgbaImage::from_pixel(300, 101, image::Rgba([5, 6, 7, 255]));
        assert_eq!(create_preview(&img, 150, 150).dimensions(), (150, 50));
        assert_eq!(create_preview(&img, 600, 600).dimensions(), (300, 101));
        let opaque = create_preview(&RgbaImage::from_pixel(64, 64, image::Rgba([200, 100, 50, 255])), 16, 16);
        assert_eq!(opaque.get_pixel(8, 8).0, [200, 100, 50, 255]);
    }

    #[test]
    fn decoding_to_fit_truncates_the_other_side() {
        let img = RgbaImage::from_pixel(401, 301, image::Rgba([1, 2, 3, 255]));
        assert_eq!(decode_to_fit(&img, 200, 150).dimensions(), (199, 150));
        let wide = RgbaImage::new(601, 151);
        assert_eq!(decode_to_fit(&wide, 200, 150).dimensions(), (200, 50));
        let small = RgbaImage::new(61, 41);
        assert_eq!(decode_to_fit(&small, 200, 150).dimensions(), (61, 41));
    }

    #[test]
    fn flat_colours_stay_flat() {
        let img = RgbaImage::from_pixel(97, 33, image::Rgba([10, 200, 30, 255]));
        let r = resize(&img, 31, 10);
        assert!(r.pixels().all(|p| p.0 == [10, 200, 30, 255]));
        let f = fit(&img, 200, 150, Stretch::Uniform, 4.0);
        assert_eq!(f.get_pixel(100, 75).0, [10, 200, 30, 255]);
        assert_eq!(f.get_pixel(0, 0)[3], 0, "outside the image");
    }

    #[test]
    fn mip_levels_halve_with_skias_taps() {
        let mut img = RgbaImage::from_pixel(3, 1, image::Rgba([0, 0, 0, 255]));
        img.put_pixel(1, 0, image::Rgba([255, 0, 0, 255]));
        let l = Level::from_image(&img).half();
        assert_eq!((l.w, l.h), (1, 1));
        // (0 + 2*255 + 0) / 4, truncated.
        assert_eq!(l.px[0][0], 127.0);
    }
}

#[cfg(test)]
mod dpi_tests {
    use super::*;

    #[test]
    fn a_down_only_preview_shows_its_natural_size_in_dips() {
        // At 125% a 100x40 image in a 400x400 px box is 125x50 px; at 100%,
        // 100x40; never more than the box.
        let img = RgbaImage::from_pixel(100, 40, image::Rgba([255, 0, 0, 255]));
        let opaque = |out: &RgbaImage| {
            let row = out.height() / 2;
            (0..out.width()).filter(|&x| out.get_pixel(x, row)[3] == 255).count()
        };
        assert_eq!(opaque(&fit_at(&img, 400, 400, Stretch::UniformDownOnly, 0.0, 1.25)), 125);
        assert_eq!(opaque(&fit(&img, 400, 400, Stretch::UniformDownOnly, 0.0)), 100);
        assert_eq!(opaque(&fit_at(&img, 80, 80, Stretch::UniformDownOnly, 0.0, 1.25)), 80);
    }
}
