//! `SkiaImageHelpers` (HelpersLib/Helpers/SkiaImageHelpers*.cs), `SkiaColorMatrixManager`,
//! `SkiaConvolutionMatrixManager` and `SkiaDrawing`: the operations the effects are
//! built from, ported line for line (same loops, same integer and float types, same
//! rounding) over [`crate::skia`] for everything ShareX hands to Skia.

use crate::Bitmap;
use crate::dotnet::{
    Color, ContentAlignment, DashStyle, GradientInfo, GradientMode, Margins, Point, Sides, Size,
};
use crate::pixels::PixelBuffer;
use crate::random::RandomFast;
use crate::skia::gradient::LinearGradient;
use crate::skia::{Blend, Canvas, Matrix, Paint, Rect, Sampling, Shader, TileMode};

/// ShareX's `ImageColorMatrix`: 5x5, row-vector form (`[row][col]`).
pub type ColorMatrix5 = [[f32; 5]; 5];

/// `ImageColorMatrix.CreateFilter`: Skia's row-major 4x5 (column-vector) form.
pub fn skia_matrix(v: &ColorMatrix5) -> [f32; 20] {
    let mut out = [0.0; 20];
    for r in 0..4 {
        for c in 0..5 {
            out[r * 5 + c] = v[c][r];
        }
    }
    out
}

/// `SkiaColorMatrixManager.Apply(matrix, bmp)`: a new bitmap of the filtered pixels.
pub fn apply_color_matrix(m: &ColorMatrix5, bmp: &Bitmap) -> Bitmap {
    let mut dest = bmp.empty_like(0, 0);
    apply_color_matrix_into(m, bmp, &mut dest, 0, 0);
    dest
}

/// `SkiaColorMatrixManager.Apply(matrix, src, dest, destRect)` (same size, at an offset).
pub fn apply_color_matrix_into(m: &ColorMatrix5, src: &Bitmap, dest: &mut Bitmap, x: i32, y: i32) {
    let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
    paint.color_filter = Some(skia_matrix(m));
    let mut canvas = Canvas::new(dest);
    let r = Rect::xywh(0.0, 0.0, src.width() as f32, src.height() as f32);
    canvas.draw_image_sx(
        src,
        r,
        Rect::xywh(x as f32, y as f32, r.width(), r.height()),
        Some(&paint),
    );
}

// Grayscale weights.
const RW: f32 = 0.212671;
const GW: f32 = 0.715160;
const BW: f32 = 0.072169;

