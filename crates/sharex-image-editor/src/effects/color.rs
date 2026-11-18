//! `SKColor` and how SkiaSharp moves pixels in and out of an `SKBitmap`, as
//! the effects' pixel loops see them:
//!
//! - `SKBitmap.Pixels` (get) and `GetPixel`: each pixel unpremultiplied
//!   through `SkUnPreMultiply`'s table (probed: identical to `GetPixel` for
//!   every colour and alpha);
//! - `SKBitmap.Pixels` (set): an unpremultiplied BGRA bitmap drawn with
//!   `kSrc`, which premultiplies as `(c * a + 127) / 255` (probed, exact);
//! - `AdjustmentImageEffectBase.ApplyPixelOperation` reads and writes the
//!   premultiplied bytes as they are (a `Bgra8888` bitmap's memory as
//!   `SKColor`s).

use sharex_skia::raster::Bitmap;

/// `SKColor`: straight (unpremultiplied) 8-bit channels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SkColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub const TRANSPARENT: SkColor = SkColor::rgba(0, 0, 0, 0);
pub const BLACK: SkColor = SkColor::rgb(0, 0, 0);
pub const WHITE: SkColor = SkColor::rgb(255, 255, 255);

impl SkColor {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        SkColor { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        SkColor { r, g, b, a: 255 }
    }

    /// `new SKColor(uint argb)`.
    pub const fn argb(v: u32) -> Self {
        SkColor {
            a: (v >> 24) as u8,
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
        }
    }

    pub const fn to_argb(self) -> u32 {
        (self.a as u32) << 24 | (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }

    /// `WithAlpha`.
    pub const fn with_alpha(self, a: u8) -> Self {
        SkColor { a, ..self }
    }

    /// `[r, g, b, a]`.
    pub const fn rgba_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// `#AARRGGBB`.
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.a, self.r, self.g, self.b)
    }

    /// `#AARRGGBB` or `#RRGGBB`.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('#');
        let v = u32::from_str_radix(s, 16).ok()?;
        match s.len() {
            8 => Some(Self::argb(v)),
            6 => Some(Self::argb(0xFF00_0000 | v)),
            _ => None,
        }
    }

    /// Premultiplied bytes as a `Bgra8888` bitmap holds them, read as a
    /// colour (`ApplyPixelOperation`).
    #[inline]
    pub const fn from_raw(p: [u8; 4]) -> Self {
        SkColor::rgba(p[0], p[1], p[2], p[3])
    }

    /// Premultiplied as `SKBitmap.Pixels`'s setter stores it.
    #[inline]
    pub fn premultiplied(self) -> [u8; 4] {
        let pm = |c: u8| ((c as u32 * self.a as u32 + 127) / 255) as u8;
        [pm(self.r), pm(self.g), pm(self.b), self.a]
    }

    /// `SKColor.FromHsl` (h in degrees, s and l in percent).
    pub fn from_hsl(h: f32, s: f32, l: f32, a: u8) -> Self {
        let [r, g, b] = hsl_f(h, s, l);
        SkColor::rgba(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            a,
        )
    }

    /// `SKColor.FromHsv` (h in degrees, s and v in percent).
    pub fn from_hsv(h: f32, s: f32, v: f32, a: u8) -> Self {
        let [r, g, b] = hsv_f(h, s, v);
        SkColor::rgba(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            a,
        )
    }

    /// `SKColor.ToHsl`: h in degrees, s and l in percent.
    pub fn to_hsl(self) -> (f32, f32, f32) {
        let (r, g, b) = (
            self.r as i32 as f32 / 255.0,
            self.g as i32 as f32 / 255.0,
            self.b as i32 as f32 / 255.0,
        );
        let min = r.min(g).min(b);
        let max = r.max(g).max(b);
        let delta = max - min;
        let mut h = 0f32;
        let mut s = 0f32;
        let l = (max + min) / 2.0;
        if delta.abs() > 0.001 {
            s = if l < 0.5 {
                delta / (max + min)
            } else {
                delta / (2.0 - max - min)
            };
            h = hue_of(r, g, b, max, delta);
        }
        (h * 360.0, s * 100.0, l * 100.0)
    }

    /// `SKColor.ToHsv`: h in degrees, s and v in percent.
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let (r, g, b) = (
            self.r as i32 as f32 / 255.0,
            self.g as i32 as f32 / 255.0,
            self.b as i32 as f32 / 255.0,
        );
        let min = r.min(g).min(b);
        let max = r.max(g).max(b);
        let delta = max - min;
        let mut h = 0f32;
        let mut s = 0f32;
        let v = max;
        if delta.abs() > 0.001 {
            s = delta / max;
            h = hue_of(r, g, b, max, delta);
        }
        (h * 360.0, s * 100.0, v * 100.0)
    }
}

