//! `CandyCaneBorderImageEffect` (`Core/ImageEffects/Filters/CandyCaneBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `CandyCaneBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct CandyCaneBorderImageEffect {
    pub border_width: i32,
    pub stripe_width: f32,
    pub color_a: SkColor,
    pub color_b: SkColor,
    pub gloss: f32,
    pub rounded_corners: bool,
}

impl Default for CandyCaneBorderImageEffect {
    fn default() -> Self {
        CandyCaneBorderImageEffect {
            border_width: 32,
            stripe_width: 12.0,
            color_a: SkColor::argb(0xFFDC1414),
            color_b: SkColor::argb(0xFFFFFFFF),
            gloss: 50.0,
            rounded_corners: true,
        }
    }
}

impl EffectImpl for CandyCaneBorderImageEffect {
    fn info() -> Info {
        Info::new("candy_cane_border", "Candy Cane Border", EffectCategory::Drawings, "image-effect/candy-cane")
            .desc("Adds a candy cane striped border around the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("border_width", "Border Width", 1, 200, 10, |e: &mut Self, v| e.border_width = v),
            float_slider::<Self>("stripe_width", "Stripe Width", 2.0, 60.0, 12.0, |e: &mut Self, v| e.stripe_width = v),
            color_param::<Self>("color_a", "Color A", SkColor::argb(0xFFDC1414), |e: &mut Self, v| e.color_a = v),
            color_param::<Self>("color_b", "Color B", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.color_b = v),
            float_slider::<Self>("gloss", "Gloss", 0.0, 100.0, 50.0, |e: &mut Self, v| e.gloss = v),
            bool_param::<Self>("rounded_corners", "Rounded Corners", true, |e: &mut Self, v| e.rounded_corners = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let bw = self.border_width.clamp(1, 500);
        let sw = clampf(self.stripe_width, 1.0, 200.0);
        let gloss = clampf(self.gloss, 0.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + bw * 2, ih + bw * 2);
        let (ca, cb) = (self.color_a, self.color_b);
        let (ar, ag, ab) = (ca.r as f32 / 255.0, ca.g as f32 / 255.0, ca.b as f32 / 255.0);
        let (br, bg, bb) = (cb.r as f32 / 255.0, cb.g as f32 / 255.0, cb.b as f32 / 255.0);
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        let corner = if self.rounded_corners { bw as f32 * 0.4 } else { 0.0 };
        let bwf = bw as f32;
        let (inner_l, inner_t) = (bwf, bwf);
        let inner_r = (bw + iw - 1) as f32;
        let inner_b = (bw + ih - 1) as f32;
        let (img_w, img_h) = (iw as f32, ih as f32);
        for y in 0..nh {
            let in_top = y < bw;
            let in_bottom = y >= bw + ih;
            for x in 0..nw {
                let in_left = x < bw;
                let in_right = x >= bw + iw;
                if !in_top && !in_bottom && !in_left && !in_right {
                    continue;
                }
                let (xf, yf) = (x as f32, y as f32);
                let cx = clampf(xf, inner_l, inner_r);
                let cy = clampf(yf, inner_t, inner_b);
                let dist = ((xf - cx) * (xf - cx) + (yf - cy) * (yf - cy)).sqrt();
                let cross = clampf(dist / bwf, 0.0, 1.0);
                let along = if cy == inner_t && cx > inner_l {
                    cx - inner_l
                } else if cx == inner_r && cy > inner_t {
                    img_w + (cy - inner_t)
                } else if cy == inner_b && cx < inner_r {
                    img_w + img_h + (inner_r - cx)
                } else if cx == inner_l && cy < inner_b {
                    img_w * 2.0 + img_h + (inner_b - cy)
                } else if cx == inner_r && cy == inner_t {
                    img_w
                } else if cx == inner_r && cy == inner_b {
                    img_w + img_h
                } else if cx == inner_l && cy == inner_b {
                    img_w * 2.0 + img_h
                } else {
                    0.0
                };
                let i = (y * nw + x) as usize;
                if self.rounded_corners && corner > 0.0 {
                    let in_cx = xf < corner || xf > (nw - 1) as f32 - corner;
                    let in_cy = yf < corner || yf > (nh - 1) as f32 - corner;
                    if in_cx && in_cy {
                        let ocx = if xf < corner { corner } else { (nw - 1) as f32 - corner };
                        let ocy = if yf < corner { corner } else { (nh - 1) as f32 - corner };
                        if ((xf - ocx) * (xf - ocx) + (yf - ocy) * (yf - ocy)).sqrt() > corner {
                            dst[i] = color::TRANSPARENT;
                            continue;
                        }
                    }
                }
                let phase = (along + cross * bwf) / sw;
                let mut t = ((phase * std::f32::consts::PI * 2.0).sin() + 1.0) * 0.5;
                t = if t < 0.5 { smooth_step(0.15, 0.45, t) } else { 1.0 - smooth_step(0.55, 0.85, 1.0 - t) };
                let mut r = lerp(ar, br, t);
                let mut g = lerp(ag, bg, t);
                let mut b = lerp(ab, bb, t);
                if gloss > 0.001 {
                    let centered = (cross - 0.5) * 2.0;
                    let hl = (-3.0 * centered * centered).exp();
                    let dark = 1.0 - 0.15 * (1.0 - hl);
                    let comb = dark + hl * 0.25 * gloss;
                    r = clampf(r * comb, 0.0, 1.0);
                    g = clampf(g * comb, 0.0, 1.0);
                    b = clampf(b * comb, 0.0, 1.0);
                }
                let to = |v: f32| (v * 255.0).round_ties_even() as u8;
                dst[i] = SkColor::rgba(to(r), to(g), to(b), 255);
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        draw_bitmap(&mut out, source, bwf, bwf);
        Ok(out)
    }
}
