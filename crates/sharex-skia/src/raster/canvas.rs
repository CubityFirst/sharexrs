//! `SKCanvas` over a [`Bitmap`], drawing as Skia's CPU backend does: integer-offset
//! bitmaps through the sprite blitter, everything else through the raster pipeline
//! (lowp when every stage has a 16-bit version, highp otherwise), with Skia's blitter
//! choices for full and partial coverage.

use super::coverage::{self, Coverage};
use super::gradient::LinearGradient;
use super::*;
use super::Bitmap;

/// `SKSamplingOptions` as ShareX uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampling {
    Nearest,
    Linear,
    /// `SKCubicResampler.Mitchell` (B = C = 1/3), ShareX's `HighQualitySampling`.
    Mitchell,
}

/// `SKShaderTileMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileMode {
    Clamp,
    Repeat,
    Mirror,
}

/// The two blend modes ShareX draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    SrcOver,
    Src,
}

/// `SKPaint.Shader` (or its colour when there's none).
#[derive(Clone)]
pub enum Shader<'a> {
    /// A straight (unpremultiplied) RGBA colour.
    Color([u8; 4]),
    /// `SKShader.CreateBitmap`/an image draw: `matrix` maps image space to the canvas's
    /// local space (the canvas matrix is applied on top).
    Image {
        image: &'a Bitmap,
        matrix: Matrix,
        sampling: Sampling,
        tile_x: TileMode,
        tile_y: TileMode,
    },
    Gradient(LinearGradient),
}

/// `SKPaint`, as far as ShareX's effects set it.
#[derive(Clone)]
pub struct Paint<'a> {
    pub shader: Shader<'a>,
    pub blend: Blend,
    pub anti_alias: bool,
    /// `SKColorFilter.CreateColorMatrix`, Skia's row-major 4x5.
    pub color_filter: Option<[f32; 20]>,
    /// The paint colour's alpha when a shader draws (`paint.Color = White.WithAlpha(a)`).
    pub alpha: u8,
}

impl<'a> Paint<'a> {
    /// `SkiaDrawing.Fill(color)`: anti-aliased solid colour.
    pub fn fill(color: [u8; 4]) -> Self {
        Paint {
            shader: Shader::Color(color),
            blend: Blend::SrcOver,
            anti_alias: true,
            color_filter: None,
            alpha: 255,
        }
    }

    pub fn shader(shader: Shader<'a>) -> Self {
        Paint {
            shader,
            blend: Blend::SrcOver,
            anti_alias: false,
            color_filter: None,
            alpha: 255,
        }
    }

    /// `SkiaDrawing.Texture(bitmap, mode)`: an anti-aliased bitmap shader, nearest sampling.
    pub fn texture(image: &'a Bitmap, tile: TileMode) -> Self {
        Paint {
            shader: Shader::Image {
                image,
                matrix: Matrix::IDENTITY,
                sampling: Sampling::Nearest,
                tile_x: tile,
                tile_y: tile,
            },
            blend: Blend::SrcOver,
            anti_alias: true,
            color_filter: None,
            alpha: 255,
        }
    }

    /// `SkiaDrawing.Translate(paint, x, y)`: `shader.WithLocalMatrix(translate)`.
    pub fn translate_shader(&mut self, x: f32, y: f32) {
        match &mut self.shader {
            Shader::Image { matrix, .. } => {
                *matrix = Matrix::concat(&Matrix::translate(x, y), matrix)
            }
            Shader::Gradient(g) => g.local = Matrix::concat(&Matrix::translate(x, y), &g.local),
            Shader::Color(_) => {}
        }
    }
}

/// `SKCanvas` over a bitmap.
pub struct Canvas<'a> {
    pub dst: &'a mut Bitmap,
    pub matrix: Matrix,
}

/// A source colour from the pipeline: highp premultiplied floats or lowp 0..255.
#[derive(Clone, Copy)]
enum Px {
    High([f32; 4]),
    Low([u32; 4]),
}

impl<'a> Canvas<'a> {
    pub fn new(dst: &'a mut Bitmap) -> Self {
        Canvas {
            dst,
            matrix: Matrix::IDENTITY,
        }
    }

