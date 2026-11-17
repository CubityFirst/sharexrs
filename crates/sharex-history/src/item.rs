//! `ShareX.HistoryLib.HistoryItem`.

use crate::datetime::DateTime;
use crate::tags::Tags;

/// One history entry. Strings are `Option`s because ShareX tells null from
/// empty: a new item's missing URL is null (stored as SQL `NULL`), while an
/// item read back from History.db has `""` for it (`DBNull.ToString()`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HistoryItem {
    /// History.db's row id (not in the JSON file).
    pub id: i64,
    pub file_name: Option<String>,
    pub file_path: Option<String>,
    pub date_time: DateTime,
    /// `EDataType` by name: `Image`, `Text`, `File`, `URL`.
    pub r#type: Option<String>,
    /// The uploader's name (`UploaderHost`), null when nothing was uploaded.
    pub host: Option<String>,
    pub url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub deletion_url: Option<String>,
    pub shortened_url: Option<String>,
    pub tags: Option<Tags>,
}

/// `string.IsNullOrEmpty`.
pub fn is_null_or_empty(s: &Option<String>) -> bool {
    s.as_deref().is_none_or(str::is_empty)
}

/// `string.IsNullOrWhiteSpace` (`char.IsWhiteSpace`).
pub fn is_null_or_white_space(s: Option<&str>) -> bool {
    s.is_none_or(|s| s.chars().all(char::is_whitespace))
}

impl HistoryItem {
    fn tag(&self, key: &str) -> Option<&str> {
        self.tags.as_ref().and_then(|t| t.get(key)).flatten()
    }

    /// `TagsWindowTitle`.
    pub fn tags_window_title(&self) -> Option<&str> {
        self.tag("WindowTitle")
    }

    /// `TagsProcessName`.
    pub fn tags_process_name(&self) -> Option<&str> {
        self.tag("ProcessName")
    }

    /// `Favorite`: a `Favorite` key, whatever its value.
    pub fn favorite(&self) -> bool {
        self.tags.as_ref().is_some_and(|t| t.contains_key("Favorite"))
    }

    /// `Favorite`'s setter: adds `"Favorite": null` or removes it (making
    /// `Tags` if there were none).
    pub fn set_favorite(&mut self, value: bool) {
        let tags = self.tags.get_or_insert_with(Tags::new);
        if value {
            tags.set("Favorite", None);
        } else {
            tags.remove("Favorite");
        }
    }

    /// `Tag`.
    pub fn tag_text(&self) -> Option<&str> {
        self.tag("Tag")
    }

    /// `Tag`'s setter: an empty tag removes it.
    pub fn set_tag(&mut self, value: Option<&str>) {
        let tags = self.tags.get_or_insert_with(Tags::new);
        match value {
            Some(v) if !v.is_empty() => tags.set("Tag", Some(v.to_string())),
            _ => {
                tags.remove("Tag");
            }
        }
    }

    /// `HistoryManager.IsValidHistoryItem`: a file name, a date, and a URL
    /// or a file path.
    pub fn is_valid(&self) -> bool {
        !is_null_or_empty(&self.file_name)
            && !self.date_time.is_min_value()
            && (!is_null_or_empty(&self.url) || !is_null_or_empty(&self.file_path))
    }

    /// `HistoryItemIconConverter`: the History list's icon, an `AppIcons`
    /// key: a star for favourites, else by type.
    pub fn icon_key(&self) -> &'static str {
        if self.favorite() {
            return "Star";
        }
        match self.r#type.as_deref().map(str::to_lowercase).as_deref() {
            Some("image") => "Image",
            Some("text") => "FileText",
            Some("file") => "File",
            _ => "Link",
        }
    }

    /// `ImageHistoryEntry.FileName`: the file name, else the path's.
    pub fn display_file_name(&self) -> String {
        match self.file_name.as_deref() {
            Some(n) if !n.chars().all(char::is_whitespace) => n.to_string(),
            _ => self.file_path.as_deref().map(crate::paths::file_name).unwrap_or_default().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorite_and_tag() {
        let mut item = HistoryItem::default();
        assert!(!item.favorite());
        item.set_favorite(false);
        assert_eq!(item.tags.as_ref().map(Tags::to_json).as_deref(), Some("{}"));
        item.set_favorite(true);
        item.set_tag(Some("work"));
        assert!(item.favorite());
        assert_eq!(item.tag_text(), Some("work"));
        item.set_tag(Some(""));
        assert_eq!(item.tags.as_ref().unwrap().to_json(), r#"{"Favorite":null}"#);
    }
}
