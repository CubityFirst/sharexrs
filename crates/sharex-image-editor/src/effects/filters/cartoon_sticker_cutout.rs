//! `CartoonStickerCutoutImageEffect` (`Core/ImageEffects/Filters/CartoonStickerCutoutImageEffect.cs`).

use crate::effects::prelude::*;

/// `CartoonStickerCutoutImageEffect`.
#[derive(Clone, Debug)]
pub struct CartoonStickerCutoutImageEffect {
    pub color_levels: i32,
    pub edge_threshold: i32,
    pub ink_strength: f32,
    pub sticker_border: i32,
    pub border_strength: f32,
}

impl Default for CartoonStickerCutoutImageEffect {
    fn default() -> Self {
        CartoonStickerCutoutImageEffect {
            color_levels: 8,
            edge_threshold: 32,
            ink_strength: 85.0,
            sticker_border: 3,
            border_strength: 90.0,
        }
    }
}

impl EffectImpl for CartoonStickerCutoutImageEffect {
    fn info() -> Info {
        Info::new("cartoon_sticker_cutout", "Cartoon sticker cutout", EffectCategory::Filters, "image-effect/sticker")
            .desc("Applies a cartoon sticker cutout effect with quantized colors and ink edges.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("colorLevels", "Color levels", 2, 20, 8, |e: &mut Self, v| e.color_levels = v),
            int_slider::<Self>("edgeThreshold", "Edge threshold", 5, 120, 32, |e: &mut Self, v| e.edge_threshold = v),
            float_slider::<Self>("inkStrength", "Ink strength", 0.0, 100.0, 85.0, |e: &mut Self, v| e.ink_strength = v),
            int_slider::<Self>("stickerBorder", "Sticker border", 0, 10, 3, |e: &mut Self, v| e.sticker_border = v),
            float_slider::<Self>("borderStrength", "Border strength", 0.0, 100.0, 90.0, |e: &mut Self, v| e.border_strength = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let levels = self.color_levels.clamp(2, 20);
        let edge_threshold = self.edge_threshold.clamp(5, 120);
        let ink = clampf(self.ink_strength, 0.0, 100.0) / 100.0;
        let border_radius = self.sticker_border.clamp(0, 10);
        let border_strength = clampf(self.border_strength, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let src = pixels(source);
        let step = 255.0 / (levels - 1) as f32;
        let q = |v: u8| clamp_to_byte((v as f32 / step).round_ties_even() * step);
        let quantized: Vec<SkColor> = src.iter().map(|c| SkColor::rgba(q(c.r), q(c.g), q(c.b), c.a)).collect();
        let mut edge_mask = vec![0u8; src.len()];
        for y in 0..height {
            let row = y * width;
            let y_up = if y > 0 { y - 1 } else { 0 };
            let y_down = if y < bottom { y + 1 } else { bottom };
            for x in 0..width {
                let x_left = if x > 0 { x - 1 } else { 0 };
                let x_right = if x < right { x + 1 } else { right };
                let l = quantized[(row + x_left) as usize];
                let r = quantized[(row + x_right) as usize];
                let u = quantized[(y_up * width + x) as usize];
                let d = quantized[(y_down * width + x) as usize];
                let ad = |a: u8, b: u8| (a as i32 - b as i32).abs();
                let gx = ad(r.r, l.r) + ad(r.g, l.g) + ad(r.b, l.b);
                let gy = ad(d.r, u.r) + ad(d.g, u.g) + ad(d.b, u.b);
                let edge = (gx + gy) / 6;
                let n = (edge - edge_threshold) * 255 / 1.max(255 - edge_threshold);
                edge_mask[(row + x) as usize] = if n <= 0 { 0 } else { n.min(255) as u8 };
            }
        }
        let border_mask = if border_radius > 0 && border_strength > 0.001 {
            let mut m = vec![0u8; src.len()];
            let r2 = border_radius * border_radius;
            for y in 0..height {
                let row = y * width;
                for x in 0..width {
                    let mut max = 0u8;
                    'done: for ky in -border_radius..=border_radius {
                        let sy = (y + ky).clamp(0, bottom);
                        for kx in -border_radius..=border_radius {
                            if kx * kx + ky * ky > r2 {
                                continue;
                            }
                            let sx = (x + kx).clamp(0, right);
                            let v = edge_mask[(sy * width + sx) as usize];
                            if v > max {
                                max = v;
                                if max == 255 {
                                    break 'done;
                                }
                            }
                        }
                    }
                    m[(row + x) as usize] = max;
                }
            }
            Some(m)
        } else {
            None
        };
        let dst: Vec<SkColor> = (0..src.len())
            .map(|i| {
                let base = quantized[i];
                let (mut r, mut g, mut b) = (base.r as f32, base.g as f32, base.b as f32);
                let edge = edge_mask[i] as f32 / 255.0;
                let ink_mix = edge * ink;
                if let Some(m) = &border_mask {
                    let border = m[i] as f32 / 255.0;
                    let only = 0f32.max(border - (edge * 0.65));
                    let mix = only * border_strength;
                    if mix > 0.001 {
                        r = lerp(r, 255.0, mix);
                        g = lerp(g, 255.0, mix);
                        b = lerp(b, 255.0, mix);
                    }
                }
                if ink_mix > 0.001 {
                    r = lerp(r, 0.0, ink_mix);
                    g = lerp(g, 0.0, ink_mix);
                    b = lerp(b, 0.0, ink_mix);
                }
                SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), base.a)
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
