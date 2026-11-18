//! `GoldenBorderImageEffect` (`Core/ImageEffects/Filters/GoldenBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `GoldenBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct GoldenBorderImageEffect {
    pub size: i32,
    pub bevel_strength: f32,
    pub color: SkColor,
    pub inner_border: bool,
    pub outer_border: bool,
}

impl Default for GoldenBorderImageEffect {
    fn default() -> Self {
        GoldenBorderImageEffect {
            size: 25,
            bevel_strength: 60.0,
            color: SkColor::argb(0xFFD4AF37),
            inner_border: true,
            outer_border: true,
        }
    }
}

impl EffectImpl for GoldenBorderImageEffect {
    fn info() -> Info {
        Info::new("golden_border", "Golden border", EffectCategory::Filters, "image-effect/crown")
            .desc("Adds a luxurious picture frame border to the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 5, 80, 25, |e: &mut Self, v| e.size = v),
            float_slider::<Self>("bevel_strength", "Bevel strength", 0.0, 100.0, 60.0, |e: &mut Self, v| e.bevel_strength = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFFD4AF37), |e: &mut Self, v| e.color = v),
            bool_param::<Self>("inner_border", "Inner border", true, |e: &mut Self, v| e.inner_border = v),
            bool_param::<Self>("outer_border", "Outer border", true, |e: &mut Self, v| e.outer_border = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let size = self.size.clamp(5, 80);
        let bevel = clampf(self.bevel_strength, 0.0, 100.0) / 100.0;
        let base = self.color;
        let dark = darken(base, 0.55);
        let light = lighten(base, 0.35);
        let highlight = lighten(base, 0.60);
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + size * 2, ih + size * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        draw_frame(&mut canvas, nw, nh, size, bevel, base, dark, light, highlight);
        let sf = size as f32;
        canvas.draw_bitmap(source, sf, sf, None);
        if self.inner_border {
            sk::draw_rect_xywh(&mut canvas, sf - 1.0, sf - 1.0, (iw + 2) as f32, (ih + 2) as f32, &solid(dark, true), Some(&Pen::new(1.5)));
        }
        if self.outer_border {
            sk::draw_rect_xywh(&mut canvas, 0.5, 0.5, (nw - 1) as f32, (nh - 1) as f32, &solid(dark, true), Some(&Pen::new(1.5)));
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn darken(c: SkColor, f: f32) -> SkColor {
    let d = |v: u8| (v as f32 * (1.0 - f)) as u8;
    SkColor::rgba(d(c.r), d(c.g), d(c.b), c.a)
}

fn lighten(c: SkColor, f: f32) -> SkColor {
    let l = |v: u8| 255f32.min(v as f32 + (255 - v) as f32 * f) as u8;
    SkColor::rgba(l(c.r), l(c.g), l(c.b), c.a)
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(
    canvas: &mut Canvas,
    width: i32,
    height: i32,
    size: i32,
    bevel: f32,
    base: SkColor,
    dark: SkColor,
    light: SkColor,
    highlight: SkColor,
) {
    let (w, h, s) = (width as f32, height as f32, size as f32);
    let bp = solid(base, true);
    sk::draw_rect_xywh(canvas, 0.0, 0.0, w, s, &bp, None);
    sk::draw_rect_xywh(canvas, 0.0, h - s, w, s, &bp, None);
    sk::draw_rect_xywh(canvas, 0.0, 0.0, s, h, &bp, None);
    sk::draw_rect_xywh(canvas, w - s, 0.0, s, h, &bp, None);
    if bevel <= 0.0 {
        return;
    }
    let clear = base.with_alpha(0);
    let g = |st: (f32, f32), e: (f32, f32), c: SkColor| sk::linear_gradient(st, e, &[c, clear], None, true);
    let a = |c: SkColor, v: f32| c.with_alpha((v * bevel) as u8);
    let bw = s * 0.35 * bevel;
    sk::draw_rect_xywh(canvas, 0.0, 0.0, w, bw, &g((0.0, 0.0), (0.0, bw), a(highlight, 200.0)), None);
    sk::draw_rect_xywh(canvas, 0.0, 0.0, bw, h, &g((0.0, 0.0), (bw, 0.0), a(highlight, 180.0)), None);
    sk::draw_rect_xywh(canvas, 0.0, h - bw, w, bw, &g((0.0, h), (0.0, h - bw), a(dark, 200.0)), None);
    sk::draw_rect_xywh(canvas, w - bw, 0.0, bw, h, &g((w, 0.0), (w - bw, 0.0), a(dark, 200.0)), None);
    let ibw = s * 0.25 * bevel;
    sk::draw_rect_xywh(canvas, s, s - ibw, (width - size * 2) as f32, ibw, &g((0.0, s), (0.0, s - ibw), a(dark, 160.0)), None);
    sk::draw_rect_xywh(canvas, s - ibw, s, ibw, (height - size * 2) as f32, &g((s, 0.0), (s - ibw, 0.0), a(dark, 140.0)), None);
    let hs = (height - size) as f32;
    let ws = (width - size) as f32;
    sk::draw_rect_xywh(canvas, s, hs, (width - size * 2) as f32, ibw, &g((0.0, hs), (0.0, hs + ibw), a(light, 140.0)), None);
    sk::draw_rect_xywh(canvas, ws, s, ibw, (height - size * 2) as f32, &g((ws, 0.0), (ws + ibw, 0.0), a(light, 140.0)), None);
    let gy = s * 0.5;
    let pen = Pen::new(1.0);
    let gd = solid(a(dark, 120.0), true);
    sk::draw_line(canvas, 0.0, gy, w, gy, &pen, &gd);
    sk::draw_line(canvas, 0.0, h - gy, w, h - gy, &pen, &gd);
    sk::draw_line(canvas, gy, 0.0, gy, h, &pen, &gd);
    sk::draw_line(canvas, w - gy, 0.0, w - gy, h, &pen, &gd);
    let gl = solid(a(highlight, 80.0), true);
    sk::draw_line(canvas, 0.0, gy + 1.0, w, gy + 1.0, &pen, &gl);
    sk::draw_line(canvas, 0.0, h - gy + 1.0, w, h - gy + 1.0, &pen, &gl);
    sk::draw_line(canvas, gy + 1.0, 0.0, gy + 1.0, h, &pen, &gl);
    sk::draw_line(canvas, w - gy + 1.0, 0.0, w - gy + 1.0, h, &pen, &gl);
}
