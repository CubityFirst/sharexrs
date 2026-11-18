//! `EtchedGlassImageEffect` (`Core/ImageEffects/Filters/EtchedGlassImageEffect.cs`).

use crate::effects::prelude::*;

/// `EtchedGlassImageEffect`.
#[derive(Clone, Debug)]
pub struct EtchedGlassImageEffect {
    pub frost: f32,
    pub engrave: f32,
    pub refraction: f32,
    pub highlight: f32,
    pub background_fade: f32,
}

impl Default for EtchedGlassImageEffect {
    fn default() -> Self {
        EtchedGlassImageEffect {
            frost: 48.0,
            engrave: 68.0,
            refraction: 18.0,
            highlight: 42.0,
            background_fade: 38.0,
        }
    }
}

impl EffectImpl for EtchedGlassImageEffect {
    fn info() -> Info {
        Info::new("etched_glass", "Etched glass", EffectCategory::Filters, "image-effect/glass")
            .desc("Simulates an etched glass surface with frost, engraving, and refraction effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("frost", "Frost", 0.0, 100.0, 48.0, |e: &mut Self, v| e.frost = v),
            float_slider::<Self>("engrave", "Engrave", 0.0, 100.0, 68.0, |e: &mut Self, v| e.engrave = v),
            float_slider::<Self>("refraction", "Refraction", 0.0, 100.0, 18.0, |e: &mut Self, v| e.refraction = v),
            float_slider::<Self>("highlight", "Highlight", 0.0, 100.0, 42.0, |e: &mut Self, v| e.highlight = v),
            float_slider::<Self>("background_fade", "Background fade", 0.0, 100.0, 38.0, |e: &mut Self, v| e.background_fade = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let frost = clampf(self.frost, 0.0, 100.0) / 100.0;
        let engrave = clampf(self.engrave, 0.0, 100.0) / 100.0;
        let refraction = clampf(self.refraction, 0.0, 100.0) / 100.0;
        let highlight = clampf(self.highlight, 0.0, 100.0) / 100.0;
        let fade = clampf(self.background_fade, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let rpx = 0.6 + (refraction * 5.4);
        let lum = |c: SkColor| (0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32);
        let p = |x: i32, y: i32| src[(y * width + x) as usize];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let nx = ((hash01(x / 3, y / 3, 2011) * 2.0) - 1.0) * rpx;
                let ny = ((hash01(x / 3, y / 3, 3001) * 2.0) - 1.0) * rpx;
                let refracted = bilinear_sample(&src, width, height, x as f32 + nx, y as f32 + ny);
                // SampleBlurred: the pixel and its neighbours two away.
                let (l, r_, t, b_) = ((x - 2).max(0), (x + 2).min(width - 1), (y - 2).max(0), (y + 2).min(height - 1));
                let cs = [p(x, y), p(l, y), p(r_, y), p(x, t), p(x, b_)];
                let avg = |f: fn(&SkColor) -> u8| clamp_to_byte(cs.iter().map(|c| f(c) as i32).sum::<i32>() as f32 / 5.0);
                let blurred = SkColor::rgba(avg(|c| c.r), avg(|c| c.g), avg(|c| c.b), avg(|c| c.a));
                let lm = lum(refracted) / 255.0;
                // SampleEdge
                let (l1, r1, t1, b1) = ((x - 1).max(0), (x + 1).min(width - 1), (y - 1).max(0), (y + 1).min(height - 1));
                let edge = clamp01(
                    ((lum(p(r1, y)) / 255.0 - lum(p(l1, y)) / 255.0).abs() + (lum(p(x, b1)) / 255.0 - lum(p(x, t1)) / 255.0).abs())
                        * 1.25,
                );
                let br = lerp(s.r as f32 / 255.0, blurred.r as f32 / 255.0, fade * 0.72);
                let bg = lerp(s.g as f32 / 255.0, blurred.g as f32 / 255.0, fade * 0.72);
                let bb = lerp(s.b as f32 / 255.0, blurred.b as f32 / 255.0, fade * 0.72);
                let gray = lm * 0.88 + 0.07;
                let mut gr = lerp(br, gray + 0.07, frost * 0.58);
                let mut gg = lerp(bg, gray + 0.08, frost * 0.60);
                let mut gb = lerp(bb, gray + 0.10, frost * 0.64);
                let en = edge * engrave;
                gr *= 1.0 - (en * 0.36);
                gg *= 1.0 - (en * 0.38);
                gb *= 1.0 - (en * 0.40);
                let spec = edge * highlight;
                gr = lerp(gr, 1.0, spec * 0.16);
                gg = lerp(gg, 1.0, spec * 0.18);
                gb = lerp(gb, 1.0, spec * 0.22);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(gr * 255.0), clamp_to_byte(gg * 255.0), clamp_to_byte(gb * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
