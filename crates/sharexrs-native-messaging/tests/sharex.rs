//! The native messaging input as ShareX's own code read it
//! (tools/cli-harness: `JsonHelpers.DeserializeFromFile<NativeMessagingInput>`
//! over `fixtures/cases.json`, written to `fixtures/results-sharex.json`).

use sharexrs_native_messaging::NativeMessagingInput;

#[test]
fn input_as_sharex_reads_it() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let read = |n: &str| -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(dir.join(n)).unwrap()).unwrap() };
    let cases = read("cases.json");
    let results = read("results-sharex.json");
    let cases = cases["inputs"].as_array().unwrap();
    let results = results["results"].as_array().unwrap();
    assert_eq!(cases.len(), results.len(), "run tools/cli-harness/run.sh again");
    let mut failures = Vec::new();
    for (case, want) in cases.iter().zip(results) {
        let json = case.as_str().unwrap();
        // An empty file is `default` before Json.NET sees it.
        let got = NativeMessagingInput::parse(json).map(|i| {
            serde_json::json!({ "action": i.action.name(), "url": i.url, "text": i.text })
        });
        let got = got.unwrap_or(serde_json::Value::Null);
        if &got != want {
            failures.push(format!("{json:?}: ShareX {want}, ShareXrs {got}"));
        }
    }
    assert!(failures.is_empty(), "{} of {} differ:\n{}", failures.len(), cases.len(), failures.join("\n"));
}
