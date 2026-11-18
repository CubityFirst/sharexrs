//! `ZigZagBorderImageEffect` (`Core/ImageEffects/Filters/ZigZagBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ZigZagBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct ZigZagBorderImageEffect {
    pub border_size: i32,
    pub teeth_height: f32,
    pub teeth_width: f32,
    pub frame_color: SkColor,
    pub accent_color: SkColor,
    pub accent_stripe: bool,
}

impl Default for ZigZagBorderImageEffect {
    fn default() -> Self {
        ZigZagBorderImageEffect {
            border_size: 36,
            teeth_height: 40.0,
            teeth_width: 24.0,
            frame_color: SkColor::argb(0xFF282832),
            accent_color: SkColor::argb(0xFFDCB43C),
            accent_stripe: true,
        }
    }
}

impl EffectImpl for ZigZagBorderImageEffect {
    fn info() -> Info {
        Info::new("zigzag_border", "Zigzag border", EffectCategory::Filters, "image-effect/waves")
            .desc("Adds a border with decorative jagged zigzag inner teeth that bite into the image edges.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                8,
                200,
                36,
                |e: &mut Self, v| e.border_size = v,
            ),
            float_slider::<Self>(
                "teeth_height",
                "Teeth height",
                5.0,
                80.0,
                40.0,
                |e: &mut Self, v| e.teeth_height = v,
            ),
            float_slider::<Self>(
                "teeth_width",
                "Teeth width",
                4.0,
                80.0,
                24.0,
                |e: &mut Self, v| e.teeth_width = v,
            ),
            color_param::<Self>(
                "frame_color",
                "Frame color",
                SkColor::argb(0xFF282832),
                |e: &mut Self, v| e.frame_color = v,
            ),
            color_param::<Self>(
                "accent_color",
                "Accent color",
                SkColor::argb(0xFFDCB43C),
                |e: &mut Self, v| e.accent_color = v,
            ),
            bool_param::<Self>("accent_stripe", "Accent stripe", true, |e: &mut Self, v| {
                e.accent_stripe = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(8, 200);
        let teeth_h = clampf(self.teeth_height, 5.0, 80.0);
        let teeth_w = clampf(self.teeth_width, 4.0, 80.0);
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(self.frame_color.rgba_array());
        let inner = zigzag_path(nw, nh, border, teeth_w, teeth_h);
        sk::with_clip_path_aa(&mut canvas, &inner, |canvas| {
            canvas.draw_bitmap(source, border as f32, border as f32, None);
        });
        if self.accent_stripe {
            let stripe_w = 2f32.max(border as f32 * 0.1);
            sk::stroke_path(
                &mut canvas,
                &inner,
                &Pen::new(stripe_w),
                &solid(self.accent_color, true),
            );
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn zigzag_path(cw: i32, ch: i32, border: i32, tw: f32, th: f32) -> SkPath {
    let mut path = SkPath::new();
    let (l, t) = (border as f32, border as f32);
    let (r, b) = ((cw - border) as f32, (ch - border) as f32);
    let mut x = l;
    path.move_to(l, t);
    while x < r {
        let nx = (x + tw).min(r);
        let mx = x + tw * 0.5;
        if mx < r {
            path.line_to(mx, t + th);
            path.line_to(nx, t);
        } else {
            path.line_to(nx, t);
        }
        x = nx;
    }
    let mut y = t;
    while y < b {
        let ny = (y + tw).min(b);
        let my = y + tw * 0.5;
        if my < b {
            path.line_to(r - th, my);
            path.line_to(r, ny);
        } else {
            path.line_to(r, ny);
        }
        y = ny;
    }
    x = r;
    while x > l {
        let nx = (x - tw).max(l);
        let mx = x - tw * 0.5;
        if mx > l {
            path.line_to(mx, b - th);
            path.line_to(nx, b);
        } else {
            path.line_to(nx, b);
        }
        x = nx;
    }
    y = b;
    while y > t {
        let ny = (y - tw).max(t);
        let my = y - tw * 0.5;
        if my > t {
            path.line_to(l + th, my);
            path.line_to(l, ny);
        } else {
            path.line_to(l, ny);
        }
        y = ny;
    }
    path.close();
    path
}
