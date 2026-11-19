//! Runs every case in tools/tools-harness/cases.json through ShareXrs's port
//! and compares the outputs with what ShareX's own code wrote (the harness's
//! `<work>/sharex`). Writes `<work>/sharexrs/*` and `<work>/report.md`.
//!
//!   cargo run --release -p sharex-tools --example tools_validate -- tools/tools-harness/cases.json <work> [case prefix]

#[path = "tools_validate/cases.rs"]
mod cases;
#[path = "tools_validate/compare.rs"]
mod compare;
#[path = "tools_validate/cases_data.rs"]
mod cases_data;
#[path = "tools_validate/cases_capture.rs"]
mod cases_capture;

use std::collections::BTreeMap;

use compare::Outcome;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: validate <cases.json> <work dir> [case prefix]");
        std::process::exit(2);
    }
    let cases: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let work = std::path::PathBuf::from(&args[2]);
    let only = args.get(3).cloned();
    let ctx = cases::Ctx {
        inputs: work.join("inputs"),
        sharex: work.join("sharex"),
        out: work.join("sharexrs"),
    };
    std::fs::create_dir_all(&ctx.out).unwrap();
    let mut by_tool: BTreeMap<String, Vec<(String, Outcome)>> = BTreeMap::new();
    for c in &cases {
        let name = c["name"].as_str().unwrap().to_string();
        if only.as_ref().is_some_and(|p| !name.starts_with(p.as_str())) {
            continue;
        }
        let tool = c["tool"].as_str().unwrap().to_string();
        if ctx.sharex.join(format!("{name}.error.txt")).exists() {
            by_tool.entry(tool).or_default().push((name, Outcome::skip("ShareX failed on this case")));
            continue;
        }
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cases::run(&ctx, c)))
            .unwrap_or_else(|e| {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
                Outcome::fail(format!("panic: {}", msg.unwrap_or_default()))
            });
        by_tool.entry(tool).or_default().push((name, outcome));
    }
    let mut report = String::from("# sharex-tools vs ShareX\n\n| Tool | Cases | Exact | Pixel-exact | Close | Differ | Skipped |\n|---|---|---|---|---|---|---|\n");
    let mut details = String::new();
    let mut total_bad = 0;
    for (tool, results) in &by_tool {
        let count = |k: compare::Kind| results.iter().filter(|r| r.1.kind == k).count();
        let bad = count(compare::Kind::Differ);
        total_bad += bad;
        report.push_str(&format!(
            "| {tool} | {} | {} | {} | {} | {} | {} |\n",
            results.len(),
            count(compare::Kind::Exact),
            count(compare::Kind::PixelExact),
            count(compare::Kind::Close),
            bad,
            count(compare::Kind::Skipped),
        ));
        for (name, o) in results {
            if o.kind != compare::Kind::Exact {
                details.push_str(&format!("- `{name}`: {:?} {}\n", o.kind, o.note));
            }
        }
    }
    report.push_str("\n## Not exact\n\n");
    report.push_str(&details);
    std::fs::write(work.join("report.md"), &report).unwrap();
    println!("{report}");
    if total_bad > 0 {
        std::process::exit(1);
    }
}
