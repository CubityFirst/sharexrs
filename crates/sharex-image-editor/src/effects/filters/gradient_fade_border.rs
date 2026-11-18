//! `GradientFadeBorderImageEffect` (`Core/ImageEffects/Filters/GradientFadeBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `GradientFadeBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct GradientFadeBorderImageEffect {
    pub border_size: i32,
    pub outer_color: SkColor,
    pub mid_color: SkColor,
    pub inner_color: SkColor,
    pub mid_position: f32,
    pub glow_edge: bool,
}

impl Default for GradientFadeBorderImageEffect {
    fn default() -> Self {
        GradientFadeBorderImageEffect {
            border_size: 50,
            outer_color: SkColor::argb(0xFF0A0A23),
            mid_color: SkColor::argb(0xFF3C1478),
            inner_color: SkColor::argb(0xFFB450C8),
            mid_position: 50.0,
            glow_edge: true,
        }
    }
}

impl EffectImpl for GradientFadeBorderImageEffect {
    fn info() -> Info {
        Info::new(
            "gradient_fade_border",
            "Gradient fade border",
            EffectCategory::Filters,
            "app/blend",
        )
        .desc("Adds a multi-stop gradient border that smoothly fades between configurable colors.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                10,
                300,
                50,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "outer_color",
                "Outer color",
                SkColor::argb(0xFF0A0A23),
                |e: &mut Self, v| e.outer_color = v,
            ),
            color_param::<Self>(
                "mid_color",
                "Mid color",
                SkColor::argb(0xFF3C1478),
                |e: &mut Self, v| e.mid_color = v,
            ),
            color_param::<Self>(
                "inner_color",
                "Inner color",
                SkColor::argb(0xFFB450C8),
                |e: &mut Self, v| e.inner_color = v,
            ),
            float_slider::<Self>(
                "mid_position",
                "Mid-color position %",
                10.0,
                90.0,
                50.0,
                |e: &mut Self, v| e.mid_position = v,
            ),
            bool_param::<Self>("glow_edge", "Inner glow edge", true, |e: &mut Self, v| {
                e.glow_edge = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(10, 300);
        let mid_t = clampf(self.mid_position, 10.0, 90.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        let lerp_color = |a: SkColor, b: SkColor, t: f32| {
            let l = |x: u8, y: u8| clamp_to_byte(x as f32 + (y as f32 - x as f32) * t);
            SkColor::rgba(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a))
        };
        for y in 0..nh {
            let top = y < border;
            let bottom = y >= border + ih;
            for x in 0..nw {
                let left = x < border;
                let right = x >= border + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let dl = if left { border - x } else { 0 };
                let dr = if right { x - border - iw + 1 } else { 0 };
                let dt = if top { border - y } else { 0 };
                let db = if bottom { y - border - ih + 1 } else { 0 };
                let dist = dl.max(dr).max(dt.max(db));
                let t = clampf(dist as f32 / border as f32, 0.0, 1.0);
                let c = if t < mid_t {
                    let seg = smooth_step(0.0, 1.0, t / mid_t);
                    lerp_color(self.inner_color, self.mid_color, seg)
                } else {
                    let seg = smooth_step(0.0, 1.0, (t - mid_t) / (1.0 - mid_t));
                    lerp_color(self.mid_color, self.outer_color, seg)
                };
                dst[(y * nw + x) as usize] = c;
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        canvas.draw_bitmap(source, border as f32, border as f32, None);
        if self.glow_edge {
            let ic = self.inner_color;
            let paint = solid(SkColor::rgba(ic.r, ic.g, ic.b, 120), true);
            let bf = border as f32;
            sk::draw_rect_blur(
                &mut canvas,
                RRect::xywh(bf, bf, iw as f32, ih as f32),
                &paint,
                Some(&Pen::new(3.0)),
                4.0,
            );
        }
        drop(canvas);
        Ok(out)
    }
}
