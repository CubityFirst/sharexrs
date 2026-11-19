//! Generates one Rust module per ShareX project from the `Strings*.resx`
//! files vendored under `resx/<Project>/` (copied verbatim from ShareX, see
//! REFERENCE_COMMIT at the repo root), so the port shows ShareX's exact text
//! in every language ShareX ships.
//!
//! `resx/ShareX.HelpersLib/Strings.de.resx` becomes `helpers_lib::Key()` with
//! the German text among its translations. Keys keep ShareX's names
//! (`MainWindow_ShareXHotkeys`), so code can be checked against ShareX's by
//! searching for the same name.
//!
//! The text itself goes to `OUT_DIR` as one file per language (`<n>.txt`,
//! all its strings back to back in key order, and `<n>.ends`, where each
//! ends), which `strings.rs` includes: the executable holds each language
//! in one piece, so only the language in use is ever read.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

/// Projects whose strings nothing uses yet: the photo and pixel editors,
/// whose port is deferred. Their `.resx` stay vendored for when it isn't.
const UNUSED: [&str; 2] = ["ShareX.PhotoEditor", "ShareX.PixelEditor"];

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("resx");
    println!("cargo:rerun-if-changed={}", root.display());

    // project -> lang -> key -> value
    let mut projects: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>> = BTreeMap::new();
    let mut langs = BTreeSet::new();
    for dir in std::fs::read_dir(&root).unwrap() {
        let dir = dir.unwrap().path();
        let project = dir.file_name().unwrap().to_string_lossy().into_owned();
        if UNUSED.contains(&project.as_str()) {
            continue;
        }
        for file in std::fs::read_dir(&dir).unwrap() {
            let file = file.unwrap().path();
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            let Some(rest) = name.strip_prefix("Strings").and_then(|r| r.strip_suffix(".resx")) else {
                continue;
            };
            let lang = rest.strip_prefix('.').unwrap_or("").to_string();
            let text = std::fs::read_to_string(&file).unwrap();
            let text = text.trim_start_matches('\u{feff}');
            let doc = roxmltree::Document::parse(text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            let entries = projects.entry(project.clone()).or_default().entry(lang.clone()).or_default();
            for data in doc.root_element().children().filter(|n| n.has_tag_name("data")) {
                // Only strings: resx can also hold embedded files and images.
                if data.attribute("type").is_some() || data.attribute("mimetype").is_some() {
                    continue;
                }
                let Some(key) = data.attribute("name") else { continue };
                let value = data
                    .children()
                    .find(|n| n.has_tag_name("value"))
                    .and_then(|v| v.text())
                    .unwrap_or("");
                entries.insert(key.to_string(), value.replace("\r\n", "\n"));
            }
            langs.insert(lang);
        }
    }

    // English ("") first: it's the fallback for anything not translated.
    let langs: Vec<String> = std::iter::once(String::new())
        .chain(langs.into_iter().filter(|l| !l.is_empty()))
        .collect();

    // Every key gets an index: modules in order, each one's keys sorted (for
    // `lookup`). Each language is then one blob of its strings in that
    // order and a table of where each ends, so showing one language reads
    // only its own pages (and English's, for what it lacks), not all 29
    // languages' strings and pointers interleaved key by key.
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let keys: Vec<(&String, &String)> = projects
        .iter()
        .flat_map(|(project, by_lang)| {
            let english = by_lang.get("").unwrap_or_else(|| panic!("{project} has no Strings.resx"));
            english.keys().map(move |k| (project, k))
        })
        .collect();
    let blob = |name: &str, values: &mut dyn Iterator<Item = Option<&str>>| {
        let (mut text, mut ends) = (String::new(), Vec::new());
        for value in values {
            text.push_str(value.unwrap_or(""));
            let end = u32::try_from(text.len()).ok().filter(|e| e & MISSING == 0).expect("blob over 2 GB");
            ends.extend_from_slice(&(end | if value.is_none() { MISSING } else { 0 }).to_le_bytes());
        }
        std::fs::write(out_dir.join(format!("{name}.txt")), text).unwrap();
        std::fs::write(out_dir.join(format!("{name}.ends")), ends).unwrap();
    };
    for (i, lang) in langs.iter().enumerate() {
        blob(&i.to_string(), &mut keys.iter().map(|(project, key)| {
            projects[*project].get(lang).and_then(|m| m.get(*key)).map(String::as_str)
        }));
    }
    blob("keys", &mut keys.iter().map(|(_, key)| Some(key.as_str())));

    let mut out = String::new();
    writeln!(out, "/// The languages ShareX ships, as .NET culture names; `\"\"` is English.").unwrap();
    writeln!(out, "pub const LANGUAGES: [&str; {}] = {:?};", langs.len(), langs).unwrap();
    let include = |name: &str| {
        format!(
            "Blob {{ text: include_str!(concat!(env!(\"OUT_DIR\"), \"/{name}.txt\")), ends: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{name}.ends\")) }}"
        )
    };
    writeln!(out, "\n/// Each language's strings, in `LANGUAGES`' order.").unwrap();
    writeln!(out, "static STRINGS: [Blob; {}] = [", langs.len()).unwrap();
    for i in 0..langs.len() {
        writeln!(out, "    {},", include(&i.to_string())).unwrap();
    }
    writeln!(out, "];\n/// Every key's name, for `lookup`.\nstatic KEYS: Blob = {};", include("keys")).unwrap();

    let mut base = 0;
    for (project, by_lang) in &projects {
        let module = module_name(project);
        let english = &by_lang[""];
        writeln!(out, "\n/// Strings from `{project}`'s `Localization/Strings.resx`.").unwrap();
        writeln!(out, "#[allow(non_snake_case, clippy::all)]\npub mod {module} {{").unwrap();
        writeln!(
            out,
            "    /// The string named `key` in the current language, as ShareX's\n    \
             /// `ResourceManager.GetString` would find it; `None` if `{project}` has no such key.\n    \
             pub fn lookup(key: &str) -> Option<&'static str> {{\n        \
             crate::lookup({base}..{}, key)\n    }}",
            base + english.len()
        )
        .unwrap();
        for (i, key) in english.keys().enumerate() {
            if !is_ident(key) {
                continue;
            }
            let doc = english[key].lines().next().unwrap_or("").replace('`', "'");
            writeln!(out, "    /// `{}`", doc.chars().take(100).collect::<String>()).unwrap();
            writeln!(out, "    pub fn {key}() -> &'static str {{ crate::get({}) }}", base + i).unwrap();
        }
        writeln!(out, "}}").unwrap();
        base += english.len();
    }

    // For the tests: every module's keys, its `lookup` and its accessors.
    writeln!(out, "\n#[cfg(test)]\n#[allow(clippy::type_complexity)]").unwrap();
    writeln!(
        out,
        "pub(crate) static MODULES: &[(&str, fn(&str) -> Option<&'static str>, &[(&str, Option<fn() -> &'static str>)])] = &["
    )
    .unwrap();
    for (project, by_lang) in &projects {
        let module = module_name(project);
        writeln!(out, "    ({module:?}, {module}::lookup, &[").unwrap();
        for key in by_lang[""].keys() {
            let accessor = if is_ident(key) { format!("Some({module}::{key})") } else { "None".into() };
            writeln!(out, "        ({key:?}, {accessor}),").unwrap();
        }
        writeln!(out, "    ]),").unwrap();
    }
    writeln!(out, "];").unwrap();

    std::fs::write(out_dir.join("strings.rs"), out).unwrap();
}

/// The flag on an end offset (see `lib.rs`'s `Blob`) for a key the language
/// doesn't translate.
const MISSING: u32 = 1 << 31;

/// `ShareX.HelpersLib` -> `helpers_lib`, `ShareX` -> `sharex`.
fn module_name(project: &str) -> String {
    let name = project.strip_prefix("ShareX.").unwrap_or("sharex");
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !matches!(s, "type" | "fn" | "mod" | "self" | "Self" | "crate" | "super" | "move" | "ref" | "use" | "where" | "impl" | "loop" | "match")
}
