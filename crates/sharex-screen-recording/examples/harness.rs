//! The ShareXrs side of `tools/recording-harness`: runs every case of
//! `cases.json` through this crate (FFmpeg replaced by a capture, as the .NET
//! harness replaces it with Harmony), compares with what ShareX's DLLs gave
//! (`sharex.json`) and writes `<work>/rust.json` and `<work>/report.md`.
//!
//!   cargo run -p sharex-screen-recording --example harness -- <cases.json> <sharex.json> <work dir>

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use sharex_screen_recording::{DesktopScreen, FFmpegCliManager, FFmpegOptions, ScreenRecorder, ScreenRecordingOptions};
use sharex_settings::types::Rectangle;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let [cases, sharex, work] = &argv[..] else {
        eprintln!("usage: harness <cases.json> <sharex.json> <work dir>");
        std::process::exit(2);
    };
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(cases).unwrap()).unwrap();
    let sharex: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(sharex).unwrap()).unwrap();
    std::fs::create_dir_all(work).unwrap();

    let mut ours = Vec::new();
    let mut report = String::from("| case | result |\n|---|---|\n");
    let mut failures = 0;
    for (case, theirs) in cases.iter().zip(&sharex) {
        let mine = run(case, work);
        let name = case["name"].as_str().unwrap();
        let diffs = compare(theirs, &mine);
        if diffs.is_empty() {
            report.push_str(&format!("| {name} | identical |\n"));
        } else {
            failures += 1;
            report.push_str(&format!("| {name} | **differs**: {} |\n", diffs.join("; ").replace('|', "\\|")));
            eprintln!("DIFF {name}:");
            for d in &diffs {
                eprintln!("  {d}");
            }
        }
        ours.push(mine);
    }
    assert_eq!(cases.len(), sharex.len(), "sharex.json has a different number of cases");
    let summary = format!("{} cases, {} identical, {} differ\n", cases.len(), cases.len() - failures, failures);
    report = format!("# Recording harness\n\n{summary}\n{report}");
    std::fs::write(format!("{work}/rust.json"), serde_json::to_string_pretty(&ours).unwrap()).unwrap();
    std::fs::write(format!("{work}/report.md"), report).unwrap();
    print!("{summary}");
    std::process::exit(if failures == 0 { 0 } else { 1 });
}

/// The fields ShareX's result has, each compared as JSON; an `error` only
/// by presence (the messages are .NET's).
fn compare(theirs: &Value, mine: &Value) -> Vec<String> {
    let mut diffs = Vec::new();
    let keys: Vec<&String> = theirs.as_object().unwrap().keys().chain(mine.as_object().unwrap().keys()).collect();
    let mut seen = std::collections::HashSet::new();
    for k in keys {
        if !seen.insert(k) {
            continue;
        }
        let (a, b) = (theirs.get(k), mine.get(k));
        if k == "error" {
            if a.is_some() != b.is_some() {
                diffs.push(format!("error: ShareX {a:?}, ShareXrs {b:?}"));
            }
        } else if a != b {
            diffs.push(format!("{k}: ShareX {}, ShareXrs {}", show(a), show(b)));
        }
    }
    diffs
}

fn show(v: Option<&Value>) -> String {
    v.map_or("(none)".into(), |v| v.to_string())
}

fn options(case: &Value) -> ScreenRecordingOptions {
    let o = &case["options"];
    let area = o.get("area").and_then(Value::as_array);
    ScreenRecordingOptions {
        is_recording: o["is_recording"].as_bool().unwrap_or(false),
        is_lossless: o["is_lossless"].as_bool().unwrap_or(false),
        input_path: o["input_path"].as_str().map(str::to_string),
        output_path: o["output_path"].as_str().map(str::to_string),
        fps: o["fps"].as_i64().unwrap_or(0) as i32,
        capture_area: area.map_or(Rectangle::default(), |a| {
            let n = |i: usize| a[i].as_i64().unwrap() as i32;
            Rectangle::new(n(0), n(1), n(2), n(3))
        }),
        duration: o["duration"].as_f64().unwrap_or(0.0) as f32,
        draw_cursor: o["draw_cursor"].as_bool().unwrap_or(false),
        ffmpeg: serde_json::from_value::<FFmpegOptions>(o.get("ffmpeg").cloned().unwrap_or(json!({}))).unwrap(),
    }
}

fn screens(case: &Value) -> Vec<DesktopScreen> {
    case.get("screens").and_then(Value::as_array).map_or(Vec::new(), |list| {
        list.iter()
            .map(|s| {
                let b = s["bounds"].as_array().unwrap();
                let n = |i: usize| b[i].as_i64().unwrap() as i32;
                DesktopScreen { bounds: Rectangle::new(n(0), n(1), n(2), n(3)), primary: s["primary"].as_bool().unwrap() }
            })
            .collect()
    })
}

/// What a run captured: the argument strings, the list files' contents.
#[derive(Default)]
struct Capture {
    runs: Vec<String>,
    lists: Vec<Option<String>>,
}

