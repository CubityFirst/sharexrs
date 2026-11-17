//! Renders every case of tests/data/cases.json and compares it with
//! ShareX's rendering of the same annotations (tests/data/sharex, made by
//! tools/editor-harness from ShareX's own code). Cases without a
//! "pending" note must be identical, pixel for pixel.

use sharex_image_editor::{Annotation, render};
use sharex_skia::Pixmap;

fn load(p: &std::path::Path) -> Option<Pixmap> {
    let im = image::open(p).ok()?.to_rgba8();
    Some(Pixmap::from_rgba(im.width(), im.height(), im.as_raw()))
}

#[test]
fn renders_like_sharex() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("cases.json")).unwrap()).unwrap();
    let base = load(&data.join(cases["base"].as_str().unwrap())).unwrap();
    let mut failures = Vec::new();
    for case in cases["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if case.get("pending").is_some() {
            continue;
        }
        // Cases that need a system font (Segoe UI, on Windows) skip without it.
        let fonts = case
            .get("fonts")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default();
        if fonts.iter().any(|f| {
            sharex_image_editor::render::text::load_face(f.as_str().unwrap_or(""), false, false)
                .is_none()
        }) {
            continue;
        }
        let Some(expected) = load(&data.join("sharex").join(format!("{name}.png"))) else {
            continue;
        };
        let anns: Vec<Annotation> = case["annotations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| Annotation::from_json(a).unwrap())
            .collect();
        let got = render(&base, &anns);
        let (a, b) = (got.to_rgba(), expected.to_rgba());
        let differing = a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| p != q).count();
        if differing > 0 {
            failures.push(format!("{name}: {differing} pixels differ"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
