//! The ShareXrs side of `tools/update-harness`: runs every case in
//! `cases.json` through `sharex-update` against the local mock server and
//! writes the same results file as the .NET harness (ShareX's own code).
//!
//!   cargo run -p sharex-update --example update_harness -- <cases.json> <results.json> <port> <work dir>
//!
//! Every request is rewritten to `http://127.0.0.1:<port>/__/<scheme>/<host><path>`
//! before it's sent (`http::set_url_rewriter`), so nothing leaves the machine.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use sharex_update::checker::{Arch, GitHubUpdateChecker, InstallFlow};
use sharex_update::downloader::{DownloadEvent, FileDownloader};
use sharex_update::text::format_fixed;
use sharex_update::version::{AppVersion, DotNetVersion};
use sharex_uploaders::HttpMethod;
use sharex_uploaders::http::{self, HttpRequest, RequestOptions, UrlRewriter};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (cases_path, results_path, port, work) = (&args[1], &args[2], args[3].parse::<u16>().expect("port"), Path::new(&args[4]));
    std::fs::create_dir_all(work).expect("work dir");
    let base = format!("http://127.0.0.1:{port}");
    let prefix = format!("{base}/__/");
    let forward_base = base.clone();
    http::set_url_rewriter(Some(UrlRewriter {
        forward: Arc::new(move |url: &str| {
            if url.starts_with(&forward_base) {
                return url.to_owned();
            }
            let (scheme, rest) = url.split_once("://").expect("absolute URL");
            format!("{forward_base}/__/{scheme}/{rest}")
        }),
        back: Arc::new(move |url: &str| match url.strip_prefix(&prefix) {
            Some(rest) => {
                let (scheme, rest) = rest.split_once('/').unwrap_or((rest, ""));
                format!("{scheme}://{rest}")
            }
            None => url.to_owned(),
        }),
    }));

    let cases: Value = serde_json::from_str(&std::fs::read_to_string(cases_path).expect("cases")).expect("cases JSON");
    let mut results = Vec::new();
    for case in cases["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("id");
        let mut result = match case["kind"].as_str() {
            Some("check") => check(case, &base),
            Some("download") => download(case, &base, work),
            Some("version") => versions(case),
            other => panic!("unknown kind {other:?}"),
        };
        result["id"] = json!(id);
        println!("{id}: {}", result["status"].as_str().or(result["exception"].as_str()).unwrap_or("ok"));
        results.push(result);
    }
    std::fs::write(results_path, serde_json::to_string_pretty(&results).expect("results")).expect("write results");
}

fn control(method: HttpMethod, url: String, body: Option<String>) -> String {
    let content = body.map(|b| http::Content::bytes(b.into_bytes(), Some("application/json")));
    let response = http::send(HttpRequest { method, url, headers: Vec::new(), content }, RequestOptions::default()).expect("mock control");
    response.response_text.unwrap_or_default()
}

fn reset(base: &str, responses: &Value) {
    control(HttpMethod::POST, format!("{base}/__mock/reset"), Some(json!({ "responses": responses }).to_string()));
}

/// The recorded requests without the user agent (ShareXrs/<version> vs ShareX/<version>).
fn requests(base: &str) -> Value {
    let log: Value = serde_json::from_str(&control(HttpMethod::GET, format!("{base}/__mock/log"), None)).expect("log");
    let list: Vec<Value> = log
        .as_array()
        .expect("log")
        .iter()
        .map(|r| {
            let headers: Vec<Value> = r["headers"]
                .as_array()
                .expect("headers")
                .iter()
                .filter(|h| !h[0].as_str().unwrap_or("").eq_ignore_ascii_case("User-Agent"))
                .map(|h| json!([h[0].as_str().unwrap_or("").to_ascii_lowercase(), h[1]]))
                .collect();
            json!({ "method": r["method"], "target": r["target"], "headers": headers })
        })
        .collect();
    Value::Array(list)
}

