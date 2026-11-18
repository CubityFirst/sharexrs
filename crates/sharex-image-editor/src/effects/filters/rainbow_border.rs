//! `RainbowBorderImageEffect` (`Core/ImageEffects/Filters/RainbowBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `RainbowBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct RainbowBorderImageEffect {
    pub border_size: i32,
    pub saturation: f32,
    pub brightness: f32,
    pub hue_offset: f32,
    pub glow_strength: f32,
    pub inner_line: bool,
}

impl Default for RainbowBorderImageEffect {
    fn default() -> Self {
        RainbowBorderImageEffect {
            border_size: 24,
            saturation: 100.0,
            brightness: 95.0,
            hue_offset: 0.0,
            glow_strength: 40.0,
            inner_line: true,
        }
    }
}

impl EffectImpl for RainbowBorderImageEffect {
    fn info() -> Info {
        Info::new(
            "rainbow_border",
            "Rainbow border",
            EffectCategory::Filters,
            "image-effect/rainbow",
        )
        .desc("Adds a vivid rainbow gradient border that cycles through the full color spectrum.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                2,
                300,
                24,
                |e: &mut Self, v| e.border_size = v,
            ),
            float_slider::<Self>(
                "saturation",
                "Saturation",
                0.0,
                100.0,
                100.0,
                |e: &mut Self, v| e.saturation = v,
            ),
            float_slider::<Self>(
                "brightness",
                "Brightness",
                0.0,
                100.0,
                95.0,
                |e: &mut Self, v| e.brightness = v,
            ),
            float_slider::<Self>(
                "hue_offset",
                "Hue offset",
                0.0,
                360.0,
                0.0,
                |e: &mut Self, v| e.hue_offset = v,
            ),
            float_slider::<Self>(
                "glow_strength",
                "Glow",
                0.0,
                100.0,
                40.0,
                |e: &mut Self, v| e.glow_strength = v,
            ),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| {
                e.inner_line = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(2, 300);
        let sat = clampf(self.saturation, 0.0, 100.0);
        let brightness = clampf(self.brightness, 0.0, 100.0);
        let hue_off = self.hue_offset % 360.0;
        let glow = clampf(self.glow_strength, 0.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let perim_w = iw as f32 + border as f32 * 2.0;
        let perim_h = ih as f32 + border as f32 * 2.0;
        let total = 2.0 * (perim_w + perim_h - 4.0);
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
                let hue = perimeter_hue(x, y, nw, nh, total, perim_w, perim_h, hue_off);
                let cross = cross_ratio(x, y, border, iw, ih);
                let scale = if glow > 0.0 {
                    1.0 - smooth_step(0.6, 1.0, cross) * glow * 0.5
                } else {
                    1.0
                };
                dst[(y * nw + x) as usize] = SkColor::from_hsv(hue, sat, brightness * scale, 255);
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        canvas.draw_bitmap(source, border as f32, border as f32, None);
        if self.inner_line {
            let p = Paint::fill([255, 255, 255, 120]);
            let b = border as f32 - 0.75;
            canvas.stroke_rect(RRect::xywh(b, b, iw as f32 + 1.5, ih as f32 + 1.5), 1.5, &p);
        }
        Ok(out)
    }
}

// ---- port ----

#[allow(clippy::too_many_arguments)]
fn perimeter_hue(x: i32, y: i32, nw: i32, nh: i32, total: f32, pw: f32, ph: f32, off: f32) -> f32 {
    let cx = clampf(x as f32, 0.0, (nw - 1) as f32);
    let cy = clampf(y as f32, 0.0, (nh - 1) as f32);
    let dt = cy;
    let db = (nh - 1) as f32 - cy;
    let dl = cx;
    let dr = (nw - 1) as f32 - cx;
    let m = dt.min(db).min(dl.min(dr));
    let along = if m == dt {
        cx
    } else if m == dr {
        pw + cy
    } else if m == db {
        pw + ph + ((nw - 1) as f32 - cx)
    } else {
        pw + ph + (nw - 1) as f32 + ((nh - 1) as f32 - cy)
    };
    (along / total * 360.0 + off) % 360.0
}

fn cross_ratio(x: i32, y: i32, border: i32, sw: i32, sh: i32) -> f32 {
    let (l, t) = (border as f32, border as f32);
    let (r, b) = ((border + sw) as f32, (border + sh) as f32);
    let cx = clampf(x as f32, l, r - 1.0);
    let cy = clampf(y as f32, t, b - 1.0);
    let (dx, dy) = (x as f32 - cx, y as f32 - cy);
    clampf((dx * dx + dy * dy).sqrt() / border as f32, 0.0, 1.0)
}
