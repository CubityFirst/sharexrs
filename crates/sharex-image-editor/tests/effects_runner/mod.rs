//! Runs the image effect cases (tests/data/effects/cases.json, generated
//! from ShareX's catalogue by `tools/editor-harness --effects`) through
//! ShareXrs and compares each with ShareX's result (tests/data/effects/sharex).
//! Shared by tests/effects_match.rs and the image_editor_effects_report
//! example.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sharex_image_editor::effects::{self, Bitmap, EffectCategory, ParamKind, ParamValue, SkColor};

pub fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/effects")
}

/// A PNG of premultiplied bytes stored as they are.
pub fn load_premul_png(path: &Path) -> Option<Bitmap> {
    let img = image::open(path).ok()?.into_rgba8();
    Some(Bitmap::from_premultiplied(img.width(), img.height(), img.into_raw()))
}

pub fn save_premul_png(b: &Bitmap, path: &Path) {
    if b.is_empty() {
        return;
    }
    image::RgbaImage::from_raw(b.width(), b.height(), b.data().to_vec())
        .expect("size")
        .save(path)
        .expect("save");
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Pixel for pixel.
    Identical,
    /// Both threw (ShareX's message, ours).
    BothThrew,
    /// Not ported yet.
    Pending,
    /// Differs: largest channel difference, share of differing pixels.
    Different { max: u8, share: f64, note: String },
}

pub struct CaseResult {
    pub name: String,
    pub effect: String,
    pub category: EffectCategory,
    pub outcome: Outcome,
}

/// Effects whose cases depend on Windows fonts (Segoe UI, Arial, Consolas):
/// skipped elsewhere.
pub const FONT_EFFECTS: &[&str] = &[
    "draw_text",
    "text_watermark",
    "ascii_art",
    "matrix_digital_rain",
    "macos_window",
    "windows11_window",
    "polaroid_frame",
];

pub fn run_cases(filter: Option<&str>, out_dir: Option<&Path>) -> Vec<CaseResult> {
    let data = data_dir();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("cases.json")).unwrap()).unwrap();
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(data.join("results.json")).unwrap()).unwrap();
    let inputs = data.join("inputs");
    let mut input_cache: BTreeMap<String, Bitmap> = BTreeMap::new();
    let mut out = Vec::new();
    for case in cases.as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if let Some(f) = filter
            && !name.starts_with(f)
        {
            continue;
        }
        let id = case["effect"].as_str().unwrap();
        let def = effects::find(id).unwrap_or_else(|| panic!("no effect {id}"));
        if !cfg!(windows) && FONT_EFFECTS.contains(&id) {
            continue;
        }
        let input = case["input"].as_str().unwrap();
        let source = input_cache
            .entry(input.to_string())
            .or_insert_with(|| load_premul_png(&inputs.join(format!("{input}.png"))).expect("input"))
            .clone();
        let mut values = def.default_values();
        if let Some(params) = case["params"].as_object() {
            for p in &def.parameters {
                let Some(v) = params.get(p.key) else { continue };
                let value = match &p.kind {
                    ParamKind::Slider { .. } | ParamKind::Numeric { .. } => ParamValue::Number(v.as_f64().unwrap()),
                    ParamKind::Checkbox { .. } => ParamValue::Bool(v.as_bool().unwrap()),
                    ParamKind::Enum { .. } => ParamValue::Choice(v.as_u64().unwrap() as usize),
                    ParamKind::Color { .. } => ParamValue::Color(SkColor::from_hex(v.as_str().unwrap()).unwrap()),
                    ParamKind::Text { .. } => ParamValue::Text(v.as_str().unwrap().to_string()),
                    ParamKind::FilePath { .. } => ParamValue::Text(
                        v.as_str()
                            .unwrap()
                            .replace("{inputs}", &inputs.to_string_lossy()),
                    ),
                };
                values.set(p.key, value);
            }
        }
        effects::random::seed_shared(case["seed"].as_u64().unwrap());
        let got = def.apply(&source, &values);
        let sharex = results[name].as_str().unwrap_or("missing");
        let file = name.replace('/', "_");
        let outcome = match got {
            Err(e) if e.0 == effects::NOT_PORTED => Outcome::Pending,
            Err(e) => {
                if sharex.starts_with("error") {
                    Outcome::BothThrew
                } else {
                    Outcome::Different {
                        max: 255,
                        share: 1.0,
                        note: format!("ShareXrs threw: {e}"),
                    }
                }
            }
            Ok(b) => {
                if let Some(dir) = out_dir {
                    save_premul_png(&b, &dir.join(format!("{file}.png")));
                }
                if sharex.starts_with("error") {
                    Outcome::Different {
                        max: 255,
                        share: 1.0,
                        note: format!("ShareX threw ({sharex})"),
                    }
                } else {
                    compare(&b, &data.join("sharex").join(format!("{file}.png")))
                }
            }
        };
        out.push(CaseResult {
            name: name.to_string(),
            effect: id.to_string(),
            category: def.category,
            outcome,
        });
    }
    out
}

