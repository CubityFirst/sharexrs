//! ShareXrs's actions against ShareX's own (`tools/actions-harness`, run on
//! Windows with the reference build): `CodeMenuEntryActions.Parse`,
//! `ExternalProgram.CheckExtension`, the classes' JSON and
//! `ExternalProgram.Run` over the same cases, each run with the same probe
//! program (`examples/sharex_settings_action_probe.rs`) in a scratch folder.
//! The runs compare the command line `CreateProcess` got, the arguments the
//! probe parsed, what `Run` returned and the files left after
//! `DeletePendingInputFile`.

use std::path::Path;

use serde_json::Value;
use sharex_settings::external_program::{CodeMenuEntryActions, ExternalProgram, check_extension};
use sharex_settings::WatchFolderSettings;

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/actions").join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn s(v: &Value) -> Option<&str> {
    v.as_str()
}

#[test]
fn parse_check_extension_and_json_as_sharex() {
    let cases = fixture("cases.json");
    let want = fixture("results-sharex.json");
    for (c, w) in cases["parse"].as_array().unwrap().iter().zip(want["parse"].as_array().unwrap()) {
        let got = CodeMenuEntryActions::parse(s(&c["pattern"]).unwrap(), s(&c["input"]), s(&c["output"]));
        assert_eq!(got, w.as_str().unwrap(), "{c}");
    }
    for (c, w) in cases["check_extension"].as_array().unwrap().iter().zip(want["check_extension"].as_array().unwrap()) {
        assert_eq!(check_extension(s(&c["path"]).unwrap(), s(&c["extensions"])), w.as_bool().unwrap(), "{c}");
    }
    for (c, w) in cases["json"].as_array().unwrap().iter().zip(want["json"].as_array().unwrap()) {
        let value = c["value"].clone();
        let got = match s(&c["kind"]).unwrap() {
            "ExternalProgram" => {
                let mut p = ExternalProgram::default();
                if let Value::Object(m) = value {
                    let mut base = serde_json::to_value(&p).unwrap();
                    base.as_object_mut().unwrap().extend(m);
                    p = serde_json::from_value(base).unwrap();
                }
                serde_json::to_string(&p).unwrap()
            }
            _ => {
                let w: WatchFolderSettings = if value.is_null() { Default::default() } else { serde_json::from_value(value).unwrap() };
                serde_json::to_string(&w).unwrap()
            }
        };
        assert_eq!(got, w.as_str().unwrap(), "{c}");
    }
}

/// `ExternalProgram.Run`'s cases: ShareX's results come from Windows
/// (`CreateProcess` command lines, Windows paths).
#[cfg(windows)]
mod runs {
    use std::path::{Path, PathBuf};

    use serde_json::{Value, json};
    use sharex_settings::external_program::{ExternalProgram, delete_pending_input_file};

    use super::{fixture, s};

    /// The probe example, built next to this test by `cargo test`.
    fn probe() -> PathBuf {
        let exe = std::env::current_exe().unwrap();
        let dir = exe.parent().unwrap().parent().unwrap().join("examples");
        let name = format!("sharex_settings_action_probe{}", std::env::consts::EXE_SUFFIX);
        let p = dir.join(name);
        assert!(p.is_file(), "{} is missing: cargo test builds it (cargo build -p sharex-settings --example sharex_settings_action_probe)", p.display());
        p
    }

    /// Files under `dir`, relative, sorted by ordinal (as the harness lists them).
    fn files(dir: &Path) -> Vec<String> {
        fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(base, &p, out);
                } else {
                    out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, dir, &mut out);
        // `StringComparer.Ordinal`: UTF-16 code units.
        out.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        out
    }

    /// `ExternalProgram.Run` on every case, as the harness ran ShareX's.
    #[test]
    fn runs_as_sharex() {
        let cases = fixture("cases.json");
        let want = fixture("results-sharex.json");
        let probe = probe();
        let probe_str = probe.to_string_lossy().into_owned();
        let scratch = std::env::temp_dir().join(format!("sharexrs-actions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        // SAFETY: this is the only test in this binary that touches the
        // environment, and no other thread reads it while it's set.
        unsafe { std::env::set_var("SHAREXRS_ACTION_PROBE_DIR", probe.parent().unwrap()) };
        let mut failures = Vec::new();
        for (c, w) in cases["runs"].as_array().unwrap().iter().zip(want["runs"].as_array().unwrap()) {
            let name = s(&c["name"]).unwrap();
            let dir = scratch.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            for f in c["files"].as_array().unwrap() {
                let file = dir.join(s(f).unwrap());
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(&file, "input").unwrap();
            }
            let log = scratch.join(format!("{name}.log"));
            // SAFETY: as above.
            unsafe { std::env::set_var("SHAREXRS_ACTION_PROBE_LOG", &log) };
            let dir_str = dir.to_string_lossy().into_owned();
            let probe_name = probe.file_name().unwrap().to_string_lossy().into_owned();
            let expand = |t: &str| t.replace("<dir>", &dir_str).replace("<probename>", &probe_name).replace("<probe>", &probe_str);
            let mut program = ExternalProgram { IsActive: true, Name: Some(name.into()), Path: Some(probe_str.clone()), ..Default::default() };
            let p = c["program"].as_object().unwrap();
            let text = |k: &str| p[k].as_str().map(str::to_owned);
            if p.contains_key("Path") {
                program.Path = text("Path").map(|t| expand(&t));
            }
            if p.contains_key("Args") {
                program.Args = text("Args");
            }
            if p.contains_key("OutputExtension") {
                program.OutputExtension = text("OutputExtension");
            }
            if p.contains_key("Extensions") {
                program.Extensions = text("Extensions");
            }
            program.HiddenWindow = p.get("HiddenWindow").and_then(Value::as_bool).unwrap_or(false);
            program.DeleteInputFile = p.get("DeleteInputFile").and_then(Value::as_bool).unwrap_or(false);
            let started = std::time::Instant::now();
            let result = program.run(&expand(s(&c["input"]).unwrap()));
            let elapsed = started.elapsed().as_millis() as u64;
            delete_pending_input_file(result.pending_input_file.as_deref());
            let normalize = |t: &str| t.replace(&dir_str, "<dir>").replace(&probe_str, "<probe>");
            let log_lines: Vec<Value> = std::fs::read_to_string(&log)
                .unwrap_or_default()
                .lines()
                .map(|l| {
                    let v: Value = serde_json::from_str(l).unwrap();
                    let args: Vec<String> = v["args"].as_array().unwrap().iter().map(|a| normalize(a.as_str().unwrap())).collect();
                    json!({ "cmdline": v["cmdline"].as_str().map(normalize), "args": args })
                })
                .collect();
            let got = json!({
                "name": name,
                "result": result.path.as_deref().map(normalize),
                "files": files(&dir),
                "log": log_lines,
            });
            let mut w = w.clone();
            let sharex_elapsed = w.as_object_mut().unwrap().remove("elapsed_ms").and_then(|e| e.as_u64()).unwrap_or(0);
            if got != w {
                failures.push(format!("{name}:\n  ShareX:   {w}\n  ShareXrs: {got}"));
            }
            // Both wait for the program to exit.
            if sharex_elapsed >= 300 && elapsed < 300 {
                failures.push(format!("{name}: returned after {elapsed} ms, ShareX after {sharex_elapsed} ms"));
            }
        }
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(failures.is_empty(), "{} of {} runs differ:\n{}", failures.len(), cases["runs"].as_array().unwrap().len(), failures.join("\n"));
    }
}
