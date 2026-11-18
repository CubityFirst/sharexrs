//! `BevelImageEffect` (`Core/ImageEffects/Filters/BevelImageEffect.cs`).

use crate::effects::prelude::*;

/// `BevelImageEffect`.
#[derive(Clone, Debug)]
pub struct BevelImageEffect {
    pub size: i32,
    pub strength: f32,
    pub light_angle: f32,
    pub highlight_color: SkColor,
    pub shadow_color: SkColor,
}

impl Default for BevelImageEffect {
    fn default() -> Self {
        BevelImageEffect {
            size: 10,
            strength: 70.0,
            light_angle: 225.0,
            highlight_color: SkColor::argb(0xD9FFFFFF),
            shadow_color: SkColor::argb(0xB0000000),
        }
    }
}

impl EffectImpl for BevelImageEffect {
    fn info() -> Info {
        Info::new("bevel", "Bevel", EffectCategory::Filters, "app/layers")
            .desc("Adds a beveled edge effect with highlights and shadows.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 1, 40, 10, |e: &mut Self, v| e.size = v),
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                70.0,
                |e: &mut Self, v| e.strength = v,
            ),
            float_slider::<Self>(
                "light_angle",
                "Light angle",
                0.0,
                360.0,
                225.0,
                |e: &mut Self, v| e.light_angle = v,
            ),
            color_param::<Self>(
                "highlight_color",
                "Highlight color",
                SkColor::argb(0xD9FFFFFF),
                |e: &mut Self, v| e.highlight_color = v,
            ),
            color_param::<Self>(
                "shadow_color",
                "Shadow color",
                SkColor::argb(0xB0000000),
                |e: &mut Self, v| e.shadow_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let size = self.size.clamp(1, 40);
        let strength = clampf(self.strength, 0.0, 100.0) / 100.0;
        if strength <= 0.0 || (self.highlight_color.a == 0 && self.shadow_color.a == 0) {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let transparent = src.iter().any(|c| c.a != 255);
        let angle = self.light_angle * std::f32::consts::PI / 180.0;
        let (dir_x, dir_y) = (angle.cos(), angle.sin());
        let sample_count = size.clamp(1, 12) as usize;
        let mut weight_sum = 0f32;
        let mut offsets = vec![0f32; sample_count];
        let mut weights = vec![0f32; sample_count];
        for i in 0..sample_count {
            let t = (i as f32 + 1.0) / sample_count as f32;
            offsets[i] = t * size as f32;
            weights[i] = 1.0 - (t * 0.72);
            weight_sum += weights[i];
        }
        let ha = self.highlight_color.a as f32 / 255.0;
        let sa = self.shadow_color.a as f32 / 255.0;
        let lw = edge_weights(width, size, true);
        let rw = edge_weights(width, size, false);
        let tw = edge_weights(height, size, true);
        let bw = edge_weights(height, size, false);
        let mut ox = vec![0i32; sample_count];
        let mut oy = vec![0i32; sample_count];
        if transparent {
            for i in 0..sample_count {
                ox[i] = (dir_x * offsets[i]).round_ties_even() as i32;
                oy[i] = (dir_y * offsets[i]).round_ties_even() as i32;
            }
        }
        let (hc, sc) = (self.highlight_color, self.shadow_color);
        for y in 0..height {
            let row = y * width;
            let (top, bottom) = (tw[y as usize], bw[y as usize]);
            for x in 0..width {
                let original = src[(row + x) as usize];
                let oa = original.a as f32 / 255.0;
                if original.a == 0 {
                    dst[(row + x) as usize] = original;
                    continue;
                }
                let (left, right) = (lw[x as usize], rw[x as usize]);
                let bound = (left * -dir_x) + (right * dir_x) + (top * -dir_y) + (bottom * dir_y);
                let presence = clamp01(
                    left.max(right).max(top.max(bottom))
                        + ((left + right + top + bottom).min(1.0) * 0.35),
                );
                let mut response = bound * 0.95;
                let mut edge = presence;
                if transparent {
                    let mut ar = 0f32;
                    let mut ae = 0f32;
                    for i in 0..sample_count {
                        let toward = sample_alpha(
                            &src,
                            width,
                            height,
                            (x + ox[i]) as f32,
                            (y + oy[i]) as f32,
                        );
                        let away = sample_alpha(
                            &src,
                            width,
                            height,
                            (x - ox[i]) as f32,
                            (y - oy[i]) as f32,
                        );
                        let delta = away - toward;
                        ar += delta * weights[i];
                        ae += delta.abs() * weights[i];
                    }
                    ar /= weight_sum;
                    ae = clamp01((ae / weight_sum) * 1.45);
                    response += ar * 1.25;
                    edge = clamp01(edge.max(ae));
                }
                let hm = clamp01(0f32.max(response) * 1.55 * strength) * edge * ha * oa;
                let sm = clamp01(0f32.max(-response) * 1.55 * strength) * edge * sa * oa;
                let (mut r, mut g, mut b) =
                    (original.r as f32, original.g as f32, original.b as f32);
                if sm > 0.0 {
                    r = lerp(r, sc.r as f32, sm);
                    g = lerp(g, sc.g as f32, sm);
                    b = lerp(b, sc.b as f32, sm);
                }
                if hm > 0.0 {
                    r = lerp(r, hc.r as f32, hm);
                    g = lerp(g, hc.g as f32, hm);
                    b = lerp(b, hc.b as f32, hm);
                }
                let lift = (hm - sm) * 26.0 * strength;
                dst[(row + x) as usize] = SkColor::rgba(
                    clamp_to_byte(r + lift),
                    clamp_to_byte(g + lift),
                    clamp_to_byte(b + lift),
                    original.a,
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn sample_alpha(px: &[SkColor], width: i32, height: i32, x: f32, y: f32) -> f32 {
    let (right, bottom) = (width - 1, height - 1);
    let x = clampf(x, 0.0, right as f32);
    let y = clampf(y, 0.0, bottom as f32);
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = if x0 < right { x0 + 1 } else { right };
    let y1 = if y0 < bottom { y0 + 1 } else { bottom };
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let a = |xx: i32, yy: i32| px[(yy * width + xx) as usize].a as f32;
    let a0 = lerp(a(x0, y0), a(x1, y0), tx);
    let a1 = lerp(a(x0, y1), a(x1, y1), tx);
    lerp(a0, a1, ty) / 255.0
}

fn edge_weights(length: i32, size: i32, from_start: bool) -> Vec<f32> {
    (0..length)
        .map(|i| {
            let d = if from_start { i } else { (length - 1) - i };
            if d < 0 || d >= size {
                0.0
            } else {
                1.0 - smooth_step(0.0, size as f32, d as f32)
            }
        })
        .collect()
}
