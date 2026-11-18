//! `MarbleBorderImageEffect` (`Core/ImageEffects/Filters/MarbleBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `MarbleBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct MarbleBorderImageEffect {
    pub border_size: i32,
    pub base_color: SkColor,
    pub vein_color: SkColor,
    pub vein_strength: f32,
    pub vein_scale: f32,
    pub inner_bevel: bool,
}

impl Default for MarbleBorderImageEffect {
    fn default() -> Self {
        MarbleBorderImageEffect {
            border_size: 50,
            base_color: SkColor::argb(0xFFEBE6DC),
            vein_color: SkColor::argb(0xFF5A5046),
            vein_strength: 60.0,
            vein_scale: 30.0,
            inner_bevel: true,
        }
    }
}

impl EffectImpl for MarbleBorderImageEffect {
    fn info() -> Info {
        Info::new(
            "marble_border",
            "Marble border",
            EffectCategory::Filters,
            "image-effect/gem",
        )
        .desc("Adds a procedural marble-veined texture border around the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                2,
                300,
                50,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "base_color",
                "Base color",
                SkColor::argb(0xFFEBE6DC),
                |e: &mut Self, v| e.base_color = v,
            ),
            color_param::<Self>(
                "vein_color",
                "Vein color",
                SkColor::argb(0xFF5A5046),
                |e: &mut Self, v| e.vein_color = v,
            ),
            float_slider::<Self>(
                "vein_strength",
                "Vein strength",
                0.0,
                100.0,
                60.0,
                |e: &mut Self, v| e.vein_strength = v,
            ),
            float_slider::<Self>(
                "vein_scale",
                "Vein scale",
                5.0,
                100.0,
                30.0,
                |e: &mut Self, v| e.vein_scale = v,
            ),
            bool_param::<Self>("inner_bevel", "Inner bevel", true, |e: &mut Self, v| {
                e.inner_bevel = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(2, 300);
        let vein = clampf(self.vein_strength, 0.0, 100.0) / 100.0;
        let scale = clampf(self.vein_scale, 5.0, 100.0) / 1000.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
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
                let mut c = marble_color(x, y, scale, vein, self.base_color, self.vein_color);
                if self.inner_bevel {
                    let d = inner_bevel(x, y, border, iw, ih);
                    if d > 0.0 {
                        c = shade(c, -d * 0.25);
                    }
                }
                dst[(y * nw + x) as usize] = c;
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        canvas.draw_bitmap(source, border as f32, border as f32, None);
        let p = Paint::fill(shade(self.vein_color, -0.3).rgba_array());
        let b = border as f32 - 0.75;
        canvas.stroke_rect(RRect::xywh(b, b, iw as f32 + 1.5, ih as f32 + 1.5), 1.5, &p);
        Ok(out)
    }
}

// ---- port ----

fn marble_color(x: i32, y: i32, scale: f32, vein: f32, base: SkColor, vc: SkColor) -> SkColor {
    let turb = fractal_noise(x as f32 * scale, y as f32 * scale, 6, 2.0, 0.5, 42);
    let diag = (x + y) as f32 * scale * 2.0;
    let mut m = (diag + vein * turb * 20.0).sin();
    m = (m + 1.0) * 0.5;
    m = m.powf(1.5);
    let l = |a: u8, b: u8| a as f32 + (b as f32 - a as f32) * m;
    SkColor::rgba(
        clamp_to_byte(l(base.r, vc.r)),
        clamp_to_byte(l(base.g, vc.g)),
        clamp_to_byte(l(base.b, vc.b)),
        255,
    )
}

fn inner_bevel(x: i32, y: i32, border: i32, sw: i32, sh: i32) -> f32 {
    let zone = border as f32 * 0.4;
    let (il, it) = (border as f32, border as f32);
    let (ir, ib) = ((border + sw) as f32, (border + sh) as f32);
    let (xf, yf) = (x as f32, y as f32);
    let mut d = f32::MAX;
    if xf < il {
        d = d.min(il - xf);
    }
    if xf >= ir {
        d = d.min(xf - (ir - 1.0));
    }
    if yf < it {
        d = d.min(it - yf);
    }
    if yf >= ib {
        d = d.min(yf - (ib - 1.0));
    }
    let t = 1.0 - clampf(d / zone, 0.0, 1.0);
    smooth_step(0.0, 1.0, t)
}

fn shade(c: SkColor, amount: f32) -> SkColor {
    let f = |v: u8| clamp_to_byte((v as f32 / 255.0 + amount) * 255.0);
    SkColor::rgba(f(c.r), f(c.g), f(c.b), c.a)
}
