//! Every image effect case (tests/data/effects/cases.json: each effect's
//! defaults on an opaque and a translucent image, all parameters at their
//! minimum, maximum and a third of the way, every enum option) run through
//! ShareXrs and compared with ShareX's own result (tests/data/effects/sharex,
//! from `tools/editor-harness --effects`). Cases not listed in
//! tests/data/effects/known.json must be identical, pixel for pixel; an
//! effect that throws in ShareX must fail here too.
//!
//! The comparison runs on Windows: ShareX's results come from .NET's
//! `MathF`/`Math`, which call the Windows C runtime (`sinf`, `powf`,
//! `atan2f`...), and so does Rust's `f32` there; other platforms' libms
//! round some results differently. Elsewhere every case still runs and
//! must give ShareX's size.

mod effects_runner;

use effects_runner::Outcome;

#[cfg(windows)]
#[test]
fn effects_match_sharex() {
    let known: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(effects_runner::data_dir().join("known.json")).unwrap(),
    )
    .unwrap();
    let results = effects_runner::run_cases(None, None);
    println!("{}", effects_runner::summary(&results));
    let mut failures = Vec::new();
    for r in &results {
        if let Outcome::Different { max, share, note } = &r.outcome {
            let listed = known.get(&r.name).is_some() || known.get(&r.effect).is_some();
            if !listed {
                failures.push(format!("{}: max {max}, {:.3}% {note}", r.name, share * 100.0));
            }
        }
    }
    assert!(failures.is_empty(), "{} cases differ:\n{}", failures.len(), failures.join("\n"));
}

#[cfg(not(windows))]
#[test]
fn effects_run_everywhere() {
    let results = effects_runner::run_cases(None, None);
    let failures: Vec<String> = results
        .iter()
        .filter_map(|r| match &r.outcome {
            Outcome::Different { note, .. } if !note.is_empty() => Some(format!("{}: {note}", r.name)),
            _ => None,
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("
"));
}

#[test]
fn shared_random_matches_dotnet() {
    // probe/random.txt: Random.Shared's draws after the harness set its state
    // to splitmix64(12345).
    let text = std::fs::read_to_string(effects_runner::data_dir().join("probe/random.txt")).unwrap();
    let mut r = sharex_image_editor::effects::random::Xoshiro::from_seed(12345);
    for line in text.lines() {
        let (call, value) = line.rsplit_once(' ').unwrap();
        let got = if call == "Next()" {
            r.next().to_string()
        } else if call == "NextSingle()" {
            format!("{:?}", r.next_single())
        } else if call == "NextDouble()" {
            format!("{:?}", r.next_double())
        } else if let Some(args) = call.strip_prefix("Next(").and_then(|s| s.strip_suffix(')')) {
            match args.split_once(',') {
                Some((lo, hi)) => r.next_range(lo.parse().unwrap(), hi.parse().unwrap()).to_string(),
                None => r.next_max(args.parse().unwrap()).to_string(),
            }
        } else {
            panic!("{call}")
        };
        let want = if call.starts_with("NextSingle") || call.starts_with("NextDouble") {
            // .NET's "R" round-trips like Rust's shortest Debug form.
            let v: f64 = value.parse().unwrap();
            if call.starts_with("NextSingle") {
                format!("{:?}", v as f32)
            } else {
                format!("{v:?}")
            }
        } else {
            value.to_string()
        };
        assert_eq!(got, want, "{line}");
    }
}
