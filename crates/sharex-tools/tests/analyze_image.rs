//! Analyze image against ShareX's own `AnalyzeImageService`, without a
//! network: `fixtures/analyze` holds what ShareX sent and made of each
//! canned answer (`tools/tools-harness/run_analyze.sh`, run on Windows
//! x64), and the images it sent. Each request ShareXrs builds must equal
//! ShareX's (apart from the user agent; PNG payloads by their pixels), and
//! each answer must read the same.

use std::io::Read;
use std::path::Path;

use base64::Engine;
use serde_json::Value;
use sharex_tools::analyze_image::{
    self, AIOptions, AIProvider, AIRequest, AIResponse, AnalyzeInput, build_analyze_request, build_models_request,
    connection_result, parse_analyze_response, parse_models,
};
use sharex_uploaders::http::{self, Content, HttpRequest};

fn gz(bytes: &[u8]) -> Value {
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes).read_to_string(&mut text).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/analyze"))
}

/// The request as it reaches the mock: the rewritten target and the wire
/// headers (names in lower case; no host, user agent or length).
fn wire(request: &AIRequest) -> (String, Vec<(String, String)>) {
    let absolute = http::dotnet_absolute_uri(&request.url).unwrap();
    let (scheme, rest) = absolute.split_once("://").unwrap();
    let target = format!("/__/{scheme}/{rest}");
    let content = request.body.as_ref().map(|(data, ct)| {
        let mut c = Content::bytes(data.clone(), None);
        c.extra_headers_mut().push(("Content-Type".into(), ct.clone()));
        c
    });
    let http_request =
        HttpRequest { method: request.method.clone(), url: request.url.clone(), headers: request.headers.clone(), content };
    let headers = http::wire_headers_for(&http_request, request.factory_client)
        .into_iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v))
        .filter(|(k, _)| !matches!(k.as_str(), "user-agent" | "content-length" | "host"))
        .collect();
    (target, headers)
}

