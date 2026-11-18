//! `BlueprintDrawingImageEffect` (`Core/ImageEffects/Filters/BlueprintDrawingImageEffect.cs`).

use crate::effects::prelude::*;

/// `BlueprintDrawingImageEffect`.
#[derive(Clone, Debug)]
pub struct BlueprintDrawingImageEffect {
    pub line_strength: f32,
    pub detail: f32,
    pub grid_intensity: f32,
    pub texture: f32,
    pub glow: f32,
}

impl Default for BlueprintDrawingImageEffect {
    fn default() -> Self {
        BlueprintDrawingImageEffect {
            line_strength: 75.0,
            detail: 45.0,
            grid_intensity: 30.0,
            texture: 25.0,
            glow: 35.0,
        }
    }
}

impl EffectImpl for BlueprintDrawingImageEffect {
    fn info() -> Info {
        Info::new("blueprint_drawing", "Blueprint drawing", EffectCategory::Filters, "image-effect/compass")
            .desc("Transforms the image into a blueprint-style drawing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("line_strength", "Line strength", 0.0, 100.0, 75.0, |e: &mut Self, v| e.line_strength = v),
            float_slider::<Self>("detail", "Detail", 0.0, 100.0, 45.0, |e: &mut Self, v| e.detail = v),
            float_slider::<Self>("grid_intensity", "Grid intensity", 0.0, 100.0, 30.0, |e: &mut Self, v| e.grid_intensity = v),
            float_slider::<Self>("texture", "Texture", 0.0, 100.0, 25.0, |e: &mut Self, v| e.texture = v),
            float_slider::<Self>("glow", "Glow", 0.0, 100.0, 35.0, |e: &mut Self, v| e.glow = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let line_strength = clampf(self.line_strength, 0.0, 100.0) / 100.0;
        let detail = clampf(self.detail, 0.0, 100.0) / 100.0;
        let grid = clampf(self.grid_intensity, 0.0, 100.0) / 100.0;
        let texture = clampf(self.texture, 0.0, 100.0) / 100.0;
        let glow = clampf(self.glow, 0.0, 100.0) / 100.0;
        let seed = random::shared(|r| r.next_range(1, i32::MAX));
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let cx = right as f32 * 0.5;
        let cy = bottom as f32 * 0.5;
        let inv_spacing = 1.0 / (10.0 + ((1.0 - detail) * 20.0));
        let src = pixels(source);
        let lum: Vec<f32> = src
            .iter()
            .map(|c| ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0)
            .collect();
        let mut dst = vec![SkColor::default(); src.len()];
        let rmax = 1f32.max(right as f32);
        let bmax = 1f32.max(bottom as f32);
        for y in 0..height {
            let row = y * width;
            let y_up = if y > 0 { y - 1 } else { 0 };
            let y_down = if y < bottom { y + 1 } else { bottom };
            for x in 0..width {
                let x_left = if x > 0 { x - 1 } else { 0 };
                let x_right = if x < right { x + 1 } else { right };
                let index = (row + x) as usize;
                let l = lum[index];
                let gx = (lum[(row + x_right) as usize] - lum[(row + x_left) as usize]).abs();
                let gy = (lum[(y_down * width + x) as usize] - lum[(y_up * width + x) as usize]).abs();
                let edge = clampf((gx + gy) * (1.7 + (line_strength * 2.4)), 0.0, 1.0);
                let tonal = (1.0 - l).powf(0.65 + ((1.0 - detail) * 1.5)) * (0.2 + (detail * 0.35));
                let line_mask = clampf((edge * (0.72 + (line_strength * 0.75))) + tonal, 0.0, 1.0);
                let nx = x as f32 / rmax;
                let ny = y as f32 / bmax;
                let dx = (x as f32 - cx) / rmax;
                let dy = (y as f32 - cy) / bmax;
                let dist = ((dx * dx) + (dy * dy)).sqrt();
                let vignette = 1.0 - smooth_step(0.35, 0.95, dist);
                let base_shade = 0.42 + (0.38 * vignette);
                let xs = x as f32 * inv_spacing;
                let ys = y as f32 * inv_spacing;
                let grid_x = (xs - xs.round_ties_even()).abs();
                let grid_y = (ys - ys.round_ties_even()).abs();
                let mut grid_line = (1.0 - (grid_x * 18.0)).max(1.0 - (grid_y * 18.0));
                grid_line = clampf(grid_line, 0.0, 1.0) * grid * 0.45;
                let pa = ((hash01((nx * 1024.0) as i32, (ny * 1024.0) as i32, seed) * 2.0) - 1.0) * texture;
                let pb = ((hash01((nx * 320.0) as i32, (ny * 320.0) as i32, seed ^ 431) * 2.0) - 1.0) * texture * 0.6;
                let paper = (pa + pb) * 12.0;
                let br = 10.0 + (12.0 * base_shade) + (16.0 * grid_line) + paper;
                let bg = 52.0 + (46.0 * base_shade) + (28.0 * grid_line) + paper;
                let bb = 122.0 + (88.0 * base_shade) + (46.0 * grid_line) + paper;
                let glow_mix = line_mask * (0.55 + (glow * 0.45));
                let mut r = lerp(br, 230.0, glow_mix);
                let mut g = lerp(bg, 242.0, glow_mix);
                let mut b = lerp(bb, 255.0, glow_mix);
                let cyan = line_mask * glow * 0.28;
                r = lerp(r, 136.0, cyan);
                g = lerp(g, 224.0, cyan);
                b = lerp(b, 255.0, cyan);
                dst[index] = SkColor::rgba(clamp_to_byte(r), clamp_to_byte(g), clamp_to_byte(b), src[index].a);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