fn capture_runs(ffmpeg: &FFmpegCliManager, canned: Option<String>) -> Arc<Mutex<Capture>> {
    let cap = Arc::new(Mutex::new(Capture::default()));
    let c = cap.clone();
    ffmpeg.set_run_override(move |args| {
        let mut c = c.lock().unwrap();
        c.runs.push(args.to_string());
        if args.starts_with("-f concat")
            && let Some(i) = args.find("-i \"")
        {
            let rest = &args[i + 4..];
            let list = &rest[..rest.find('"').unwrap()];
            c.lists.push(std::fs::read_to_string(list).ok());
        }
        // ShareX appends the canned text to Output as is; split it the way
        // the output reader would and the lines come back the same.
        (0, canned.iter().flat_map(|t| t.split("\r\n").flat_map(|l| l.split(['\r', '\n']))).map(str::to_string).collect())
    });
    cap
}

fn run(case: &Value, work: &str) -> Value {
    let name = case["name"].clone();
    let kind = case["kind"].as_str().unwrap();
    let mut r = json!({ "name": name });
    let mut runs: Vec<String> = Vec::new();
    let mut registry: Vec<Value> = Vec::new();
    let mut lists: Vec<Option<String>> = Vec::new();
    let new_recorder = |o: ScreenRecordingOptions| ScreenRecorder::new(o, screens(case), "ffmpeg.exe");
    match kind {
        "record" => {
            let o = options(case);
            let f = o.ffmpeg.clone();
            r["extension"] = json!(f.extension());
            r["even"] = json!(f.is_even_size_required());
            r["source_selected"] = json!(f.is_source_selected());
            match new_recorder(o.clone()) {
                Err(_) => r["error"] = json!(true),
                Ok(rec) => {
                    let cap = capture_runs(rec.ffmpeg(), None);
                    let reg = Arc::new(Mutex::new(Vec::new()));
                    let reg2 = reg.clone();
                    rec.set_registry_writer(move |path, values| {
                        for (n, v) in values {
                            reg2.lock().unwrap().push(json!([path, n, v]));
                        }
                    });
                    // ShareX throws where the builder gives null (no source).
                    if o.get_ffmpeg_commands(&screens(case)).is_none() {
                        r["error"] = json!(true);
                    }
                    rec.start_recording();
                    runs = cap.lock().unwrap().runs.clone();
                    registry = reg.lock().unwrap().clone();
                }
            }
        }
        "commands" => {
            let o = options(case);
            registry = o
                .screen_capture_recorder_registry()
                .iter()
                .map(|(n, v)| json!([sharex_screen_recording::args::SCREEN_CAPTURE_RECORDER_REGISTRY_PATH, n, v]))
                .collect();
            match o.get_ffmpeg_commands(&screens(case)) {
                Some(s) => r["result"] = json!(s),
                None => r["error"] = json!(true),
            }
        }
        "custom_args" => r["result"] = json!(options(case).get_ffmpeg_args(true, &screens(case))),
        "encode_video" | "encode_gif" => {
            let rec = new_recorder(options(case)).unwrap();
            let cap = capture_runs(rec.ffmpeg(), None);
            let (input, output) = (case["input"].as_str().unwrap(), case["output"].as_str().unwrap());
            if kind == "encode_video" {
                rec.ffmpeg_encode_video(input, output);
            } else {
                rec.ffmpeg_encode_as_gif(input, output);
            }
            runs = cap.lock().unwrap().runs.clone();
        }
        "concat" => {
            let ffmpeg = FFmpegCliManager::new("ffmpeg.exe");
            let cap = capture_runs(&ffmpeg, None);
            // `Path.Combine`: a separator between, nothing normalised.
            let output = format!("{work}\\{}", case["output"].as_str().unwrap());
            let inputs: Vec<&str> = case["inputs"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
            ffmpeg.concatenate_videos(&inputs, &output, false);
            r["list_file"] = json!(format!("{output}.txt"));
            runs = cap.lock().unwrap().runs.clone();
            lists = cap.lock().unwrap().lists.clone();
        }
        "dshow" => {
            let ffmpeg = FFmpegCliManager::new("ffmpeg.exe");
            let cap = capture_runs(&ffmpeg, case["output"].as_str().map(str::to_string));
            let d = ffmpeg.get_direct_show_devices();
            r["video"] = json!(d.video_devices);
            r["audio"] = json!(d.audio_devices);
            runs = cap.lock().unwrap().runs.clone();
        }
        "progress" => {
            let rec = new_recorder(options(case)).unwrap();
            let ffmpeg = rec.ffmpeg().clone();
            ffmpeg.set_track_encode_progress(case["track"].as_bool().unwrap_or(true));
            let events = Arc::new(Mutex::new(Vec::<String>::new()));
            let e = events.clone();
            rec.on_recording_started(move || e.lock().unwrap().push("started".into()));
            let e = events.clone();
            rec.on_encoding_progress(move |p| e.lock().unwrap().push(format!("progress {p}")));
            // After ScreenRecorder's handler, as the .NET harness subscribes.
            let e = events.clone();
            ffmpeg.on_encode_progress(move |p| e.lock().unwrap().push(format!("raw {p}")));
            for line in case["lines"].as_array().unwrap() {
                ffmpeg.receive_line(line.as_str().unwrap());
            }
            r["events"] = json!(*events.lock().unwrap());
            let state = ffmpeg.state();
            r["output"] = json!(state.output);
            r["duration_ticks"] = json!(state.video_duration);
        }
        other => panic!("unknown kind {other}"),
    }
    r["runs"] = json!(runs);
    r["registry"] = json!(registry);
    if !lists.is_empty() {
        r["lists"] = json!(lists);
    }
    r
}
