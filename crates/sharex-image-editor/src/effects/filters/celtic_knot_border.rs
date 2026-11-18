//! `CelticKnotBorderImageEffect` (`Core/ImageEffects/Filters/CelticKnotBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `CelticKnotBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct CelticKnotBorderImageEffect {
    pub border_size: i32,
    pub knot_color: SkColor,
    pub bg_color: SkColor,
    pub strand_width: f32,
    pub outline_strands: bool,
    pub inner_line: bool,
}

impl Default for CelticKnotBorderImageEffect {
    fn default() -> Self {
        CelticKnotBorderImageEffect {
            border_size: 52,
            knot_color: SkColor::argb(0xFFB48C3C),
            bg_color: SkColor::argb(0xFF233719),
            strand_width: 35.0,
            outline_strands: true,
            inner_line: true,
        }
    }
}

impl EffectImpl for CelticKnotBorderImageEffect {
    fn info() -> Info {
        Info::new("celtic_knot_border", "Celtic knot border", EffectCategory::Filters, "image-effect/clover")
            .desc("Adds an interweaving Celtic knot pattern border inspired by Insular art manuscripts.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 16, 200, 52, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("knot_color", "Knot color", SkColor::argb(0xFFB48C3C), |e: &mut Self, v| e.knot_color = v),
            color_param::<Self>("bg_color", "Background color", SkColor::argb(0xFF233719), |e: &mut Self, v| e.bg_color = v),
            float_slider::<Self>("strand_width", "Strand width", 10.0, 80.0, 35.0, |e: &mut Self, v| e.strand_width = v),
            bool_param::<Self>("outline_strands", "Outline strands", true, |e: &mut Self, v| e.outline_strands = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(16, 200);
        let frac = clampf(self.strand_width, 10.0, 80.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let bg = solid(self.bg_color, false);
        let (bf, nwf, nhf) = (border as f32, nw as f32, nh as f32);
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, nwf, bf, &bg, None);
        sk::draw_rect_xywh(&mut canvas, 0.0, (nh - border) as f32, nwf, bf, &bg, None);
        sk::draw_rect_xywh(&mut canvas, 0.0, bf, bf, ih as f32, &bg, None);
        sk::draw_rect_xywh(&mut canvas, (nw - border) as f32, bf, bf, ih as f32, &bg, None);
        let strand_w = bf * frac;
        let knot = solid(self.knot_color, true);
        let knot_pen = Pen::new(strand_w).cap(Cap::Round).join(Join::Round);
        let outline = solid(darken(self.bg_color, 0.3), true);
        let outline_pen = Pen::new(strand_w + 3.0).cap(Cap::Round).join(Join::Round);
        let o = self.outline_strands.then_some((&outline, &outline_pen));
        let cell = bf * 1.2;
        band_knots(&mut canvas, &knot, &knot_pen, o, 0.0, 0.0, nwf, bf, true, cell);
        band_knots(&mut canvas, &knot, &knot_pen, o, 0.0, (nh - border) as f32, nwf, bf, true, cell);
        band_knots(&mut canvas, &knot, &knot_pen, o, 0.0, bf, bf, ih as f32, false, cell);
        band_knots(&mut canvas, &knot, &knot_pen, o, (nw - border) as f32, bf, bf, ih as f32, false, cell);
        canvas.draw_bitmap(source, bf, bf, None);
        if self.inner_line {
            let pen = Pen::new(2.5);
            sk::draw_rect_xywh(&mut canvas, bf - 1.25, bf - 1.25, iw as f32 + 2.5, ih as f32 + 2.5, &knot, Some(&pen));
        }
        sk::draw_rect_xywh(&mut canvas, 1.0, 1.0, nwf - 2.0, nhf - 2.0, &knot, Some(&Pen::new(2.0)));
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

#[allow(clippy::too_many_arguments)]
fn band_knots(
    canvas: &mut Canvas,
    knot: &Paint,
    knot_pen: &Pen,
    outline: Option<(&Paint, &Pen)>,
    rx: f32,
    ry: f32,
    rw: f32,
    rh: f32,
    horizontal: bool,
    cell: f32,
) {
    sk::with_clip_rect(canvas, RRect::ltrb(rx, ry, rx + rw, ry + rh), |canvas| {
        let primary = if horizontal { rw } else { rh };
        let mid = if horizontal { ry + rh * 0.5 } else { rx + rw * 0.5 };
        let amplitude = if horizontal { rh * 0.25 } else { rw * 0.25 };
        let cells = 2.max((primary / cell) as i32 + 1);
        for strand in 0..2 {
            let phase = strand as f32 * std::f32::consts::PI;
            let mut path = SkPath::new();
            let segments = cells * 8;
            let seg_len = primary / segments as f32;
            for s in 0..=segments {
                let along = s as f32 * seg_len;
                let angle = (along / cell) * std::f32::consts::PI * 2.0 + phase;
                let wave = angle.sin() * amplitude;
                let (px, py) = if horizontal { (rx + along, mid + wave) } else { (mid + wave, ry + along) };
                if s == 0 {
                    path.move_to(px, py);
                } else {
                    path.line_to(px, py);
                }
            }
            if let Some((p, pen)) = outline {
                sk::stroke_path(canvas, &path, pen, p);
            }
            sk::stroke_path(canvas, &path, knot_pen, knot);
        }
    });
}

fn darken(c: SkColor, f: f32) -> SkColor {
    let d = |v: u8| 0.max((v as f32 * (1.0 - f)) as i32) as u8;
    SkColor::rgba(d(c.r), d(c.g), d(c.b), c.a)
}
