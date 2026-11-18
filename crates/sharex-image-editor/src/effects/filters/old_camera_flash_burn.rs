//! `OldCameraFlashBurnImageEffect` (`Core/ImageEffects/Filters/OldCameraFlashBurnImageEffect.cs`).

use crate::effects::prelude::*;

/// `OldCameraFlashBurnImageEffect`.
#[derive(Clone, Debug)]
pub struct OldCameraFlashBurnImageEffect {
    pub flash_strength: f32,
    pub flash_radius: f32,
    pub edge_burn: f32,
    pub warmth: f32,
    pub grain: f32,
    pub center_x: f32,
    pub center_y: f32,
}

impl Default for OldCameraFlashBurnImageEffect {
    fn default() -> Self {
        OldCameraFlashBurnImageEffect {
            flash_strength: 70.0,
            flash_radius: 68.0,
            edge_burn: 45.0,
            warmth: 35.0,
            grain: 20.0,
            center_x: 50.0,
            center_y: 50.0,
        }
    }
}

impl EffectImpl for OldCameraFlashBurnImageEffect {
    fn info() -> Info {
        Info::new("old_camera_flash_burn", "Old camera flash burn", EffectCategory::Filters, "image-effect/flashlight")
            .desc("Simulates an old camera flash burn with warm glow and edge burn effects.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("flash_strength", "Flash strength", 0.0, 100.0, 70.0, |e: &mut Self, v| e.flash_strength = v),
            float_slider::<Self>("flash_radius", "Flash radius", 20.0, 100.0, 68.0, |e: &mut Self, v| e.flash_radius = v),
            float_slider::<Self>("edge_burn", "Edge burn", 0.0, 100.0, 45.0, |e: &mut Self, v| e.edge_burn = v),
            float_slider::<Self>("warmth", "Warmth", 0.0, 100.0, 35.0, |e: &mut Self, v| e.warmth = v),
            float_slider::<Self>("grain", "Grain", 0.0, 100.0, 20.0, |e: &mut Self, v| e.grain = v),
            float_slider::<Self>("center_x", "Center X", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_x = v),
            float_slider::<Self>("center_y", "Center Y", 0.0, 100.0, 50.0, |e: &mut Self, v| e.center_y = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let flash_strength = clampf(self.flash_strength, 0.0, 100.0) / 100.0;
        let flash_radius = clampf(self.flash_radius, 20.0, 100.0) / 100.0;
        let edge_burn = clampf(self.edge_burn, 0.0, 100.0) / 100.0;
        let warmth = clampf(self.warmth, 0.0, 100.0);
        let grain = clampf(self.grain, 0.0, 100.0) / 100.0;
        if flash_strength <= 0.0001 && edge_burn <= 0.0001 && grain <= 0.0001 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let right = 1.max(width - 1);
        let bottom = 1.max(height - 1);
        let cx = (clampf(self.center_x, 0.0, 100.0) / 100.0) * right as f32;
        let cy = (clampf(self.center_y, 0.0, 100.0) / 100.0) * bottom as f32;
        let inv_w = 1.0 / right as f32;
        let inv_h = 1.0 / bottom as f32;
        let sigma = 0.06 + (flash_radius * 0.38);
        let sigma_inv = 1.0 / 0.0001f32.max(2.0 * sigma * sigma);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let s = src[(row + x) as usize];
                let (mut r, mut g, mut b, a) = (s.r as f32, s.g as f32, s.b as f32, s.a as f32);
                let dx = (x as f32 - cx) * inv_w;
                let dy = (y as f32 - cy) * inv_h;
                let dist = ((dx * dx) + (dy * dy)).sqrt();
                let flash = flash_strength * (-(dist * dist) * sigma_inv).exp();
                if flash > 0.0001 {
                    let shoulder = flash.powf(0.72);
                    r = lerp(r, 255.0, shoulder);
                    g = lerp(g, 242.0 + (warmth * 0.10), shoulder * 0.94);
                    b = lerp(b, 218.0 - (warmth * 0.08), shoulder * 0.82);
                }
                if edge_burn > 0.0001 {
                    let ring = smooth_step(0.54, 1.05, dist);
                    if ring > 0.0001 {
                        let na = hash01((x as f32 * 0.12) as i32, (y as f32 * 0.12) as i32, 611);
                        let nb = hash01((x as f32 * 0.032) as i32, (y as f32 * 0.032) as i32, 1201);
                        let burn = ring * edge_burn * (0.68 + (0.32 * ((na * 0.6) + (nb * 0.4))));
                        r = lerp(r, 220.0 + (warmth * 0.22), burn * 0.44);
                        g = lerp(g, 98.0 + (warmth * 0.18), burn * 0.38);
                        b = lerp(b, 36.0 + (warmth * 0.12), burn * 0.30);
                        let dark = burn * 0.45;
                        r *= 1.0 - (dark * 0.60);
                        g *= 1.0 - (dark * 0.68);
                        b *= 1.0 - (dark * 0.78);
                    }
                }
                if grain > 0.0001 {
                    let noise = ((hash01(x, y, 1703) * 2.0) - 1.0) * grain * 18.0;
                    r += noise * 1.05;
                    g += noise;
                    b += noise * 0.9;
                }
                dst[(row + x) as usize] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), clamp_to_byte(a));
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
