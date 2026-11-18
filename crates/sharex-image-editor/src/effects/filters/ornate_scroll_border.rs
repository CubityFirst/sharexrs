//! `OrnateScrollBorderImageEffect` (`Core/ImageEffects/Filters/OrnateScrollBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `OrnateScrollBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct OrnateScrollBorderImageEffect {
    pub border_size: i32,
    pub frame_color: SkColor,
    pub ornament_color: SkColor,
    pub line_thickness: f32,
    pub fill_background: bool,
    pub shadow: bool,
}

impl Default for OrnateScrollBorderImageEffect {
    fn default() -> Self {
        OrnateScrollBorderImageEffect {
            border_size: 48,
            frame_color: SkColor::argb(0xFFF5F0DC),
            ornament_color: SkColor::argb(0xFFA0823C),
            line_thickness: 2.0,
            fill_background: true,
            shadow: true,
        }
    }
}

impl EffectImpl for OrnateScrollBorderImageEffect {
    fn info() -> Info {
        Info::new("ornate_scroll_border", "Ornate scroll border", EffectCategory::Filters, "image-effect/scroll")
            .desc("Adds an elegant Victorian-style ornate border with decorative corner scrollwork and double-line detailing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 200, 48, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("frame_color", "Frame color", SkColor::argb(0xFFF5F0DC), |e: &mut Self, v| e.frame_color = v),
            color_param::<Self>("ornament_color", "Ornament color", SkColor::argb(0xFFA0823C), |e: &mut Self, v| e.ornament_color = v),
            float_slider::<Self>("line_thickness", "Line thickness", 0.5, 6.0, 2.0, |e: &mut Self, v| e.line_thickness = v),
            bool_param::<Self>("fill_background", "Fill background", true, |e: &mut Self, v| e.fill_background = v),
            bool_param::<Self>("shadow", "Shadow", true, |e: &mut Self, v| e.shadow = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(10, 200);
        let lt = clampf(self.line_thickness, 0.5, 6.0);
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let (w, h, b) = (nw as f32, nh as f32, border as f32);
        if self.shadow {
            let sp = solid(SkColor::rgba(0, 0, 0, 40), true);
            sk::draw_rect_blur(&mut canvas, RRect::xywh((border + 3) as f32, (border + 4) as f32, iw as f32, ih as f32), &sp, None, b * 0.18);
        }
        if self.fill_background {
            sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, w, h, &solid(self.frame_color, true), None);
        }
        canvas.draw_bitmap(source, b, b, None);
        let is = sk::linear_gradient((b, b), (b + b * 0.15, b + b * 0.15), &[SkColor::rgba(0, 0, 0, 45), color::TRANSPARENT], None, true);
        sk::draw_rect_xywh(&mut canvas, b, b, iw as f32, ih as f32, &is, None);
        let orn = solid(self.ornament_color, true);
        let pen = Pen::new(lt).cap(Cap::Round).join(Join::Round);
        sk::draw_rect_xywh(&mut canvas, lt / 2.0, lt / 2.0, w - lt, h - lt, &orn, Some(&pen));
        let inset = b * 0.30;
        sk::draw_rect_xywh(&mut canvas, inset, inset, w - inset * 2.0, h - inset * 2.0, &orn, Some(&pen));
        let inset2 = b * 0.70;
        sk::draw_rect_xywh(&mut canvas, inset2, inset2, w - inset2 * 2.0, h - inset2 * 2.0, &orn, Some(&pen));
        let thin = Pen::new(lt * 0.5).cap(Cap::Round).join(Join::Round);
        let mid = b * 0.50;
        sk::draw_rect_xywh(&mut canvas, mid, mid, w - mid * 2.0, h - mid * 2.0, &orn, Some(&thin));
        let orn_size = (b * 0.35).min(18.0);
        for (cx, cy) in [(inset, inset), (w - inset, inset), (inset, h - inset), (w - inset, h - inset)] {
            corner_ornament(&mut canvas, &orn, cx, cy, orn_size);
        }
        let small = orn_size * 0.65;
        for (cx, cy) in [(w * 0.5, inset), (w * 0.5, h - inset), (inset, h * 0.5), (w - inset, h * 0.5)] {
            corner_ornament(&mut canvas, &orn, cx, cy, small);
        }
        let scroll_pen = Pen::new(lt * 0.9).cap(Cap::Round).join(Join::Round);
        let ss = b * 0.40;
        let hl = lt * 0.5;
        scrollwork(&mut canvas, &orn, &scroll_pen, hl, hl, ss, 0);
        scrollwork(&mut canvas, &orn, &scroll_pen, w - hl, hl, ss, 1);
        scrollwork(&mut canvas, &orn, &scroll_pen, hl, h - hl, ss, 2);
        scrollwork(&mut canvas, &orn, &scroll_pen, w - hl, h - hl, ss, 3);
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn corner_ornament(canvas: &mut Canvas, paint: &Paint, cx: f32, cy: f32, size: f32) {
    let mut d = SkPath::new();
    d.move_to(cx, cy - size);
    d.line_to(cx + size * 0.55, cy);
    d.line_to(cx, cy + size);
    d.line_to(cx - size * 0.55, cy);
    d.close();
    canvas.fill_sk_path(&d, paint);
    sk::draw_circle(canvas, cx, cy, size * 0.18, paint, None);
}

fn scrollwork(canvas: &mut Canvas, paint: &Paint, pen: &Pen, x: f32, y: f32, size: f32, corner: i32) {
    let sx = if corner == 0 || corner == 2 { 1.0 } else { -1.0 };
    let sy = if corner == 0 || corner == 1 { 1.0 } else { -1.0 };
    let mut hs = SkPath::new();
    hs.move_to(x, y);
    hs.cubic_to(
        x + sx * size * 0.4,
        y,
        x + sx * size * 0.7,
        y + sy * size * 0.2,
        x + sx * size * 0.8,
        y + sy * size * 0.5,
    );
    hs.cubic_to(
        x + sx * size * 0.85,
        y + sy * size * 0.7,
        x + sx * size * 0.65,
        y + sy * size * 0.85,
        x + sx * size * 0.45,
        y + sy * size * 0.75,
    );
    sk::stroke_path(canvas, &hs, pen, paint);
    let mut vs = SkPath::new();
    vs.move_to(x, y);
    vs.cubic_to(
        x,
        y + sy * size * 0.4,
        x + sx * size * 0.2,
        y + sy * size * 0.7,
        x + sx * size * 0.5,
        y + sy * size * 0.8,
    );
    vs.cubic_to(
        x + sx * size * 0.7,
        y + sy * size * 0.85,
        x + sx * size * 0.85,
        y + sy * size * 0.65,
        x + sx * size * 0.75,
        y + sy * size * 0.45,
    );
    sk::stroke_path(canvas, &vs, pen, paint);
}