    /// `SKCanvas.Translate`.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        if dx != 0.0 || dy != 0.0 {
            self.matrix = self.matrix.m44_pre_translate(dx, dy);
        }
    }

    /// `SKCanvas.Scale`.
    pub fn scale(&mut self, sx: f32, sy: f32) {
        if sx != 1.0 || sy != 1.0 {
            self.matrix = self.matrix.m44_pre_scale(sx, sy);
        }
    }

    /// `SKCanvas.RotateDegrees`.
    pub fn rotate(&mut self, degrees: f32) {
        let m = Matrix::rotate(degrees);
        if !m.is_identity() {
            self.matrix = self.matrix.m44_pre_concat(&m);
        }
    }

    pub fn reset_matrix(&mut self) {
        self.matrix = Matrix::IDENTITY;
    }

    fn width(&self) -> u32 {
        self.dst.width()
    }

    fn height(&self) -> u32 {
        self.dst.height()
    }

    /// `SKCanvas.Clear(color)`: every pixel set to the premultiplied colour.
    pub fn clear(&mut self, color: [u8; 4]) {
        let p = uniform_lowp(color).map(|v| v as u8);
        for i in 0..(self.width() * self.height()) as usize {
            self.dst.set_px(i, p);
        }
    }

    /// `SKCanvas.DrawBitmap(bitmap, x, y)` / `DrawImage(image, x, y)`: default (nearest)
    /// sampling.
    pub fn draw_bitmap(&mut self, image: &Bitmap, x: f32, y: f32, paint: Option<&Paint>) {
        let src = Rect::xywh(0.0, 0.0, image.width() as f32, image.height() as f32);
        self.draw_image_rect(
            image,
            None,
            Rect::xywh(x, y, src.width(), src.height()),
            Sampling::Nearest,
            paint,
        );
    }

    /// `SkiaImageHelpers.DrawImage(canvas, bitmap, source, destination, paint)`: a same-size,
    /// whole-pixel draw goes through `DrawBitmap` (nearest), anything else through Mitchell.
    pub fn draw_image_sx(&mut self, image: &Bitmap, src: Rect, dst: Rect, paint: Option<&Paint>) {
        let trunc = |v: f32| v == v.trunc();
        if src.width() == dst.width()
            && src.height() == dst.height()
            && self.matrix.is_identity()
            && trunc(src.left)
            && trunc(src.top)
            && trunc(dst.left)
            && trunc(dst.top)
        {
            self.draw_image_rect(image, Some(src), dst, Sampling::Nearest, paint);
        } else {
            self.draw_image_rect(image, Some(src), dst, Sampling::Mitchell, paint);
        }
    }

    /// `SkiaDrawing.DrawImage(canvas, bitmap, x, y, width, height)`.
    pub fn draw_image_xywh(&mut self, image: &Bitmap, x: f32, y: f32, w: f32, h: f32) {
        let src = Rect::xywh(0.0, 0.0, image.width() as f32, image.height() as f32);
        self.draw_image_sx(image, src, Rect::xywh(x, y, w, h), None);
    }

    /// `SkCanvas::drawImageRect` → `SkBitmapDevice::drawImageRect` (`kFast` constraint).
    pub fn draw_image_rect(
        &mut self,
        image: &Bitmap,
        src: Option<Rect>,
        dst: Rect,
        sampling: Sampling,
        paint: Option<&Paint>,
    ) {
        if image.is_empty() || self.dst.is_empty() {
            return;
        }
        let bounds = Rect::xywh(0.0, 0.0, image.width() as f32, image.height() as f32);
        let mut tmp_src = src.unwrap_or(bounds);
        let matrix = Matrix::rect_to_rect(tmp_src, dst);
        let mut dst = dst;
        if let Some(s) = src {
            let contained = bounds.left <= s.left
                && bounds.top <= s.top
                && bounds.right >= s.right
                && bounds.bottom >= s.bottom;
            if !contained {
                tmp_src = Rect::ltrb(
                    s.left.max(0.0),
                    s.top.max(0.0),
                    s.right.min(bounds.right),
                    s.bottom.min(bounds.bottom),
                );
                if tmp_src.is_empty() {
                    return;
                }
                dst = matrix.map_rect(tmp_src);
            }
        }
        let default_paint;
        let paint = match paint {
            Some(p) => p,
            None => {
                default_paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
                &default_paint
            }
        };
        let subset_needed = src.is_some_and(|s| {
            !(s.left <= 0.0 && s.top <= 0.0 && s.right >= bounds.right && s.bottom >= bounds.bottom)
        });
        if subset_needed && sampling != Sampling::Nearest {
            // USE_SHADER: the whole bitmap, clamped, through the src->dst matrix.
            self.draw_with_image_shader(image, matrix, sampling, dst, paint);
            return;
        }
        if src.is_some() {
            // Extract the (rounded-out) subset and draw it through the adjusted matrix.
            let ir = (
                tmp_src.left.floor() as i32,
                tmp_src.top.floor() as i32,
                tmp_src.right.ceil() as i32,
                tmp_src.bottom.ceil() as i32,
            );
            let subset = extract_subset(image, ir);
            let mut m = matrix;
            if ir.0 > 0 || ir.1 > 0 {
                m = pre_translate(&m, ir.0.max(0) as f32, ir.1.max(0) as f32);
            }
            let extracted = Rect::xywh(0.0, 0.0, subset.width() as f32, subset.height() as f32);
            if extracted == tmp_src {
                self.draw_bitmap_matrix(&subset, m, Some(dst), sampling, paint);
            } else {
                self.draw_with_image_shader(&subset, m, sampling, dst, paint);
            }
            return;
        }
        self.draw_bitmap_matrix(image, matrix, Some(dst), sampling, paint);
    }

    /// `SkDraw::drawBitmap`: a sprite when the total matrix is an integer translate and
    /// the sampling can't change pixels, else a shader-filled rect.
    fn draw_bitmap_matrix(
        &mut self,
        image: &Bitmap,
        prematrix: Matrix,
        dst: Option<Rect>,
        sampling: Sampling,
        paint: &Paint,
    ) {
        let total = Matrix::concat(&self.matrix, &prematrix);
        if sampling != Sampling::Mitchell && total.is_translate() {
            let ix = (total.tx() + 0.5).floor() as i32;
            let iy = (total.ty() + 0.5).floor() as i32;
            self.sprite(image, ix, iy, paint);
            return;
        }
        let _ = dst;
        let bounds = Rect::xywh(0.0, 0.0, image.width() as f32, image.height() as f32);
        let device = total.map_rect(bounds);
        let shader = Shader::Image {
            image,
            matrix: prematrix,
            sampling,
            tile_x: TileMode::Clamp,
            tile_y: TileMode::Clamp,
        };
        let p = Paint {
            shader,
            ..paint.clone()
        };
        if total.rect_stays_rect() {
            let cov = if p.anti_alias {
                coverage::anti_fill_rect(device, self.width(), self.height())
            } else {
                coverage::fill_rect(device, self.width(), self.height())
            };
            self.blit(&cov, &p);
        } else {
            let path = rect_path(bounds, &total);
            if let Some(path) = path {
                let cov = coverage::fill_path(&path, p.anti_alias, self.width(), self.height());
                self.blit(&cov, &p);
            }
        }
    }

    /// The `USE_SHADER` branch: the image (clamped) through `matrix`, filling `dst`.
    fn draw_with_image_shader(
        &mut self,
        image: &Bitmap,
        matrix: Matrix,
        sampling: Sampling,
        dst: Rect,
        paint: &Paint,
    ) {
        let shader = Shader::Image {
            image,
            matrix,
            sampling,
            tile_x: TileMode::Clamp,
            tile_y: TileMode::Clamp,
        };
        let p = Paint {
            shader,
            ..paint.clone()
        };
        self.draw_rect(dst, &p);
    }

    /// `SkCanvas::drawRect` (fill).
    pub fn draw_rect(&mut self, rect: Rect, paint: &Paint) {
        if self.dst.is_empty() {
            return;
        }
        let r = Rect::ltrb(
            rect.left.min(rect.right),
            rect.top.min(rect.bottom),
            rect.left.max(rect.right),
            rect.top.max(rect.bottom),
        );
        if self.matrix.rect_stays_rect() {
            let device = self.matrix.map_rect(r);
            let cov = if paint.anti_alias {
                coverage::anti_fill_rect(device, self.width(), self.height())
            } else {
                coverage::fill_rect(device, self.width(), self.height())
            };
            self.blit(&cov, paint);
        } else if let Some(path) = rect_path(r, &self.matrix) {
            let cov = coverage::fill_path(&path, paint.anti_alias, self.width(), self.height());
            self.blit(&cov, paint);
        }
    }

    /// `SkCanvas::drawPath` for a fill (strokes are turned into fills by the caller).
    pub fn fill_path(&mut self, path: &tiny_skia::Path, paint: &Paint) {
        if self.dst.is_empty() {
            return;
        }
        let device = if self.matrix.is_identity() {
            Some(path.clone())
        } else {
            let m = &self.matrix;
            path.clone().transform(tiny_skia::Transform::from_row(
                m.sx(),
                m.ky(),
                m.kx(),
                m.sy(),
                m.tx(),
                m.ty(),
            ))
        };
        if let Some(device) = device {
            let cov = coverage::fill_path(&device, paint.anti_alias, self.width(), self.height());
            self.blit(&cov, paint);
        }
    }

    /// The sprite blitter (`SkBlitter::ChooseSprite`).
    fn sprite(&mut self, image: &Bitmap, ix: i32, iy: i32, paint: &Paint) {
        let (w, h) = (self.width() as i32, self.height() as i32);
        let x0 = ix.max(0);
        let y0 = iy.max(0);
        let x1 = (ix + image.width() as i32).min(w);
        let y1 = (iy + image.height() as i32).min(h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let dw = w as usize;
        let iw = image.width() as usize;
        let plain = paint.color_filter.is_none() && paint.alpha == 255;
        for y in y0..y1 {
            for x in x0..x1 {
                let si = (y - iy) as usize * iw + (x - ix) as usize;
                let di = y as usize * dw + x as usize;
                let s = image.px(si);
                let out = if plain {
                    match paint.blend {
                        // SkSpriteBlitter_Memcpy.
                        Blend::Src => s,
                        // Sprite_D32_S32 → S32A_Opaque_BlitRow32: s + (d * (256 - sa) >> 8).
                        Blend::SrcOver => {
                            let d = self.dst.px(di);
                            let scale = 256 - s[3] as u32;
                            [0, 1, 2, 3].map(|c| {
                                (s[c] as u32 + ((d[c] as u32 * scale) >> 8)).min(255) as u8
                            })
                        }
                    }
                } else {
                    // SkRasterPipelineSpriteBlitter: load_8888, the paint's alpha and colour
                    // filter (highp), then the pipeline blitter's blitRect.
                    let mut c = load(s);
                    if paint.alpha != 255 {
                        let f = from_byte(paint.alpha);
                        c = c.map(|v| v * f);
                    }
                    if let Some(m) = &paint.color_filter {
                        c = color_filter(m, c);
                    }
                    self.blend_full(di, Px::High(clamp_01(c)), paint.blend)
                };
                self.dst.set_px(di, out);
            }
        }
    }

    /// Runs the paint's pipeline over the covered pixels.
    fn blit(&mut self, cov: &Coverage, paint: &Paint) {
        let sampler = Sampler::new(paint, &self.matrix);
        let Some(sampler) = sampler else { return };
        let lowp = sampler.lowp;
        let w = self.width() as usize;
        // An opaque solid colour turns srcover into src (and src of a constant is a memset).
        let blend = match (&paint.shader, paint.blend) {
            (Shader::Color(c), Blend::SrcOver)
                if c[3] == 255 && paint.alpha == 255 && paint.color_filter.is_none() =>
            {
                Blend::Src
            }
            (_, b) => b,
        };
        let legacy = use_legacy_blitter(paint, &sampler);
        let pixel = |this: &mut Self, x: i32, y: i32, c: u8| {
            let di = y as usize * w + x as usize;
            let out = match &legacy {
                Some(Legacy::Color(pm)) => legacy_color(this.dst.px(di), *pm, c),
                Some(Legacy::Image) => {
                    let SamplerKind::Image(img) = &sampler.kind else {
                        unreachable!()
                    };
                    let s = img.gather_nearest(x, y);
                    let d = this.dst.px(di);
                    if c == 255 {
                        s32a_opaque(s, d)
                    } else {
                        blend_argb32(s, d, c)
                    }
                }
                None => {
                    let s = sampler.shade(x, y);
                    if c == 255 {
                        this.blend_full(di, s, blend)
                    } else {
                        this.blend_partial(di, s, blend, c, lowp)
                    }
                }
            };
            this.dst.set_px(di, out);
        };
        match cov {
            Coverage::Rect {
                left,
                top,
                right,
                bottom,
            } => {
                for y in *top..*bottom {
                    for x in *left..*right {
                        pixel(self, x, y, 255);
                    }
                }
            }
            Coverage::Mask(mask) => {
                for (i, &c) in mask.iter().enumerate() {
                    if c != 0 {
                        pixel(self, (i % w) as i32, (i / w) as i32, c);
                    }
                }
            }
        }
    }

    /// `blitRect`: srcover through `srcover_rgba_8888`, src through `store_8888`.
    fn blend_full(&self, di: usize, s: Px, blend: Blend) -> [u8; 4] {
        match (s, blend) {
            (Px::High(c), Blend::Src) => store(c),
            (Px::Low(c), Blend::Src) => c.map(|v| v.min(255) as u8),
            (Px::High(c), Blend::SrcOver) => {
                let d = self.dst.px(di);
                let inv = 1.0 - c[3];
                [0, 1, 2, 3].map(|i| to_unorm(mad(d[i] as f32, inv, c[i] * 255.0), 1.0, 255.0))
            }
            (Px::Low(c), Blend::SrcOver) => {
                let d = self.dst.px(di);
                let inv = 255 - c[3];
                [0, 1, 2, 3].map(|i| (c[i] + div255(d[i] as u32 * inv)).min(255) as u8)
            }
        }
    }

    /// `blitAntiH`/`blitMask`: srcover pre-scales by coverage, src lerps.
    fn blend_partial(&self, di: usize, s: Px, blend: Blend, cov: u8, _lowp: bool) -> [u8; 4] {
        let d = self.dst.px(di);
        match (s, blend) {
            (Px::High(c), Blend::SrcOver) => {
                let f = cov as f32 * (1.0 / 255.0);
                let c = c.map(|v| v * f);
                let dd = load(d);
                let inv = 1.0 - c[3];
                store([0, 1, 2, 3].map(|i| mad(dd[i], inv, c[i])))
            }
            (Px::High(c), Blend::Src) => {
                let f = cov as f32 * (1.0 / 255.0);
                let dd = load(d);
                store([0, 1, 2, 3].map(|i| mad(c[i] - dd[i], f, dd[i])))
            }
            (Px::Low(c), Blend::SrcOver) => {
                let f = lowp_from_float(cov as f32 * (1.0 / 255.0));
                let c = c.map(|v| div255(v * f));
                let inv = 255 - c[3];
                [0, 1, 2, 3].map(|i| (c[i] + div255_accurate(d[i] as u32 * inv)).min(255) as u8)
            }
            (Px::Low(c), Blend::Src) => {
                let t = lowp_from_float(cov as f32 * (1.0 / 255.0));
                [0, 1, 2, 3].map(|i| div255(d[i] as u32 * (255 - t) + c[i] * t).min(255) as u8)
            }
        }
    }
}

