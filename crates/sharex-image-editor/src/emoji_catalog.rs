//! `EmojiCatalogService` and `EmojiCatalogEntry`
//! (`Presentation/Emoji/EmojiCatalogEntry.cs`): ShareX's emoji catalogue
//! (`Assets/emoji-catalog.json`, bundled as ShareX embeds it), its groups in
//! ShareX's order and the picker's search scoring.

use std::sync::OnceLock;

use serde::Deserialize;

/// One emoji (`EmojiCatalogEntry`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EmojiEntry {
    pub name: String,
    pub group: String,
    /// The code points in hex, space separated (`Unicode`).
    pub unicode: String,
    pub keywords: Vec<String>,
}

/// `OrderedGroups`.
const ORDERED_GROUPS: [&str; 9] = [
    "Smileys & Emotion",
    "People & Body",
    "Animals & Nature",
    "Food & Drink",
    "Travel & Places",
    "Activities",
    "Objects",
    "Symbols",
    "Flags",
];

/// `GetCatalog`.
pub fn catalog() -> &'static [EmojiEntry] {
    static CATALOG: OnceLock<Vec<EmojiEntry>> = OnceLock::new();
    CATALOG.get_or_init(|| serde_json::from_str(include_str!("../assets/emoji-catalog.json")).unwrap_or_default())
}

/// `GetGroups`: ShareX's order, then any other group by ordinal order.
pub fn groups() -> Vec<String> {
    let present: std::collections::BTreeSet<&str> = catalog().iter().map(|e| e.group.as_str()).collect();
    let mut out: Vec<String> = ORDERED_GROUPS.iter().filter(|g| present.contains(*g)).map(|g| g.to_string()).collect();
    for g in present {
        if !out.iter().any(|o| o == g) {
            out.push(g.to_string());
        }
    }
    out
}

impl EmojiEntry {
    /// `DisplayName`.
    pub fn display_name(&self) -> &str {
        &self.name
    }

    fn search_index(&self) -> String {
        let mut s = format!("{} {} {} {}", self.name, self.group, self.name, self.group);
        for k in &self.keywords {
            s.push(' ');
            s.push_str(k);
        }
        s.to_lowercase()
    }

    /// `GetSearchScore`: 0 for the name, 1 for its start, 2 for a keyword's
    /// start, 3 anywhere in the index; `None` for no match.
    pub fn search_score(&self, search: &str) -> Option<u8> {
        if search.trim().is_empty() {
            return None;
        }
        let search = search.trim().to_lowercase();
        let name = self.name.to_lowercase();
        if name == search {
            return Some(0);
        }
        if name.starts_with(&search) {
            return Some(1);
        }
        if self.keywords.iter().any(|k| k.to_lowercase().starts_with(&search)) {
            return Some(2);
        }
        if self.search_index().contains(&search) {
            return Some(3);
        }
        None
    }
}

/// `BuildQueryResult`: the group's emoji, or those matching `search`
/// (best score first, stable), and how many the group has.
pub fn query(group: &str, search: &str) -> (Vec<&'static EmojiEntry>, usize) {
    let entries: Vec<&EmojiEntry> = catalog().iter().filter(|e| e.group == group).collect();
    let count = entries.len();
    if search.trim().is_empty() {
        return (entries, count);
    }
    let mut scored: Vec<(u8, &EmojiEntry)> = entries.into_iter().filter_map(|e| e.search_score(search).map(|s| (s, e))).collect();
    scored.sort_by_key(|(s, _)| *s);
    (scored.into_iter().map(|(_, e)| e).collect(), count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_and_search() {
        assert!(catalog().len() > 1000);
        assert_eq!(groups()[0], "Smileys & Emotion");
        let (all, n) = query("Smileys & Emotion", "");
        assert_eq!(all.len(), n);
        assert_eq!(all[0].unicode, "1f600");
        let (hits, _) = query("Smileys & Emotion", "grinning face");
        assert_eq!(hits[0].name, "grinning face");
    }
}
