//! Renders tests/data/cases.json with ShareXrs and compares each case with
//! ShareX's rendering of it (the editor harness's output).
//!
//!     cargo run -p sharex-image-editor --example render_cases -- <sharex dir> [out dir] [filter]

use sharex_image_editor::{Annotation, render};
use sharex_skia::Pixmap;

fn load(p: &std::path::Path) -> Option<Pixmap> {
    let im = image::open(p).ok()?.to_rgba8();
    Some(Pixmap::from_rgba(im.width(), im.height(), im.as_raw()))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sharex = std::path::PathBuf::from(&args[1]);
    let out = args.get(2).map(std::path::PathBuf::from);
    let filter = args.get(3).cloned();
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("cases.json")).unwrap()).unwrap();
    let base = load(&data.join(cases["base"].as_str().unwrap())).unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if filter.as_deref().is_some_and(|f| !name.contains(f)) {
            continue;
        }
        let anns: Vec<Annotation> = case["annotations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| Annotation::from_json(a).unwrap())
            .collect();
        let pm = render(&base, &anns);
        if let Some(out) = &out {
            std::fs::create_dir_all(out).unwrap();
            image::RgbaImage::from_raw(pm.width(), pm.height(), pm.to_rgba())
                .unwrap()
                .save(out.join(format!("{name}.png")))
                .unwrap();
        }
        match load(&sharex.join(format!("{name}.png"))) {
            Some(r) => {
                let (max, n) = diff(&pm, &r);
                let pct = 100.0 * n as f64 / (pm.width() * pm.height()) as f64;
                println!("{name:28} max {max:3}  differing {n:6} px ({pct:.3}%)");
            }
            None => println!("{name:28} (no ShareX rendering)"),
        }
    }
}

fn diff(a: &Pixmap, b: &Pixmap) -> (u32, usize) {
    let mut m = 0;
    let mut n = 0;
    for (x, y) in a.to_rgba().chunks(4).zip(b.to_rgba().chunks(4)) {
        let d = x
            .iter()
            .zip(y)
            .map(|(p, q)| p.abs_diff(*q) as u32)
            .max()
            .unwrap();
        m = m.max(d);
        n += (d > 0) as usize;
    }
    (m, n)
}
