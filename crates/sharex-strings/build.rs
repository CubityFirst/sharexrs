//! Generates one Rust module per ShareX project from the `Strings*.resx`
//! files vendored under `resx/<Project>/` (copied verbatim from ShareX, see
//! REFERENCE_COMMIT at the repo root), so the port shows ShareX's exact text
//! in every language ShareX ships.
//!
//! `resx/ShareX.HelpersLib/Strings.de.resx` becomes `helpers_lib::Key()` with
//! the German text among its translations. Keys keep ShareX's names
//! (`MainWindow_ShareXHotkeys`), so code can be checked against ShareX's by
//! searching for the same name.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("resx");
    println!("cargo:rerun-if-changed={}", root.display());

    // project -> lang -> key -> value
    let mut projects: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>> = BTreeMap::new();
    let mut langs = BTreeSet::new();
    for dir in std::fs::read_dir(&root).unwrap() {
        let dir = dir.unwrap().path();
        let project = dir.file_name().unwrap().to_string_lossy().into_owned();
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

    let mut out = String::new();
    writeln!(out, "/// The languages ShareX ships, as .NET culture names; `\"\"` is English.").unwrap();
    writeln!(out, "pub const LANGUAGES: [&str; {}] = {:?};", langs.len(), langs).unwrap();

    for (project, by_lang) in &projects {
        let module = module_name(project);
        let english = by_lang.get("").unwrap_or_else(|| panic!("{project} has no Strings.resx"));
        writeln!(out, "\n/// Strings from `{project}`'s `Localization/Strings.resx`.").unwrap();
        writeln!(out, "#[allow(non_snake_case, clippy::all)]\npub mod {module} {{").unwrap();
        // Sorted by key, for `lookup`.
        writeln!(out, "    static TABLE: &[(&str, [Option<&str>; {}])] = &[", langs.len()).unwrap();
        for key in english.keys() {
            let values: Vec<Option<&String>> = langs.iter().map(|l| by_lang.get(l).and_then(|m| m.get(key))).collect();
            writeln!(out, "        ({key:?}, {values:?}),").unwrap();
        }
        writeln!(out, "    ];").unwrap();
        writeln!(
            out,
            "    /// The string named `key` in the current language, as ShareX's\n    \
             /// `ResourceManager.GetString` would find it; `None` if `{project}` has no such key.\n    \
             pub fn lookup(key: &str) -> Option<&'static str> {{\n        \
             TABLE.binary_search_by(|(k, _)| k.cmp(&key)).ok().map(|i| crate::pick(&TABLE[i].1))\n    }}"
        )
        .unwrap();
        for (i, key) in english.keys().enumerate() {
            if !is_ident(key) {
                continue;
            }
            let doc = english[key].lines().next().unwrap_or("").replace('`', "'");
            writeln!(out, "    /// `{}`", doc.chars().take(100).collect::<String>()).unwrap();
            writeln!(out, "    pub fn {key}() -> &'static str {{ crate::pick(&TABLE[{i}].1) }}").unwrap();
        }
        writeln!(out, "}}").unwrap();
    }

    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join("strings.rs");
    std::fs::write(dest, out).unwrap();
}

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