/// `SKColorF.ToHsl`/`ToHsv`'s hue, 0..1.
fn hue_of(r: f32, g: f32, b: f32, max: f32, delta: f32) -> f32 {
    let dr = ((max - r) / 6.0 + delta / 2.0) / delta;
    let dg = ((max - g) / 6.0 + delta / 2.0) / delta;
    let db = ((max - b) / 6.0 + delta / 2.0) / delta;
    let mut h = if (r - max).abs() < 0.001 {
        db - dg
    } else if (g - max).abs() < 0.001 {
        1.0 / 3.0 + dr - db
    } else {
        2.0 / 3.0 + dg - dr
    };
    if h < 0.0 {
        h += 1.0;
    }
    if h > 1.0 {
        h -= 1.0;
    }
    h
}

/// `SKColorF.FromHsl`.
pub fn hsl_f(h: f32, s: f32, l: f32) -> [f32; 3] {
    let h = h / 360.0;
    let s = s / 100.0;
    let l = l / 100.0;
    if s.abs() > 0.001 {
        let v2 = if !(l < 0.5) {
            l + s - s * l
        } else {
            l * (1.0 + s)
        };
        let v1 = 2.0 * l - v2;
        [
            hue_to_rgb(v1, v2, h + 1.0 / 3.0),
            hue_to_rgb(v1, v2, h),
            hue_to_rgb(v1, v2, h - 1.0 / 3.0),
        ]
    } else {
        [l, l, l]
    }
}

fn hue_to_rgb(v1: f32, v2: f32, mut vh: f32) -> f32 {
    if vh < 0.0 {
        vh += 1.0;
    }
    if vh > 1.0 {
        vh -= 1.0;
    }
    if 6.0 * vh < 1.0 {
        return v1 + (v2 - v1) * 6.0 * vh;
    }
    if 2.0 * vh < 1.0 {
        return v2;
    }
    if 3.0 * vh < 2.0 {
        return v1 + (v2 - v1) * (2.0 / 3.0 - vh) * 6.0;
    }
    v1
}

/// `SKColorF.FromHsv`.
pub fn hsv_f(h: f32, s: f32, v: f32) -> [f32; 3] {
    let mut h = h / 360.0;
    let s = s / 100.0;
    let v = v / 100.0;
    if s.abs() > 0.001 {
        h *= 6.0;
        if (h - 6.0).abs() < 0.001 {
            h = 0.0;
        }
        let i = h as i32;
        let p = v * (1.0 - s);
        let q = v * (1.0 - s * (h - i as f32));
        let t = v * (1.0 - s * (1.0 - (h - i as f32)));
        match i {
            0 => [v, t, p],
            1 => [q, v, p],
            2 => [p, v, t],
            3 => [p, q, v],
            4 => [t, p, v],
            _ => [v, p, q],
        }
    } else {
        [v, v, v]
    }
}

/// `SkUnPreMultiply::PMColorToColor`.
#[inline]
pub fn unpremultiply(p: [u8; 4]) -> SkColor {
    let a = p[3] as u32;
    let un = |v: u8| -> u8 {
        if a == 0 {
            0
        } else if a == 255 {
            v
        } else {
            let scale = ((255u32 << 24) + a / 2) / a;
            ((v as u32 * scale + (1 << 23)) >> 24) as u8
        }
    };
    SkColor::rgba(un(p[0]), un(p[1]), un(p[2]), p[3])
}

/// `SKBitmap.Pixels` (get).
pub fn pixels(b: &Bitmap) -> Vec<SkColor> {
    b.data()
        .chunks_exact(4)
        .map(|p| unpremultiply([p[0], p[1], p[2], p[3]]))
        .collect()
}

/// `SKBitmap.Pixels` (set): every pixel replaced.
pub fn set_pixels(b: &mut Bitmap, px: &[SkColor]) {
    assert_eq!(px.len(), b.width() as usize * b.height() as usize);
    for (d, c) in b.data_mut().chunks_exact_mut(4).zip(px) {
        d.copy_from_slice(&c.premultiplied());
    }
}

/// `new SKBitmap(w, h) { Pixels = px }`.
pub fn from_pixels(width: u32, height: u32, px: &[SkColor]) -> Bitmap {
    let mut b = Bitmap::new(width, height);
    set_pixels(&mut b, px);
    b
}

/// `SKBitmap.GetPixel`.
#[inline]
pub fn get_pixel(b: &Bitmap, x: u32, y: u32) -> SkColor {
    unpremultiply(b.pixel(x, y))
}
