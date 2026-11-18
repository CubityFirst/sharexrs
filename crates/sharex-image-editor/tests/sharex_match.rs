//! Renders every case of tests/data/cases.json and compares it with
//! ShareX's rendering of the same annotations and operations
//! (tests/data/sharex, made by tools/editor-harness from ShareX's own
//! code). Cases without a "pending" note must be identical, pixel for
//! pixel, and the same size.

mod common;

#[test]
fn renders_like_sharex() {
    let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("cases.json")).unwrap()).unwrap();
    let base = common::load(&data.join(cases["base"].as_str().unwrap())).unwrap();
    let mut failures = Vec::new();
    for case in cases["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if common::skip_reason(case, &data).is_some() {
            continue;
        }
        let Some(expected) = common::load(&data.join("sharex").join(format!("{name}.png"))) else {
            continue;
        };
        let got = common::render_case(case, &data, &base);
        if (got.width(), got.height()) != (expected.width(), expected.height()) {
            failures.push(format!(
                "{name}: {}x{} instead of {}x{}",
                got.width(),
                got.height(),
                expected.width(),
                expected.height()
            ));
            continue;
        }
        let (a, b) = (got.to_rgba(), expected.to_rgba());
        let differing = a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| p != q).count();
        if differing > 0 {
            failures.push(format!("{name}: {differing} pixels differ"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
