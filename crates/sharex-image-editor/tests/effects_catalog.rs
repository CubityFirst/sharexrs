//! The effect catalogue against ShareX's own (tests/data/effects/catalog.json,
//! dumped by `tools/editor-harness --effects` from
//! `ImageEffectCatalog.Definitions`): the same effects in the same order,
//! each with the same id, name, browser label, icon key, description,
//! category, immediate flag and parameters (kind, key, label, description,
//! range, default, tick/snap/format, increment, options, file filter).

use sharex_image_editor::effects::{self, EffectCategory, ParamKind};

fn num(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("number: {v}"))
}

#[test]
fn catalog_matches_sharex() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/effects/catalog.json");
    let sharex: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let ours = effects::catalog();
    let ids: Vec<&str> = ours.iter().map(|d| d.id).collect();
    let theirs: Vec<&str> = sharex.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids, theirs, "effects or their order");

    let mut problems = Vec::new();
    for (d, e) in ours.iter().zip(&sharex) {
        let mut check = |what: &str, a: String, b: String| {
            if a != b {
                problems.push(format!("{} {what}: {a:?} vs ShareX {b:?}", d.id));
            }
        };
        check("name", d.name.into(), e["name"].as_str().unwrap().into());
        check(
            "browserLabel",
            d.browser_label.clone(),
            e["browserLabel"].as_str().unwrap().into(),
        );
        check("icon", d.icon.into(), e["icon"].as_str().unwrap().into());
        check(
            "description",
            d.description.into(),
            e["description"].as_str().unwrap().into(),
        );
        check(
            "category",
            d.category.name().into(),
            e["category"].as_str().unwrap().into(),
        );
        check(
            "applyImmediately",
            d.apply_immediately.to_string(),
            e["applyImmediately"].as_bool().unwrap().to_string(),
        );
        let params = e["parameters"].as_array().unwrap();
        check(
            "parameter count",
            d.parameters.len().to_string(),
            params.len().to_string(),
        );
        for (p, q) in d.parameters.iter().zip(params) {
            let key = p.key;
            check(
                &format!("{key} key"),
                p.key.into(),
                q["key"].as_str().unwrap().into(),
            );
            check(
                &format!("{key} label"),
                p.label.into(),
                q["label"].as_str().unwrap().into(),
            );
            check(
                &format!("{key} description"),
                p.description.unwrap_or("").into(),
                q["description"].as_str().unwrap_or("").into(),
            );
            let kind = q["kind"].as_str().unwrap();
            let ours = match &p.kind {
                ParamKind::Slider {
                    min,
                    max,
                    default,
                    tick,
                    snap,
                    format,
                } => {
                    format!("slider {min} {max} {default} {tick} {snap} {format}")
                }
                ParamKind::Numeric {
                    min,
                    max,
                    default,
                    increment,
                    format,
                } => {
                    format!("numeric {min} {max} {default} {increment} {format}")
                }
                ParamKind::Checkbox { default } => format!("checkbox {default}"),
                ParamKind::Enum { options, default } => {
                    format!("enum {default} {}", options.join("|"))
                }
                ParamKind::Color { default } => format!("color {}", default.to_hex()),
                ParamKind::Text { default } => format!("text {default}"),
                ParamKind::FilePath { default, filter } => {
                    format!("file {default} {}", filter.unwrap_or(""))
                }
            };
            let theirs = match kind {
                "slider" => format!(
                    "slider {} {} {} {} {} {}",
                    num(&q["min"]),
                    num(&q["max"]),
                    num(&q["default"]),
                    num(&q["tick"]),
                    q["snap"].as_bool().unwrap(),
                    q["format"].as_str().unwrap()
                ),
                "numeric" => format!(
                    "numeric {} {} {} {} {}",
                    num(&q["min"]),
                    num(&q["max"]),
                    num(&q["default"]),
                    num(&q["increment"]),
                    q["format"].as_str().unwrap()
                ),
                "checkbox" => format!("checkbox {}", q["default"].as_bool().unwrap()),
                "enum" => format!(
                    "enum {} {}",
                    q["default"].as_u64().unwrap(),
                    q["options"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|o| o["label"].as_str().unwrap())
                        .collect::<Vec<_>>()
                        .join("|")
                ),
                "color" => format!("color {}", q["default"].as_str().unwrap()),
                "text" => format!("text {}", q["default"].as_str().unwrap()),
                "file" => format!(
                    "file {} {}",
                    q["default"].as_str().unwrap(),
                    q["filter"].as_str().unwrap_or("")
                ),
                k => panic!("kind {k}"),
            };
            check(&format!("{key} kind"), ours, theirs);
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn lookups() {
    assert_eq!(
        effects::find("GAUSSIAN_BLUR").map(|d| d.id),
        Some("gaussian_blur")
    );
    assert!(effects::find("nope").is_none());
    let total: usize = EffectCategory::ALL
        .iter()
        .map(|c| effects::by_category(*c).len())
        .sum();
    assert_eq!(total, effects::catalog().len());
    assert!(
        effects::find("rotate_180")
            .unwrap()
            .editor_operation()
            .is_some()
    );
    assert!(effects::find("invert").unwrap().apply_immediately);
}
