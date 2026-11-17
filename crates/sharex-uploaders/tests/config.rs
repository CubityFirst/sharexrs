//! `UploadersConfig.json` files ShareX wrote survive a load and save byte
//! for byte, and a new config is written as a fresh ShareX profile's.
//!
//! `fixtures/UploadersConfig.fresh.json`: the reference build's first run in
//! an empty folder. `fixtures/UploadersConfig.harness.json`: written by
//! ShareX's own `UploadersConfig.Save` in the validation harness
//! (`tools/uploaders-harness`), with custom uploaders and settings filled in
//! (made-up values, no DPAPI).

use std::fs;
use std::path::{Path, PathBuf};

use sharex_settings::settings_base::{load_file, to_json};
use sharex_uploaders::UploadersConfig;
use sharex_uploaders::enums::{CustomUploaderBody, HttpMethod};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn expected(name: &str) -> String {
    let text = fs::read_to_string(fixture(name)).unwrap();
    if cfg!(windows) { text } else { text.replace("\r\n", "\n") }
}

fn assert_round_trip(name: &str) -> UploadersConfig {
    let config: UploadersConfig = load_file(&fixture(name)).unwrap();
    let written = to_json(&config);
    let want = expected(name);
    if written != want {
        let (w, e): (Vec<_>, Vec<_>) = (written.lines().collect(), want.lines().collect());
        let first = w.iter().zip(&e).position(|(a, b)| a != b).unwrap_or(w.len().min(e.len()));
        panic!("{name} differs at line {}:\n  wrote:    {:?}\n  expected: {:?}", first + 1, w.get(first), e.get(first));
    }
    config
}

#[test]
fn fresh_config_round_trips_byte_for_byte() {
    let config = assert_round_trip("UploadersConfig.fresh.json");
    assert!(config.extra.is_empty(), "unknown properties: {:?}", config.extra.keys().collect::<Vec<_>>());
}

#[test]
fn new_config_is_written_as_sharex_writes_a_fresh_one() {
    let config = UploadersConfig { ApplicationVersion: Some("22.4.0".into()), ..Default::default() };
    assert_eq!(to_json(&config), expected("UploadersConfig.fresh.json"));
}

#[test]
fn harness_config_round_trips_byte_for_byte() {
    if !fixture("UploadersConfig.harness.json").exists() {
        return;
    }
    let config = assert_round_trip("UploadersConfig.harness.json");
    assert!(config.extra.is_empty(), "unknown properties: {:?}", config.extra.keys().collect::<Vec<_>>());
    let list = &config.CustomUploadersList.0;
    assert!(!list.is_empty());
    assert!(list.iter().any(|i| i.body == CustomUploaderBody::JSON));
    assert!(list.iter().any(|i| i.request_method == HttpMethod::PUT));
}

#[test]
fn saves_and_loads_with_sharex_backups() {
    let dir = std::env::temp_dir().join(format!("sharexrs-uploaders-config-{}", std::process::id()));
    let backup = dir.join("Backup");
    std::fs::create_dir_all(&backup).unwrap();
    let path = dir.join("UploadersConfig.json");
    let mut config: UploadersConfig = load_file(&fixture("UploadersConfig.harness.json")).unwrap();
    config.save(&path, &backup).unwrap();
    let loaded = UploadersConfig::load(Some(&path), Some(&backup), true);
    assert_eq!(loaded.source.as_deref(), Some(path.as_path()));
    // Loading runs ShareX's backward compatibility tasks (the 13.1.0 uploader is migrated).
    let mut migrated = config.clone();
    migrated.backward_compatibility_tasks();
    assert_ne!(migrated.CustomUploadersList, config.CustomUploadersList);
    assert_eq!(loaded.settings.CustomUploadersList, migrated.CustomUploadersList);
    assert_eq!(loaded.settings.ApplicationVersion.as_deref(), Some("22.4.0"));
    // A missing file falls back to the backup the save left.
    std::fs::remove_file(&path).unwrap();
    let fallback = UploadersConfig::load(Some(&path), Some(&backup), true);
    assert!(fallback.source.as_ref().is_some_and(|s| s.starts_with(&backup)), "loaded from {:?}", fallback.source);
    let _ = std::fs::remove_dir_all(&dir);
}