fn compare(got: &Bitmap, expected_path: &Path) -> Outcome {
    let empty = PathBuf::from(format!("{}.empty", expected_path.display()));
    if empty.exists() {
        return if got.is_empty() {
            Outcome::Identical
        } else {
            Outcome::Different {
                max: 255,
                share: 1.0,
                note: "ShareX's result is empty".into(),
            }
        };
    }
    let Some(expected) = load_premul_png(expected_path) else {
        return Outcome::Different {
            max: 255,
            share: 1.0,
            note: "no reference".into(),
        };
    };
    if (got.width(), got.height()) != (expected.width(), expected.height()) {
        return Outcome::Different {
            max: 255,
            share: 1.0,
            note: format!(
                "{}x{} instead of {}x{}",
                got.width(),
                got.height(),
                expected.width(),
                expected.height()
            ),
        };
    }
    let mut max = 0u8;
    let mut differing = 0usize;
    for (a, b) in got.data().chunks_exact(4).zip(expected.data().chunks_exact(4)) {
        let d = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
        if d > 0 {
            differing += 1;
            max = max.max(d);
        }
    }
    if differing == 0 {
        Outcome::Identical
    } else {
        Outcome::Different {
            max,
            share: differing as f64 / (got.width() * got.height()) as f64,
            note: String::new(),
        }
    }
}

/// Per category: identical (incl. both threw), different, pending.
pub fn summary(results: &[CaseResult]) -> String {
    let mut s = String::new();
    let mut effects_by_cat: BTreeMap<EffectCategory, BTreeMap<&str, Vec<&Outcome>>> = BTreeMap::new();
    for r in results {
        effects_by_cat
            .entry(r.category)
            .or_default()
            .entry(r.effect.as_str())
            .or_default()
            .push(&r.outcome);
    }
    for (cat, effects) in &effects_by_cat {
        let (mut cases, mut same, mut pending) = (0, 0, 0);
        let (mut eff_same, mut eff_pending) = (0, 0);
        for outcomes in effects.values() {
            cases += outcomes.len();
            let ok = outcomes
                .iter()
                .filter(|o| matches!(o, Outcome::Identical | Outcome::BothThrew))
                .count();
            let pend = outcomes.iter().filter(|o| matches!(o, Outcome::Pending)).count();
            same += ok;
            pending += pend;
            if ok == outcomes.len() {
                eff_same += 1;
            }
            if pend == outcomes.len() {
                eff_pending += 1;
            }
        }
        s += &format!(
            "{:<14} effects {:>3}/{:<3} identical ({} pending)   cases {:>4}/{:<4} identical ({} pending)\n",
            cat.name(),
            eff_same,
            effects.len(),
            eff_pending,
            same,
            cases,
            pending
        );
    }
    s
}