/// Which legacy (pre-raster-pipeline) blitter `SkBlitter::Choose` picks: src-over onto
/// N32 without a colour filter, for a solid colour (`SkARGB32_Blitter` and friends) or
/// an image shader that has a legacy context (`SkBitmapProcLegacyShader`, nearest here).
enum Legacy {
    Color([u8; 4]),
    Image,
}

fn use_legacy_blitter(paint: &Paint, sampler: &Sampler) -> Option<Legacy> {
    if paint.blend != Blend::SrcOver || paint.color_filter.is_some() {
        return None;
    }
    match (&paint.shader, &sampler.kind) {
        (Shader::Color(c), _) => {
            let a = c[3] as u32;
            // SkARGB32_Blitter: SkAlphaMul(c, SkAlpha255To256(a)).
            let pm = if a == 255 {
                *c
            } else {
                [
                    (c[0] as u32 * (a + 1)) >> 8,
                    (c[1] as u32 * (a + 1)) >> 8,
                    (c[2] as u32 * (a + 1)) >> 8,
                    a,
                ]
                .map(|v| v as u8)
            };
            Some(Legacy::Color(pm))
        }
        (Shader::Image { .. }, SamplerKind::Image(img))
            if img.sampling == Sampling::Nearest && paint.alpha == 255 =>
        {
            Some(Legacy::Image)
        }
        _ => None,
    }
}

