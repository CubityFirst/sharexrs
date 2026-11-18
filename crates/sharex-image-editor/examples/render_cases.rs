//! Renders tests/data/cases.json with ShareXrs and compares each case with
//! ShareX's rendering of it (the editor harness's output).
//!
//!     cargo run -p sharex-image-editor --example render_cases -- <sharex dir> [out dir] [filter]

#[path = "../tests/common/mod.rs"]
mod common;

use sharex_skia::Pixmap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let sharex = std::path::PathBuf::from(&args[1]);
    let out = args.get(2).map(std::path::PathBuf::from);
    let filter = args.get(3).cloned();
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("cases.json")).unwrap()).unwrap();
    let base = common::load(&data.join(cases["base"].as_str().unwrap())).unwrap();
    let (mut identical, mut total) = (0, 0);
    for case in cases["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if filter.as_deref().is_some_and(|f| !name.contains(f)) {
            continue;
        }
        let pm = common::render_case(case, &data, &base);
        if let Some(out) = &out {
            std::fs::create_dir_all(out).unwrap();
            image::RgbaImage::from_raw(pm.width(), pm.height(), pm.to_rgba())
                .unwrap()
                .save(out.join(format!("{name}.png")))
                .unwrap();
        }
        let note = match common::skip_reason(case, &data) {
            Some(r) => format!("  [{r}]"),
            None => String::new(),
        };
        match common::load(&sharex.join(format!("{name}.png"))) {
            Some(r) if (r.width(), r.height()) != (pm.width(), pm.height()) => {
                total += 1;
                println!(
                    "{name:32} size {}x{} instead of {}x{}{note}",
                    pm.width(),
                    pm.height(),
                    r.width(),
                    r.height()
                );
            }
            Some(r) => {
                let (max, n) = diff(&pm, &r);
                total += 1;
                identical += (n == 0) as usize;
                let pct = 100.0 * n as f64 / (pm.width() * pm.height()) as f64;
                println!("{name:32} max {max:3}  differing {n:6} px ({pct:.3}%){note}");
            }
            None => println!("{name:32} (no ShareX rendering){note}"),
        }
    }
    println!("{identical}/{total} identical");
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
