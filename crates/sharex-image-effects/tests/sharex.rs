//! Checks against output of ShareX itself (from `tools/effects-harness`, kept in
//! `tests/fixtures`): every effect's defaults serialise exactly as ShareX's do, ShareX's
//! JSON reads back unchanged, and a few small images come out byte for byte the same.

use std::path::Path;

use serde_json::Value;
use sharex_image_effects::{
    Bitmap, CATALOG, Context, EffectKind, ImageEffect, ImageEffectPreset, RandomFast, TypeNames,
    dotnet::Color,
};

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

fn norm(s: &str) -> String {
    s.trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .trim()
        .to_string()
}

#[test]
fn defaults_serialise_as_sharex() {
    for (_, t) in CATALOG {
        let kind = EffectKind::create(t, &mut RandomFast::seeded(1)).unwrap();
        let preset = ImageEffectPreset {
            name: String::new(),
            effects: vec![ImageEffect::new(kind)],
        };
        for (form, file) in [
            (TypeNames::Full, format!("{t}.json")),
            (TypeNames::Short, format!("{t}.sxie.json")),
        ] {
            let theirs = std::fs::read_to_string(fixtures().join("defaults").join(&file)).unwrap();
            assert_eq!(norm(&preset.to_json_string(form)), norm(&theirs), "{file}");
            let back = ImageEffectPreset::from_json_with(
                &serde_json::from_str(&theirs).unwrap(),
                &mut RandomFast::seeded(1),
            )
            .unwrap();
            assert_eq!(
                norm(&back.to_json_string(form)),
                norm(&theirs),
                "{file} read back"
            );
        }
    }
}

#[test]
fn default_preset_and_names() {
    let theirs = std::fs::read_to_string(fixtures().join("defaults/default-preset.json")).unwrap();
    assert_eq!(
        norm(&ImageEffectPreset::default_preset().to_json_string(TypeNames::Full)),
        norm(&theirs)
    );
    let named = std::fs::read_to_string(fixtures().join("defaults/named.json")).unwrap();
    let p = ImageEffectPreset::from_json_str(&named).unwrap();
    assert_eq!(norm(&p.to_json_string(TypeNames::Full)), norm(&named));
    assert_eq!(p.effects[0].label(), "My background");
    assert!(!p.effects[0].enabled);
}

#[test]
fn effect_labels_as_sharex() {
    // ImageEffect.ToString() for every default (names.txt: type, category, name, label).
    let names = std::fs::read_to_string(fixtures().join("defaults/names.txt")).unwrap();
    for line in names.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let kind = EffectKind::create(f[0], &mut RandomFast::seeded(1)).unwrap();
        assert_eq!(kind.category().name(), f[1], "{}", f[0]);
        assert_eq!(kind.display_name(), f[2], "{}", f[0]);
        assert_eq!(ImageEffect::new(kind).label(), f[3], "{}", f[0]);
    }
}

#[test]
fn colors_convert_as_dotnet() {
    assert_eq!(Color::parse("0, 0, 128").unwrap().to_string(), "Navy");
    assert_eq!(
        Color::parse("128, 0, 0, 255").unwrap().to_string(),
        "128, 0, 0, 255"
    );
    assert_eq!(
        Color::parse("235, 235, 235").unwrap().to_string(),
        "235, 235, 235"
    );
    assert_eq!(
        Color::parse("transparent").unwrap().to_string(),
        "Transparent"
    );
    assert_eq!(Color::parse("#FF0000").unwrap().to_string(), "Red");
}

fn read_sxrb(path: &Path) -> Bitmap {
    let d = std::fs::read(path).unwrap();
    let w = u32::from_le_bytes(d[4..8].try_into().unwrap());
    let h = u32::from_le_bytes(d[8..12].try_into().unwrap());
    let mut px = d[12..].to_vec();
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    Bitmap::from_premultiplied(w, h, px)
}

#[test]
fn pixels_match_sharex() {
    let dir = fixtures().join("pixels");
    let cases: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("cases.json")).unwrap()).unwrap();
    let input = read_sxrb(&dir.join("tiny.sxrb"));
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let preset = ImageEffectPreset::from_json(
            &serde_json::json!({ "Name": "", "Effects": c["effects"] }),
        )
        .unwrap();
        let mut ctx = Context {
            random: RandomFast::seeded(1),
            ..Default::default()
        };
        let ours = preset.apply(&input, &mut ctx).unwrap();
        let theirs = read_sxrb(&dir.join(format!("{name}.sxrb")));
        assert!(ours == theirs, "{name} differs from ShareX");
    }
}
