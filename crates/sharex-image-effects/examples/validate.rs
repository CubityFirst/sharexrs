//! Compares ShareXrs's image effects with ShareX's, pixel for pixel.
//!
//! Run `tools/effects-harness` first (it writes `<work>/inputs`, `<work>/sharex`,
//! `<work>/roundtrip`, `<work>/defaults`), then:
//!
//!   cargo run --release -p sharex-image-effects --example validate -- <cases.json> <work> [case prefix]
//!
//! Writes `<work>/sharexrs/<case>.sxrb|png`, `<work>/diff/<case>.png` (differing pixels
//! in red) and `<work>/report.md`: per case the size check, the largest difference per
//! channel (premultiplied B, G, R, A, as ShareX holds them) and the share of pixels
//! that differ at all; then the preset JSON round trips and every effect's defaults.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sharex_image_effects::{
    Bitmap, CATALOG, Context, EffectKind, ImageEffect, ImageEffectPreset, RandomFast, TypeNames,
};

fn read_sxrb(path: &Path) -> Option<Bitmap> {
    let d = std::fs::read(path).ok()?;
    if &d[..4] != b"SXRB" {
        return None;
    }
    let w = u32::from_le_bytes(d[4..8].try_into().ok()?);
    let h = u32::from_le_bytes(d[8..12].try_into().ok()?);
    // BGRA -> RGBA.
    let mut px = d[12..].to_vec();
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    Some(Bitmap::from_premultiplied(w, h, px))
}

fn write_sxrb(path: &Path, b: &Bitmap) {
    let mut out = b"SXRB".to_vec();
    out.extend_from_slice(&b.width().to_le_bytes());
    out.extend_from_slice(&b.height().to_le_bytes());
    for p in b.data().chunks_exact(4) {
        out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
    }
    std::fs::write(path, out).unwrap();
}