/// `SkAlphaMulQ(c, scale)`.
fn alpha_mul_q(c: [u8; 4], scale: u32) -> [u8; 4] {
    c.map(|v| ((v as u32 * scale) >> 8) as u8)
}

/// `SkBlitRow::Color32` (`blit_row_color32`): `(d * invA + (c << 8) + 128) >> 8`.
fn color32(d: [u8; 4], c: [u8; 4]) -> [u8; 4] {
    match c[3] {
        0 => d,
        255 => c,
        a => {
            let mut inv = 255 - a as u32;
            inv += inv >> 7;
            [0, 1, 2, 3].map(|i| ((d[i] as u32 * inv + ((c[i] as u32) << 8) + 128) >> 8) as u8)
        }
    }
}

/// `SkARGB32_Blitter::blitRect`/`blitAntiH` for one pixel.
fn legacy_color(d: [u8; 4], pm: [u8; 4], cov: u8) -> [u8; 4] {
    if cov == 255 {
        color32(d, pm)
    } else {
        color32(d, alpha_mul_q(pm, cov as u32 + 1))
    }
}

/// `S32A_Opaque_BlitRow32`: `s + (d * (256 - sa) >> 8)`.
fn s32a_opaque(s: [u8; 4], d: [u8; 4]) -> [u8; 4] {
    let scale = 256 - s[3] as u32;
    [0, 1, 2, 3].map(|c| (s[c] as u32 + ((d[c] as u32 * scale) >> 8)).min(255) as u8)
}