fn check(case: &Value, base: &str) -> Value {
    reset(base, &case["responses"]);
    let flow = if case["portable"].as_bool().unwrap_or(false) { InstallFlow::WindowsPortable } else { InstallFlow::WindowsSetup };
    let mut checker = GitHubUpdateChecker::new("CubityFirst", "sharexrs", flow);
    checker.arch = Arch::current();
    checker.include_pre_release = case["include_pre_release"].as_bool().unwrap_or(false);
    checker.ignore_revision = case["ignore_revision"].as_bool().unwrap_or(false);
    checker.is_dev = case["is_dev"].as_bool().unwrap_or(false);
    checker.current_version = Some(AppVersion::new(DotNetVersion::parse(case["current"].as_str().expect("current")).expect("current")));
    checker.check_update();
    let file_name = if checker.asset_name().is_none_or(str::is_empty) && checker.download_url.is_none() {
        "<NullReferenceException>".to_owned()
    } else {
        checker.file_name()
    };
    json!({
        "status": format!("{:?}", checker.status),
        "current_version": checker.current_version.as_ref().map(ToString::to_string),
        "latest_version": checker.latest_version.as_ref().map(ToString::to_string),
        "download_url": checker.download_url,
        "file_name": file_name,
        "is_pre_release": checker.is_pre_release,
        "requests": requests(base),
        "error": checker.error,
    })
}

fn download(case: &Value, base: &str, work: &Path) -> Value {
    reset(base, &case["responses"]);
    let path = work.join(format!("sharexrs-{}.bin", case["id"].as_str().expect("id")));
    let _ = std::fs::remove_file(&path);
    let mut downloader = FileDownloader::new(case["url"].as_str().expect("url"), &path);
    downloader.accept_header = case["accept"].as_str().unwrap_or("").to_owned();
    let size_events = AtomicUsize::new(0);
    let events = |e: DownloadEvent| {
        if e == DownloadEvent::FileSizeReceived {
            size_events.fetch_add(1, Ordering::SeqCst);
        }
    };
    let mut result = json!({});
    match downloader.start_download(&events) {
        Ok(returned) => result["returned"] = json!(returned),
        Err(e) => result["exception"] = json!(e),
    }
    let p = downloader.progress();
    result["file_size"] = json!(p.file_size);
    result["downloaded_size"] = json!(p.downloaded_size);
    result["percentage"] = json!(format_fixed(p.percentage(), 1));
    result["size_events"] = json!(size_events.load(Ordering::SeqCst));
    result["file_exists"] = json!(path.exists());
    result["file_sha256"] = json!(path.exists().then(|| sharex_update::install::sha256_file(&path).expect("hash")));
    result["requests"] = requests(base);
    result
}

fn versions(case: &Value) -> Value {
    let parsed: Vec<Value> = case["parse"]
        .as_array()
        .expect("parse")
        .iter()
        .map(|s| match DotNetVersion::parse(s.as_str().expect("version")) {
            Ok(v) => json!(v.to_string()),
            Err(e) => {
                let kind = if e.starts_with("Version string portion") {
                    "ArgumentException"
                } else if e.starts_with("Input string") {
                    "FormatException"
                } else if e.starts_with("Value was either too large") {
                    "OverflowException"
                } else {
                    "ArgumentOutOfRangeException"
                };
                json!(format!("<{kind}>"))
            }
        })
        .collect();
    let compared: Vec<Value> = case["compare"]
        .as_array()
        .expect("compare")
        .iter()
        .map(|p| {
            let a = DotNetVersion::parse(p[0].as_str().expect("a")).expect("a");
            let b = DotNetVersion::parse(p[1].as_str().expect("b")).expect("b");
            json!(a.compare_normalized(&b, p[2].as_bool().expect("ignore")) as i32)
        })
        .collect();
    json!({ "parse": parsed, "compare": compared })
}
