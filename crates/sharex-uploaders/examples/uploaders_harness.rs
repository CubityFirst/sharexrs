//! The ShareXrs side of `tools/uploaders-harness`: runs every case in
//! `cases.json` through ShareXrs's uploaders against the local mock server
//! and writes the same results file the .NET harness writes for ShareX.
//!
//! ```text
//! cargo run -p sharex-uploaders --example uploaders_harness -- <cases.json> <results.json> [port]
//! ```
//!
//! Every request is rewritten to `http://127.0.0.1:<port>/__/<scheme>/<host><path>`
//! just before it's sent: nothing leaves the machine.

use std::io::Cursor;
use std::sync::Arc;

use base64::Engine;
use serde_json::{Value, json};
use sharex_uploaders::custom::{CustomFileUploader, CustomTextUploader, CustomUploaderInput, CustomUploaderItem, CustomUrlSharer, CustomUrlShortener};
use sharex_uploaders::factory::{self, Ported, TaskReferenceHelper};
use sharex_uploaders::http::{self, UrlRewriter};
use sharex_uploaders::uploader::{self, GenericUploader, UrlSharer, UrlShortener};
use sharex_uploaders::{UploadResult, UploadersConfig, UploaderErrorManager};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cases_path = &args[1];
    let results_path = &args[2];
    let port: u16 = args.get(3).and_then(|p| p.parse().ok()).unwrap_or(18765);

    let prefix = format!("http://127.0.0.1:{port}/__/");
    let back_prefix = prefix.clone();
    http::set_url_rewriter(Some(UrlRewriter {
        forward: Arc::new(move |url: &str| {
            let (scheme, rest) = url.split_once("://").expect("absolute URL");
            format!("http://127.0.0.1:{port}/__/{scheme}/{rest}")
        }),
        back: Arc::new(move |url: &str| match url.strip_prefix(&back_prefix) {
            Some(rest) => {
                let (scheme, rest) = rest.split_once('/').expect("scheme");
                format!("{scheme}://{rest}")
            }
            None => url.to_owned(),
        }),
    }));

    let cases: Value = serde_json::from_str(&std::fs::read_to_string(cases_path).expect("cases")).expect("cases JSON");
    let only = std::env::var("HARNESS_ONLY").ok().filter(|s| !s.is_empty());
    let mut results = Vec::new();
    for case in cases["cases"].as_array().expect("cases array") {
        let id = case["id"].as_str().unwrap_or("").to_owned();
        if only.as_deref().is_some_and(|o| o != id) {
            continue;
        }
        let mut out = run_case(case, port);
        out["id"] = json!(id);
        println!("{id}: {}", if out.get("exception").is_some_and(|e| !e.is_null()) { "exception" } else { "ok" });
        results.push(out);
    }
    std::fs::write(results_path, serde_json::to_string_pretty(&results).expect("results")).expect("write results");
}

fn control(port: u16) -> ureq::Agent {
    let _ = port;
    ureq::Agent::config_builder().http_status_as_error(false).build().into()
}

fn reset(port: u16, responses: &Value) {
    let body = json!({ "responses": responses.clone() }).to_string();
    control(port).post(&format!("http://127.0.0.1:{port}/__mock/reset")).send(body).expect("mock reset");
}

fn log(port: u16) -> Value {
    let mut resp = control(port).get(&format!("http://127.0.0.1:{port}/__mock/log")).call().expect("mock log");
    serde_json::from_str(&resp.body_mut().read_to_string().expect("log body")).expect("log JSON")
}

fn errors_json(errors: &UploaderErrorManager) -> Value {
    Value::Array(errors.errors.iter().map(|e| json!(e.text)).collect())
}

fn result_json(r: &Option<UploadResult>) -> Value {
    let Some(r) = r else { return Value::Null };
    json!({
        "url": r.url,
        "thumbnail_url": r.thumbnail_url,
        "deletion_url": r.deletion_url,
        "shortened_url": r.shortened_url,
        "is_success": r.is_success(),
        "is_error": r.is_error(),
        "is_url_expected": r.is_url_expected,
        "response": r.response,
        "errors": errors_json(&r.errors),
        "response_url": r.response_info.as_ref().and_then(|i| i.response_url.clone()),
        "status_code": r.response_info.as_ref().map(|i| i.status_code),
    })
}

enum Runner {
    Generic(GenericUploader),
    Shortener(Box<dyn UrlShortener>),
    Sharer(Box<dyn UrlSharer>),
}

