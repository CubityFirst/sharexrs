//! Runs the image effect cases and prints, per category, how many effects
//! and cases are identical to ShareX's, then each differing case's largest
//! channel difference and share of differing pixels.
//!
//!     cargo run -p sharex-image-editor --example image_editor_effects_report -- [case prefix] [out dir]

#[path = "../tests/effects_runner/mod.rs"]
mod effects_runner;

use effects_runner::Outcome;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let filter = args.first().map(String::as_str).filter(|s| !s.is_empty());
    let out = args.get(1).map(std::path::PathBuf::from);
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir).unwrap();
    }
    let results = effects_runner::run_cases(filter, out.as_deref());
    for r in &results {
        if let Outcome::Different { max, share, note } = &r.outcome {
            println!("{:<48} max {max:>3}  {:>7.3}%  {note}", r.name, share * 100.0);
        }
    }
    print!("{}", effects_runner::summary(&results));
}