fn write_png(path: &Path, b: &Bitmap) {
    if b.is_empty() {
        return;
    }
    image::save_buffer(
        path,
        &b.to_straight(),
        b.width(),
        b.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_string()
}

struct Diff {
    max: [u8; 4],
    differing: usize,
    total: usize,
}

fn compare(a: &Bitmap, b: &Bitmap) -> Diff {
    let mut max = [0u8; 4];
    let mut differing = 0;
    for (p, q) in a.data().chunks_exact(4).zip(b.data().chunks_exact(4)) {
        let mut any = false;
        // Report in ShareX's BGRA order.
        for (i, c) in [2usize, 1, 0, 3].iter().enumerate() {
            let d = p[*c].abs_diff(q[*c]);
            max[i] = max[i].max(d);
            any |= d != 0;
        }
        differing += any as usize;
    }
    Diff {
        max,
        differing,
        total: (a.width() * a.height()) as usize,
    }
}

fn diff_image(a: &Bitmap, b: &Bitmap) -> Bitmap {
    let mut out = Bitmap::new(a.width(), a.height());
    for y in 0..a.height() {
        for x in 0..a.width() {
            let (p, q) = (a.pixel(x, y), b.pixel(x, y));
            let d = (0..4).map(|i| p[i].abs_diff(q[i])).max().unwrap();
            let g = (p[0] as u32 + p[1] as u32 + p[2] as u32) / 6;
            out.set_pixel(
                x,
                y,
                if d == 0 {
                    [g as u8, g as u8, g as u8, 255]
                } else {
                    [255, (255 - d).min(200), 0, 255]
                },
            );
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cases_path = PathBuf::from(args.get(1).expect("cases.json"));
    let work = PathBuf::from(args.get(2).expect("work dir"));
    let only = args.get(3).cloned();
    let inputs = std::path::absolute(work.join("inputs")).expect("inputs (run the harness first)");
    let inputs_str = inputs
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    for d in ["sharexrs", "diff"] {
        std::fs::create_dir_all(work.join(d)).unwrap();
    }
    let cases: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(&cases_path).unwrap()).unwrap();
    let mut report = String::new();
    writeln!(
        report,
        "| Case | Size | max Δ B,G,R,A | pixels differing |\n|---|---|---|---|"
    )
    .unwrap();
    let mut roundtrip_failures = Vec::new();
    let mut exact = 0;
    let mut within1 = 0;
    let mut n = 0;
    for c in &cases {
        let name = c["name"].as_str().unwrap();
        if only.as_ref().is_some_and(|o| !name.starts_with(o.as_str())) {
            continue;
        }
        let seed = c.get("seed").and_then(Value::as_i64).unwrap_or(1) as i32;
        let preset_json = serde_json::json!({ "Name": c.get("presetName").cloned().unwrap_or(Value::String(String::new())), "Effects": c["effects"] });
        let text = serde_json::to_string(&preset_json)
            .unwrap()
            .replace("{inputs}", &inputs_str.replace('\\', "\\\\"));
        let preset = ImageEffectPreset::from_json_with(
            &serde_json::from_str(&text).unwrap(),
            &mut RandomFast::seeded(seed),
        )
        .unwrap();

        // The preset as ShareX writes it back (export form).
        if let Ok(theirs) =
            std::fs::read_to_string(work.join("roundtrip").join(format!("{name}.json")))
        {
            let ours = preset.to_json_string(TypeNames::Short);
            if normalize_newlines(&ours) != normalize_newlines(&theirs) {
                roundtrip_failures.push(name.to_string());
                std::fs::write(work.join("sharexrs").join(format!("{name}.json")), ours).unwrap();
            }
        }

        let input = read_sxrb(
            &work
                .join("inputs")
                .join(format!("{}.sxrb", c["input"].as_str().unwrap())),
        )
        .unwrap();
        let mut ctx = Context {
            random: RandomFast::seeded(seed),
            base_dir: work.clone(),
            ..Default::default()
        };
        let t = std::time::Instant::now();
        let ours = preset.apply(&input, &mut ctx);
        let ms = t.elapsed().as_millis();
        let theirs = read_sxrb(&work.join("sharex").join(format!("{name}.sxrb")));
        n += 1;
        let row = match (ours, theirs) {
            (Some(o), Some(t)) => {
                write_sxrb(&work.join("sharexrs").join(format!("{name}.sxrb")), &o);
                write_png(&work.join("sharexrs").join(format!("{name}.png")), &o);
                if (o.width(), o.height()) != (t.width(), t.height()) {
                    format!(
                        "| {name} | **{}x{} vs ShareX {}x{}** | | |",
                        o.width(),
                        o.height(),
                        t.width(),
                        t.height()
                    )
                } else {
                    let d = compare(&o, &t);
                    if d.differing > 0 {
                        write_png(
                            &work.join("diff").join(format!("{name}.png")),
                            &diff_image(&o, &t),
                        );
                    }
                    let m = *d.max.iter().max().unwrap();
                    if m == 0 {
                        exact += 1;
                    } else if m <= 1 {
                        within1 += 1;
                    }
                    let pct = 100.0 * d.differing as f64 / d.total.max(1) as f64;
                    format!(
                        "| {name} | {}x{} | {},{},{},{} | {:.3}% {}|",
                        o.width(),
                        o.height(),
                        d.max[0],
                        d.max[1],
                        d.max[2],
                        d.max[3],
                        pct,
                        if m == 0 { "✅ " } else { "" }
                    )
                }
            }
            (None, None) => {
                exact += 1;
                format!("| {name} | null in both | | ✅ |")
            }
            (o, t) => format!(
                "| {name} | **ShareXrs {} / ShareX {}** | | |",
                if o.is_some() { "image" } else { "null" },
                if t.is_some() { "image" } else { "null" }
            ),
        };
        println!("{row}  ({ms} ms)");
        writeln!(report, "{row}").unwrap();
    }
    writeln!(
        report,
        "\n{n} cases: {exact} identical, {within1} within ±1.\n"
    )
    .unwrap();
    writeln!(
        report,
        "Preset round trips differing from ShareX's: {roundtrip_failures:?}\n"
    )
    .unwrap();

    // Every effect's defaults, in both JSON forms.
    let mut default_failures = Vec::new();
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
            if let Ok(theirs) = std::fs::read_to_string(work.join("defaults").join(&file)) {
                let ours = preset.to_json_string(form);
                if normalize_newlines(&ours) != normalize_newlines(&theirs) {
                    default_failures.push(file.clone());
                    std::fs::write(work.join("sharexrs").join(format!("default-{file}")), ours)
                        .unwrap();
                }
                // And ShareX's JSON read back and written again is unchanged.
                let back = ImageEffectPreset::from_json_with(
                    &serde_json::from_str(&theirs).unwrap(),
                    &mut RandomFast::seeded(1),
                )
                .unwrap();
                if normalize_newlines(&back.to_json_string(form)) != normalize_newlines(&theirs) {
                    default_failures.push(format!("{file} (read back)"));
                }
            }
        }
    }
    let default_preset =
        std::fs::read_to_string(work.join("defaults").join("default-preset.json")).ok();
    if default_preset.is_some_and(|t| {
        normalize_newlines(&ImageEffectPreset::default_preset().to_json_string(TypeNames::Full))
            != normalize_newlines(&t)
    }) {
        default_failures.push("default-preset.json".into());
    }
    if let Ok(theirs) = std::fs::read_to_string(work.join("defaults").join("named.json")) {
        let back = ImageEffectPreset::from_json_str(&theirs).unwrap();
        if normalize_newlines(&back.to_json_string(TypeNames::Full)) != normalize_newlines(&theirs)
        {
            default_failures.push("named.json".into());
        }
    }
    writeln!(
        report,
        "Defaults differing from ShareX's: {default_failures:?}"
    )
    .unwrap();
    println!("\n{n} cases: {exact} identical, {within1} within ±1");
    println!("round trips differing: {roundtrip_failures:?}");
    println!("defaults differing: {default_failures:?}");
    std::fs::write(work.join("report.md"), report).unwrap();
}
