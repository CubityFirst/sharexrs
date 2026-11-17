//! What the History and Image history windows ask of the items: ShareX
//! loads every row and filters in memory, newest first.
//!
//! - `HistoryFilter` (History window): the search box alone (file name
//!   wildcards, also matched against tag values, plus the favourites
//!   toggle), or the advanced panel (file name, URL, type, host, dates).
//! - [`ImageHistoryFilter`] (Image history window's `ApplyFilterAsync`).
//! - The type and host lists of the advanced panel, the process names the
//!   search box suggests (`HistoryHelpers.GetProcessNames`).
//! - Paging: the Image history window shows `MaxItemCount` at a time and
//!   loads more near the end of the scroll ([`next_batch`]).

use std::ops::Range;

use regex::Regex;

use crate::culture;
use crate::datetime::DateTime;
use crate::item::{HistoryItem, is_null_or_empty, is_null_or_white_space};

/// The search pattern both windows build: `Regex.Escape(text)` with `?`
/// any one character and `*` any run, ignoring case.
pub fn wildcard_regex(text: &str) -> Regex {
    let mut pattern = String::from("(?i)");
    for c in text.chars() {
        match c {
            '?' => pattern.push('.'),
            '*' => pattern.push_str(".*"),
            c => pattern.push_str(&regex::escape(c.encode_utf8(&mut [0; 4]))),
        }
    }
    Regex::new(&pattern).expect("escaped pattern")
}

fn tags_match(item: &HistoryItem, regex: &Regex) -> bool {
    item.tags.as_ref().is_some_and(|t| t.iter().any(|(_, v)| v.is_some_and(|v| !v.is_empty() && regex.is_match(v))))
}

/// `ShareX.HistoryLib.HistoryFilter`.
#[derive(Clone, Debug)]
pub struct HistoryFilter {
    pub filename: String,
    pub url: String,
    pub filter_date: bool,
    pub from_date: DateTime,
    pub to_date: DateTime,
    pub filter_type: bool,
    pub r#type: Option<String>,
    pub filter_host: bool,
    pub host: String,
    pub filter_favorites: bool,
    pub max_item_count: i32,
    pub search_in_tags: bool,
}

impl Default for HistoryFilter {
    fn default() -> Self {
        Self {
            filename: String::new(),
            url: String::new(),
            filter_date: false,
            from_date: DateTime::default(),
            to_date: DateTime::default(),
            filter_type: false,
            r#type: None,
            filter_host: false,
            host: String::new(),
            filter_favorites: false,
            max_item_count: 0,
            search_in_tags: true,
        }
    }
}

impl HistoryFilter {
    /// `HistoryWindow.CreateCurrentFilter` with the advanced panel closed.
    pub fn search(text: &str, favorites: bool) -> Self {
        Self { filename: text.to_string(), filter_favorites: favorites, ..Default::default() }
    }

    /// `ApplyFilter`: favourites alone when that's on (the other criteria
    /// are ignored), else type (whole, ignoring case), host (contains),
    /// file name or tag value (wildcards), URL (contains) and date (the
    /// item's day within `from_date..=to_date`); then at most
    /// `max_item_count` when that's positive.
    pub fn apply<'a>(&self, items: &'a [HistoryItem]) -> Vec<&'a HistoryItem> {
        let mut out: Vec<&HistoryItem> = if self.filter_favorites {
            items.iter().filter(|x| x.favorite()).collect()
        } else {
            let regex = (!self.filename.is_empty()).then(|| wildcard_regex(&self.filename));
            let ty = self.r#type.as_deref().filter(|t| !t.is_empty() && self.filter_type);
            let host = (self.filter_host && !self.host.is_empty()).then_some(self.host.as_str());
            items
                .iter()
                .filter(|x| ty.is_none_or(|t| x.r#type.as_deref().is_some_and(|xt| !xt.is_empty() && culture::invariant_ignore_case_eq(xt, t))))
                .filter(|x| {
                    host.is_none_or(|h| x.host.as_deref().is_some_and(|xh| !xh.is_empty() && culture::invariant_ignore_case_contains(xh, h)))
                })
                .filter(|x| {
                    regex.as_ref().is_none_or(|r| {
                        x.file_name.as_deref().is_some_and(|n| r.is_match(n)) || (self.search_in_tags && tags_match(x, r))
                    })
                })
                .filter(|x| {
                    self.url.is_empty() || x.url.as_deref().is_some_and(|u| culture::invariant_ignore_case_contains(u, &self.url))
                })
                .filter(|x| !self.filter_date || (x.date_time.date() >= self.from_date && x.date_time.date() <= self.to_date))
                .collect()
        };
        if self.max_item_count > 0 {
            out.truncate(self.max_item_count as usize);
        }
        out
    }
}

