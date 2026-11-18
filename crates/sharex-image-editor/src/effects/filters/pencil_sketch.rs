//! `PencilSketchImageEffect` (`Core/ImageEffects/Filters/PencilSketchImageEffect.cs`).

use crate::effects::prelude::*;

/// `PencilSketchImageEffect`.
#[derive(Clone, Debug)]
pub struct PencilSketchImageEffect {
    pub blur_radius: i32,
    pub edge_strength: f32,
    pub pencil_darkness: f32,
    pub paper_brightness: f32,
}

impl Default for PencilSketchImageEffect {
    fn default() -> Self {
        PencilSketchImageEffect {
            blur_radius: 8,
            edge_strength: 65.0,
            pencil_darkness: 70.0,
            paper_brightness: 100.0,
        }
    }
}

impl EffectImpl for PencilSketchImageEffect {
    fn info() -> Info {
        Info::new("pencil_sketch", "Pencil sketch", EffectCategory::Filters, "app/edit")
            .desc("Converts the image to a pencil sketch style drawing.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("blur_radius", "Blur radius", 1, 24, 8, |e: &mut Self, v| e.blur_radius = v),
            float_slider::<Self>("edge_strength", "Edge strength", 0.0, 160.0, 65.0, |e: &mut Self, v| e.edge_strength = v),
            float_slider::<Self>("pencil_darkness", "Pencil darkness", 0.0, 100.0, 70.0, |e: &mut Self, v| e.pencil_darkness = v),
            float_slider::<Self>("paper_brightness", "Paper brightness", 40.0, 130.0, 100.0, |e: &mut Self, v| e.paper_brightness = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if source.is_empty() {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let radius = self.blur_radius.clamp(1, 24);
        let edge_strength = clampf(self.edge_strength, 0.0, 160.0) / 100.0;
        let darkness = clampf(self.pencil_darkness, 0.0, 100.0) / 100.0;
        let paper = clampf(self.paper_brightness, 40.0, 130.0) / 100.0;
        let src = pixels(source);
        let gray: Vec<f32> = src
            .iter()
            .map(|c| ((0.2126 * c.r as f32) + (0.7152 * c.g as f32) + (0.0722 * c.b as f32)) / 255.0)
            .collect();
        let inverse: Vec<f32> = gray.iter().map(|g| 1.0 - g).collect();
        let blurred = sketch_box_blur(&inverse, width, height, radius);
        let edges = sketch_sobel(&gray, width, height);
        let dst: Vec<SkColor> = (0..src.len())
            .map(|i| {
                let dodge = gray[i] / (1.0 - blurred[i]).max(0.03);
                let sketch = clampf(dodge, 0.0, 1.0);
                let ed = edges[i] * edge_strength * 0.85;
                let mut v = clampf(sketch - ed, 0.0, 1.0);
                v = v.powf(1.0 + (darkness * 1.75));
                v = lerp(v, 1.0, clampf(paper - 1.0, 0.0, 0.6));
                let b = clamp_to_byte(v * 255.0);
                SkColor::rgba(b, b, b, src[i].a)
            })
            .collect();
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn sketch_box_blur(src: &[f32], width: i32, height: i32, radius: i32) -> Vec<f32> {
    let (right, bottom) = (width - 1, height - 1);
    let window = (radius * 2 + 1) as f32;
    let mut h = vec![0f32; src.len()];
    let mut v = vec![0f32; src.len()];
    for y in 0..height {
        let row = y * width;
        let mut acc = 0f32;
        for k in -radius..=radius {
            acc += src[(row + k.clamp(0, right)) as usize];
        }
        for x in 0..width {
            h[(row + x) as usize] = acc / window;
            let rm = (x - radius).clamp(0, right);
            let ad = (x + radius + 1).clamp(0, right);
            acc += src[(row + ad) as usize] - src[(row + rm) as usize];
        }
    }
    for x in 0..width {
        let mut acc = 0f32;
        for k in -radius..=radius {
            acc += h[(k.clamp(0, bottom) * width + x) as usize];
        }
        for y in 0..height {
            v[(y * width + x) as usize] = acc / window;
            let rm = (y - radius).clamp(0, bottom);
            let ad = (y + radius + 1).clamp(0, bottom);
            acc += h[(ad * width + x) as usize] - h[(rm * width + x) as usize];
        }
    }
    v
}

fn sketch_sobel(gray: &[f32], width: i32, height: i32) -> Vec<f32> {
    let (right, bottom) = (width - 1, height - 1);
    let mut edges = vec![0f32; gray.len()];
    let g = |x: i32, y: i32| gray[(y * width + x) as usize];
    for y in 0..height {
        let (y0, y2) = (if y > 0 { y - 1 } else { 0 }, if y < bottom { y + 1 } else { bottom });
        for x in 0..width {
            let (x0, x2) = (if x > 0 { x - 1 } else { 0 }, if x < right { x + 1 } else { right });
            let (tl, tc, tr) = (g(x0, y0), g(x, y0), g(x2, y0));
            let (ml, mr) = (g(x0, y), g(x2, y));
            let (bl, bc, br) = (g(x0, y2), g(x, y2), g(x2, y2));
            let gx = (-tl + tr) + (-2.0 * ml + (2.0 * mr)) + (-bl + br);
            let gy = (-tl - (2.0 * tc) - tr) + (bl + (2.0 * bc) + br);
            edges[(y * width + x) as usize] = clampf(((gx * gx) + (gy * gy)).sqrt() / 4.0, 0.0, 1.0);
        }
    }
    edges
}