/// `SkBlendARGB32(src, dst, aa)` (`S32A_Blend_BlitRow32`).
fn blend_argb32(s: [u8; 4], d: [u8; 4], aa: u8) -> [u8; 4] {
    let src_scale = aa as u32 + 1;
    let prod = 0xFFFF - s[3] as u32 * src_scale;
    let dst_scale = (prod + (prod >> 8)) >> 8;
    [0, 1, 2, 3].map(|c| (((s[c] as u32 * src_scale + d[c] as u32 * dst_scale) >> 8) & 0xFF) as u8)
}

/// `SkMatrix::preTranslate` (plain float, as `SkMatrix` does).
fn pre_translate(m: &Matrix, dx: f32, dy: f32) -> Matrix {
    let mut out = *m;
    if m.is_scale_translate() {
        out.0[2] = m.0[0] * dx + m.0[2];
        out.0[5] = m.0[4] * dy + m.0[5];
        if m.0[0] == 1.0 && m.0[4] == 1.0 {
            out.0[2] = m.0[2] + dx;
            out.0[5] = m.0[5] + dy;
        }
    } else {
        out.0[2] += m.0[0] * dx + m.0[1] * dy;
        out.0[5] += m.0[3] * dx + m.0[4] * dy;
    }
    out
}

fn extract_subset(image: &Bitmap, ir: (i32, i32, i32, i32)) -> Bitmap {
    let l = ir.0.max(0) as u32;
    let t = ir.1.max(0) as u32;
    let r = (ir.2.max(0) as u32).min(image.width());
    let b = (ir.3.max(0) as u32).min(image.height());
    let mut out = Bitmap::new(r.saturating_sub(l), b.saturating_sub(t));
    for y in 0..out.height() {
        for x in 0..out.width() {
            out.set_pixel(x, y, image.pixel(x + l, y + t));
        }
    }
    out
}