/// The Image history window's filter (`ImageHistoryWindow.ApplyFilterAsync`):
/// items with a file path, favourites only when on, images and videos only
/// (`ImageOnly`), files that exist (`FilterMissingFiles`), and the trimmed
/// search text against the file name (null as "") or a tag value.
#[derive(Clone, Debug, Default)]
pub struct ImageHistoryFilter {
    pub search: String,
    pub favorites: bool,
    pub media_only: bool,
    pub filter_missing: bool,
}

impl ImageHistoryFilter {
    pub fn apply<'a>(&self, items: &'a [HistoryItem], file_exists: impl Fn(&str) -> bool) -> Vec<&'a HistoryItem> {
        use sharex_helpers::file_helpers::{is_image_file, is_video_file};
        let search = self.search.trim();
        let regex = (!search.is_empty()).then(|| wildcard_regex(search));
        items
            .iter()
            .filter(|item| {
                let Some(path) = item.file_path.as_deref().filter(|p| !is_null_or_white_space(Some(p))) else { return false };
                if self.favorites && !item.favorite() {
                    return false;
                }
                if self.media_only && !is_image_file(path) && !is_video_file(path) {
                    return false;
                }
                if self.filter_missing && !file_exists(path) {
                    return false;
                }
                regex.as_ref().is_none_or(|r| r.is_match(item.file_name.as_deref().unwrap_or("")) || tags_match(item, r))
            })
            .collect()
    }
}

/// `items.Select(f).Where(!IsNullOrWhiteSpace).Distinct(OrdinalIgnoreCase)`:
/// the first spelling of each, in item order.
fn distinct<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    values
        .flatten()
        .filter(|v| !is_null_or_white_space(Some(v)))
        .filter(|v| seen.insert(culture::ordinal_upper(v)))
        .map(str::to_string)
        .collect()
}

