//! Analyze image against ShareX's own `AnalyzeImageService` (the tools
//! harness's `--analyze` mode, `tools/tools-harness/AnalyzeCases.cs`), on the
//! same mock server (`tools/uploaders-harness/mock_server.py`).
//!
//!   tools_analyze_harness <analyze_cases.json> <results.json> <port> <work dir>
//!   tools_analyze_harness --compare <analyze_cases.json> <sharex.json> <sharexrs.json> [--report <md>]
//!
//! Every request is rewritten to `http://127.0.0.1:<port>/__/<scheme>/<host><path>`
//! before it's sent (`http::set_url_rewriter`): nothing leaves the machine.

use std::path::Path;
use std::sync::Arc;

use base64::Engine;
use serde_json::{Value, json};
use sharex_tools::analyze_image::{self, AIOptions, AnalyzeInput};
use sharex_uploaders::HttpMethod;
use sharex_uploaders::http::{self, HttpRequest, RequestOptions, UrlRewriter};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--compare") {
        std::process::exit(compare(&args[2..]));
    }
    let (cases_path, results_path, port, work) = (&args[1], &args[2], args[3].parse::<u16>().expect("port"), Path::new(&args[4]));
    let base = format!("http://127.0.0.1:{port}");
    let forward_base = base.clone();
    http::set_url_rewriter(Some(UrlRewriter {
        forward: Arc::new(move |url: &str| {
            if url.starts_with(&forward_base) {
                return url.to_owned();
            }
            let (scheme, rest) = url.split_once("://").expect("absolute URL");
            format!("{forward_base}/__/{scheme}/{rest}")
        }),
        back: Arc::new(|url: &str| url.to_owned()),
    }));
    // As .NET's `Path.GetFullPath` gives it (the error texts name the file).
    let inputs = std::path::absolute(work).expect("work dir").join("analyze-inputs");
    assert!(inputs.join("opaque.png").is_file(), "run the .NET side first: it writes {}", inputs.display());

    let cases: Value = serde_json::from_str(&std::fs::read_to_string(cases_path).expect("cases")).expect("cases JSON");
    let mut results = Vec::new();
    for case in cases["cases"].as_array().expect("cases") {
        reset(&base, &case["responses"]);
        let options = AIOptions::from_json(&case["options"]);
        let mut result = json!({});
        let outcome: Result<(), String> = (|| {
            match case["kind"].as_str() {
                Some("analyze") => {
                    let file = case["file"].as_str().map(|f| inputs.join(f));
                    let data = match case["data_file"].as_str() {
                        Some(f) => Some(std::fs::read(inputs.join(f)).expect("input")),
                        None if case.get("empty_data").is_some() => Some(Vec::new()),
                        None => None,
                    };
                    let input = match (&file, &data) {
                        (Some(path), _) => AnalyzeInput::File(path),
                        (None, Some(d)) => AnalyzeInput::Data(d),
                        _ => return Ok(()),
                    };
                    result["result"] = json!(analyze_image::analyze(&options, input)?);
                }
                Some("test") => {
                    let r = analyze_image::test_connection(&options)?;
                    result["success"] = json!(r.success);
                    result["result"] = json!(r.message);
                }
                Some("models") => result["models"] = json!(analyze_image::load_models(&options)?),
                other => panic!("unknown kind {other:?}"),
            }
            Ok(())
        })();
        if let Err(e) = outcome {
            result["error"] = json!(e);
        }
        result["requests"] = requests(&base);
        result["id"] = case["id"].clone();
        println!("{}: {}", case["id"].as_str().unwrap_or(""), result["error"].as_str().unwrap_or("ok"));
        results.push(result);
    }
    std::fs::write(results_path, serde_json::to_string_pretty(&results).expect("results")).expect("write");
}

fn control(method: HttpMethod, url: String, body: Option<String>) -> String {
    let content = body.map(|b| http::Content::bytes(b.into_bytes(), Some("application/json")));
    let response = http::send(HttpRequest { method, url, headers: Vec::new(), content }, RequestOptions::default()).expect("mock control");
    response.response_text.unwrap_or_default()
}

fn reset(base: &str, responses: &Value) {
    control(HttpMethod::POST, format!("{base}/__mock/reset"), Some(json!({ "responses": responses }).to_string()));
}