fn rect_path(r: Rect, m: &Matrix) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    let p = [
        m.map(r.left, r.top),
        m.map(r.right, r.top),
        m.map(r.right, r.bottom),
        m.map(r.left, r.bottom),
    ];
    pb.move_to(p[0].0, p[0].1);
    for q in &p[1..] {
        pb.line_to(q.0, q.1);
    }
    pb.close();
    pb.finish()
}

/// `append_constant_color` for lowp: premultiplied in float, `* 255 + 0.5`, truncated.
pub(crate) fn uniform_lowp(c: [u8; 4]) -> [u32; 4] {
    let f = uniform_high(c);
    f.map(|v| (v * 255.0 + 0.5) as u32)
}

/// `SkColor4f::FromColor(c).premul()`.
pub(crate) fn uniform_high(c: [u8; 4]) -> [f32; 4] {
    let a = from_byte(c[3]);
    [
        from_byte(c[0]) * a,
        from_byte(c[1]) * a,
        from_byte(c[2]) * a,
        a,
    ]
}

/// The shader half of the pipeline, set up once per draw.
struct Sampler<'a> {
    kind: SamplerKind<'a>,
    alpha: Option<f32>,
    color_filter: Option<[f32; 20]>,
    lowp: bool,
}

enum SamplerKind<'a> {
    Color { high: [f32; 4], low: [u32; 4] },
    Image(ImageSampler<'a>),
    Gradient(gradient::GradientSampler),
}

struct ImageSampler<'a> {
    image: &'a Bitmap,
    inv: Matrix,
    sampling: Sampling,
    tile_x: TileMode,
    tile_y: TileMode,
}

