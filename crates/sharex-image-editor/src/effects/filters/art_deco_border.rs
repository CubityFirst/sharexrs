//! `ArtDecoBorderImageEffect` (`Core/ImageEffects/Filters/ArtDecoBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ArtDecoBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct ArtDecoBorderImageEffect {
    pub border_size: i32,
    pub bg_color: SkColor,
    pub accent_color: SkColor,
    pub fan_spacing: f32,
    pub fan_rays: i32,
    pub inner_lines: bool,
}

impl Default for ArtDecoBorderImageEffect {
    fn default() -> Self {
        ArtDecoBorderImageEffect {
            border_size: 64,
            bg_color: SkColor::argb(0xFF0F0F19),
            accent_color: SkColor::argb(0xFFD4AF37),
            fan_spacing: 80.0,
            fan_rays: 7,
            inner_lines: true,
        }
    }
}

impl EffectImpl for ArtDecoBorderImageEffect {
    fn info() -> Info {
        Info::new("art_deco_border", "Art deco border", EffectCategory::Filters, "app/trophy")
            .desc("Adds a bold Art Deco border with geometric fan motifs, stepped lines, and metallic gold accents.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                20,
                300,
                64,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "bg_color",
                "Background color",
                SkColor::argb(0xFF0F0F19),
                |e: &mut Self, v| e.bg_color = v,
            ),
            color_param::<Self>(
                "accent_color",
                "Accent color",
                SkColor::argb(0xFFD4AF37),
                |e: &mut Self, v| e.accent_color = v,
            ),
            float_slider::<Self>(
                "fan_spacing",
                "Fan spacing",
                30.0,
                200.0,
                80.0,
                |e: &mut Self, v| e.fan_spacing = v,
            ),
            int_slider::<Self>("fan_rays", "Fan rays", 3, 12, 7, |e: &mut Self, v| {
                e.fan_rays = v
            }),
            bool_param::<Self>(
                "inner_lines",
                "Inner stepped lines",
                true,
                |e: &mut Self, v| e.inner_lines = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(20, 300);
        let spacing = clampf(self.fan_spacing, 30.0, 200.0);
        let rays = self.fan_rays.clamp(3, 12);
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(self.bg_color.rgba_array());
        let ac = self.accent_color;
        let accent = solid(ac, true);
        let accent_pen = Pen::new(1.5);
        let accent_fill = solid(SkColor::rgba(ac.r, ac.g, ac.b, 40), true);
        let bf = border as f32;
        let fan_r = bf * 0.65;
        fan_row(
            &mut canvas,
            &accent,
            &accent_pen,
            &accent_fill,
            spacing,
            fan_r,
            rays,
            bf,
            bf * 0.5,
            (nw - border) as f32,
            false,
        );
        fan_row(
            &mut canvas,
            &accent,
            &accent_pen,
            &accent_fill,
            spacing,
            fan_r,
            rays,
            bf,
            nh as f32 - bf * 0.5,
            (nw - border) as f32,
            true,
        );
        fan_column(
            &mut canvas,
            &accent,
            &accent_pen,
            &accent_fill,
            spacing,
            fan_r,
            rays,
            bf * 0.5,
            bf,
            (nh - border) as f32,
            false,
        );
        fan_column(
            &mut canvas,
            &accent,
            &accent_pen,
            &accent_fill,
            spacing,
            fan_r,
            rays,
            nw as f32 - bf * 0.5,
            bf,
            (nh - border) as f32,
            true,
        );
        let line_pen = Pen::new(2.0);
        sk::draw_rect_xywh(
            &mut canvas,
            2.0,
            2.0,
            (nw - 4) as f32,
            (nh - 4) as f32,
            &accent,
            Some(&line_pen),
        );
        sk::draw_rect_xywh(
            &mut canvas,
            6.0,
            6.0,
            (nw - 12) as f32,
            (nh - 12) as f32,
            &accent,
            Some(&line_pen),
        );
        sk::draw_rect_xywh(
            &mut canvas,
            (border - 2) as f32,
            (border - 2) as f32,
            (iw + 4) as f32,
            (ih + 4) as f32,
            &accent,
            Some(&line_pen),
        );
        if self.inner_lines {
            let thin = solid(SkColor::rgba(ac.r, ac.g, ac.b, 100), true);
            let thin_pen = Pen::new(1.0);
            let inset = bf * 0.25;
            sk::draw_rect_xywh(
                &mut canvas,
                inset,
                inset,
                nw as f32 - inset * 2.0,
                nh as f32 - inset * 2.0,
                &thin,
                Some(&thin_pen),
            );
            let inset2 = bf * 0.75;
            sk::draw_rect_xywh(
                &mut canvas,
                inset2,
                inset2,
                nw as f32 - inset2 * 2.0,
                nh as f32 - inset2 * 2.0,
                &thin,
                Some(&thin_pen),
            );
        }
        let ds = bf * 0.3;
        for (cx, cy) in [
            (4.0, 4.0),
            ((nw - 4) as f32, 4.0),
            (4.0, (nh - 4) as f32),
            ((nw - 4) as f32, (nh - 4) as f32),
        ] {
            let mut d = SkPath::new();
            d.move_to(cx, cy - ds);
            d.line_to(cx + ds, cy);
            d.line_to(cx, cy + ds);
            d.line_to(cx - ds, cy);
            d.close();
            sk::stroke_path(&mut canvas, &d, &accent_pen, &accent);
        }
        canvas.draw_bitmap(source, bf, bf, None);
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

#[allow(clippy::too_many_arguments)]
fn fan_row(
    canvas: &mut Canvas,
    stroke: &Paint,
    pen: &Pen,
    fill: &Paint,
    spacing: f32,
    radius: f32,
    rays: i32,
    start_x: f32,
    center_y: f32,
    end_x: f32,
    flip: bool,
) {
    let pi = std::f32::consts::PI;
    let angle0 = if flip { 0.0 } else { pi };
    let mut cx = start_x;
    while cx <= end_x {
        let mut path = SkPath::new();
        for i in 0..=rays {
            let a = angle0 + pi * i as f32 / rays as f32;
            path.move_to(cx, center_y);
            path.line_to(cx + a.cos() * radius, center_y + a.sin() * radius);
        }
        let oval = sharex_skia::Rect::from_ltrb(
            cx - radius,
            center_y - radius,
            cx + radius,
            center_y + radius,
        );
        path.add_arc(oval, if flip { 0.0 } else { 180.0 }, 180.0);
        canvas.fill_sk_path(&path, fill);
        sk::stroke_path(canvas, &path, pen, stroke);
        cx += spacing;
    }
}

#[allow(clippy::too_many_arguments)]
fn fan_column(
    canvas: &mut Canvas,
    stroke: &Paint,
    pen: &Pen,
    fill: &Paint,
    spacing: f32,
    radius: f32,
    rays: i32,
    center_x: f32,
    start_y: f32,
    end_y: f32,
    flip: bool,
) {
    let pi = std::f32::consts::PI;
    let angle0 = if flip { -pi / 2.0 } else { pi / 2.0 };
    let mut cy = start_y;
    while cy <= end_y {
        let mut path = SkPath::new();
        for i in 0..=rays {
            let a = angle0 + pi * i as f32 / rays as f32;
            path.move_to(center_x, cy);
            path.line_to(center_x + a.cos() * radius, cy + a.sin() * radius);
        }
        let oval = sharex_skia::Rect::from_ltrb(
            center_x - radius,
            cy - radius,
            center_x + radius,
            cy + radius,
        );
        path.add_arc(oval, if flip { 270.0 } else { 90.0 }, 180.0);
        canvas.fill_sk_path(&path, fill);
        sk::stroke_path(canvas, &path, pen, stroke);
        cy += spacing;
    }
}
