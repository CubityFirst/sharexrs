//! ShareXrs's update check against what ShareX's own `GitHubUpdateChecker`
//! decided on the same canned responses (`tools/update-harness`, run on
//! Windows x64; `fixtures/results-sharex.json`). The responses are served
//! by the in-process mock, so this needs neither .NET nor Python.

mod mock;

use std::io::Read;

use mock::{Mock, Response};
use serde_json::{Value, json};
use sharex_update::checker::{Arch, GitHubUpdateChecker, InstallFlow};
use sharex_update::version::{AppVersion, DotNetVersion};

fn cases() -> Vec<Value> {
    let mut text = String::new();
    flate2::read::GzDecoder::new(&include_bytes!("fixtures/cases.json.gz")[..]).read_to_string(&mut text).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    v["cases"].as_array().unwrap().clone()
}

fn sharex_results() -> Vec<Value> {
    serde_json::from_str(include_str!("fixtures/results-sharex.json")).unwrap()
}

fn canned(r: &Value) -> Response {
    let body = r["body"].as_str().unwrap_or("").as_bytes().to_vec();
    let reason: &'static str = Box::leak(r["reason"].as_str().unwrap_or("").to_owned().into_boxed_str());
    let mut response = Response { status: r["status"].as_u64().unwrap_or(200) as u16, reason, body, ..Default::default() };
    for h in r["headers"].as_array().into_iter().flatten() {
        response.headers.push((h[0].as_str().unwrap().to_owned(), h[1].as_str().unwrap().to_owned()));
    }
    response
}

/// Cases whose answer depends on the machine, not on the port.
fn machine_dependent(id: &str) -> bool {
    // A `+02:00` date becomes local time while a `Z` one stays UTC, and
    // `DateTime.CompareTo` compares the clock times: the order depends on
    // the time zone.
    id == "prerelease_channel_offsets"
}

#[test]
fn update_check_matches_sharex() {
    let expected: Vec<Value> = sharex_results();
    let mut compared = 0;
    for case in cases().iter().filter(|c| c["kind"] == "check") {
        let id = case["id"].as_str().unwrap();
        if case.get("addition").is_some() || machine_dependent(id) {
            continue;
        }
        let mock = Mock::start();
        let response = canned(&case["responses"][0]);
        mock.route("/repos/CubityFirst/sharexrs/releases/latest", response.clone());
        mock.route("/repos/CubityFirst/sharexrs/releases", response);
        let portable = case["portable"].as_bool().unwrap();
        let mut c = GitHubUpdateChecker::new(
            "CubityFirst",
            "sharexrs",
            if portable { InstallFlow::WindowsPortable } else { InstallFlow::WindowsSetup },
        );
        c.api_url = mock.base.clone();
        c.arch = Arch::X64;
        c.include_pre_release = case["include_pre_release"].as_bool().unwrap();
        c.ignore_revision = case["ignore_revision"].as_bool().unwrap();
        c.is_dev = case["is_dev"].as_bool().unwrap();
        c.current_version = Some(AppVersion::new(DotNetVersion::parse(case["current"].as_str().unwrap()).unwrap()));
        c.check_update();
        let file_name = if c.asset_name().is_none_or(str::is_empty) && c.download_url.is_none() {
            "<NullReferenceException>".to_owned()
        } else {
            c.file_name()
        };
        let ours = json!({
            "status": format!("{:?}", c.status),
            "current_version": c.current_version.as_ref().map(ToString::to_string),
            "latest_version": c.latest_version.as_ref().map(ToString::to_string),
            "download_url": c.download_url,
            "file_name": file_name,
            "is_pre_release": c.is_pre_release,
        });
        let theirs = expected.iter().find(|r| r["id"] == id).unwrap();
        for key in ["status", "current_version", "latest_version", "download_url", "file_name", "is_pre_release"] {
            assert_eq!(ours[key], theirs[key], "{id}: {key}");
        }
        compared += 1;
    }
    assert!(compared > 60, "{compared}");
}

#[test]
fn versions_match_sharex() {
    let case = cases().into_iter().find(|c| c["kind"] == "version").unwrap();
    let theirs = sharex_results().into_iter().find(|r| r["id"] == "versions").unwrap();
    for (input, expected) in case["parse"].as_array().unwrap().iter().zip(theirs["parse"].as_array().unwrap()) {
        let ours = DotNetVersion::parse(input.as_str().unwrap()).map(|v| v.to_string());
        match expected.as_str().unwrap() {
            e if e.starts_with('<') => assert!(ours.is_err(), "{input}: {ours:?}"),
            e => assert_eq!(ours.as_deref(), Ok(e), "{input}"),
        }
    }
    for (pair, expected) in case["compare"].as_array().unwrap().iter().zip(theirs["compare"].as_array().unwrap()) {
        let a = DotNetVersion::parse(pair[0].as_str().unwrap()).unwrap();
        let b = DotNetVersion::parse(pair[1].as_str().unwrap()).unwrap();
        assert_eq!(a.compare_normalized(&b, pair[2].as_bool().unwrap()) as i64, expected.as_i64().unwrap(), "{pair}");
    }
}