impl<'a> Sampler<'a> {
    fn new(paint: &'a Paint<'a>, ctm: &Matrix) -> Option<Self> {
        let alpha = (paint.alpha != 255).then(|| from_byte(paint.alpha));
        let (kind, lowp_shader) = match &paint.shader {
            Shader::Color(c) => {
                let mut high = uniform_high(*c);
                if let Some(a) = alpha {
                    // The paint's alpha is folded into a solid colour.
                    high = high.map(|v| v * a);
                }
                let low = high.map(|v| (v * 255.0 + 0.5) as u32);
                (SamplerKind::Color { high, low }, true)
            }
            Shader::Image {
                image,
                matrix,
                sampling,
                tile_x,
                tile_y,
            } => {
                let total = Matrix::concat(ctm, matrix);
                let inv = total.invert()?;
                // `tweak_sampling`: bilinear through an integer translate is nearest.
                let mut sampling = *sampling;
                if sampling == Sampling::Linear
                    && inv.is_translate()
                    && inv.tx() == inv.tx().trunc()
                    && inv.ty() == inv.ty().trunc()
                {
                    sampling = Sampling::Nearest;
                }
                let lowp = sampling != Sampling::Mitchell
                    && *tile_x == TileMode::Clamp
                    && *tile_y == TileMode::Clamp;
                (
                    SamplerKind::Image(ImageSampler {
                        image,
                        inv,
                        sampling,
                        tile_x: *tile_x,
                        tile_y: *tile_y,
                    }),
                    lowp,
                )
            }
            Shader::Gradient(g) => {
                let s = g.sampler(ctm)?;
                let lowp = s.lowp_capable();
                (SamplerKind::Gradient(s), lowp)
            }
        };
        let lowp = lowp_shader
            && paint.color_filter.is_none()
            && !matches!(
                paint.shader,
                Shader::Image {
                    sampling: Sampling::Linear,
                    ..
                }
            );
        let alpha = if matches!(paint.shader, Shader::Color(_)) {
            None
        } else {
            alpha
        };
        Some(Sampler {
            kind,
            alpha,
            color_filter: paint.color_filter,
            lowp,
        })
    }

    fn shade(&self, x: i32, y: i32) -> Px {
        if self.lowp {
            let mut c = match &self.kind {
                SamplerKind::Color { low, .. } => *low,
                SamplerKind::Image(s) => s.sample_lowp(x, y),
                SamplerKind::Gradient(g) => g.shade_lowp(x, y),
            };
            if let Some(a) = self.alpha {
                let f = lowp_from_float(a);
                c = c.map(|v| div255(v * f));
            }
            Px::Low(c.map(|v| v.min(255)))
        } else {
            let mut c = match &self.kind {
                SamplerKind::Color { high, .. } => *high,
                SamplerKind::Image(s) => s.sample_highp(x, y),
                SamplerKind::Gradient(g) => g.shade_highp(x, y),
            };
            if let Some(a) = self.alpha {
                c = c.map(|v| v * a);
            }
            if let Some(m) = &self.color_filter {
                c = color_filter(m, c);
            }
            Px::High(clamp_01(c))
        }
    }
}

/// `clamp_ex` + `roundDownAtInteger` + `trunc`: the index `ix_and_ptr` gathers.
#[inline]
fn gather_index(v: f32, limit: u32, round_down: bool) -> u32 {
    let inclusive = f32::from_bits((limit as f32).to_bits() - 1);
    let mut v = v.max(f32::MIN_POSITIVE).min(inclusive);
    if round_down {
        v = f32::from_bits(v.to_bits() - 1);
    }
    v as u32
}