fn their_headers(r: &Value) -> Vec<(String, String)> {
    r["headers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| (h[0].as_str().unwrap().to_owned(), h[1].as_str().unwrap().to_owned()))
        .filter(|(k, _)| !matches!(k.as_str(), "user-agent" | "content-length" | "host"))
        .collect()
}

/// Bodies equal, or equal once their PNGs (which only decode the same)
/// are taken out and those decode to the same pixels.
fn same_body(theirs: &[u8], ours: &[u8]) -> bool {
    if theirs == ours {
        return true;
    }
    let split = |body: &[u8]| -> Option<(String, Vec<Vec<u8>>)> {
        let mut text = String::from_utf8(body.to_vec()).ok()?;
        let mut images = Vec::new();
        for marker in ["data:image/png;base64,", "\"data\":\""] {
            while let Some(start) = text.find(marker) {
                let from = start + marker.len();
                let end = from + text[from..].find('"')?;
                images.push(base64::engine::general_purpose::STANDARD.decode(&text[from..end]).ok()?);
                text.replace_range(start..end, "<png>");
            }
        }
        Some((text, images))
    };
    match (split(theirs), split(ours)) {
        (Some((a, ia)), Some((b, ib))) => {
            a == b
                && !ia.is_empty()
                && ia.len() == ib.len()
                && ia.iter().zip(&ib).all(|(x, y)| {
                    image::load_from_memory(x).unwrap().to_rgba8() == image::load_from_memory(y).unwrap().to_rgba8()
                })
        }
        _ => false,
    }
}

fn response(case: &Value) -> AIResponse {
    let r = &case["responses"][0];
    AIResponse {
        status_code: r["status"].as_u64().unwrap() as u16,
        reason: r["reason"].as_str().unwrap_or("").to_owned(),
        text: r["body"].as_str().unwrap_or("").trim_start_matches('\u{feff}').to_owned(),
    }
}

/// ShareX's error, or (for its JSON readers' exceptions, whose wording
/// ShareXrs doesn't copy) just that there is one.
fn same_error(theirs: &Value, ours: &str) -> bool {
    let parser = matches!(theirs["error_type"].as_str(), Some("JsonException" | "JsonReaderException" | "JsonSerializationException"));
    parser || theirs["error"].as_str() == Some(ours)
}

fn check_request(id: &str, theirs: &Value, ours: &AIRequest) {
    let requests = theirs["requests"].as_array().unwrap();
    assert_eq!(requests.len(), 1, "{id}: ShareX sent {} requests", requests.len());
    let r = &requests[0];
    let (target, headers) = wire(ours);
    assert_eq!(r["method"].as_str().unwrap(), ours.method.name(), "{id}");
    assert_eq!(r["target"].as_str().unwrap(), target, "{id}");
    assert_eq!(their_headers(r), headers, "{id}");
    let body = base64::engine::general_purpose::STANDARD.decode(r["body_base64"].as_str().unwrap()).unwrap();
    let ours_body = ours.body.as_ref().map(|(b, _)| b.clone()).unwrap_or_default();
    assert!(same_body(&body, &ours_body), "{id}: bodies differ");
}

#[test]
fn requests_and_answers_match_sharex() {
    let cases = gz(include_bytes!("fixtures/analyze/cases.json.gz"));
    let results = gz(include_bytes!("fixtures/analyze/results-sharex.json.gz"));
    let mut checked = 0;
    for case in cases["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if case.get("expect_differ").is_some() || case.get("file").and_then(Value::as_str) == Some("missing.png") {
            // The leak ShareXrs doesn't port; a missing file's message names the harness's path.
            continue;
        }
        let theirs = results.as_array().unwrap().iter().find(|r| r["id"] == case["id"]).unwrap();
        let options = AIOptions::from_json(&case["options"]);
        match case["kind"].as_str().unwrap() {
            "analyze" => {
                let file = case["file"].as_str().map(|f| fixtures().join(f));
                let data = match case["data_file"].as_str() {
                    Some(f) => std::fs::read(fixtures().join(f)).unwrap(),
                    None => Vec::new(),
                };
                let input = match &file {
                    Some(path) => AnalyzeInput::File(path),
                    None => AnalyzeInput::Data(&data),
                };
                match build_analyze_request(&options, input) {
                    Err(e) => {
                        assert!(theirs["requests"].as_array().unwrap().is_empty(), "{id}");
                        assert!(same_error(theirs, &e), "{id}: {e} vs {}", theirs["error"]);
                    }
                    Ok(None) => assert_eq!(theirs["result"], "", "{id}"),
                    Ok(Some(request)) => {
                        check_request(id, theirs, &request);
                        match parse_analyze_response(options.provider.unwrap(), &response(case)) {
                            Ok(text) => assert_eq!(theirs["result"].as_str(), text.as_deref(), "{id}"),
                            Err(e) => assert!(same_error(theirs, &e), "{id}: {e} vs {}", theirs["error"]),
                        }
                    }
                }
            }
            "test" => match build_models_request(&options) {
                Err(e) => {
                    assert_eq!(theirs["success"], false, "{id}");
                    assert_eq!(theirs["result"].as_str(), Some(e.as_str()), "{id}");
                }
                Ok(request) => {
                    check_request(id, theirs, &request);
                    let r = connection_result(&response(case));
                    assert_eq!(theirs["success"], r.success, "{id}");
                    assert_eq!(theirs["result"].as_str(), Some(r.message.as_str()), "{id}");
                }
            },
            "models" => {
                if !matches!(options.provider, Some(AIProvider::OpenAI | AIProvider::OpenAILegacy)) {
                    assert_eq!(theirs["models"], serde_json::json!([]), "{id}");
                    assert!(theirs["requests"].as_array().unwrap().is_empty(), "{id}");
                } else {
                    match build_models_request(&options) {
                        Err(e) => assert_eq!(theirs["error"].as_str(), Some(e.as_str()), "{id}"),
                        Ok(request) => {
                            check_request(id, theirs, &request);
                            match parse_models(&response(case)) {
                                Ok(models) => assert_eq!(theirs["models"], serde_json::json!(models), "{id}"),
                                Err(e) => assert!(same_error(theirs, &e), "{id}: {e} vs {}", theirs["error"]),
                            }
                        }
                    }
                }
            }
            other => panic!("{other}"),
        }
        checked += 1;
    }
    assert!(checked > 130, "{checked}");
}

/// The prompts ShareX offers, and its defaults.
#[test]
fn defaults() {
    let o = AIOptions::default();
    assert_eq!(o.input.as_deref(), Some("What is in this image?"));
    assert!(!o.has_api_key());
    assert_eq!(analyze_image::preset_prompts()[0], "What is in this image?");
    assert_eq!(o.api_key_page(), Some("https://platform.openai.com/api-keys"));
}
