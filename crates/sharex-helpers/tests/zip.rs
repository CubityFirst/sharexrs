//! `ZipManager` against ShareX's own (`tools/settings-harness`): the
//! archives in `fixtures/zip` extracted as ShareX extracted them
//! (`results-sharex.json`), and ShareXrs's settings export laid out as
//! ShareX's (`export-sharex.json`).
//!
//! ShareX's results come from its Windows build (backslashes are separators
//! there, drive letters root a path), so the comparisons run on Windows.
// Elsewhere only the fixture writer runs: the comparisons' helpers go unused.
#![cfg_attr(not(windows), allow(dead_code, unused_imports))]

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate, TimeZone};
use serde_json::Value;
use sharex_helpers::zip_manager::{self, ExtractError, ZipEntryInfo};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("zip")
}

fn read_json(name: &str) -> Option<Value> {
    let text = fs::read_to_string(fixtures().join(name)).ok()?;
    serde_json::from_str(&text).ok()
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The history file the harness exports, with its time stamp.
fn history_file(dir: &Path) -> PathBuf {
    let path = dir.join("History.db");
    let mut data = b"SQLite format 3\0".to_vec();
    data.extend([0u8; 48]);
    fs::write(&path, data).unwrap();
    let local = NaiveDate::from_ymd_opt(2026, 11, 19).unwrap().and_hms_opt(10, 20, 30).unwrap();
    let when = Local.from_local_datetime(&local).earliest().unwrap();
    fs::File::options().write(true).open(&path).unwrap().set_modified(when.into()).unwrap();
    path
}

/// `SettingManager.Export`'s entries, as the harness gives ShareX's.
fn export_entries(dir: &Path) -> Vec<ZipEntryInfo> {
    vec![
        ZipEntryInfo::data(b"{\r\n  \"ShowTray\": true\r\n}".to_vec(), "ApplicationConfig.json"),
        ZipEntryInfo::data(b"{\r\n  \"ImgurAccountType\": \"Anonymous\"\r\n}".to_vec(), "UploadersConfig.json"),
        ZipEntryInfo::data(b"{\r\n  \"Hotkeys\": []\r\n}".to_vec(), "HotkeysConfig.json"),
        ZipEntryInfo::file(history_file(dir)),
        ZipEntryInfo::file(dir.join("Missing.db")),
    ]
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sharexrs-zip-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes `fixtures/zip/sharexrs-export.sxb` for the harness (only with
/// `SHAREXRS_WRITE_FIXTURES=1`, from `tools/settings-harness/run.sh`).
#[test]
fn write_sharexrs_export() {
    if std::env::var_os("SHAREXRS_WRITE_FIXTURES").is_none() {
        return;
    }
    let dir = scratch("export-fixture");
    zip_manager::compress(&fixtures().join("sharexrs-export.sxb"), &export_entries(&dir)).unwrap();
    let _ = fs::remove_dir_all(&dir);
}

/// Every file and folder under `root` as the harness lists them.
fn tree(root: &Path) -> Vec<Value> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Value)>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let path = e.path();
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            if path.is_dir() {
                out.push((rel.clone(), serde_json::json!({ "path": format!("{rel}/") })));
                walk(root, &path, out);
            } else {
                let mtime = chrono::DateTime::<Local>::from(fs::metadata(&path).unwrap().modified().unwrap()).format("%Y-%m-%d %H:%M:%S").to_string();
                out.push((rel.clone(), serde_json::json!({ "path": rel, "data": base64(&fs::read(&path).unwrap()), "mtime": mtime })));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, v)| v).collect()
}

#[cfg(windows)]
#[test]
fn extracts_as_sharex() {
    let Some(results) = read_json("results-sharex.json") else {
        eprintln!("no results-sharex.json: run tools/settings-harness/run.sh");
        return;
    };
    let cases = read_json("cases.json").unwrap();
    let mut compared = 0;
    for c in cases["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let r = results["results"].as_array().unwrap().iter().find(|r| r["name"] == name).unwrap_or_else(|| panic!("{name}: no ShareX result"));
        if r.get("skipped").is_some() {
            continue;
        }
        let root = scratch(name);
        let dest = root.join("dest");
        let filter = |_: &str, short: &str| sharex_helpers::file_helpers::check_extension(short, &["json", "xml"]);
        let f: Option<zip_manager::EntryFilter> = if c["filter"] == "json-xml" { Some(&filter) } else { None };
        let got = zip_manager::extract(&fixtures().join(c["archive"].as_str().unwrap()), &dest, c["retain"].as_bool().unwrap(), f, c["max"].as_u64().unwrap());
        assert_eq!(got.is_ok(), r["ok"].as_bool().unwrap(), "{name}: {got:?} vs {}", r["message"]);
        if let Err(ExtractError::TooLarge(..)) = &got {
            assert_eq!(got.as_ref().unwrap_err().to_string(), r["message"].as_str().unwrap(), "{name}");
        }
        assert_eq!(Value::Array(tree(&root)), r["tree"], "{name}");
        let _ = fs::remove_dir_all(&root);
        compared += 1;
    }
    assert!(compared >= 8, "only {compared} cases compared");
}

/// The fields of an archive listing that must agree (the time stamps of
/// the settings entries are "now" on both sides).
fn layout(list: &Value) -> Vec<(String, u64, u64, String)> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|e| (e["name"].as_str().unwrap().to_owned(), e["length"].as_u64().unwrap(), e["crc32"].as_u64().unwrap(), e["data"].as_str().unwrap().to_owned()))
        .collect()
}

#[cfg(windows)]
#[test]
fn exports_as_sharex() {
    let Some(export) = read_json("export-sharex.json") else {
        eprintln!("no export-sharex.json: run tools/settings-harness/run.sh");
        return;
    };
    let sharex = layout(&export["sharex"]);
    let sharexrs = layout(export.get("sharexrs").expect("the harness read ShareXrs's export"));
    assert_eq!(sharexrs, sharex);
    // The history file keeps its own time; the settings files get the time of the export.
    let history_time = export["historyTime"].as_str().unwrap();
    for list in [&export["sharex"], &export["sharexrs"]] {
        let entries = list.as_array().unwrap();
        assert_eq!(entries[3]["mtime"], history_time);
        assert_ne!(entries[0]["mtime"], "1980-01-01 00:00:00");
    }
}
