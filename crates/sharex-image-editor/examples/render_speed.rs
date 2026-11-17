//! How long rendering a desktop-sized image with a few annotations takes.
use sharex_image_editor::{Annotation, render};
use sharex_skia::Pixmap;
fn main() {
    let mut base = Pixmap::new(2560, 1440);
    base.pixels_mut().iter_mut().enumerate().for_each(|(i, p)| *p = 0xFF00_0000 | (i as u32).wrapping_mul(2654435761) >> 8);
    let mut list = Vec::new();
    for (i, t) in ["Rectangle", "Ellipse", "Arrow", "Blur", "Pixelate", "Highlight"].iter().enumerate() {
        let mut a = Annotation::new(t).unwrap();
        a.common.start_point = sharex_image_editor::model::Point::new(100.0 + i as f32 * 300.0, 200.0);
        a.common.end_point = sharex_image_editor::model::Point::new(350.0 + i as f32 * 300.0, 500.0);
        list.push(a);
    }
    let mut cache = render::Cache::default();
    for n in 0..3 {
        let t = std::time::Instant::now();
        let _ = render::render_cached(&base, &list, &mut cache);
        println!("render {n}: {:?}", t.elapsed());
    }
}