impl ImageSampler<'_> {
    /// `seed_shader` and the matrix stage.
    fn coords(&self, x: i32, y: i32) -> (f32, f32) {
        let mad_ = mad;
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        let m = &self.inv;
        if m.is_identity() {
            (fx, fy)
        } else if m.is_translate() {
            (fx + m.tx(), fy + m.ty())
        } else if m.is_scale_translate() {
            (mad_(fx, m.sx(), m.tx()), mad_(fy, m.sy(), m.ty()))
        } else {
            (
                mad_(fx, m.sx(), mad_(fy, m.kx(), m.tx())),
                mad_(fx, m.ky(), mad_(fy, m.sy(), m.ty())),
            )
        }
    }

    fn tile(&self, v: f32, mode: TileMode, limit: u32) -> f32 {
        let scale = limit as f32;
        let inv = 1.0 / scale;
        match mode {
            TileMode::Clamp => v,
            // `exclusive_repeat`: v - floor(v * invScale) * scale.
            TileMode::Repeat => v - (v * inv).floor() * scale,
            TileMode::Mirror => {
                let u = v - (v * inv * 0.5).floor() * 2.0 * scale;
                let s = (u * inv).floor();
                let m = u - 2.0 * s * (u - scale);
                let bias = s as i32;
                if self.sampling == Sampling::Nearest {
                    f32::from_bits((m.to_bits() as i32 + bias) as u32)
                } else {
                    m
                }
            }
        }
    }

    fn gather(&self, x: f32, y: f32, round_down: bool) -> [u8; 4] {
        let ix = gather_index(x, self.image.width(), round_down);
        let iy = gather_index(y, self.image.height(), round_down);
        self.image.pixel(ix, iy)
    }

    /// Nearest sampling through the pipeline's coordinates (the legacy shader picks the
    /// same pixels for the matrices ShareX uses).
    fn gather_nearest(&self, x: i32, y: i32) -> [u8; 4] {
        let (fx, fy) = self.coords(x, y);
        let fx = self.tile(fx, self.tile_x, self.image.width());
        let fy = self.tile(fy, self.tile_y, self.image.height());
        self.gather(fx, fy, true)
    }

    fn sample_lowp(&self, x: i32, y: i32) -> [u32; 4] {
        let (fx, fy) = self.coords(x, y);
        self.gather(fx, fy, self.sampling == Sampling::Nearest)
            .map(|v| v as u32)
    }

    fn sample_highp(&self, x: i32, y: i32) -> [f32; 4] {
        let (fx, fy) = self.coords(x, y);
        match self.sampling {
            Sampling::Nearest => {
                let fx = self.tile(fx, self.tile_x, self.image.width());
                let fy = self.tile(fy, self.tile_y, self.image.height());
                load(self.gather(fx, fy, true))
            }
            Sampling::Linear => {
                // bilerp_clamp_8888.
                let frac = |v: f32| v - v.floor();
                let (tx, ty) = (frac(fx + 0.5), frac(fy + 0.5));
                let mut acc = [0.0f32; 4];
                for py in [-0.5f32, 0.5] {
                    for px in [-0.5f32, 0.5] {
                        let s = load(self.gather(fx + px, fy + py, false));
                        let sx = if px > 0.0 { tx } else { 1.0 - tx };
                        let sy = if py > 0.0 { ty } else { 1.0 - ty };
                        let area = sx * sy;
                        for i in 0..4 {
                            acc[i] += s[i] * area;
                        }
                    }
                }
                acc
            }
            Sampling::Mitchell => {
                // bicubic_clamp_8888 with CubicResamplerMatrix(1/3, 1/3), then clamp_gamut.
                let w = mitchell_weights();
                let frac = |v: f32| v - v.floor();
                let (tx, ty) = (frac(fx + 0.5), frac(fy + 0.5));
                let wts =
                    |t: f32, i: usize| mad(t, mad(t, mad(t, w[12 + i], w[8 + i]), w[4 + i]), w[i]);
                let sx = [wts(tx, 0), wts(tx, 1), wts(tx, 2), wts(tx, 3)];
                let sy = [wts(ty, 0), wts(ty, 1), wts(ty, 2), wts(ty, 3)];
                let mut acc = [0.0f32; 4];
                let mut sample_y = fy - 1.5;
                for wy in sy {
                    let mut sample_x = fx - 1.5;
                    for wx in sx {
                        let scale = wx * wy;
                        let s = load(self.gather(sample_x, sample_y, false));
                        for i in 0..4 {
                            acc[i] = mad(scale, s[i], acc[i]);
                        }
                        sample_x += 1.0;
                    }
                    sample_y += 1.0;
                }
                let a = acc[3].clamp(0.0, 1.0);
                [
                    acc[0].max(0.0).min(a),
                    acc[1].max(0.0).min(a),
                    acc[2].max(0.0).min(a),
                    a,
                ]
            }
        }
    }
}

/// `SkImageShader::CubicResamplerMatrix(1/3, 1/3).getColMajor()`.
fn mitchell_weights() -> [f32; 16] {
    let b = 1.0f32 / 3.0;
    let c = 1.0f32 / 3.0;
    // SkM44 rows as written (row-major), stored column-major.
    let rows = [
        [
            (1.0 / 6.0) * b,
            -(3.0 / 6.0) * b - c,
            (3.0 / 6.0) * b + 2.0 * c,
            -(1.0 / 6.0) * b - c,
        ],
        [
            1.0 - (2.0 / 6.0) * b,
            0.0,
            -3.0 + (12.0 / 6.0) * b + c,
            2.0 - (9.0 / 6.0) * b - c,
        ],
        [
            (1.0 / 6.0) * b,
            (3.0 / 6.0) * b + c,
            3.0 - (15.0 / 6.0) * b - 2.0 * c,
            -2.0 + (9.0 / 6.0) * b + c,
        ],
        [0.0, 0.0, -c, (1.0 / 6.0) * b + c],
    ];
    let mut out = [0.0f32; 16];
    for (r, row) in rows.iter().enumerate() {
        for (col, v) in row.iter().enumerate() {
            out[col * 4 + r] = *v;
        }
    }
    out
}
