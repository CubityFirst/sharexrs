//! `HalftoneImageEffect` (`Core/ImageEffects/Filters/HalftoneImageEffect.cs`).

use crate::effects::prelude::*;

/// `HalftoneImageEffect`.
#[derive(Clone, Debug)]
pub struct HalftoneImageEffect {
    pub cell_size: i32,
    pub dot_softness: f32,
    pub ink_strength: f32,
    pub source_blend: f32,
    pub angle_offset: f32,
}

impl Default for HalftoneImageEffect {
    fn default() -> Self {
        HalftoneImageEffect {
            cell_size: 8,
            dot_softness: 18.0,
            ink_strength: 90.0,
            source_blend: 20.0,
            angle_offset: 0.0,
        }
    }
}

impl EffectImpl for HalftoneImageEffect {
    fn info() -> Info {
        Info::new("halftone", "Halftone", EffectCategory::Filters, "image-effect/circle-dot")
            .desc("Converts the image into a CMYK halftone dot pattern.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("cell_size", "Cell size", 3, 24, 8, |e: &mut Self, v| e.cell_size = v),
            float_slider::<Self>("dot_softness", "Dot softness", 0.0, 100.0, 18.0, |e: &mut Self, v| e.dot_softness = v),
            float_slider::<Self>("ink_strength", "Ink strength", 0.0, 100.0, 90.0, |e: &mut Self, v| e.ink_strength = v),
            float_slider::<Self>("source_blend", "Source blend", 0.0, 100.0, 20.0, |e: &mut Self, v| e.source_blend = v),
            float_slider::<Self>("angle_offset", "Angle offset", -45.0, 45.0, 0.0, |e: &mut Self, v| e.angle_offset = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let cell = self.cell_size.clamp(3, 24);
        let softness = clampf(self.dot_softness, 0.0, 100.0) / 100.0;
        let ink = clampf(self.ink_strength, 0.0, 100.0) / 100.0;
        let source_blend = clampf(self.source_blend, 0.0, 100.0) / 100.0;
        let deg = std::f32::consts::PI / 180.0;
        let off = clampf(self.angle_offset, -45.0, 45.0) * deg;
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        let rot = |a: f32| (a.cos(), a.sin());
        let (c_cos, c_sin) = rot(15.0 * deg + off);
        let (m_cos, m_sin) = rot(75.0 * deg + off);
        let (y_cos, y_sin) = rot(0.0 + off);
        let (k_cos, k_sin) = rot(45.0 * deg + off);
        let dot = |x: i32, y: i32, cos: f32, sin: f32, intensity: f32| -> f32 {
            if intensity <= 0.0001 {
                return 0.0;
            }
            let u = ((x as f32 * cos) + (y as f32 * sin)) / cell as f32;
            let v = ((-x as f32 * sin) + (y as f32 * cos)) / cell as f32;
            let fu = (u - u.floor()) - 0.5;
            let fv = (v - v.floor()) - 0.5;
            let distance = ((fu * fu) + (fv * fv)).sqrt();
            let radius = 0.5 * clampf(intensity, 0.0, 1.0).sqrt();
            let s = 0.01 + (softness * 0.14);
            let inner = 0f32.max(radius - s);
            let outer = radius + s;
            1.0 - smooth_step(inner, outer, distance)
        };
        let blend_ink = |o: &mut (f32, f32, f32), ink_c: (f32, f32, f32), alpha: f32| {
            let t = clampf(alpha, 0.0, 1.0);
            o.0 = lerp(o.0, ink_c.0, t);
            o.1 = lerp(o.1, ink_c.1, t);
            o.2 = lerp(o.2, ink_c.2, t);
        };
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (r, g, b) = (s.r as f32 / 255.0, s.g as f32 / 255.0, s.b as f32 / 255.0);
                let mut k = 1.0 - r.max(g.max(b));
                let denom = 1.0 - k;
                let (c, m, yv) = if denom <= 0.0001 {
                    (0.0, 0.0, 0.0)
                } else {
                    let c = clampf((1.0 - r - k) / denom, 0.0, 1.0);
                    let m = clampf((1.0 - g - k) / denom, 0.0, 1.0);
                    let yv = clampf((1.0 - b - k) / denom, 0.0, 1.0);
                    k = clampf(k, 0.0, 1.0);
                    (c, m, yv)
                };
                let cd = dot(x, y, c_cos, c_sin, c);
                let md = dot(x, y, m_cos, m_sin, m);
                let yd = dot(x, y, y_cos, y_sin, yv);
                let kd = dot(x, y, k_cos, k_sin, k);
                let mut o = (1f32, 1f32, 1f32);
                blend_ink(&mut o, (0.0, 1.0, 1.0), cd * ink);
                blend_ink(&mut o, (1.0, 0.0, 1.0), md * ink);
                blend_ink(&mut o, (1.0, 1.0, 0.0), yd * ink);
                blend_ink(&mut o, (0.0, 0.0, 0.0), kd * ink * 1.1);
                let or = lerp(o.0, r, source_blend);
                let og = lerp(o.1, g, source_blend);
                let ob = lerp(o.2, b, source_blend);
                dst[(row + x) as usize] =
                    SkColor::rgba(clamp_to_byte(or * 255.0), clamp_to_byte(og * 255.0), clamp_to_byte(ob * 255.0), s.a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
