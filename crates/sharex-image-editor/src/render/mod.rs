//! Renders annotations onto an image as ShareX's editor exports them
//! (`EditorView.GetSnapshot`: the annotation layer's Avalonia visuals,
//! rendered by Avalonia's Skia backend into a `RenderTargetBitmap`).
//!
//! Each annotation becomes what its `CreateVisual` makes (an Avalonia
//! `Rectangle`, `Ellipse` or `Path`, a `StepControl`, ...), laid out on the
//! annotation `Canvas` with Avalonia's layout rounding, and drawn with the
//! Skia calls Avalonia makes for it, through `sharex-skia`.

mod color;
mod effects;
mod layout;
mod shapes;
pub mod text;

use sharex_skia::Pixmap;

use crate::model::{Annotation, Kind};

pub use color::parse_color;

/// Renders `annotations` (in z-order, first at the bottom) over `base`.
pub fn render(base: &Pixmap, annotations: &[Annotation]) -> Pixmap {
    let mut pm = base.clone();
    effects::draw_spotlights(&mut pm, base, annotations);
    for a in annotations {
        draw(&mut pm, base, a);
    }
    pm
}

fn draw(pm: &mut Pixmap, base: &Pixmap, a: &Annotation) {
    match &a.kind {
        Kind::Rectangle(p) => {
            shapes::draw_rectangle(pm, &a.common, p.border_style.clone(), p.corner_radius)
        }
        Kind::Ellipse(p) => shapes::draw_ellipse(pm, &a.common, p.border_style.clone()),
        Kind::Line(p) => shapes::draw_line(pm, &a.common, p),
        Kind::Arrow(p) => shapes::draw_arrow(pm, &a.common, p),
        Kind::Freehand(p) => shapes::draw_freehand(pm, &a.common, p),
        Kind::Text(p) => text::draw_text_annotation(pm, &a.common, p),
        Kind::Number(p) => text::draw_step(pm, &a.common, p),
        Kind::SpeechBalloon(p) => text::draw_balloon(pm, &a.common, p),
        Kind::Highlight(_) | Kind::Blur(_) | Kind::Pixelate(_) | Kind::Magnify(_) => {
            effects::draw_effect(pm, base, a)
        }
        _ => {}
    }
}
