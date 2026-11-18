//! `--compare`: ShareX's outputs against ShareXrs's, case by case.
//!
//! - GIF, PCM, composited camera frames, NV12 encoder input: byte for byte.
//! - JSON (encoder, sizes, sample times, GIF progress): value for value.
//! - MP4: the bytes; then the bytes with the container's creation and
//!   modification times zeroed; the box tree; the sample tables and codec
//!   configuration (`stsd`, `stts`, `ctts`, `stss`, `stsz`); then both files
//!   decoded with Media Foundation's software decoders (frame count,
//!   timestamps, durations, the largest pixel and PCM differences).

#[path = "decode.rs"]
mod decode;
#[path = "mp4.rs"]
mod mp4;

use std::fmt::Write as _;
use std::path::Path;

pub fn run(sharex: &Path, ours: &Path, report: Option<&Path>) -> i32 {
    let mut names: Vec<String> = std::fs::read_dir(sharex)
        .expect("sharex dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    let mut md = String::from("| Output | Result |\n|---|---|\n");
    let (mut identical, mut close, mut different) = (0, 0, 0);
    for name in names {
        let (a, b) = (sharex.join(&name), ours.join(&name));
        let ext = name.rsplit('.').next().unwrap_or("");
        let result = if !b.is_file() {
            Err("missing from ShareXrs's output".to_string())
        } else {
            let (x, y) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
            match ext {
                "json" => compare_json(&x, &y).map(|_| Outcome::Identical),
                "mp4" => mp4::compare(&a, &b, &x, &y),
                _ if x == y => Ok(Outcome::Identical),
                _ => Err(first_difference(&x, &y)),
            }
        };
        let line = match &result {
            Ok(Outcome::Identical) => {
                identical += 1;
                "identical".to_string()
            }
            Ok(Outcome::Close(note)) => {
                close += 1;
                note.clone()
            }
            Err(e) => {
                different += 1;
                format!("**differs**: {e}")
            }
        };
        println!("{name}: {line}");
        let _ = writeln!(md, "| `{name}` | {} |", line.replace('\n', "<br>"));
    }
    let summary = format!("{identical} identical, {close} equivalent (see notes), {different} different");
    println!("{summary}");
    if let Some(r) = report {
        let _ = std::fs::write(r, format!("# Native recorder: ShareX vs ShareXrs\n\n{summary}\n\n{md}"));
    }
    if different > 0 { 1 } else { 0 }
}

pub enum Outcome {
    Identical,
    /// Equivalent, with what differs.
    Close(String),
}

fn first_difference(x: &[u8], y: &[u8]) -> String {
    let at = x.iter().zip(y).position(|(a, b)| a != b).unwrap_or(x.len().min(y.len()));
    let diff = x.iter().zip(y).filter(|(a, b)| a != b).count();
    let max = x.iter().zip(y).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap_or(0);
    format!("{} vs {} bytes, first difference at {at}, {diff} bytes differ (largest {max})", x.len(), y.len())
}

fn compare_json(x: &[u8], y: &[u8]) -> Result<(), String> {
    let a: serde_json::Value = serde_json::from_slice(x).map_err(|e| e.to_string())?;
    let b: serde_json::Value = serde_json::from_slice(y).map_err(|e| e.to_string())?;
    if a == b {
        return Ok(());
    }
    let keys: Vec<String> = a
        .as_object()
        .map(|o| o.iter().filter(|(k, v)| b.get(k.as_str()) != Some(v)).map(|(k, v)| format!("{k}: {v} vs {}", b.get(k.as_str()).unwrap_or(&serde_json::Value::Null))).collect())
        .unwrap_or_default();
    Err(keys.join("; "))
}