fn run_case(case: &Value, port: u16) -> Value {
    reset(port, &case["responses"]);
    let mut out = json!({});
    let input = &case["input"];
    let file_name = input["file_name"].as_str().unwrap_or("").to_owned();
    let text = input["text"].as_str().map(str::to_owned);
    let url = input["url"].as_str().map(str::to_owned);
    let data: Vec<u8> = match input["content_base64"].as_str() {
        Some(b) => base64::engine::general_purpose::STANDARD.decode(b).expect("base64"),
        None => text.clone().unwrap_or_default().into_bytes(),
    };

    let runner: Result<Runner, String> = (|| {
        if case["kind"] == "custom" {
            let mut item = CustomUploaderItem::from_value(case["sxcu"].clone()).ok_or("bad sxcu")?;
            item.check_backward_compatibility()?;
            out["sxcu_export"] = json!(item.to_sxcu());
            out["request_url"] = json!(match item.get_request_url(&CustomUploaderInput::new(&file_name, text.as_deref().or(url.as_deref()).unwrap_or(""))) {
                Ok(u) => u,
                Err(e) => format!("EXCEPTION: {e}"),
            });
            return Ok(match case["dest"].as_str().unwrap_or("") {
                "image" | "file" => Runner::Generic(GenericUploader::File(Box::new(CustomFileUploader::new(item)))),
                "text" => Runner::Generic(GenericUploader::Text(Box::new(CustomTextUploader::new(item)))),
                "url_shortener" => Runner::Shortener(Box::new(CustomUrlShortener::new(item))),
                "url_sharing" => Runner::Sharer(Box::new(CustomUrlSharer::new(item))),
                other => return Err(format!("unknown dest {other}")),
            });
        }
        let mut config: UploadersConfig = serde_json::from_value(case["config"].clone()).map_err(|e| e.to_string())?;
        let task = TaskReferenceHelper { text_format: "text".into(), ..Default::default() };
        let service = case["service"].as_str().unwrap_or("");
        let (kind, name) = service.split_once(':').ok_or("bad service")?;
        let parse = |n: &str| serde_json::Value::String(n.to_owned());
        let ported = |p: Ported| match p {
            Ported::Yes(b) => Ok(b),
            Ported::No => Err(format!("{service} isn't ported")),
        };
        let runner = match kind {
            "image" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_image(&d, &config))?);
                Runner::Generic(factory::create_image(&d, &mut config, &task).ok_or("no uploader")?)
            }
            "text" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_text(&d, &config))?);
                Runner::Generic(factory::create_text(&d, &mut config, &task).ok_or("no uploader")?)
            }
            "file" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_file(&d, &config))?);
                Runner::Generic(factory::create_file(&d, &mut config, &task).ok_or("no uploader")?)
            }
            "shortener" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_url_shortener(&d, &config))?);
                Runner::Shortener(factory::create_url_shortener(&d, &mut config, &task).ok_or("no shortener")?)
            }
            "sharing" => {
                let d = serde_json::from_value(parse(name)).map_err(|e| e.to_string())?;
                out["check_config"] = json!(ported(factory::check_url_sharing(&d, &config))?);
                Runner::Sharer(factory::create_url_sharing(&d, &mut config, &task).ok_or("no sharer")?)
            }
            other => return Err(format!("unknown service kind {other}")),
        };
        Ok(runner)
    })();

    let (result, errors) = match runner {
        Err(e) => {
            out["exception"] = json!(e);
            (None, None)
        }
        Ok(Runner::Generic(mut g)) => {
            let r = match &mut g {
                GenericUploader::Text(t) => uploader::upload_text(t.as_mut(), text.as_deref().unwrap_or(""), &file_name),
                GenericUploader::File(_) => g.upload(&http::shared_stream(Cursor::new(data)), &file_name),
            };
            let errors = errors_json(&g.uploader().errors);
            (Some(r), Some(errors))
        }
        Ok(Runner::Shortener(mut s)) => {
            let r = uploader::shorten_url(s.as_mut(), url.as_deref().unwrap_or(""));
            (Some(r), Some(errors_json(&s.uploader().errors)))
        }
        Ok(Runner::Sharer(mut s)) => {
            let r = uploader::share_url(s.as_mut(), url.as_deref().unwrap_or(""));
            (Some(r), Some(errors_json(&s.uploader().errors)))
        }
    };
    match result {
        Some(Ok(r)) => out["result"] = result_json(&r),
        Some(Err(e)) => out["exception"] = json!(e),
        None => {}
    }
    if let Some(e) = errors {
        out["uploader_errors"] = e;
    }
    out["requests"] = log(port);
    out
}
