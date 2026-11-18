//! Renders annotations onto an image as ShareX's editor exports them
//! (`EditorView.GetSnapshot`: the annotation layer's Avalonia visuals,
//! rendered by Avalonia's Skia backend into a `RenderTargetBitmap`).
//!
//! Each annotation becomes what its `CreateVisual` makes (an Avalonia
//! `Rectangle`, `Ellipse` or `Path`, a `StepControl`, ...), laid out on the
//! annotation `Canvas` with Avalonia's layout rounding, and drawn with the
//! Skia calls Avalonia makes for it, through `sharex-skia`.

mod color;
pub mod cursor;
mod effects;
pub mod emoji;
mod image;
mod layout;
mod shadow;
mod shapes;
pub mod text;

use sharex_skia::Pixmap;

use crate::model::{Annotation, Kind};

pub use color::parse_color;
pub use text::step_radius;

/// Renders `annotations` (in z-order, first at the bottom) over `base`.
pub fn render(base: &Pixmap, annotations: &[Annotation]) -> Pixmap {
    render_cached(base, annotations, &mut Cache::default())
}

/// [`render`], keeping what the effects compute from the whole image (the
/// blurred and pixelated sources, `CreateInteractionCacheBitmap`) in
/// `cache` for the next render over the same `base`, as the editor keeps
/// its interaction caches while annotations are drawn and moved.
pub fn render_cached(base: &Pixmap, annotations: &[Annotation], cache: &mut Cache) -> Pixmap {
    let mut pm = base.clone();
    effects::draw_spotlights(&mut pm, base, annotations, cache);
    for a in annotations {
        draw(&mut pm, base, a, cache);
    }
    pm
}

/// The effects' whole-image sources, per effect and strength, for one base
/// image.
#[derive(Default)]
pub struct Cache {
    base: Option<(usize, u32, u32)>,
    sources: std::collections::HashMap<(u8, u32), Pixmap>,
}

impl Cache {
    fn source(&mut self, base: &Pixmap, kind: u8, amount: f32, make: fn(&Pixmap, f32) -> Pixmap) -> &Pixmap {
        let key = (base.pixels().as_ptr() as usize, base.width(), base.height());
        if self.base != Some(key) {
            self.base = Some(key);
            self.sources.clear();
        }
        self.sources.entry((kind, amount.to_bits())).or_insert_with(|| make(base, amount))
    }
}

fn draw(pm: &mut Pixmap, base: &Pixmap, a: &Annotation, cache: &mut Cache) {
    if a.common.shadow_enabled
        && let Some((size, ctm)) = shapes::visual_frame(a)
    {
        shadow::with_shadow(pm, &a.common, size, &ctm, |layer| {
            draw_visual(layer, base, a, cache)
        });
        return;
    }
    draw_visual(pm, base, a, cache);
}

fn draw_visual(pm: &mut Pixmap, base: &Pixmap, a: &Annotation, cache: &mut Cache) {
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
            effects::draw_effect(pm, base, a, cache)
        }
        Kind::Image(p) => {
            if let Some(bitmap) = &p.bitmap {
                let bounds = shapes::bounds(&a.common);
                image::draw_image_control(pm, bounds, a.common.rotation_angle, bitmap, image::Stretch::Fill);
            }
        }
        Kind::Cursor(p) => {
            if let Some(bitmap) = cursor::cursor_bitmap(&p.cursor_type) {
                let bounds = shapes::bounds(&a.common);
                image::draw_image_control(pm, bounds, a.common.rotation_angle, &bitmap, image::Stretch::Uniform);
            }
        }
        Kind::Emoji(p) => {
            // EmojiAnnotation.UpdateVisual: a square sticker as big as the
            // bounds' larger side, shown with the Image's Stretch.Uniform.
            let bounds = shapes::bounds(&a.common);
            let side = (bounds.2 - bounds.0).max(bounds.3 - bounds.1);
            let size = (side.ceil() as i64).max(1) as u32;
            if let Some(bitmap) = emoji::sticker_bitmap(&p.unicode_sequence, size) {
                image::draw_image_control(pm, bounds, a.common.rotation_angle, &bitmap, image::Stretch::Uniform);
            }
        }
        Kind::SmartEraser(p) => image::draw_smart_eraser(pm, &a.common, p),
        Kind::Spotlight(_) | Kind::Crop => {}
    }
}
