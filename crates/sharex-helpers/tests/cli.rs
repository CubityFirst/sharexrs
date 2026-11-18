//! ShareX's own `CLIManager` and `SingleInstanceManager` message over
//! `fixtures/cli/cases.json` (tools/cli-harness wrote
//! `fixtures/cli/results-sharex.json`), replayed with ShareXrs's.

use serde_json::{Value, json};
use sharex_helpers::cli::{CliManager, parse_cli};
use sharex_helpers::single_instance::encode_arguments;

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect()
}

#[test]
fn cli_as_sharex() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli");
    let read = |n: &str| -> Value { serde_json::from_str(&std::fs::read_to_string(dir.join(n)).unwrap()).unwrap() };
    let cases = read("cases.json");
    let results = read("results-sharex.json");
    let queries: Vec<Vec<String>> = cases["queries"].as_array().unwrap().iter().map(strings).collect();
    let parameters = strings(&cases["parameters"]);
    let arrays = cases["arrays"].as_array().unwrap();
    let want_arrays = results["arrays"].as_array().unwrap();
    assert_eq!(arrays.len(), want_arrays.len(), "run tools/cli-harness/run.sh again");
    let mut failures = Vec::new();
    for (case, want) in arrays.iter().zip(want_arrays) {
        let arguments = strings(case);
        let mut cli = CliManager::new(&arguments);
        let parsed = cli.parse_commands();
        let commands: Vec<Value> = cli
            .commands
            .iter()
            .map(|c| json!({ "command": c.command, "parameter": c.parameter, "is_command": c.is_command }))
            .collect();
        let exists: Vec<bool> = queries
            .iter()
            .map(|q| cli.is_command_exist(&q.iter().map(String::as_str).collect::<Vec<_>>()))
            .collect();
        let values: Vec<Option<&str>> = parameters.iter().map(|p| cli.get_parameter(p)).collect();
        let message: String = encode_arguments(&arguments).iter().map(|b| format!("{b:02X}")).collect();
        let got = json!({
            "parsed": parsed,
            "commands": commands,
            "exists": exists,
            "parameters": values,
            "text": cli.to_string(),
            "message": message,
        });
        if &got != want {
            failures.push(format!("{arguments:?}:\n  ShareX   {want}\n  ShareXrs {got}"));
        }
    }
    let strings_cases = cases["strings"].as_array().unwrap();
    let want_strings = results["strings"].as_array().unwrap();
    for (case, want) in strings_cases.iter().zip(want_strings) {
        let got = json!(parse_cli(case.as_str().unwrap()));
        if &got != want {
            failures.push(format!("ParseCLI({case}): ShareX {want}, ShareXrs {got}"));
        }
    }
    let total = arrays.len() + strings_cases.len();
    assert!(failures.is_empty(), "{} of {total} differ:\n{}", failures.len(), failures.join("\n"));
}
