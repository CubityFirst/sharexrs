//! `ChromeMetallicBorderImageEffect` (`Core/ImageEffects/Filters/ChromeMetallicBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ChromeMetallicBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct ChromeMetallicBorderImageEffect {
    pub border_size: i32,
    pub tint_color: SkColor,
    pub reflectivity: f32,
    pub bevel_strength: f32,
    pub inner_line: bool,
    pub outer_line: bool,
}

impl Default for ChromeMetallicBorderImageEffect {
    fn default() -> Self {
        ChromeMetallicBorderImageEffect {
            border_size: 36,
            tint_color: SkColor::argb(0xFFC8CDD2),
            reflectivity: 80.0,
            bevel_strength: 70.0,
            inner_line: true,
            outer_line: true,
        }
    }
}

impl EffectImpl for ChromeMetallicBorderImageEffect {
    fn info() -> Info {
        Info::new("chrome_metallic_border", "Chrome metallic border", EffectCategory::Filters, "app/sparkles")
            .desc("Adds a shiny chrome metallic border with realistic specular highlights and reflections.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 4, 300, 36, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("tint_color", "Tint color", SkColor::argb(0xFFC8CDD2), |e: &mut Self, v| e.tint_color = v),
            float_slider::<Self>("reflectivity", "Reflectivity", 0.0, 100.0, 80.0, |e: &mut Self, v| e.reflectivity = v),
            float_slider::<Self>("bevel_strength", "Bevel strength", 0.0, 100.0, 70.0, |e: &mut Self, v| e.bevel_strength = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
            bool_param::<Self>("outer_line", "Outer line", true, |e: &mut Self, v| e.outer_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(4, 300);
        let refl = clampf(self.reflectivity, 0.0, 100.0) / 100.0;
        let bevel = clampf(self.bevel_strength, 0.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let tc = self.tint_color;
        let (tr, tg, tb) = (tc.r as f32 / 200.0, tc.g as f32 / 200.0, tc.b as f32 / 200.0);
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        for y in 0..nh {
            let top = y < border;
            let bottom = y >= border + ih;
            for x in 0..nw {
                let left = x < border;
                let right = x >= border + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let cross = cross_ratio(x, y, border, iw, ih);
                let mut v = chrome_gradient(cross, refl);
                let side = side_variation(x, y, border, iw, ih, bevel);
                v = clampf(v + side * 0.12, 0.0, 1.0);
                let br = v * 255.0;
                dst[(y * nw + x) as usize] = SkColor::rgba(clamp_to_byte(br * tr), clamp_to_byte(br * tg), clamp_to_byte(br * tb), 255);
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        canvas.draw_bitmap(source, border as f32, border as f32, None);
        if self.inner_line {
            let p = Paint::fill([40, 40, 45, 255]);
            let b = border as f32 - 0.5;
            canvas.stroke_rect(RRect::xywh(b, b, iw as f32 + 1.0, ih as f32 + 1.0), 1.0, &p);
        }
        if self.outer_line {
            let p = Paint::fill([20, 20, 22, 255]);
            canvas.stroke_rect(RRect::xywh(0.75, 0.75, nw as f32 - 1.5, nh as f32 - 1.5), 1.5, &p);
        }
        Ok(out)
    }
}

// ---- port ----

const CHROME_STOPS: [(f32, f32); 9] = [
    (0.00, 0.15),
    (0.10, 0.80),
    (0.22, 0.48),
    (0.38, 0.12),
    (0.52, 0.72),
    (0.68, 0.90),
    (0.80, 0.55),
    (0.90, 0.22),
    (1.00, 0.35),
];

fn chrome_gradient(t: f32, refl: f32) -> f32 {
    for i in 1..CHROME_STOPS.len() {
        let (p0, v0) = CHROME_STOPS[i - 1];
        let (p1, v1) = CHROME_STOPS[i];
        if t <= p1 {
            let local = (t - p0) / (p1 - p0);
            return lerp(0.5, lerp(v0, v1, local), refl);
        }
    }
    0.5
}

fn cross_ratio(x: i32, y: i32, border: i32, sw: i32, sh: i32) -> f32 {
    let (l, t) = (border as f32, border as f32);
    let (r, b) = ((border + sw) as f32, (border + sh) as f32);
    let cx = clampf(x as f32, l, r - 1.0);
    let cy = clampf(y as f32, t, b - 1.0);
    let (dx, dy) = (x as f32 - cx, y as f32 - cy);
    clampf((dx * dx + dy * dy).sqrt() / border as f32, 0.0, 1.0)
}

fn side_variation(x: i32, y: i32, border: i32, sw: i32, sh: i32, bevel: f32) -> f32 {
    let in_x = x >= border && x < border + sw;
    let in_y = y >= border && y < border + sh;
    if y < border && in_x {
        bevel * 0.5
    } else if y >= border + sh && in_x {
        bevel * -0.4
    } else if x < border && in_y {
        bevel * 0.2
    } else if x >= border + sw && in_y {
        bevel * -0.2
    } else {
        0.0
    }
}
