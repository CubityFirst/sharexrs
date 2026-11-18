//! `SKCanvas` over a [`Bitmap`], drawing as Skia's CPU backend does: integer-offset
//! bitmaps through the sprite blitter, everything else through the raster pipeline
//! (lowp when every stage has a 16-bit version, highp otherwise), with Skia's blitter
//! choices for full and partial coverage.

use super::coverage::{self, Coverage};
use super::gradient::LinearGradient;
use super::*;
use super::Bitmap;
use crate::blitter::{Call, LegacyColor};

/// `SKSamplingOptions` as ShareX uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampling {
    Nearest,
    Linear,
    /// `SKCubicResampler.Mitchell` (B = C = 1/3), ShareX's `HighQualitySampling`.
    Mitchell,
    /// `SKCubicResampler.CatmullRom` (B = 0, C = 1/2), what ShareX's icon and image
    /// converters draw with.
    CatmullRom,
    /// `SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.Linear)`: bilinear within
    /// the two nearest mipmap levels, blended (what ShareX's OCR scales down with).
    LinearMipmapLinear,
}

impl Sampling {
    /// A cubic resampler (`bicubic_clamp_8888`, highp only).
    pub fn is_cubic(self) -> bool {
        matches!(self, Sampling::Mitchell | Sampling::CatmullRom)
    }
}

/// `SKShaderTileMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileMode {
    Clamp,
    Repeat,
    Mirror,
    /// Transparent outside the image (nearest sampling only).
    Decal,
}