pub fn cm_inverse() -> ColorMatrix5 {
    [
        [-1.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, -1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [1.0, 1.0, 1.0, 0.0, 1.0],
    ]
}

pub fn cm_alpha(value: f32, add: f32) -> ColorMatrix5 {
    [
        [1.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, value, 0.0],
        [0.0, 0.0, 0.0, add, 1.0],
    ]
}

pub fn cm_brightness(v: f32) -> ColorMatrix5 {
    [
        [1.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [v, v, v, 0.0, 1.0],
    ]
}

pub fn cm_contrast(v: f32) -> ColorMatrix5 {
    [
        [v, 0.0, 0.0, 0.0, 0.0],
        [0.0, v, 0.0, 0.0, 0.0],
        [0.0, 0.0, v, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn cm_black_white() -> ColorMatrix5 {
    [
        [1.5, 1.5, 1.5, 0.0, 0.0],
        [1.5, 1.5, 1.5, 0.0, 0.0],
        [1.5, 1.5, 1.5, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [-1.0, -1.0, -1.0, 0.0, 1.0],
    ]
}

pub fn cm_polaroid() -> ColorMatrix5 {
    [
        [1.438, -0.062, -0.062, 0.0, 0.0],
        [-0.122, 1.378, -0.122, 0.0, 0.0],
        [-0.016, -0.016, 1.483, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [-0.03, 0.05, -0.02, 0.0, 1.0],
    ]
}

pub fn cm_grayscale(v: f32) -> ColorMatrix5 {
    [
        [RW * v, RW * v, RW * v, 0.0, 0.0],
        [GW * v, GW * v, GW * v, 0.0, 0.0],
        [BW * v, BW * v, BW * v, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn cm_sepia(v: f32) -> ColorMatrix5 {
    [
        [0.393 * v, 0.349 * v, 0.272 * v, 0.0, 0.0],
        [0.769 * v, 0.686 * v, 0.534 * v, 0.0, 0.0],
        [0.189 * v, 0.168 * v, 0.131 * v, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn cm_hue(angle: f32) -> ColorMatrix5 {
    let a = angle * (std::f64::consts::PI / 180.0) as f32;
    let c = (a as f64).cos() as f32;
    let s = (a as f64).sin() as f32;
    [
        [
            (RW + (c * (1.0 - RW))) + (s * -RW),
            (RW + (c * -RW)) + (s * 0.143),
            (RW + (c * -RW)) + (s * -(1.0 - RW)),
            0.0,
            0.0,
        ],
        [
            (GW + (c * -GW)) + (s * -GW),
            (GW + (c * (1.0 - GW))) + (s * 0.14),
            (GW + (c * -GW)) + (s * GW),
            0.0,
            0.0,
        ],
        [
            (BW + (c * -BW)) + (s * (1.0 - BW)),
            (BW + (c * -BW)) + (s * -0.283),
            (BW + (c * (1.0 - BW))) + (s * BW),
            0.0,
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn cm_saturation(v: f32) -> ColorMatrix5 {
    let i = 1.0 - v;
    [
        [(i * RW) + v, i * RW, i * RW, 0.0, 0.0],
        [i * GW, (i * GW) + v, i * GW, 0.0, 0.0],
        [i * BW, i * BW, (i * BW) + v, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0],
    ]
}

pub fn cm_colorize(color: &Color, value: f32) -> ColorMatrix5 {
    let r = color.r as f32 / 255.0;
    let g = color.g as f32 / 255.0;
    let b = color.b as f32 / 255.0;
    let inv = 1.0 - value;
    [
        [
            inv + (value * r * RW),
            value * g * RW,
            value * b * RW,
            0.0,
            0.0,
        ],
        [
            value * r * GW,
            inv + (value * g * GW),
            value * b * GW,
            0.0,
            0.0,
        ],
        [
            value * r * BW,
            value * g * BW,
            inv + (value * b * BW),
            0.0,
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, 1.0],
    ]
}

/// `Remap(v, 0, 255, 0, 1)`: `((v - 0) / (255 - 0) * (1 - 0)) + 0`.
fn remap01(v: u8) -> f32 {
    (v as f32 - 0.0) / (255.0 - 0.0) * (1.0 - 0.0) + 0.0
}

pub fn cm_mask(opacity: f32, color: &Color) -> ColorMatrix5 {
    [
        [0.0; 5],
        [0.0; 5],
        [0.0; 5],
        [0.0, 0.0, 0.0, color.a as f32 / 255.0 * opacity, 0.0],
        [
            remap01(color.r),
            remap01(color.g),
            remap01(color.b),
            0.0,
            1.0,
        ],
    ]
}

/// `SkiaColorMatrixManager.ChangeGamma`.
pub fn change_gamma(bmp: &Bitmap, value: f32) -> Bitmap {
    let value = value.clamp(0.1, 5.0);
    let table: Vec<u8> = (0..256)
        .map(|x| {
            (255.0 * (x as f64 / 255.0).powf(1.0 / value as f64))
                .round_ties_even()
                .clamp(0.0, 255.0) as u8
        })
        .collect();
    let mut px = PixelBuffer::read(bmp);
    for p in &mut px.px {
        p[0] = table[p[0] as usize];
        p[1] = table[p[1] as usize];
        p[2] = table[p[2] as usize];
    }
    px.into_bitmap()
}

/// `ConvolutionMatrix` (`[y][x]`, row-major).
pub struct Kernel {
    pub w: usize,
    pub h: usize,
    pub m: Vec<f64>,
    pub offset: u8,
    pub consider_alpha: bool,
}

impl Kernel {
    pub fn new(h: usize, w: usize) -> Self {
        Kernel {
            w,
            h,
            m: vec![0.0; w * h],
            offset: 0,
            consider_alpha: false,
        }
    }

    pub fn set(&mut self, y: usize, x: usize, v: f64) {
        self.m[y * self.w + x] = v;
    }

    pub fn get(&self, y: usize, x: usize) -> f64 {
        self.m[y * self.w + x]
    }

    pub fn set_all(&mut self, v: f64) {
        self.m.fill(v);
    }

    /// `SkiaConvolutionMatrixManager.Apply`.
    pub fn apply(&self, bmp: &Bitmap) -> Bitmap {
        let src = PixelBuffer::read(bmp);
        let mut dest = PixelBuffer::blank(src.width, src.height);
        let ox = (self.w as i32 - 1) / 2;
        let oy = (self.h as i32 - 1) / 2;
        let (w, h) = (src.width as i32, src.height as i32);
        for y in 0..h {
            for x in 0..w {
                let (mut r, mut g, mut b, mut a) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                for fy in 0..self.h {
                    let sy = (y + fy as i32 - oy).clamp(0, h - 1);
                    for fx in 0..self.w {
                        let sx = (x + fx as i32 - ox).clamp(0, w - 1);
                        let c = src.get(sx as usize, sy as usize);
                        let k = self.get(fy, fx);
                        r += k * c[0] as f64;
                        g += k * c[1] as f64;
                        b += k * c[2] as f64;
                        if self.consider_alpha {
                            a += k * c[3] as f64;
                        }
                    }
                }
                let off = self.offset as f64;
                let clamp = |v: f64| v.clamp(0.0, 255.0);
                r = clamp(r + off);
                g = clamp(g + off);
                b = clamp(b + off);
                let alpha = if self.consider_alpha {
                    clamp(a + off) as u8
                } else {
                    src.get(x as usize, y as usize)[3]
                };
                dest.set(x as usize, y as usize, [r as u8, g as u8, b as u8, alpha]);
            }
        }
        dest.into_bitmap()
    }

    pub fn smooth() -> Self {
        let mut k = Kernel::new(3, 3);
        let factor = 1.0 + 8.0;
        k.set_all(1.0 / factor);
        k.set(1, 1, 1.0 / factor);
        k
    }

    pub fn mean_removal() -> Self {
        let mut k = Kernel::new(3, 3);
        let factor = 9.0 - 8.0;
        k.set_all(-1.0 / factor);
        k.set(1, 1, 9.0 / factor);
        k
    }

    pub fn sharpen() -> Self {
        let mut k = Kernel::new(3, 3);
        let factor = 11.0 - 8.0;
        k.set_all(0.0);
        k.set(1, 1, 11.0 / factor);
        for (y, x) in [(1, 0), (0, 1), (2, 1), (1, 2)] {
            k.set(y, x, -2.0 / factor);
        }
        k
    }

    pub fn emboss() -> Self {
        let mut k = Kernel::new(3, 3);
        k.set_all(-1.0);
        k.set(1, 1, 4.0);
        for (y, x) in [(1, 0), (0, 1), (2, 1), (1, 2)] {
            k.set(y, x, 0.0);
        }
        k.offset = 127;
        k
    }

    pub fn edge_detect() -> Self {
        let mut k = Kernel::new(3, 3);
        for x in 0..3 {
            k.set(0, x, -1.0);
            k.set(1, x, 0.0);
            k.set(2, x, 1.0);
        }
        k.offset = 127;
        k
    }

    /// `SkiaConvolutionMatrixManager.GaussianBlur(height, width, sigma)`.
    pub fn gaussian(h: usize, w: usize, sigma: f64) -> Self {
        let mut k = Kernel::new(h, w);
        k.consider_alpha = true;
        let gf = |x: f64| {
            let left = 1.0 / ((2.0 * std::f64::consts::PI).sqrt() * sigma);
            let num = -x * x;
            let den = 2.0 * sigma.powf(2.0);
            left * (num / den).exp()
        };
        let mut sum = 0.0;
        let mx = (w as f64 - 1.0) / 2.0;
        let my = (h as f64 - 1.0) / 2.0;
        for y in 0..h {
            for x in 0..w {
                let v = gf(x as f64 - mx) * gf(y as f64 - my);
                k.set(y, x, v);
                sum += v;
            }
        }
        for v in &mut k.m {
            *v /= sum;
        }
        k
    }
}

/// `SkiaImageHelpers.GaussianBlur`: separable, horizontal then vertical.
pub fn gaussian_blur(bmp: &Bitmap, radius: i32) -> Bitmap {
    let size = (radius * 2 + 1) as usize;
    let sigma = radius as f64 / 3.0;
    let kh = Kernel::gaussian(1, size, sigma);
    let mut kv = Kernel::new(size, 1);
    kv.consider_alpha = kh.consider_alpha;
    for i in 0..size {
        kv.set(i, 0, kh.get(0, i));
    }
    let pass = kh.apply(bmp);
    kv.apply(&pass)
}

/// `SkiaImageHelpers.BoxBlur(bmp, range)`, in place.
pub fn box_blur(bmp: &mut Bitmap, range: i32) {
    if range <= 1 {
        return;
    }
    let range = if range % 2 == 0 { range + 1 } else { range };
    let mut px = PixelBuffer::read(bmp);
    let (w, h) = (px.width, px.height);
    box_blur_h(&mut px, range, w, h);
    box_blur_v(&mut px, range, w, h);
    box_blur_h(&mut px, range, w, h);
    box_blur_v(&mut px, range, w, h);
    px.write(bmp);
}

fn box_blur_h(px: &mut PixelBuffer, range: i32, w: usize, h: usize) {
    let half = range / 2;
    let right = w as i32;
    let mut new_colors = vec![[0u8; 4]; w];
    for y in 0..h {
        let mut hits = 0i32;
        let (mut r, mut g, mut b, mut a) = (0i32, 0i32, 0i32, 0i32);
        for x in -half..right {
            let old = x - half - 1;
            if old >= 0 {
                let c = px.get(old as usize, y);
                if c != [0; 4] {
                    r -= c[0] as i32;
                    g -= c[1] as i32;
                    b -= c[2] as i32;
                    a -= c[3] as i32;
                }
                hits -= 1;
            }
            let new = x + half;
            if new < right {
                let c = px.get(new as usize, y);
                if c != [0; 4] {
                    r += c[0] as i32;
                    g += c[1] as i32;
                    b += c[2] as i32;
                    a += c[3] as i32;
                }
                hits += 1;
            }
            if x >= 0 {
                new_colors[x as usize] = [
                    (r / hits) as u8,
                    (g / hits) as u8,
                    (b / hits) as u8,
                    (a / hits) as u8,
                ];
            }
        }
        for (x, c) in new_colors.iter().enumerate() {
            px.set(x, y, *c);
        }
    }
}

fn box_blur_v(px: &mut PixelBuffer, range: i32, w: usize, h: usize) {
    let half = range / 2;
    let bottom = h as i32;
    let mut new_colors = vec![[0u8; 4]; h];
    for x in 0..w {
        let mut hits = 0i32;
        let (mut r, mut g, mut b, mut a) = (0i32, 0i32, 0i32, 0i32);
        for y in -half..bottom {
            let old = y - half - 1;
            if old >= 0 {
                let c = px.get(x, old as usize);
                if c != [0; 4] {
                    r -= c[0] as i32;
                    g -= c[1] as i32;
                    b -= c[2] as i32;
                    a -= c[3] as i32;
                }
                hits -= 1;
            }
            let new = y + half;
            if new < bottom {
                let c = px.get(x, new as usize);
                if c != [0; 4] {
                    r += c[0] as i32;
                    g += c[1] as i32;
                    b += c[2] as i32;
                    a += c[3] as i32;
                }
                hits += 1;
            }
            if y >= 0 {
                new_colors[y as usize] = [
                    (r / hits) as u8,
                    (g / hits) as u8,
                    (b / hits) as u8,
                    (a / hits) as u8,
                ];
            }
        }
        for (y, c) in new_colors.iter().enumerate() {
            px.set(x, y, *c);
        }
    }
}

/// `SkiaImageHelpers.ColorDepth`.
pub fn color_depth(bmp: &mut Bitmap, bits: i32) {
    if !(1..=8).contains(&bits) {
        return;
    }
    let colors = 2f64.powf(bits as f64);
    let interval = 255.0 / (colors - 1.0);
    let remap =
        |c: u8| ((c as f64 / interval).round_ties_even() * interval).round_ties_even() as u8;
    let mut px = PixelBuffer::read(bmp);
    for p in &mut px.px {
        p[0] = remap(p[0]);
        p[1] = remap(p[1]);
        p[2] = remap(p[2]);
    }
    px.write(bmp);
}

/// `SkiaImageHelpers.Pixelate(bmp, pixelSize)`.
pub fn pixelate(bmp: &mut Bitmap, size: i32) {
    if size <= 1 {
        return;
    }
    let mut px = PixelBuffer::read(bmp);
    let (w, h) = (px.width as i32, px.height as i32);
    let mut y = 0;
    while y < h {
        let mut x = 0;
        while x < w {
            let xl = (x + size).min(w);
            let yl = (y + size).min(h);
            let count = ((xl - x) * (yl - y)) as f32;
            let (mut r, mut g, mut b, mut a, mut wc) = (0f32, 0f32, 0f32, 0f32, 0f32);
            for y2 in y..yl {
                for x2 in x..xl {
                    let c = px.get(x2 as usize, y2 as usize);
                    let pw = c[3] as f32 / 255.0;
                    r += c[0] as f32 * pw;
                    g += c[1] as f32 * pw;
                    b += c[2] as f32 * pw;
                    a += c[3] as f32 * pw;
                    wc += pw;
                }
            }
            let avg = [
                (r / wc) as u8,
                (g / wc) as u8,
                (b / wc) as u8,
                (a / count) as u8,
            ];
            for y2 in y..yl {
                for x2 in x..xl {
                    px.set(x2 as usize, y2 as usize, avg);
                }
            }
            x += size;
        }
        y += size;
    }
    px.write(bmp);
}

/// `SkiaImageHelpers.Pixelate(bmp, pixelSize, borderSize, borderColor)`.
pub fn pixelate_border(bmp: &mut Bitmap, size: i32, border: i32, color: &Color) {
    pixelate(bmp, size);
    if size > 1 && border > 0 && color.a > 0 {
        let mut tex = Bitmap::new(size as u32, size as u32);
        {
            let mut c = Canvas::new(&mut tex);
            draw_rectangle_proper(
                &mut c,
                &stroke_paint(Shader::Color(color.rgba()), border as f32),
                Rect::xywh(0.0, 0.0, size as f32, size as f32),
                None,
            );
        }
        let (w, h) = (bmp.width() as f32, bmp.height() as f32);
        let mut c = Canvas::new(bmp);
        c.draw_rect(
            Rect::xywh(0.0, 0.0, w, h),
            &Paint::texture(&tex, TileMode::Repeat),
        );
    }
}

/// A stroke paint (`SkiaDrawing.Stroke`): anti-aliased, round joins.
pub struct Stroke<'a> {
    pub paint: Paint<'a>,
    pub width: f32,
    pub dash: Option<Vec<f32>>,
}

pub fn stroke_paint(shader: Shader<'_>, width: f32) -> Stroke<'_> {
    let mut paint = Paint::shader(shader);
    paint.anti_alias = true;
    Stroke {
        paint,
        width,
        dash: None,
    }
}

/// `SkiaDrawing.DashEffect`.
pub fn dash_intervals(style: DashStyle, width: f32) -> Option<Vec<f32>> {
    match style {
        DashStyle::Dash => Some(vec![3.0 * width, width]),
        DashStyle::Dot => Some(vec![width, width]),
        DashStyle::DashDot => Some(vec![3.0 * width, width, width, width]),
        DashStyle::DashDotDot => Some(vec![3.0 * width, width, width, width, width, width]),
        _ => None,
    }
}

/// Strokes `path` and fills the outline.
pub fn stroke_path(canvas: &mut Canvas, path: &tiny_skia::Path, stroke: &Stroke) {
    stroke_sk_path(canvas, &sharex_skia::draw::from_tiny_skia(path), stroke);
}

/// Strokes an `SkPath` (`SkStroke`, round joins, butt caps; dashes through tiny-skia's
/// dasher) and fills the outline.
pub fn stroke_sk_path(canvas: &mut Canvas, path: &sharex_skia::Path, stroke: &Stroke) {
    let s = sharex_skia::Stroke {
        width: stroke.width,
        cap: sharex_skia::Cap::Butt,
        join: sharex_skia::Join::Round,
        miter_limit: 4.0,
        dash: stroke.dash.clone().map(|d| (d, 0.0)),
    };
    if stroke.paint.anti_alias
        && let Some(src) = sharex_skia::draw::dash_path(path, &s)
        && canvas.stroke_hairline(&src, s.width, s.cap, &stroke.paint)
    {
        return;
    }
    if let Some(outline) = sharex_skia::draw::stroke_to_path(path, &s) {
        canvas.fill_sk_path(&outline, &stroke.paint);
    }
}

/// `SkiaDrawing.DrawRectangleProper`: a stroke inset by half its width.
pub fn draw_rectangle_proper(canvas: &mut Canvas, stroke: &Stroke, r: Rect, _unused: Option<()>) {
    let inset = stroke.width / 2.0;
    let rect = Rect::xywh(
        r.left + inset,
        r.top + inset,
        (r.width() - stroke.width).max(0.0),
        (r.height() - stroke.width).max(0.0),
    );
    if let Some(path) = rect_path(rect) {
        stroke_path(canvas, &path, stroke);
    }
}

/// A closed rectangle path as `SkPath::addRect` (clockwise from the top left).
pub fn rect_path(r: Rect) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(r.left, r.top);
    pb.line_to(r.right, r.top);
    pb.line_to(r.right, r.bottom);
    pb.line_to(r.left, r.bottom);
    pb.close();
    pb.finish()
}

/// `SKPath.AddRoundRect(rect, r, r)` (`SkPath::addRoundRect`: `SkRRect` radii scaled
/// down to fit, conic corners, Skia's start point and direction).
pub fn round_rect_path(r: Rect, radius: f32) -> Option<sharex_skia::Path> {
    if r.is_empty() {
        return None;
    }
    let radius = radius.max(0.0);
    let mut path = sharex_skia::Path::new();
    path.add_round_rect(
        sharex_skia::Rect::from_ltrb(r.left, r.top, r.right, r.bottom),
        radius,
        radius,
        sharex_skia::Direction::Cw,
    );
    Some(path)
}

/// `SkiaDrawing.AddRoundedRectangleProper(rect, radius, penWidth)`.
pub fn round_rect_proper(r: Rect, radius: f32, pen: f32) -> Option<sharex_skia::Path> {
    let i = pen / 2.0;
    round_rect_path(
        Rect::ltrb(r.left + i, r.top + i, r.right - i, r.bottom - i),
        radius,
    )
}

/// `GradientInfo.GetSkiaPaint(rect)` → `SkiaDrawing.Gradient`.
pub fn gradient_shader(g: &GradientInfo, x: i32, y: i32, w: i32, h: i32) -> Shader<'static> {
    let mut stops: Vec<_> = g.colors.iter().collect();
    stops.sort_by(|a, b| {
        a.location
            .partial_cmp(&b.location)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let colors: Vec<[u8; 4]> = stops.iter().map(|s| s.color.rgba()).collect();
    let positions: Vec<f32> = stops.iter().map(|s| s.location / 100.0).collect();
    gradient_shader_raw(&colors, &positions, g.kind, x, y, w, h)
}

/// `SkiaDrawing.Gradient(rectangle, colors, positions, mode)`.
pub fn gradient_shader_raw(
    colors: &[[u8; 4]],
    positions: &[f32],
    mode: GradientMode,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Shader<'static> {
    match colors.len() {
        0 => return Shader::Color([255, 255, 255, 0]),
        1 => return Shader::Color(colors[0]),
        _ => {}
    }
    let (l, t, r, b) = (x as f32, y as f32, (x + w) as f32, (y + h) as f32);
    let (start, end) = match mode {
        GradientMode::Horizontal => ((l, t), (r, t)),
        GradientMode::Vertical => ((l, t), (l, b)),
        GradientMode::BackwardDiagonal => ((r, t), (l, b)),
        GradientMode::ForwardDiagonal => ((l, t), (r, b)),
    };
    Shader::Gradient(LinearGradient {
        start,
        end,
        colors: colors.to_vec(),
        positions: Some(positions.to_vec()),
        local: Matrix::IDENTITY,
    })
}

/// `Helpers.GetPosition(placement, offset, backgroundSize, objectSize)`.
pub fn get_position(placement: ContentAlignment, offset: Point, bg: Size, obj: Size) -> Point {
    let mid_x = ((bg.width as f32 / 2.0) - (obj.width as f32 / 2.0)).round_ties_even() as i32;
    let mid_y = ((bg.height as f32 / 2.0) - (obj.height as f32 / 2.0)).round_ties_even() as i32;
    let right = bg.width - obj.width;
    let bottom = bg.height - obj.height;
    use ContentAlignment::*;
    match placement {
        TopLeft => Point::new(offset.x, offset.y),
        TopCenter => Point::new(mid_x, offset.y),
        TopRight => Point::new(right - offset.x, offset.y),
        MiddleLeft => Point::new(offset.x, mid_y),
        MiddleCenter => Point::new(mid_x, mid_y),
        MiddleRight => Point::new(right - offset.x, mid_y),
        BottomLeft => Point::new(offset.x, bottom - offset.y),
        BottomCenter => Point::new(mid_x, bottom - offset.y),
        BottomRight => Point::new(right - offset.x, bottom - offset.y),
    }
}

/// `Rectangle.Contains(Rectangle)` against the image bounds.
pub fn bounds_contain(w: u32, h: u32, x: i32, y: i32, rw: i32, rh: i32) -> bool {
    x >= 0 && y >= 0 && x + rw <= w as i32 && y + rh <= h as i32
}

/// `SkiaImageHelpers.AddCanvas(img, margin, color)`: `None` when ShareX returns null.
pub fn add_canvas(img: &Bitmap, m: Margins, color: &Color) -> Option<Bitmap> {
    if m.all_value() == 0
        || img.width() as i32 + m.horizontal() < 1
        || img.height() as i32 + m.vertical() < 1
    {
        return None;
    }
    let mut bmp = img.empty_like(m.horizontal(), m.vertical());
    let (bw, bh) = (bmp.width() as f32, bmp.height() as f32);
    let mut g = Canvas::new(&mut bmp);
    g.draw_image_xywh(
        img,
        m.left as f32,
        m.top as f32,
        img.width() as f32,
        img.height() as f32,
    );
    if color.a > 0 {
        let mut brush = Paint::fill(color.rgba());
        brush.blend = Blend::Src;
        let fill = |g: &mut Canvas, x: f32, y: f32, w: f32, h: f32| {
            g.draw_rect(Rect::xywh(x, y, w, h), &brush)
        };
        if m.left > 0 {
            fill(&mut g, 0.0, 0.0, m.left as f32, bh);
        }
        if m.top > 0 {
            fill(&mut g, 0.0, 0.0, bw, m.top as f32);
        }
        if m.right > 0 {
            fill(&mut g, bw - m.right as f32, 0.0, m.right as f32, bh);
        }
        if m.bottom > 0 {
            fill(&mut g, 0.0, bh - m.bottom as f32, bw, m.bottom as f32);
        }
    }
    Some(bmp)
}

/// `SkiaImageHelpers.CropBitmap`: `None` when the rectangle isn't inside.
pub fn crop_bitmap(src: &Bitmap, x: i32, y: i32, w: i32, h: i32) -> Option<Bitmap> {
    if w <= 0 || h <= 0 || !bounds_contain(src.width(), src.height(), x, y, w, h) {
        return None;
    }
    let mut result = Bitmap::new(w as u32, h as u32);
    Canvas::new(&mut result).draw_bitmap(src, -x as f32, -y as f32, None);
    Some(result)
}

/// `SkiaImageHelpers.ResizeImage(source, width, height)` (Mitchell).
pub fn resize_image(src: Bitmap, w: i32, h: i32) -> Bitmap {
    if w < 1 || h < 1 || (src.width() as i32 == w && src.height() as i32 == h) {
        return src;
    }
    let mut result = Bitmap::new(w as u32, h as u32);
    Canvas::new(&mut result).draw_image_rect(
        &src,
        None,
        Rect::xywh(0.0, 0.0, w as f32, h as f32),
        Sampling::Mitchell,
        None,
    );
    result
}

/// `SkiaImageHelpers.ResizeImage(source, width, height, allowEnlarge, centerImage, backColor)`.
pub fn resize_image_fit(
    src: &Bitmap,
    w: i32,
    h: i32,
    allow_enlarge: bool,
    center: bool,
    back: &Color,
) -> Bitmap {
    let mut scale = (w as f64 / src.width() as f64).min(h as f64 / src.height() as f64);
    if !allow_enlarge {
        scale = scale.min(1.0);
    }
    let rw = ((src.width() as f64 * scale) as i32).max(1);
    let rh = ((src.height() as f64 * scale) as i32).max(1);
    let x = if center { (w - rw) / 2 } else { 0 };
    let y = if center { (h - rh) / 2 } else { 0 };
    let mut result = Bitmap::new(w.max(0) as u32, h.max(0) as u32);
    let mut c = Canvas::new(&mut result);
    c.clear(back.rgba());
    c.draw_image_xywh(src, x as f32, y as f32, rw as f32, rh as f32);
    result
}

/// `SkiaImageHelpers.ApplyAspectRatio`.
pub fn apply_aspect_ratio(width: i32, height: i32, bw: u32, bh: u32) -> (i32, i32) {
    if width == 0 {
        (
            ((height as f32 / bh as f32) * bw as f32).round_ties_even() as i32,
            height,
        )
    } else if height == 0 {
        (
            width,
            ((width as f32 / bw as f32) * bh as f32).round_ties_even() as i32,
        )
    } else {
        (width, height)
    }
}

/// `FlipInPlace`.
pub fn flip(bmp: &Bitmap, horizontal: bool, vertical: bool) -> Bitmap {
    let mut result = bmp.empty_like(0, 0);
    let mut c = Canvas::new(&mut result);
    c.translate(
        if horizontal { bmp.width() as f32 } else { 0.0 },
        if vertical { bmp.height() as f32 } else { 0.0 },
    );
    c.scale(
        if horizontal { -1.0 } else { 1.0 },
        if vertical { -1.0 } else { 1.0 },
    );
    c.draw_bitmap(bmp, 0.0, 0.0, None);
    result
}

/// `RotateFlipInPlace`.
pub fn rotate_flip(bmp: &Bitmap, rotate_flip: i32) -> Bitmap {
    let rotation = rotate_flip & 3;
    let swap = rotation % 2 == 1;
    let (w, h) = if swap {
        (bmp.height(), bmp.width())
    } else {
        (bmp.width(), bmp.height())
    };
    let mut result = Bitmap::new(w, h);
    let mut c = Canvas::new(&mut result);
    c.translate(w as f32 / 2.0, h as f32 / 2.0);
    if rotate_flip & 4 != 0 {
        c.scale(-1.0, 1.0);
    }
    c.rotate((rotation * 90) as f32);
    c.translate(-(bmp.width() as f32) / 2.0, -(bmp.height() as f32) / 2.0);
    c.draw_bitmap(bmp, 0.0, 0.0, None);
    result
}

/// `SkiaImageHelpers.RotateImage`.
pub fn rotate_image(bmp: &Bitmap, angle: f32, upsize: bool, clip: bool) -> Bitmap {
    if angle == 0.0 {
        return bmp.clone();
    }
    let (ow, oh) = (bmp.width() as i32, bmp.height() as i32);
    let (mut nw, mut nh) = (ow, oh);
    let mut scale = 1f32;
    if upsize || !clip {
        let rad = angle as f64 * std::f64::consts::PI / 180.0;
        let (cos, sin) = (rad.cos().abs(), rad.sin().abs());
        nw = (ow as f64 * cos + oh as f64 * sin).round_ties_even() as i32;
        nh = (ow as f64 * sin + oh as f64 * cos).round_ties_even() as i32;
    }
    if !upsize && !clip {
        scale = (ow as f32 / nw as f32).min(oh as f32 / nh as f32);
        nw = ow;
        nh = oh;
    }
    let mut result = Bitmap::new(nw as u32, nh as u32);
    let mut g = Canvas::new(&mut result);
    g.translate(nw as f32 / 2.0, nh as f32 / 2.0);
    if scale != 1.0 {
        g.scale(scale, scale);
    }
    g.rotate(angle);
    g.translate(-ow as f32 / 2.0, -oh as f32 / 2.0);
    g.draw_image_xywh(bmp, 0.0, 0.0, ow as f32, oh as f32);
    result
}

/// `SkiaImageHelpers.RoundedCorners`.
pub fn rounded_corners(bmp: &Bitmap, radius: i32) -> Bitmap {
    let mut result = bmp.empty_like(0, 0);
    if let Some(path) = round_rect_proper(
        Rect::xywh(0.0, 0.0, bmp.width() as f32, bmp.height() as f32),
        radius as f32,
        0.0,
    ) {
        Canvas::new(&mut result).fill_sk_path(&path, &Paint::texture(bmp, TileMode::Repeat));
    }
    result
}

/// `SkiaImageHelpers.MakeOutline`.
pub fn make_outline(bmp: &Bitmap, min_radius: i32, max_radius: i32, color: &Color) -> Bitmap {
    let src = PixelBuffer::read(bmp);
    let mut dest = PixelBuffer::blank(src.width, src.height);
    let (w, h) = (src.width as i32, src.height as i32);
    // Opaque pixels, for DistanceToThreshold.
    for x in 0..w {
        for y in 0..h {
            let minx = (x - max_radius).max(0);
            let maxx = (x + max_radius).min(w - 1);
            let miny = (y - max_radius).max(0);
            let maxy = (y + max_radius).min(h - 1);
            let mut dist2 = max_radius * max_radius + 1;
            for tx in minx..=maxx {
                for ty in miny..=maxy {
                    if src.get(tx as usize, ty as usize)[3] == 255 {
                        let (dx, dy) = (tx - x, ty - y);
                        let d = dx * dx + dy * dy;
                        if d < dist2 {
                            dist2 = d;
                        }
                    }
                }
            }
            let dist = (dist2 as f64).sqrt() as f32;
            if dist > min_radius as f32 && dist < max_radius as f32 {
                let mut alpha = 255u8;
                if dist - (min_radius as f32) < 1.0 {
                    alpha = (255.0 * (dist - min_radius as f32)) as u8;
                } else if max_radius as f32 - dist < 1.0 {
                    alpha = (255.0 * (max_radius as f32 - dist)) as u8;
                }
                dest.set(x as usize, y as usize, [color.r, color.g, color.b, alpha]);
            }
        }
    }
    dest.into_bitmap()
}

/// `SkiaImageHelpers.Outline`.
pub fn outline(bmp: Bitmap, size: i32, color: &Color, padding: i32, outline_only: bool) -> Bitmap {
    let outline = make_outline(&bmp, padding, padding + size + 1, color);
    if outline_only {
        return outline;
    }
    let mut bmp = bmp;
    let (w, h) = (outline.width() as f32, outline.height() as f32);
    Canvas::new(&mut bmp).draw_image_xywh(&outline, 0.0, 0.0, w, h);
    bmp
}

/// `SkiaImageHelpers.AddReflection`.
pub fn add_reflection(src: &Bitmap, percentage: i32, max_alpha: i32, min_alpha: i32) -> Bitmap {
    let height = ((src.height() as f32 * percentage.clamp(1, 100) as f32 / 100.0) as i32).max(1);
    let mut result = Bitmap::new(src.width(), height as u32);
    {
        let mut c = Canvas::new(&mut result);
        c.translate(0.0, src.height() as f32);
        c.scale(1.0, -1.0);
        c.draw_bitmap(src, 0.0, 0.0, None);
    }
    let mut px = PixelBuffer::read(&result);
    for y in 0..height {
        for x in 0..px.width {
            let mut c = px.get(x, y as usize);
            let alpha = (max_alpha as f32
                - ((max_alpha - min_alpha) * y) as f32 / (height - 1).max(1) as f32)
                as i32;
            c[3] = c[3].min(alpha.clamp(0, 255) as u8);
            px.set(x, y as usize, c);
        }
    }
    px.into_bitmap()
}

/// `SkiaImageHelpers.DrawReflection`.
pub fn draw_reflection(
    bmp: &Bitmap,
    percentage: i32,
    max_alpha: i32,
    min_alpha: i32,
    offset: i32,
    skew: bool,
    skew_size: i32,
) -> Bitmap {
    let mut reflection = add_reflection(bmp, percentage, max_alpha, min_alpha);
    if skew {
        reflection = add_skew(&reflection, skew_size, 0);
    }
    let mut result = Bitmap::new(
        reflection.width(),
        (bmp.height() as i32 + reflection.height() as i32 + offset).max(0) as u32,
    );
    let mut g = Canvas::new(&mut result);
    g.draw_image_xywh(bmp, 0.0, 0.0, bmp.width() as f32, bmp.height() as f32);
    g.draw_image_xywh(
        &reflection,
        0.0,
        (bmp.height() as i32 + offset) as f32,
        reflection.width() as f32,
        reflection.height() as f32,
    );
    result
}

/// `SkiaImageHelpers.AddSkew`.
pub fn add_skew(img: &Bitmap, x: i32, y: i32) -> Bitmap {
    let mut result = img.empty_like(x.abs(), y.abs());
    let sx = -x.min(0);
    let sy = -y.min(0);
    let ex = x.max(0);
    let ey = y.max(0);
    let pts = [
        (sx, sy),
        (sx + img.width() as i32 - 1, ey),
        (ex, sy + img.height() as i32 - 1),
    ];
    let mut g = Canvas::new(&mut result);
    draw_image_points(&mut g, img, pts);
    result
}

/// `SkiaDrawing.DrawImage(canvas, bitmap, Point[])`: an affine map of three corners.
fn draw_image_points(g: &mut Canvas, bmp: &Bitmap, p: [(i32, i32); 3]) {
    let (w, h) = (bmp.width() as f32, bmp.height() as f32);
    let m = Matrix([
        (p[1].0 - p[0].0) as f32 / w,
        (p[2].0 - p[0].0) as f32 / h,
        p[0].0 as f32,
        (p[1].1 - p[0].1) as f32 / w,
        (p[2].1 - p[0].1) as f32 / h,
        p[0].1 as f32,
        0.0,
        0.0,
        1.0,
    ]);
    let saved = g.matrix;
    if !m.is_identity() {
        g.matrix = g.matrix.m44_pre_concat(&m);
    }
    g.draw_bitmap(bmp, 0.0, 0.0, None);
    g.matrix = saved;
}

/// `SkiaImageHelpers.DrawBorder(bmp, pen, type)` for a solid or gradient stroke.
pub fn draw_border(
    bmp: Bitmap,
    shader: Shader,
    size: i32,
    outside: bool,
    dash: DashStyle,
) -> Bitmap {
    let mut stroke = stroke_paint(shader, size as f32);
    stroke.dash = dash_intervals(dash, size as f32);
    if !outside {
        let mut bmp = bmp;
        let (w, h) = (bmp.width() as f32, bmp.height() as f32);
        draw_rectangle_proper(
            &mut Canvas::new(&mut bmp),
            &stroke,
            Rect::xywh(0.0, 0.0, w, h),
            None,
        );
        bmp
    } else {
        let mut result = bmp.empty_like(size * 2, size * 2);
        let (w, h) = (result.width() as f32, result.height() as f32);
        let mut g = Canvas::new(&mut result);
        draw_rectangle_proper(&mut g, &stroke, Rect::xywh(0.0, 0.0, w, h), None);
        g.draw_image_xywh(
            &bmp,
            size as f32,
            size as f32,
            bmp.width() as f32,
            bmp.height() as f32,
        );
        result
    }
}

/// `SkiaImageHelpers.FillBackground(img, brush)`.
pub fn fill_background(img: &Bitmap, shader: Shader) -> Bitmap {
    let mut result = img.empty_like(0, 0);
    let (w, h) = (result.width() as f32, result.height() as f32);
    let mut g = Canvas::new(&mut result);
    let mut p = Paint::shader(shader);
    p.anti_alias = true;
    g.draw_rect(Rect::xywh(0.0, 0.0, w, h), &p);
    g.draw_image_xywh(img, 0.0, 0.0, w, h);
    result
}

/// `SkiaImageHelpers.FillBackground(source, color)`.
pub fn fill_background_color(img: &Bitmap, color: &Color) -> Bitmap {
    let mut result = img.empty_like(0, 0);
    let (w, h) = (result.width() as f32, result.height() as f32);
    let mut g = Canvas::new(&mut result);
    g.clear(color.rgba());
    g.draw_image_xywh(img, 0.0, 0.0, w, h);
    result
}

/// `SkiaImageHelpers.CreateCheckerPattern(width, height, color1, color2)`.
pub fn checker_pattern(w: i32, h: i32, c1: &Color, c2: &Color) -> Bitmap {
    let mut bmp = Bitmap::new((w * 2) as u32, (h * 2) as u32);
    let mut g = Canvas::new(&mut bmp);
    let (b1, b2) = (Paint::fill(c1.rgba()), Paint::fill(c2.rgba()));
    let (w, h) = (w as f32, h as f32);
    g.draw_rect(Rect::xywh(0.0, 0.0, w, h), &b1);
    g.draw_rect(Rect::xywh(w, h, w, h), &b1);
    g.draw_rect(Rect::xywh(w, 0.0, w, h), &b2);
    g.draw_rect(Rect::xywh(0.0, h, w, h), &b2);
    bmp
}

/// `SkiaImageHelpers.DrawCheckers(img, size, color1, color2)`.
pub fn draw_checkers(img: &Bitmap, size: i32, c1: &Color, c2: &Color) -> Bitmap {
    let mut result = img.empty_like(0, 0);
    let checker = checker_pattern(size, size, c1, c2);
    let (w, h) = (result.width() as f32, result.height() as f32);
    let mut g = Canvas::new(&mut result);
    g.draw_rect(
        Rect::xywh(0.0, 0.0, w, h),
        &Paint::texture(&checker, TileMode::Repeat),
    );
    g.draw_image_xywh(img, 0.0, 0.0, img.width() as f32, img.height() as f32);
    result
}

/// `SkiaImageHelpers.AddShadow`.
pub fn add_shadow(
    bmp: &Bitmap,
    opacity: f32,
    size: i32,
    darkness: f32,
    color: &Color,
    offset: Point,
    auto_resize: bool,
) -> Bitmap {
    let mut shadow = bmp.empty_like(size * 2, size * 2);
    apply_color_matrix_into(&cm_mask(opacity, color), bmp, &mut shadow, size, size);
    if size > 0 {
        box_blur(&mut shadow, size);
    }
    if darkness > 1.0 {
        shadow = apply_color_matrix(&cm_alpha(darkness, 0.0), &shadow);
    }
    if auto_resize {
        let mut result = shadow.empty_like(offset.x.abs(), offset.y.abs());
        let mut g = Canvas::new(&mut result);
        g.draw_image_xywh(
            &shadow,
            offset.x.max(0) as f32,
            offset.y.max(0) as f32,
            shadow.width() as f32,
            shadow.height() as f32,
        );
        g.draw_image_xywh(
            bmp,
            size.max(-offset.x + size) as f32,
            size.max(-offset.y + size) as f32,
            bmp.width() as f32,
            bmp.height() as f32,
        );
        result
    } else {
        let mut result = bmp.empty_like(0, 0);
        let mut g = Canvas::new(&mut result);
        g.draw_image_xywh(
            &shadow,
            (-size + offset.x) as f32,
            (-size + offset.y) as f32,
            shadow.width() as f32,
            shadow.height() as f32,
        );
        g.draw_image_xywh(bmp, 0.0, 0.0, bmp.width() as f32, bmp.height() as f32);
        result
    }
}

/// `SkiaImageHelpers.CreateGradientMask`.
pub fn gradient_mask(bmp: &Bitmap, gradient: &GradientInfo, opacity: f32) -> Bitmap {
    let mut mask = bmp.empty_like(0, 0);
    if opacity <= 0.0 {
        return mask;
    }
    if gradient.is_valid() {
        let (w, h) = (mask.width() as i32, mask.height() as i32);
        let mut p = Paint::shader(gradient_shader(gradient, 0, 0, w, h));
        p.anti_alias = true;
        Canvas::new(&mut mask).draw_rect(Rect::xywh(0.0, 0.0, w as f32, h as f32), &p);
    }
    let src = PixelBuffer::read(bmp);
    let mut m = PixelBuffer::read(&mask);
    for i in 0..src.px.len() {
        let s = src.px[i][3] as f32;
        let ma = m.px[i][3] as f32;
        m.px[i][3] = (s * (ma / 255.0) * opacity).min(255.0) as u8;
    }
    m.into_bitmap()
}

/// `SkiaImageHelpers.AddGlow`.
pub fn add_glow(
    bmp: &Bitmap,
    size: i32,
    strength: f32,
    color: &Color,
    offset: Point,
    gradient: Option<&GradientInfo>,
) -> Bitmap {
    if size < 0 || strength < 0.1 {
        return bmp.clone();
    }
    let blur = if size > 0 {
        let Some(mut b) = add_canvas(bmp, Margins::all(size), &Color::transparent()) else {
            return bmp.clone();
        };
        box_blur(&mut b, size);
        b
    } else {
        bmp.clone()
    };
    let mask = match gradient {
        Some(g) if g.is_valid() => gradient_mask(&blur, g, strength),
        _ => apply_color_matrix(&cm_mask(strength, color), &blur),
    };
    let mut result = mask.empty_like(offset.x.abs(), offset.y.abs());
    let mut g = Canvas::new(&mut result);
    g.draw_image_xywh(
        &mask,
        offset.x.max(0) as f32,
        offset.y.max(0) as f32,
        mask.width() as f32,
        mask.height() as f32,
    );
    g.draw_image_xywh(
        bmp,
        size.max(-offset.x + size) as f32,
        size.max(-offset.y + size) as f32,
        bmp.width() as f32,
        bmp.height() as f32,
    );
    result
}

fn polygon_path(points: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    let (x0, y0) = *points.first()?;
    pb.move_to(x0, y0);
    for &(x, y) in &points[1..] {
        pb.line_to(x, y);
    }
    pb.close();
    pb.finish()
}

/// `SkiaDrawing.FillClosedCurve`: a Catmull-Rom style closed spline as cubics.
fn closed_curve_path(points: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    if points.len() < 3 {
        return polygon_path(points);
    }
    let n = points.len();
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);
    for i in 0..n {
        let prev = points[(i + n - 1) % n];
        let cur = points[i];
        let next = points[(i + 1) % n];
        let after = points[(i + 2) % n];
        pb.cubic_to(
            cur.0 + (next.0 - prev.0) / 6.0,
            cur.1 + (next.1 - prev.1) / 6.0,
            next.0 - (after.0 - cur.0) / 6.0,
            next.1 - (after.1 - cur.1) / 6.0,
            next.0,
            next.1,
        );
    }
    pb.close();
    pb.finish()
}

fn fill_with_texture(bmp: &Bitmap, path: Option<tiny_skia::Path>) -> Bitmap {
    let mut result = bmp.empty_like(0, 0);
    if let Some(path) = path {
        Canvas::new(&mut result).fill_path(&path, &Paint::texture(bmp, TileMode::Repeat));
    }
    result
}

/// `SkiaImageHelpers.WavyEdges`.
pub fn wavy_edges(bmp: Bitmap, depth: i32, range: i32, sides: Sides) -> Bitmap {
    if depth < 1 || range < 1 || sides == Sides::NONE {
        return bmp;
    }
    let (w, h) = (bmp.width() as i32, bmp.height() as i32);
    let mut pts: Vec<(i32, i32)> = Vec::new();
    let hcount = 2.max((w / range + 1) / 2 * 2) - 1;
    let vcount = 2.max((h / range + 1) / 2 * 2) - 1;
    let hrange = w / hcount;
    let vrange = h / vcount;
    let step = (range / depth).clamp(1, 10);
    let wave = |t: i32, max: i32, d: i32| {
        ((1.0 - (t as f64 * std::f64::consts::PI / max as f64).cos()) * d as f64 / 2.0) as i32
    };
    let (top, bottom, left, right) = (
        sides.has(Sides::TOP),
        sides.has(Sides::BOTTOM),
        sides.has(Sides::LEFT),
        sides.has(Sides::RIGHT),
    );
    if top {
        let sx = if left { depth } else { 0 };
        let ex = if right { w - depth } else { w };
        let mut x = sx;
        while x < ex {
            pts.push((x, wave(x, hrange, depth)));
            x += step;
        }
        pts.push((ex, wave(ex, hrange, depth)));
    } else {
        pts.push((0, 0));
    }
    if right {
        let sy = if top { depth } else { 0 };
        let ey = if bottom { h - depth } else { h };
        let mut y = sy;
        while y < ey {
            pts.push((w - depth + wave(y, vrange, depth), y));
            y += step;
        }
        pts.push((w - depth + wave(ey, vrange, depth), ey));
    } else {
        let last = pts.last().unwrap().1;
        pts.push((w, last));
    }
    if bottom {
        let sx = if right { w - depth } else { w };
        let ex = if left { depth } else { 0 };
        let mut x = sx;
        while x >= ex {
            pts.push((x, h - depth + wave(x, hrange, depth)));
            x -= step;
        }
        pts.push((ex, h - depth + wave(ex, hrange, depth)));
    } else {
        let last = pts.last().unwrap().0;
        pts.push((last, h));
    }
    if left {
        let sy = if bottom { h - depth } else { h };
        let ey = if top { depth } else { 0 };
        let mut y = sy;
        while y >= ey {
            pts.push((wave(y, vrange, depth), y));
            y -= step;
        }
        pts.push((wave(ey, vrange, depth), ey));
    } else {
        let last = pts.last().unwrap().1;
        pts.push((0, last));
    }
    if !top {
        let lastx = pts.last().unwrap().0;
        pts[0] = (lastx, 0);
    }
    let f: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
    fill_with_texture(&bmp, polygon_path(&f))
}

/// `SkiaImageHelpers.TornEdges` (random).
pub fn torn_edges(
    bmp: Bitmap,
    depth: i32,
    range: i32,
    sides: Sides,
    curved: bool,
    rng: &mut RandomFast,
) -> Bitmap {
    if depth < 1 || range < 1 || sides == Sides::NONE {
        return bmp;
    }
    let (w, h) = (bmp.width() as i32, bmp.height() as i32);
    let hc = w / range;
    let vc = h / range;
    if hc < 2 && vc < 2 {
        return bmp;
    }
    let (top, bottom, left, right) = (
        sides.has(Sides::TOP),
        sides.has(Sides::BOTTOM),
        sides.has(Sides::LEFT),
        sides.has(Sides::RIGHT),
    );
    let mut pts: Vec<(i32, i32)> = Vec::new();
    if top && hc > 1 {
        let sx = if left && vc > 1 { depth } else { 0 };
        let ex = if right && vc > 1 { w - depth } else { w };
        let mut x = sx;
        while x < ex {
            pts.push((x, rng.next(0, depth)));
            x += range;
        }
    } else {
        pts.push((0, 0));
        pts.push((w, 0));
    }
    if right && vc > 1 {
        let sy = if top && hc > 1 { depth } else { 0 };
        let ey = if bottom && hc > 1 { h - depth } else { h };
        let mut y = sy;
        while y < ey {
            let x = rng.next(0, depth);
            pts.push((w - depth + x, y));
            y += range;
        }
    } else {
        pts.push((w, 0));
        pts.push((w, h));
    }
    if bottom && hc > 1 {
        let sx = if right && vc > 1 { w - depth } else { w };
        let ex = if left && vc > 1 { depth } else { 0 };
        let mut x = sx;
        while x >= ex {
            let y = rng.next(0, depth);
            pts.push((x, h - depth + y));
            x = (x / range - 1) * range;
        }
    } else {
        pts.push((w, h));
        pts.push((0, h));
    }
    if left && vc > 1 {
        let sy = if bottom && hc > 1 { h - depth } else { h };
        let ey = if top && hc > 1 { depth } else { 0 };
        let mut y = sy;
        while y >= ey {
            pts.push((rng.next(0, depth), y));
            y = (y / range - 1) * range;
        }
    } else {
        pts.push((0, h));
        pts.push((0, 0));
    }
    // points.Distinct(): first occurrence order.
    let mut distinct: Vec<(i32, i32)> = Vec::new();
    for p in pts {
        if !distinct.contains(&p) {
            distinct.push(p);
        }
    }
    let f: Vec<(f32, f32)> = distinct
        .iter()
        .map(|&(x, y)| (x as f32, y as f32))
        .collect();
    let path = if curved {
        closed_curve_path(&f)
    } else {
        polygon_path(&f)
    };
    fill_with_texture(&bmp, path)
}

/// `SkiaImageHelpers.Slice`.
pub fn slice(
    bmp: &Bitmap,
    min_h: i32,
    max_h: i32,
    min_shift: i32,
    max_shift: i32,
    rng: &mut RandomFast,
) -> Bitmap {
    let mut result = bmp.empty_like(0, 0);
    let mut g = Canvas::new(&mut result);
    let mut y = 0;
    while y < bmp.height() as i32 {
        let sh = rng.next(min_h, max_h);
        let src = Rect::xywh(0.0, y as f32, bmp.width() as f32, sh as f32);
        let dx = if rng.next_max(1) == 0 {
            rng.next(-max_shift, -min_shift)
        } else {
            rng.next(min_shift, max_shift)
        };
        let dst = Rect::xywh(dx as f32, y as f32, bmp.width() as f32, sh as f32);
        g.draw_image_sx(bmp, src, dst, None);
        y += sh;
    }
    result
}

/// `SkiaImageHelpers.FindAutoCropRectangle`.
pub fn find_auto_crop(bmp: &Bitmap, same_color_crop: bool, sides: Sides) -> (i32, i32, i32, i32) {
    let (w, h) = (bmp.width() as i32, bmp.height() as i32);
    let mut crop = (0, 0, w, h);
    if sides == Sides::NONE {
        return crop;
    }
    let px = PixelBuffer::read(bmp);
    let bgra = |c: [u8; 4]| u32::from_le_bytes([c[2], c[1], c[0], c[3]]);
    let get = |x: i32, y: i32| bgra(px.get(x as usize, y as usize));
    let check_color = px.get(0, 0);
    let mut mask: u32 = if check_color[3] == 0 {
        0xFF00_0000
    } else {
        0xFFFF_FFFF
    };
    let mut check = bgra(check_color) & mask;
    if sides.has(Sides::LEFT) {
        let mut found = false;
        'outer: for x in 0..w {
            for y in 0..h {
                if get(x, y) & mask != check {
                    crop.0 = x;
                    crop.2 -= x;
                    found = true;
                    break 'outer;
                }
            }
        }
        // All pixels the same colour.
        if !found {
            return crop;
        }
    }
    if sides.has(Sides::TOP) {
        'outer: for y in 0..h {
            for x in 0..w {
                if get(x, y) & mask != check {
                    crop.1 = y;
                    crop.3 -= y;
                    break 'outer;
                }
            }
        }
    }
    if !same_color_crop {
        let c = px.get(w as usize - 1, h as usize - 1);
        mask = if c[3] == 0 { 0xFF00_0000 } else { 0xFFFF_FFFF };
        check = bgra(c) & mask;
    }
    if sides.has(Sides::RIGHT) {
        'outer: for x in (0..w).rev() {
            for y in 0..h {
                if get(x, y) & mask != check {
                    crop.2 = x - crop.0 + 1;
                    break 'outer;
                }
            }
        }
    }
    if sides.has(Sides::BOTTOM) {
        'outer: for y in (0..h).rev() {
            for x in 0..w {
                if get(x, y) & mask != check {
                    crop.3 = y - crop.1 + 1;
                    break 'outer;
                }
            }
        }
    }
    crop
}

/// `SkiaImageHelpers.AutoCropImage`.
pub fn auto_crop(bmp: Bitmap, same_color_crop: bool, sides: Sides, padding: i32) -> Bitmap {
    let rect = find_auto_crop(&bmp, same_color_crop, sides);
    if rect == (0, 0, bmp.width() as i32, bmp.height() as i32) {
        return bmp;
    }
    let Some(cropped) = crop_bitmap(&bmp, rect.0, rect.1, rect.2, rect.3) else {
        return bmp;
    };
    if padding > 0 {
        // bmp.GetPixel(0, 0): `SkUnPreMultiply::PMColorToColor`.
        let p = bmp.pixel(0, 0);
        let a = p[3] as u32;
        let un = |c: u8| {
            if a == 0 {
                0
            } else if a == 255 {
                c
            } else {
                ((((255u32 << 24) + (a >> 1)) / a * c as u32 + (1 << 23)) >> 24) as u8
            }
        };
        let color = Color::argb(p[3], un(p[0]), un(p[1]), un(p[2]));
        return add_canvas(&cropped, Margins::all(padding), &color).unwrap_or(cropped);
    }
    cropped
}

/// `ColorHelpers.PerceivedBrightness`.
fn perceived_brightness(r: u8, g: u8, b: u8) -> i32 {
    let (r, g, b) = (r as i32, g as i32, b as i32);
    (((r * r) as f64 * 0.299) + ((g * g) as f64 * 0.587) + ((b * b) as f64 * 0.114)).sqrt() as i32
}

/// `SkiaImageHelpers.SelectiveColor`.
pub fn selective_color(bmp: &mut Bitmap, light: &Color, dark: &Color, palette: i32) {
    let palette = palette.max(2);
    let mut colors: Vec<(i32, [u8; 3])> = Vec::new();
    for i in 0..palette {
        let amount = i as f32 / (palette - 1) as f32;
        let lerp = |a: u8, b: u8| (a as f32 + ((b as f32 - a as f32) * amount)) as i32 as u8;
        let c = [
            lerp(light.r, dark.r),
            lerp(light.g, dark.g),
            lerp(light.b, dark.b),
        ];
        let pb = perceived_brightness(c[0], c[1], c[2]);
        if !colors.iter().any(|(k, _)| *k == pb) {
            colors.push((pb, c));
        }
    }
    let mut px = PixelBuffer::read(bmp);
    for p in &mut px.px {
        let pb = perceived_brightness(p[0], p[1], p[2]);
        // Aggregate: keep current while strictly closer, else take next.
        let mut best = colors[0];
        for next in &colors[1..] {
            if (best.0 - pb).abs() >= (next.0 - pb).abs() {
                best = *next;
            }
        }
        p[0] = best.1[0];
        p[1] = best.1[1];
        p[2] = best.1[2];
    }
    px.write(bmp);
}

/// `SkiaImageHelpers.ReplaceColor`.
pub fn replace_color(
    bmp: &mut Bitmap,
    source: &Color,
    target: &Color,
    auto_source: bool,
    threshold: i32,
) {
    let mut px = PixelBuffer::read(bmp);
    let mut src = [source.r, source.g, source.b, source.a];
    if auto_source && !px.px.is_empty() {
        src = px.px[0];
    }
    let tgt = [target.r, target.g, target.b, target.a];
    for p in &mut px.px {
        if threshold == 0 {
            if *p == src {
                *p = tgt;
            }
        } else {
            let diff = (p[0] as i32 - src[0] as i32).abs()
                + (p[1] as i32 - src[1] as i32).abs()
                + (p[2] as i32 - src[2] as i32).abs();
            if diff <= threshold {
                *p = tgt;
            }
        }
    }
    px.write(bmp);
}

/// `SkiaImageHelpers.DrawBackgroundImage(bmp, backgroundImage, center, tile)`.
pub fn draw_background_image(bmp: &Bitmap, bg: &Bitmap, center: bool, tile: bool) -> Bitmap {
    let mut result = bmp.empty_like(0, 0);
    let (rw, rh) = (result.width() as i32, result.height() as i32);
    let mut g = Canvas::new(&mut result);
    if tile {
        let mut brush = Paint::texture(bg, TileMode::Repeat);
        if center {
            let tx = (rw - bg.width() as i32) / 2 % bg.width() as i32;
            let ty = (rh - bg.height() as i32) / 2 % bg.height() as i32;
            brush.translate_shader(tx as f32, ty as f32);
        }
        g.draw_rect(Rect::xywh(0.0, 0.0, rw as f32, rh as f32), &brush);
    } else {
        let aspect = bg.width() as f32 / bg.height() as f32;
        let mut width = rw;
        let mut height = (width as f32 / aspect) as i32;
        if height < rh {
            height = rh;
            width = (height as f32 * aspect) as i32;
        }
        let (mut x, mut y) = (0, 0);
        if center {
            x = (rw - width) / 2;
            y = (rh - height) / 2;
        }
        g.draw_image_xywh(bg, x as f32, y as f32, width as f32, height as f32);
    }
    g.draw_image_xywh(bmp, 0.0, 0.0, bmp.width() as f32, bmp.height() as f32);
    result
}
