//! `ImageEffectsViewModel.CreateSampleImage`: the image the Image effects
//! window previews presets on when it has none of its own (Presets mode):
//! a 720x480 diagonal gradient of the theme's main and panel backgrounds
//! with an isometric cube in the accent colours, drawn as ShareX draws it
//! (`SkiaDrawing.Gradient`, anti-aliased `FillPolygon`s, a 2px round-joined
//! outline and three lines).

use crate::Bitmap;
use crate::dotnet::GradientMode;
use crate::ops::{gradient_shader_raw, stroke_path, stroke_paint};
use crate::skia::{Canvas, Paint, Rect, Shader};

/// The theme colours it's drawn with (`GetThemeColor`), straight RGBA.
#[derive(Clone, Copy, Debug)]
pub struct SampleColors {
    pub background_main: [u8; 4],
    pub background_panel: [u8; 4],
    pub accent_start: [u8; 4],
    pub accent_end: [u8; 4],
    pub accent_foreground: [u8; 4],
}

impl Default for SampleColors {
    /// `GetThemeColor`'s fallbacks.
    fn default() -> Self {
        Self {
            background_main: [39, 39, 39, 255],
            background_panel: [36, 36, 36, 255],
            accent_start: [62, 131, 242, 255],
            accent_end: [57, 117, 213, 255],
            accent_foreground: [216, 218, 219, 255],
        }
    }
}

/// `BlendColors`: each channel truncated, alpha included.
fn blend(first: [u8; 4], second: [u8; 4], amount: f32) -> [u8; 4] {
    let amount = amount.clamp(0.0, 1.0);
    let inverse = 1.0 - amount;
    let c = |i: usize| (first[i] as f32 * inverse + second[i] as f32 * amount) as u8;
    [c(0), c(1), c(2), c(3)]
}

/// `CreateHexagon(bounds, 0)`.
fn hexagon(l: f32, t: f32, w: f32, h: f32) -> [(f32, f32); 6] {
    let quarter = h / 4.0;
    let cx = l + w / 2.0;
    let (r, b) = (l + w, t + h);
    [(cx, t), (r, t + quarter), (r, b - quarter), (cx, b), (l, b - quarter), (l, t + quarter)]
}

fn polygon(points: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    let (first, rest) = points.split_first()?;
    pb.move_to(first.0, first.1);
    for p in rest {
        pb.line_to(p.0, p.1);
    }
    pb.close();
    pb.finish()
}

fn line(a: (f32, f32), b: (f32, f32)) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(a.0, a.1);
    pb.line_to(b.0, b.1);
    pb.finish()
}

/// The sample image.
pub fn create_sample_image(colors: &SampleColors) -> Bitmap {
    let (w, h) = (720u32, 480u32);
    let mut bmp = Bitmap::new(w, h);
    {
        let mut g = Canvas::new(&mut bmp);
        let mut background = Paint::shader(gradient_shader_raw(&[colors.background_main, colors.background_panel], &[0.0, 1.0], GradientMode::ForwardDiagonal, 0, 0, w as i32, h as i32));
        background.anti_alias = true;
        g.draw_rect(Rect::xywh(0.0, 0.0, w as f32, h as f32), &background);
        let size = 200.0f32;
        let (l, t) = ((w as f32 - size) / 2.0, (h as f32 - size) / 2.0);
        let outer = hexagon(l, t, size, size);
        let center = (l + size / 2.0, t + size / 2.0);
        let top = [outer[0], outer[1], center, outer[5]];
        let left = [outer[5], center, outer[3], outer[4]];
        let right = [outer[1], outer[2], outer[3], center];
        let fill = |g: &mut Canvas, pts: &[(f32, f32)], c: [u8; 4]| {
            if let Some(p) = polygon(pts) {
                let mut paint = Paint::fill(c);
                paint.anti_alias = true;
                g.fill_path(&p, &paint);
            }
        };
        fill(&mut g, &top, blend(colors.accent_start, colors.accent_foreground, 0.16));
        fill(&mut g, &left, colors.accent_start);
        fill(&mut g, &right, colors.accent_end);
        let fg = colors.accent_foreground;
        let outline = stroke_paint(Shader::Color([fg[0], fg[1], fg[2], 100]), 2.0);
        if let Some(p) = polygon(&outer) {
            stroke_path(&mut g, &p, &outline);
        }
        for a in [outer[3], outer[1], outer[5]] {
            if let Some(p) = line(a, center) {
                stroke_path(&mut g, &p, &outline);
            }
        }
    }
    bmp
}