/// The two blend modes ShareX draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    SrcOver,
    Src,
    /// `SKBlendMode.Multiply`: `s*(1-da) + d*(1-sa) + s*d`, coverage lerped
    /// (it doesn't support coverage as alpha).
    Multiply,
    /// `SKBlendMode.Clear`: 0, coverage lerped.
    Clear,
    /// `SKBlendMode.Overlay`, coverage lerped.
    Overlay,
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
    /// An anti-aliased clip (`clipPath(path, kIntersect, true)`); only path
    /// fills honour it.
    pub aa_clip: Option<crate::aaclip::AAClip>,
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
            aa_clip: None,
        }
    }

    /// `SKCanvas.ClipPath(path, Intersect, antialias: true)` on the device's
    /// rectangular clip: `SkRasterClip::op` makes it an `SkAAClip`
    /// (`setPath` with the clip's bounds).
    pub fn clip_path_aa(&mut self, path: &crate::Path) {
        assert!(self.aa_clip.is_none(), "intersecting two anti-aliased clips isn't ported");
        let device = path.transformed(&self.ctm());
        let bounds = crate::geom::IRect::from_ltrb(0, 0, self.width() as i32, self.height() as i32);
        self.aa_clip = Some(crate::aaclip::AAClip::from_path(&device, bounds));
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
        if treat_as_sprite(&total, image.width(), image.height(), sampling, paint.anti_alias) {
            let ix = (total.tx() + 0.5).floor() as i32;
            let iy = (total.ty() + 0.5).floor() as i32;
            // clipHandlesSprite: a BW clip, or an AA clip that contains the sprite.
            let handles = self.aa_clip.as_ref().is_none_or(|c| {
                c.quick_contains(ix, iy, ix + image.width() as i32, iy + image.height() as i32)
            });
            if handles {
                self.sprite(image, ix, iy, paint);
                return;
            }
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
            let cov = if let Some(clip) = &self.aa_clip {
                coverage::rect_aa_clip(device, p.anti_alias, clip, self.width(), self.height())
            } else if p.anti_alias {
                coverage::anti_fill_rect(device, self.width(), self.height())
            } else {
                coverage::fill_rect(device, self.width(), self.height())
            };
            self.blit(&cov, &p);
        } else {
            let path = rect_path(bounds, &total);
            let cov = coverage::fill_path(&path, p.anti_alias, self.width(), self.height());
            self.blit(&cov, &p);
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

    /// `SkCanvas::drawRect` with `Style = Stroke` and a miter join
    /// (`SkDraw::drawRect`'s `kStroke_RectType`: `SkScan::AntiFrameRect`
    /// when anti-aliased, `SkScan::FrameRect` otherwise). A rotated canvas
    /// or a zero width isn't ported (it'd go through the path stroker or the
    /// hairline).
    pub fn stroke_rect(&mut self, rect: Rect, stroke_width: f32, paint: &Paint) {
        if self.dst.is_empty() || !self.matrix.rect_stays_rect() || stroke_width <= 0.0 {
            return;
        }
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            // easy_rect_join: an empty rect goes through the path stroker.
            return;
        }
        let (x0, y0) = self.matrix.map(rect.left, rect.top);
        let (x1, y1) = self.matrix.map(rect.right, rect.bottom);
        let device = Rect::ltrb(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
        // compute_stroke_size: the width mapped as a vector.
        let m = &self.matrix;
        let sx = (m.sx() * stroke_width + m.kx() * stroke_width).abs();
        let sy = (m.ky() * stroke_width + m.sy() * stroke_width).abs();
        let cov = if paint.anti_alias {
            coverage::anti_frame_rect(device, sx, sy, self.width(), self.height())
        } else {
            coverage::frame_rect(device, sx, sy, self.width(), self.height())
        };
        self.blit(&cov, paint);
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
            let cov = if let Some(clip) = &self.aa_clip {
                coverage::rect_aa_clip(device, paint.anti_alias, clip, self.width(), self.height())
            } else if paint.anti_alias {
                coverage::anti_fill_rect(device, self.width(), self.height())
            } else {
                coverage::fill_rect(device, self.width(), self.height())
            };
            self.blit(&cov, paint);
        } else {
            let path = rect_path(r, &self.matrix);
            let cov = coverage::fill_path(&path, paint.anti_alias, self.width(), self.height());
            self.blit(&cov, paint);
        }
    }

    /// `SkCanvas::drawPath` for a fill (strokes are turned into fills by the caller).
    pub fn fill_path(&mut self, path: &tiny_skia::Path, paint: &Paint) {
        self.fill_sk_path(&crate::draw::from_tiny_skia(path), paint);
    }

    /// `SkCanvas::drawPath` for a fill of an `SkPath` (conics kept, as Skia counts
    /// points when it picks a scan converter).
    pub fn fill_sk_path(&mut self, path: &crate::Path, paint: &Paint) {
        if self.dst.is_empty() {
            return;
        }
        let device = if self.matrix.is_identity() {
            path.clone()
        } else {
            let m = &self.matrix;
            path.transformed(&crate::Matrix {
                sx: m.sx(),
                kx: m.kx(),
                tx: m.tx(),
                ky: m.ky(),
                sy: m.sy(),
                ty: m.ty(),
            })
        };
        let cov = match &self.aa_clip {
            Some(clip) => coverage::fill_path_aa_clip(&device, clip, self.width(), self.height()),
            None => coverage::fill_path(&device, paint.anti_alias, self.width(), self.height()),
        };
        self.blit(&cov, paint);
    }

    /// The canvas matrix as a path matrix.
    fn ctm(&self) -> crate::Matrix {
        let m = &self.matrix;
        crate::Matrix {
            sx: m.sx(),
            kx: m.kx(),
            tx: m.tx(),
            ky: m.ky(),
            sy: m.sy(),
            ty: m.ty(),
        }
    }

    /// The matrix and device bounds a stroke's dash and stroker see.
    pub fn stroke_context(&self) -> crate::draw::StrokeContext {
        crate::draw::StrokeContext::new(
            self.ctm(),
            crate::geom::IRect::from_ltrb(0, 0, self.width() as i32, self.height() as i32),
        )
    }

    /// Whether an anti-aliased stroke of `width` is a hairline here
    /// (`SkDrawTreatAsHairline`).
    pub fn is_hairline(&self, width: f32) -> bool {
        crate::hair::hairline_coverage(width, &self.ctm()).is_some()
    }

    /// `SkCanvas::drawPath` with an anti-aliased stroke paint of `width` when
    /// Skia draws it as a hairline (`SkDrawTreatAsHairline`): `path` is
    /// dashed already, `cap` is the paint's. False (nothing drawn) when the
    /// stroke is wider than a device pixel.
    pub fn stroke_hairline(
        &mut self,
        path: &crate::Path,
        width: f32,
        cap: crate::Cap,
        paint: &Paint,
    ) -> bool {
        let ctm = self.ctm();
        let Some(coverage) = crate::hair::hairline_coverage(width, &ctm) else {
            return false;
        };
        if self.dst.is_empty() {
            return true;
        }
        let mut paint = paint.clone();
        match &mut paint.shader {
            Shader::Color(c) => c[3] = crate::hair::hairline_alpha(c[3], coverage),
            _ => paint.alpha = crate::hair::hairline_alpha(paint.alpha, coverage),
        }
        let device = path.transformed(&ctm);
        let cov = coverage::anti_hair_path(&device, cap, self.width(), self.height());
        self.blit(&cov, &paint);
        true
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
        // SkSpriteBlitter_Memcpy and the legacy sprite blitters need an N32 device.
        let plain = paint.color_filter.is_none() && paint.alpha == 255 && !crate::blitter::pipeline_blitters();
        // SkSpriteBlitter::ChooseL32: src-over with a global alpha and no colour filter is
        // Sprite_D32_S32 with kGlobalAlpha (an SKBitmap is never kOpaque, so kSrcPixelAlpha
        // too): S32A_Blend_BlitRow32, SkBlendARGB32 per pixel.
        let global_alpha = paint.color_filter.is_none()
            && paint.alpha != 255
            && paint.blend == Blend::SrcOver
            && !crate::blitter::pipeline_blitters();
        for y in y0..y1 {
            for x in x0..x1 {
                let si = (y - iy) as usize * iw + (x - ix) as usize;
                let di = y as usize * dw + x as usize;
                let s = image.px(si);
                let out = if global_alpha {
                    blend_argb32(s, self.dst.px(di), paint.alpha)
                } else if plain {
                    match paint.blend {
                        // SkSpriteBlitter_Memcpy.
                        Blend::Src => s,
                        // SkRasterPipelineSpriteBlitter: load_8888, multiply (lowp), store.
                        Blend::Multiply | Blend::Clear | Blend::Overlay => {
                            self.blend_full(di, Px::Low(s.map(u32::from)), paint.blend)
                        }
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
        let pixel = |this: &mut Self, x: i32, y: i32, call: Call, c: u8, span_left: i32| {
            let di = y as usize * w + x as usize;
            let d = this.dst.px(di);
            let out = match &legacy {
                Some(Legacy::Color(lc)) => from_argb(lc.blend(to_argb(d), call, c)),
                Some(Legacy::Image) => {
                    // SkARGB32_Shader_Blitter: blitAntiH2 goes through blitAntiH, blitV
                    // blends as blitAntiH, blitMask through `blend_row_A8`.
                    if c == 0 {
                        return;
                    }
                    let SamplerKind::Image(img) = &sampler.kind else {
                        unreachable!()
                    };
                    let s = img.legacy_shade(x, y, span_left);
                    if call == Call::Mask {
                        blend_row_a8(s, d, c)
                    } else if c == 255 {
                        s32a_opaque(s, d)
                    } else {
                        blend_argb32(s, d, c)
                    }
                }
                None => {
                    // SkRasterPipelineBlitter: blitAntiH at full coverage is blitRect;
                    // blitV and blitAntiH2 are blitMask, which always lerps.
                    if c == 0 {
                        return;
                    }
                    let s = sampler.shade(x, y);
                    if c == 255 && matches!(call, Call::H | Call::AntiH) {
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
                        pixel(self, x, y, Call::H, 255, *left);
                    }
                }
            }
            Coverage::Mask(cells) => {
                for (i, cell) in cells.iter().enumerate() {
                    if let Some((call, c)) = *cell {
                        let x = (i % w) as i32;
                        pixel(self, x, (i / w) as i32, call, c, x);
                    }
                }
            }
            Coverage::Hits(hits) => {
                for &(x, y, call, c) in hits {
                    pixel(self, x, y, call, c, x);
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
            (_, Blend::Clear) => [0, 0, 0, 0],
            (Px::High(c), b @ (Blend::Multiply | Blend::Overlay)) => store(separable_high(b, c, load(self.dst.px(di)))),
            (Px::Low(c), b @ (Blend::Multiply | Blend::Overlay)) => {
                separable_low(b, c, self.dst.px(di)).map(|v| v.min(255) as u8)
            }
            (Px::Low(c), Blend::SrcOver) => {
                let d = self.dst.px(di);
                let inv = 255 - c[3];
                // srcover_rgba_8888 (div255), or on a device with a colour space
                // load_dst + srcover (div255_accurate).
                if crate::blitter::pipeline_blitters() {
                    [0, 1, 2, 3].map(|i| (c[i] + div255_accurate(d[i] as u32 * inv)).min(255) as u8)
                } else {
                    [0, 1, 2, 3].map(|i| (c[i] + div255(d[i] as u32 * inv)).min(255) as u8)
                }
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
            (Px::High(_), Blend::Clear) => {
                let f = cov as f32 * (1.0 / 255.0);
                let dd = load(d);
                store([0, 1, 2, 3].map(|i| mad(0.0 - dd[i], f, dd[i])))
            }
            (Px::Low(_), Blend::Clear) => {
                let t = lowp_from_float(cov as f32 * (1.0 / 255.0));
                [0, 1, 2, 3].map(|i| div255(d[i] as u32 * (255 - t)).min(255) as u8)
            }
            (Px::High(c), b @ (Blend::Multiply | Blend::Overlay)) => {
                let f = cov as f32 * (1.0 / 255.0);
                let dd = load(d);
                let m = separable_high(b, c, dd);
                store([0, 1, 2, 3].map(|i| mad(m[i] - dd[i], f, dd[i])))
            }
            (Px::Low(c), b @ (Blend::Multiply | Blend::Overlay)) => {
                let t = lowp_from_float(cov as f32 * (1.0 / 255.0));
                let m = separable_low(b, c, d);
                [0, 1, 2, 3].map(|i| div255(d[i] as u32 * (255 - t) + m[i] * t).min(255) as u8)
            }
        }
    }
}

/// The highp `multiply`/`overlay` blend stages.
fn separable_high(mode: Blend, s: [f32; 4], d: [f32; 4]) -> [f32; 4] {
    let (sa, da) = (s[3], d[3]);
    match mode {
        Blend::Multiply => [0, 1, 2, 3].map(|i| s[i] * (1.0 - da) + d[i] * (1.0 - sa) + s[i] * d[i]),
        Blend::Overlay => {
            let ch = |s: f32, d: f32| {
                s * (1.0 - da)
                    + d * (1.0 - sa)
                    + if d + d <= da { s * d + s * d } else { sa * da - ((da - d) * (sa - s) + (da - d) * (sa - s)) }
            };
            [ch(s[0], d[0]), ch(s[1], d[1]), ch(s[2], d[2]), mad(da, 1.0 - sa, sa)]
        }
        _ => unreachable!(),
    }
}

/// The lowp `multiply`/`overlay` blend stages (`div255`, even with accurate blends).
fn separable_low(mode: Blend, s: [u32; 4], d: [u8; 4]) -> [u32; 4] {
    let (sa, da) = (s[3], d[3] as u32);
    let d = d.map(u32::from);
    match mode {
        Blend::Multiply => [0, 1, 2, 3].map(|i| div255(s[i] * (255 - da) + d[i] * (255 - sa) + s[i] * d[i])),
        Blend::Overlay => {
            let ch = |s: u32, d: u32| {
                let tail = if 2 * d <= da { 2 * s * d } else { sa * da - 2 * (sa - s) * (da - d) };
                div255(s * (255 - da) + d * (255 - sa) + tail)
            };
            [ch(s[0], d[0]), ch(s[1], d[1]), ch(s[2], d[2]), sa + div255(da * (255 - sa))]
        }
        _ => unreachable!(),
    }
}

/// Which legacy (pre-raster-pipeline) blitter `SkBlitter::Choose` picks: src-over onto
/// N32 without a colour filter, for a solid colour (`SkARGB32_Blitter` and friends) or
/// an image shader that has a legacy context (`SkBitmapProcLegacyShader`, nearest here).
enum Legacy {
    Color(LegacyColor),
    Image,
}

fn use_legacy_blitter(paint: &Paint, sampler: &Sampler) -> Option<Legacy> {
    if paint.blend != Blend::SrcOver || paint.color_filter.is_some() || crate::blitter::pipeline_blitters() {
        return None;
    }
    match (&paint.shader, &sampler.kind) {
        (Shader::Color(c), _) => Some(Legacy::Color(LegacyColor::new(c[3], c[0], c[1], c[2]))),
        // SkImageShader::onMakeContext: no decal, the same tiling in x and y,
        // nearest or (clamped, scale-translate) bilinear.
        (Shader::Image { .. }, SamplerKind::Image(img))
            if paint.alpha == 255
                && !img.mipmapped
                && img.tile_x != TileMode::Decal
                && img.tile_y != TileMode::Decal
                && (img.sampling == Sampling::Nearest
                    || (img.sampling == Sampling::Linear
                        && img.tile_x == TileMode::Clamp
                        && img.tile_y == TileMode::Clamp
                        && img.inv.is_scale_translate())) =>
        {
            Some(Legacy::Image)
        }
        _ => None,
    }
}

/// RGBA bytes as Skia's packed `SkPMColor` (`a << 24 | r << 16 | g << 8 | b`; the
/// legacy blitters treat the colour channels alike, so the order doesn't matter).
fn to_argb(p: [u8; 4]) -> u32 {
    (p[3] as u32) << 24 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32
}

fn from_argb(c: u32) -> [u8; 4] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]
}

/// `blend_row_A8` (`SkARGB32_Shader_Blitter::blitMask`, a shader that isn't opaque):
/// `s' = approx_scale(s, c)`, `s' + approx_scale(d, 255 - s'.a)`.
fn blend_row_a8(s: [u8; 4], d: [u8; 4], c: u8) -> [u8; 4] {
    let approx = |x: u32, y: u32| (x * y + x) / 256;
    let s = s.map(|v| approx(v as u32, c as u32));
    let inv = 255 - s[3];
    [0, 1, 2, 3].map(|i| (s[i] + approx(d[i] as u32, inv)) as u8)
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

/// `SkTreatAsSprite`: the bitmap lands on whole pixels (to 1/16 of a pixel when
/// anti-aliased) with no scale, so it can be copied as a sprite.
fn treat_as_sprite(m: &Matrix, width: u32, height: u32, sampling: Sampling, anti_alias: bool) -> bool {
    // SkSamplingPriv::NoChangeWithIdentityMatrix: not a cubic with B != 0.
    if sampling == Sampling::Mitchell {
        return false;
    }
    if !m.is_scale_translate() {
        return false;
    }
    let subpixel_bits: i32 = if anti_alias { 4 } else { 0 };
    if subpixel_bits == 0 && m.is_translate() {
        return true;
    }
    if m.sx() < 0.0 || m.sy() < 0.0 {
        return false;
    }
    let round = |v: f32| (v + 0.5).floor() as i32;
    let mut dst = m.map_rect(Rect::xywh(0.0, 0.0, width as f32, height as f32));
    let (ox, oy) = (round(m.tx()), round(m.ty()));
    let mut isrc = [ox, oy, ox + width as i32, oy + height as i32];
    if subpixel_bits != 0 {
        for v in &mut isrc {
            *v <<= subpixel_bits;
        }
        let scale = (1 << subpixel_bits) as f32;
        dst = Rect::ltrb(dst.left * scale, dst.top * scale, dst.right * scale, dst.bottom * scale);
    }
    isrc == [round(dst.left), round(dst.top), round(dst.right), round(dst.bottom)]
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

fn rect_path(r: Rect, m: &Matrix) -> crate::Path {
    let mut path = crate::Path::new();
    let p = [
        m.map(r.left, r.top),
        m.map(r.right, r.top),
        m.map(r.right, r.bottom),
        m.map(r.left, r.bottom),
    ];
    path.move_to(p[0].0, p[0].1);
    for q in &p[1..] {
        path.line_to(q.0, q.1);
    }
    path.close();
    path
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
    /// The image, or for a mipmapped draw the level `SkMipmapAccessor` picked.
    image: std::borrow::Cow<'a, Bitmap>,
    inv: Matrix,
    sampling: Sampling,
    tile_x: TileMode,
    tile_y: TileMode,
    /// A linear mipmap was asked for: never the legacy blitter (`onMakeContext` only
    /// takes no mipmaps or nearest ones).
    mipmapped: bool,
    /// The smaller mipmap level blended in (`mipmap_linear_*`).
    lower: Option<LowerLevel>,
}

/// `SkRasterPipeline_MipmapCtx`: the lower level, its weight and how its coordinates
/// scale from the upper level's.
struct LowerLevel {
    image: Bitmap,
    weight: f32,
    scale_x: f32,
    scale_y: f32,
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
                let mut inv = total.invert()?;
                let mipmapped = *sampling == Sampling::LinearMipmapLinear;
                let mut sampling = *sampling;
                let mut level: std::borrow::Cow<'a, Bitmap> = std::borrow::Cow::Borrowed(*image);
                let mut lower = None;
                if mipmapped {
                    // `SkMipmapAccessor`: sample the level the scale picks (the image when
                    // not shrinking) through `Scale(level / image) * inv`, and blend in
                    // the next smaller level.
                    let choice = super::mipmap::choose_linear(&inv, image.width(), image.height());
                    if choice.upper.is_some() || choice.lower.is_some() {
                        let mut levels = super::mipmap::build(image);
                        let upper = match choice.upper {
                            Some(i) => std::borrow::Cow::Owned(levels[i].clone()),
                            None => std::borrow::Cow::Borrowed(*image),
                        };
                        let scale_to = |b: &Bitmap| {
                            Matrix::scale_translate(
                                b.width() as f32 / image.width() as f32,
                                b.height() as f32 / image.height() as f32,
                                0.0,
                                0.0,
                            )
                        };
                        inv = Matrix::concat(&scale_to(&upper), &inv);
                        if let Some((i, weight)) = choice.lower {
                            let lower_image = levels.swap_remove(i);
                            lower = Some(LowerLevel {
                                scale_x: lower_image.width() as f32 / upper.width() as f32,
                                scale_y: lower_image.height() as f32 / upper.height() as f32,
                                image: lower_image,
                                weight,
                            });
                        }
                        level = upper;
                    }
                    sampling = Sampling::Linear;
                }
                // `tweak_sampling`: bilinear through an integer translate is nearest.
                if sampling == Sampling::Linear
                    && inv.is_translate()
                    && inv.tx() == inv.tx().trunc()
                    && inv.ty() == inv.ty().trunc()
                {
                    sampling = Sampling::Nearest;
                }
                // The mipmap stages and the generic bilinear sampler are float only;
                // lowp has the clamp and (nearest) decal stages and bilerp_clamp_8888,
                // not repeat or mirror.
                let lowp_tile = |t: &TileMode| {
                    *t == TileMode::Clamp || (*t == TileMode::Decal && sampling == Sampling::Nearest)
                };
                let lowp = !sampling.is_cubic()
                    && lowp_tile(tile_x)
                    && lowp_tile(tile_y)
                    && !(mipmapped && (sampling == Sampling::Linear || lower.is_some()));
                (
                    SamplerKind::Image(ImageSampler {
                        image: level,
                        inv,
                        sampling,
                        tile_x: *tile_x,
                        tile_y: *tile_y,
                        mipmapped,
                        lower,
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
        let lowp = lowp_shader && paint.color_filter.is_none();
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
            TileMode::Clamp | TileMode::Decal => v,
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

    /// The legacy shader context (`SkBitmapProcLegacyShader`): nearest as the
    /// pipeline picks it, or `SkBitmapProcState`'s clamped bilinear filter for
    /// a span starting at `span_left`.
    fn legacy_shade(&self, x: i32, y: i32, span_left: i32) -> [u8; 4] {
        if self.sampling == Sampling::Linear
            && let Some(st) = LegacyBilerp::new(&self.inv, &self.image)
        {
            return st.shade(&self.image, x, y, span_left);
        }
        if self.sampling == Sampling::Nearest
            && self.tile_x == TileMode::Clamp
            && self.tile_y == TileMode::Clamp
            && self.inv.is_scale_translate()
        {
            return self.legacy_nearest_scale(x, y, span_left);
        }
        self.gather_nearest(x, y)
    }

    /// `SkBitmapProcState`'s `nofilter_scale<clamp, clamp>`: the span's first
    /// pixel centre mapped in float (`ScaleTrans_xy`), converted to 32.32
    /// fixed point less the nearest bias (`SkBitmapProcStateAutoMapper`:
    /// 1/65536, so exact pixel edges round down), then stepped by the
    /// inverse x scale in 32.32 for each pixel along the span.
    fn legacy_nearest_scale(&self, x: i32, y: i32, span_left: i32) -> [u8; 4] {
        // SkFloatToFixed3232: (int64)(x * 2^32), x times an exact power of two.
        let to3232 = |v: f32| (v * (65536.0f32 * 65536.0f32)) as i64;
        let m = &self.inv;
        let px = (span_left as f32 + 0.5) * m.sx() + m.tx();
        let py = (y as f32 + 0.5) * m.sy() + m.ty();
        let bias = 1i64 << 16;
        let fy = to3232(py).wrapping_sub(bias);
        let fx0 = to3232(px).wrapping_sub(bias);
        let dx = to3232(m.sx());
        let fx = fx0.wrapping_add(dx.wrapping_mul((x - span_left) as i64));
        let max_x = self.image.width() as i32 - 1;
        let max_y = self.image.height() as i32 - 1;
        let ix = ((fx >> 32) as i32).clamp(0, max_x);
        let iy = ((fy >> 32) as i32).clamp(0, max_y);
        self.image.pixel(ix as u32, iy as u32)
    }

    fn sample_lowp(&self, x: i32, y: i32) -> [u32; 4] {
        let (fx, fy) = self.coords(x, y);
        // lowp's decal_x / decal_y: inside is 0 <= v < limit.
        let outside = |v: f32, limit: u32| !(0.0 <= v && v < limit as f32);
        if (self.tile_x == TileMode::Decal && outside(fx, self.image.width()))
            || (self.tile_y == TileMode::Decal && outside(fy, self.image.height()))
        {
            return [0; 4];
        }
        if self.sampling == Sampling::Linear {
            return self.bilerp_lowp(fx, fy);
        }
        self.gather(fx, fy, self.sampling == Sampling::Nearest)
            .map(|v| v as u32)
    }

    /// lowp's `bilerp_clamp_8888`: the sample point quantised to 16.16, the
    /// four pixels (clamped) lerped in Q15 fixed point.
    fn bilerp_lowp(&self, x: f32, y: f32) -> [u32; 4] {
        let qx = (65536.0f32 * x + 0.5).floor() as i32 - 32768;
        let qy = (65536.0f32 * y + 0.5).floor() as i32 - 32768;
        let (sx, sy) = (qx >> 16, qy >> 16);
        let tx = (qx ^ 0x8000) as i16;
        let ty = (qy ^ 0x8000) as i16;
        // _mm_mulhrs_epi16
        let mulhrs = |a: i16, b: i16| ((a as i32 * b as i32 + (1 << 14)) >> 15) as i16;
        let lerp_x = |l: u16, r: u16| -> u16 {
            let width = (r.wrapping_sub(l) as i16).wrapping_shl(7);
            let middle = r.wrapping_add(l).wrapping_shl(7);
            let v2 = (mulhrs(tx, width) as u16).wrapping_add(middle).wrapping_add(1);
            v2 >> 1
        };
        let lerp_y = |t: u16, b: u16| -> u16 {
            let width = (b as i16).wrapping_sub(t as i16);
            let middle = b.wrapping_add(t);
            let blend = (mulhrs(ty, width) as u16).wrapping_add(middle).wrapping_add(0x80);
            blend >> 8
        };
        let (w, h) = (self.image.width() as i32, self.image.height() as i32);
        let px = |ix: i32, iy: i32| self.image.pixel(ix.clamp(0, w - 1) as u32, iy.clamp(0, h - 1) as u32);
        let (tl, tr, bl, br) = (px(sx, sy), px(sx + 1, sy), px(sx, sy + 1), px(sx + 1, sy + 1));
        [0, 1, 2, 3].map(|i| {
            let top = lerp_x(tl[i] as u16, tr[i] as u16);
            let bottom = lerp_x(bl[i] as u16, br[i] as u16);
            lerp_y(top, bottom) as u32
        })
    }

    fn sample_highp(&self, x: i32, y: i32) -> [f32; 4] {
        let (fx, fy) = self.coords(x, y);
        match self.sampling {
            Sampling::Nearest => {
                // decal_x / decal_y (nearest: the right and bottom edges are inside).
                let decal = |v: f32, limit: u32| {
                    let w = limit as f32;
                    (0.0 < v && v < w) || v == w
                };
                if (self.tile_x == TileMode::Decal && !decal(fx, self.image.width()))
                    || (self.tile_y == TileMode::Decal && !decal(fy, self.image.height()))
                {
                    return [0.0; 4];
                }
                let fx = self.tile(fx, self.tile_x, self.image.width());
                let fy = self.tile(fy, self.tile_y, self.image.height());
                load(self.gather(fx, fy, true))
            }
            Sampling::Linear | Sampling::LinearMipmapLinear => {
                // bilerp_clamp_8888, or for a mipmapped draw the generic
                // bilinear_setup/accumulate stages (the same sums) on each level,
                // blended by mipmap_linear_finish (`lerp`).
                let upper = bilinear_clamp(&self.image, fx, fy);
                match &self.lower {
                    None => upper,
                    Some(l) => {
                        let lower = bilinear_clamp(&l.image, fx * l.scale_x, fy * l.scale_y);
                        [0, 1, 2, 3].map(|i| mad(lower[i] - upper[i], l.weight, upper[i]))
                    }
                }
            }
            Sampling::Mitchell | Sampling::CatmullRom => {
                // bicubic_clamp_8888 with CubicResamplerMatrix(B, C), then clamp_gamut.
                let w = if self.sampling == Sampling::Mitchell {
                    cubic_weights(1.0 / 3.0, 1.0 / 3.0)
                } else {
                    cubic_weights(0.0, 0.5)
                };
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

/// Bilinear sampling of a clamped 8888 image at (`fx`, `fy`): the four pixels at
/// +-0.5 weighted by their overlap, accumulated in the pipeline's order.
fn bilinear_clamp(image: &Bitmap, fx: f32, fy: f32) -> [f32; 4] {
    let frac = |v: f32| v - v.floor();
    let (tx, ty) = (frac(fx + 0.5), frac(fy + 0.5));
    let gather = |x: f32, y: f32| {
        let ix = gather_index(x, image.width(), false);
        let iy = gather_index(y, image.height(), false);
        image.pixel(ix, iy)
    };
    let mut acc = [0.0f32; 4];
    for py in [-0.5f32, 0.5] {
        for px in [-0.5f32, 0.5] {
            let s = load(gather(fx + px, fy + py));
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

/// `SkBitmapProcState` for a clamped bilinear image through a scale-translate
/// inverse matrix (`ClampX_ClampY_filter_scale` and `S32_alpha_D32_filter_DX`,
/// 4-bit weights). `None` when Skia turns the filter off.
struct LegacyBilerp {
    sx: f32,
    tx: f32,
    sy: f32,
    ty: f32,
}

/// `SkFloatToFixed3232`: `x * 2^32`, saturated.
fn to_fixed3232(x: f32) -> i64 {
    let v = x * (65536.0f32 * 65536.0);
    v as i64
}

impl LegacyBilerp {
    fn new(inv: &Matrix, image: &Bitmap) -> Option<Self> {
        let (mut sx, mut sy, mut tx, mut ty) = (inv.sx(), inv.sy(), inv.tx(), inv.ty());
        let nearly_eq = |a: f32, b: f32, tol: f32| (a - b).abs() <= tol;
        let integral = |sx: f32, sy: f32, tx: f32, ty: f32| {
            let tol = 1.0f32 / 256.0;
            sx == 1.0
                && sy == 1.0
                && nearly_eq(tx, tx.round_ties_even_like(), tol)
                && nearly_eq(ty, ty.round_ties_even_like(), tol)
        };
        let mut integral_only = integral(sx, sy, tx, ty);
        if !integral_only {
            if sx != 1.0 || sy != 1.0 {
                // just_trans_general on the forward matrix
                let fsx = 1.0f32 / sx;
                let fsy = 1.0f32 / sy;
                let tol = 1.0f32 / 32768.0;
                if (fsx - 1.0).abs() <= tol && (fsy - 1.0).abs() <= tol {
                    let ftx = -tx * fsx;
                    let fty = -ty * fsy;
                    sx = 1.0;
                    sy = 1.0;
                    tx = -ftx;
                    ty = -fty;
                }
            }
            integral_only = integral(sx, sy, tx, ty);
        }
        let valid = ((image.width() | image.height()) & !0x3FFF) == 0;
        if !valid || integral_only {
            return None;
        }
        Some(Self { sx, tx, sy, ty })
    }

    /// `SkBitmapProcStateAutoMapper`: 32.32 coordinates of a pixel centre,
    /// biased by half a filter step.
    fn map(&self, x: i32, y: i32) -> (i64, i64) {
        let px = x as f32 + 0.5;
        let py = y as f32 + 0.5;
        let mx = px * self.sx + self.tx;
        let my = py * self.sy + self.ty;
        let bias = ((1i64 << 16) >> 1) << 16;
        (
            (to_fixed3232(mx) as u64).wrapping_sub(bias as u64) as i64,
            (to_fixed3232(my) as u64).wrapping_sub(bias as u64) as i64,
        )
    }

    /// `pack<clamp, extract_low_bits_clamp_clamp>`: (low, weight, high).
    fn pack(f: i32, max: i32) -> (i32, u32, i32) {
        let clamp = |v: i32| v.clamp(0, max);
        (
            clamp(f >> 16),
            ((f >> 12) & 0xF) as u32,
            clamp(f.wrapping_add(1 << 16) >> 16),
        )
    }

    fn shade(&self, image: &Bitmap, x: i32, y: i32, span_left: i32) -> [u8; 4] {
        // shadeSpan works in chunks of maxCountForBufferSize(512) = 127 pixels,
        // each starting from its own mapped coordinate.
        const CHUNK: i32 = 127;
        let chunk_left = span_left + (x - span_left) / CHUNK * CHUNK;
        let (fx0, fy) = self.map(chunk_left, y);
        let max_x = image.width() as i32 - 1;
        let max_y = image.height() as i32 - 1;
        let (y0, wy, y1) = Self::pack((fy >> 16) as i32, max_y);
        let dx = to_fixed3232(self.sx);
        let fx = fx0.wrapping_add(dx.wrapping_mul((x - chunk_left) as i64));
        let (x0, wx, x1) = Self::pack((fx >> 16) as i32, max_x);
        let (tl, tr) = (image.pixel(x0 as u32, y0 as u32), image.pixel(x1 as u32, y0 as u32));
        let (bl, br) = (image.pixel(x0 as u32, y1 as u32), image.pixel(x1 as u32, y1 as u32));
        [0, 1, 2, 3].map(|c| {
            let top = tl[c] as u32 * (16 - wx) + tr[c] as u32 * wx;
            let bot = bl[c] as u32 * (16 - wx) + br[c] as u32 * wx;
            let px = (top * 16).wrapping_add(bot.wrapping_sub(top).wrapping_mul(wy)) & 0xFFFF;
            (px >> 8).min(255) as u8
        })
    }
}

/// `SkScalarRoundToScalar`: `floor(x + 0.5)`.
trait RoundLike {
    fn round_ties_even_like(self) -> f32;
}

impl RoundLike for f32 {
    fn round_ties_even_like(self) -> f32 {
        (self + 0.5).floor()
    }
}

/// `SkImageShader::CubicResamplerMatrix(1/3, 1/3).getColMajor()`.
/// `CubicResamplerMatrix(B, C)`.
fn cubic_weights(b: f32, c: f32) -> [f32; 16] {
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