/// The advanced panel's type list (`RefreshHistoryAsync`): distinct
/// ignoring case (first spelling, items newest first), sorted by the
/// current culture.
pub fn distinct_types(items_newest_first: &[HistoryItem]) -> Vec<String> {
    let mut v = distinct(items_newest_first.iter().map(|i| i.r#type.as_deref()));
    v.sort_by(|a, b| culture::current_culture_cmp(a, b));
    v
}

/// The advanced panel's host list, as [`distinct_types`].
pub fn distinct_hosts(items_newest_first: &[HistoryItem]) -> Vec<String> {
    let mut v = distinct(items_newest_first.iter().map(|i| i.host.as_deref()));
    v.sort_by(|a, b| culture::current_culture_cmp(a, b));
    v
}

/// `HistoryHelpers.GetProcessNames`: distinct ignoring case, sorted ignoring case.
pub fn process_names(items: &[HistoryItem]) -> Vec<String> {
    let mut v = distinct(items.iter().map(|i| i.tags_process_name()));
    v.sort_by(|a, b| culture::ordinal_ignore_case_cmp(a, b));
    v
}

/// `ImageHistoryWindow.LoadNextBatch`: the next `max_item_count` (all when
/// that isn't positive) after `next_index`.
pub fn next_batch(next_index: usize, filtered_len: usize, max_item_count: i32) -> Range<usize> {
    if next_index >= filtered_len {
        return next_index..next_index;
    }
    let size = if max_item_count <= 0 { filtered_len - next_index } else { max_item_count as usize };
    next_index..(next_index + size).min(filtered_len)
}

/// `OnScrollChanged`: load more when what's left below the view is at most
/// a thumbnail's height (120 at least).
pub fn should_load_more(extent_height: f64, viewport_height: f64, offset_y: f64, thumbnail_height: i32) -> bool {
    extent_height - viewport_height - offset_y <= 120f64.max(thumbnail_height as f64)
}

/// `UpdateColumnCount`: cards per row, each the thumbnail's width plus 18
/// (margins, borders, padding), 50 at least.
pub fn image_history_columns(available_width: f64, thumbnail_width: i32) -> usize {
    let card = 50f64.max(thumbnail_width as f64 + 18.0);
    1.max((available_width.max(1.0) / card).floor() as usize)
}

/// `ShowSelectedImage`: the image files around the selected item (from 100
/// before, 201 at most) for the image viewer, and which one is selected.
pub fn image_viewer_files(
    filtered: &[&HistoryItem],
    selected: usize,
    file_exists: impl Fn(&str) -> bool,
) -> (Vec<String>, usize) {
    let start = selected.saturating_sub(100);
    let end = filtered.len().min(start + 201);
    let mut files = Vec::new();
    let mut selected_index = 0;
    for (i, item) in filtered.iter().enumerate().take(end).skip(start) {
        let Some(path) = item.file_path.as_deref() else { continue };
        if !file_exists(path) || !sharex_helpers::file_helpers::is_image_file(path) {
            continue;
        }
        if i == selected {
            selected_index = files.len();
        }
        files.push(path.to_string());
    }
    (files, selected_index)
}

/// Whether an item has the given text field set (`!string.IsNullOrEmpty`).
pub fn has(s: &Option<String>) -> bool {
    !is_null_or_empty(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::Tags;

    fn item(name: &str, ty: &str, host: Option<&str>) -> HistoryItem {
        HistoryItem {
            file_name: Some(name.into()),
            file_path: Some(format!("C:\\s\\{name}")),
            r#type: Some(ty.into()),
            host: host.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn filters() {
        let mut items = vec![item("a.png", "Image", Some("Imgur")), item("b.txt", "Text", None), item("c.PNG", "image", Some("Amazon S3"))];
        items[1].tags = Some([("WindowTitle", Some("notes"))].into_iter().collect::<Tags>());
        let f = HistoryFilter::search("*.png", false);
        assert_eq!(f.apply(&items).len(), 2);
        assert_eq!(HistoryFilter::search("note?", false).apply(&items).len(), 1);
        let f = HistoryFilter { filter_type: true, r#type: Some("IMAGE".into()), filter_host: true, host: "s3".into(), ..Default::default() };
        assert_eq!(f.apply(&items).iter().map(|i| i.file_name.as_deref().unwrap()).collect::<Vec<_>>(), ["c.PNG"]);
        items[0].set_favorite(true);
        assert_eq!(HistoryFilter::search("b*", true).apply(&items).len(), 1);
        assert_eq!(distinct_types(&items), ["Image", "Text"]);
        let img = ImageHistoryFilter { media_only: true, ..Default::default() };
        assert_eq!(img.apply(&items, |_| true).len(), 2);
    }

    #[test]
    fn paging() {
        assert_eq!(next_batch(0, 1200, 500), 0..500);
        assert_eq!(next_batch(1000, 1200, 500), 1000..1200);
        assert_eq!(next_batch(0, 7, 0), 0..7);
        assert_eq!(image_history_columns(900.0, 250), 3);
    }
}