fn requests(base: &str) -> Value {
    let log: Value = serde_json::from_str(&control(HttpMethod::GET, format!("{base}/__mock/log"), None)).expect("log");
    Value::Array(
        log.as_array()
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
                json!({ "method": r["method"], "target": r["target"], "headers": headers, "body_base64": r["body_base64"] })
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Comparing.

/// Errors from `System.Text.Json`/Newtonsoft parsing: ShareXrs reports the
/// same failure in its own words.
fn parser_error(theirs: &Value) -> bool {
    matches!(theirs["error_type"].as_str(), Some("JsonException" | "JsonReaderException" | "JsonSerializationException"))
}

/// A request body with its PNG images (base 64) taken out, and the images.
fn split_images(body: &[u8]) -> Option<(String, Vec<Vec<u8>>)> {
    let mut text = String::from_utf8(body.to_vec()).ok()?;
    let mut images = Vec::new();
    for marker in ["data:image/png;base64,", "\"data\":\""] {
        while let Some(start) = text.find(marker) {
            let from = start + marker.len();
            let end = from + text[from..].find('"')?;
            let data = base64::engine::general_purpose::STANDARD.decode(&text[from..end]).ok()?;
            if !data.starts_with(b"\x89PNG") {
                return None;
            }
            images.push(data);
            text.replace_range(start..end, "<png>");
        }
    }
    (!images.is_empty()).then_some((text, images))
}

fn same_pixels(a: &[u8], b: &[u8]) -> bool {
    match (image::load_from_memory(a), image::load_from_memory(b)) {
        (Ok(a), Ok(b)) => a.to_rgba8() == b.to_rgba8(),
        _ => false,
    }
}

enum Verdict {
    Exact,
    PixelExact,
    Differ(String),
}

fn compare_case(theirs: &Value, ours: &Value) -> Verdict {
    let mut pixel = false;
    for key in ["result", "success", "models"] {
        if theirs.get(key) != ours.get(key) {
            return Verdict::Differ(format!("{key}: {} vs {}", theirs[key], ours[key]));
        }
    }
    match (theirs.get("error"), ours.get("error")) {
        (None, None) => {}
        (Some(_), Some(_)) if parser_error(theirs) => {}
        (a, b) if a == b => {}
        (a, b) => return Verdict::Differ(format!("error: {a:?} vs {b:?}")),
    }
    let (a, b) = (theirs["requests"].as_array().cloned().unwrap_or_default(), ours["requests"].as_array().cloned().unwrap_or_default());
    if a.len() != b.len() {
        return Verdict::Differ(format!("{} requests vs {}", a.len(), b.len()));
    }
    for (x, y) in a.iter().zip(&b) {
        for key in ["method", "target"] {
            if x[key] != y[key] {
                return Verdict::Differ(format!("{key}: {} vs {}", x[key], y[key]));
            }
        }
        // Content-Length follows the body, compared below (PNG bodies differ in length).
        let headers = |r: &Value| -> Vec<Value> {
            r["headers"].as_array().into_iter().flatten().filter(|h| h[0] != "content-length").cloned().collect()
        };
        if headers(x) != headers(y) {
            return Verdict::Differ(format!("headers: {} vs {}", x["headers"], y["headers"]));
        }
        let decode = |v: &Value| base64::engine::general_purpose::STANDARD.decode(v.as_str().unwrap_or("")).unwrap_or_default();
        let (bx, by) = (decode(&x["body_base64"]), decode(&y["body_base64"]));
        if bx != by {
            match (split_images(&bx), split_images(&by)) {
                (Some((tx, ix)), Some((ty, iy)))
                    if tx == ty && ix.len() == iy.len() && ix.iter().zip(&iy).all(|(p, q)| same_pixels(p, q)) =>
                {
                    pixel = true
                }
                _ => {
                    let (sx, sy) = (String::from_utf8_lossy(&bx), String::from_utf8_lossy(&by));
                    let at = sx.chars().zip(sy.chars()).position(|(p, q)| p != q).unwrap_or(sx.len().min(sy.len()));
                    let around = |s: &str| s.chars().skip(at.saturating_sub(40)).take(100).collect::<String>();
                    return Verdict::Differ(format!("body at {at}: {:?} vs {:?}", around(&sx), around(&sy)));
                }
            }
        }
    }
    if pixel { Verdict::PixelExact } else { Verdict::Exact }
}

fn compare(args: &[String]) -> i32 {
    let read = |p: &String| -> Value { serde_json::from_str(&std::fs::read_to_string(p).expect("read")).expect("JSON") };
    let cases = read(&args[0]);
    let (theirs, ours) = (read(&args[1]), read(&args[2]));
    let find = |list: &Value, id: &Value| list.as_array().and_then(|l| l.iter().find(|r| r["id"] == *id).cloned());
    let (mut exact, mut pixel, mut differ, mut expected) = (0, 0, Vec::new(), Vec::new());
    for case in cases["cases"].as_array().expect("cases") {
        let id = &case["id"];
        let (Some(t), Some(o)) = (find(&theirs, id), find(&ours, id)) else {
            differ.push(format!("{id}: missing"));
            continue;
        };
        match (compare_case(&t, &o), case.get("expect_differ")) {
            (Verdict::Differ(why), Some(reason)) => expected.push(format!("{id}: {reason} ({why})")),
            (Verdict::Differ(why), None) => differ.push(format!("{id}: {why}")),
            (_, Some(reason)) => differ.push(format!("{id}: expected to differ ({reason}) but didn't")),
            (Verdict::Exact, None) => exact += 1,
            (Verdict::PixelExact, None) => pixel += 1,
        }
    }
    let mut report = format!(
        "# Analyze image: {exact} exact, {pixel} pixel-exact (PNG bytes differ), {} differ, {} differ on purpose\n\n",
        differ.len(),
        expected.len()
    );
    for d in &differ {
        report.push_str(&format!("- DIFFER {d}\n"));
    }
    for e in &expected {
        report.push_str(&format!("- on purpose: {e}\n"));
    }
    print!("{report}");
    if let Some(i) = args.iter().position(|a| a == "--report") {
        std::fs::write(&args[i + 1], &report).expect("report");
    }
    if differ.is_empty() { 0 } else { 1 }
}
