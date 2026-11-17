//! The windows' settings, kept in `ApplicationConfig.json`
//! (`HistorySettings`, `ImageHistorySettings`), and the History window's
//! column layout (`HistoryColumnLayout`), in ShareX's JSON shape.

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn window_state() -> Value {
    serde_json::json!({ "Location": "0, 0", "Size": "0, 0", "IsMaximized": false })
}

/// `HistorySettings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct HistorySettings {
    pub remember_window_state: bool,
    /// `HelpersLib.WindowState` as ShareX writes it.
    pub window_state: Value,
    pub splitter_distance: i32,
    pub remember_search_text: bool,
    pub search_text: String,
    pub favorites: bool,
    pub history_list_width: f64,
    pub columns: Option<Vec<HistoryColumnSettings>>,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self {
            remember_window_state: true,
            window_state: window_state(),
            splitter_distance: 550,
            remember_search_text: false,
            search_text: String::new(),
            favorites: false,
            history_list_width: 720.0,
            columns: None,
        }
    }
}

/// `ImageHistoryThumbnailHeaderPosition`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImageHistoryThumbnailHeaderPosition {
    None,
    #[default]
    Top,
    Bottom,
}

/// `ImageHistorySettings`. `ThumbnailSize` is a `System.Drawing.Size`,
/// which Json.NET writes as `"250, 150"`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ImageHistorySettings {
    pub remember_window_state: bool,
    pub window_state: Value,
    pub thumbnail_size: String,
    pub thumbnail_header_position: ImageHistoryThumbnailHeaderPosition,
    pub max_item_count: i32,
    pub auto_load_more_items: bool,
    pub filter_missing_files: bool,
    pub image_only: bool,
    pub remember_search_text: bool,
    pub search_text: String,
    pub favorites: bool,
}

impl Default for ImageHistorySettings {
    fn default() -> Self {
        Self {
            remember_window_state: true,
            window_state: window_state(),
            thumbnail_size: "250, 150".into(),
            thumbnail_header_position: ImageHistoryThumbnailHeaderPosition::Top,
            max_item_count: 500,
            auto_load_more_items: true,
            filter_missing_files: false,
            image_only: true,
            remember_search_text: false,
            search_text: String::new(),
            favorites: false,
        }
    }
}

impl ImageHistorySettings {
    /// `ThumbnailSize` as width and height.
    pub fn thumbnail_size(&self) -> (i32, i32) {
        let mut parts = self.thumbnail_size.split(',').map(|p| p.trim().parse().unwrap_or(0));
        (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
    }

    pub fn set_thumbnail_size(&mut self, w: i32, h: i32) {
        self.thumbnail_size = format!("{w}, {h}");
    }

    /// `ImageHistoryEntry`: a card's thumbnail is 32 × 32 at least.
    pub fn card_thumbnail_size(&self) -> (i32, i32) {
        let (w, h) = self.thumbnail_size();
        (w.max(32), h.max(32))
    }
}

/// `HistoryColumn`, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HistoryColumn {
    Icon,
    Date,
    FileName,
    FilePath,
    Type,
    Host,
    URL,
    ShortenedURL,
    ProcessName,
    WindowTitle,
    Tag,
}

impl HistoryColumn {
    pub const CONFIGURABLE: [HistoryColumn; 10] = [
        Self::Date,
        Self::FileName,
        Self::FilePath,
        Self::Type,
        Self::Host,
        Self::URL,
        Self::ShortenedURL,
        Self::ProcessName,
        Self::WindowTitle,
        Self::Tag,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Icon => "Icon",
            Self::Date => "Date",
            Self::FileName => "FileName",
            Self::FilePath => "FilePath",
            Self::Type => "Type",
            Self::Host => "Host",
            Self::URL => "URL",
            Self::ShortenedURL => "ShortenedURL",
            Self::ProcessName => "ProcessName",
            Self::WindowTitle => "WindowTitle",
            Self::Tag => "Tag",
        }
    }

    /// `Enum.TryParse` (case-sensitive, names only... and numbers).
    pub fn parse(s: &str) -> Option<Self> {
        if let Ok(n) = s.trim().parse::<usize>() {
            return [Self::Icon].iter().chain(Self::CONFIGURABLE.iter()).nth(n).copied();
        }
        [Self::Icon].into_iter().chain(Self::CONFIGURABLE).find(|c| c.name() == s.trim())
    }

    /// `Defaults`: shown, width.
    fn default_layout(self) -> (bool, f64) {
        match self {
            Self::Icon => (true, HistoryColumnLayout::ICON_WIDTH),
            Self::Date => (true, 105.0),
            Self::FileName => (true, 220.0),
            Self::FilePath => (false, 250.0),
            Self::Type => (false, 60.0),
            Self::Host => (false, 120.0),
            Self::URL => (true, 250.0),
            Self::ShortenedURL => (false, 180.0),
            Self::ProcessName => (false, 110.0),
            Self::WindowTitle => (false, 200.0),
            Self::Tag => (false, 120.0),
        }
    }
}

/// `HistoryColumnSettings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct HistoryColumnSettings {
    pub name: String,
    pub visible: bool,
    pub width: f64,
}

/// `HistoryColumnLayout`: which columns show and how wide they are; the
/// last visible one fills the rest of the row.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryColumnLayout {
    visible: [bool; 10],
    widths: [f64; 10],
}

impl Default for HistoryColumnLayout {
    fn default() -> Self {
        let mut l = Self { visible: [false; 10], widths: [0.0; 10] };
        l.reset();
        l
    }
}

impl HistoryColumnLayout {
    pub const ICON_WIDTH: f64 = 18.0;
    pub const MIN_COLUMN_WIDTH: f64 = 40.0;

    fn index(c: HistoryColumn) -> Option<usize> {
        HistoryColumn::CONFIGURABLE.iter().position(|&x| x == c)
    }

    pub fn is_visible(&self, c: HistoryColumn) -> bool {
        Self::index(c).is_none_or(|i| self.visible[i])
    }

    pub fn width(&self, c: HistoryColumn) -> f64 {
        Self::index(c).map_or(Self::ICON_WIDTH, |i| self.widths[i])
    }

    /// `FillColumn`.
    pub fn fill_column(&self) -> Option<HistoryColumn> {
        HistoryColumn::CONFIGURABLE.into_iter().rev().find(|&c| self.is_visible(c))
    }

    pub fn visible_count(&self) -> usize {
        self.visible.iter().filter(|&&v| v).count()
    }

    /// `SetVisible`: the last text column can't be hidden. Returns whether it changed.
    pub fn set_visible(&mut self, c: HistoryColumn, visible: bool) -> bool {
        let Some(i) = Self::index(c) else { return false };
        if self.visible[i] == visible || (!visible && self.visible_count() <= 1) {
            return false;
        }
        self.visible[i] = visible;
        true
    }

    /// `Resize`: by `delta`, never under 40.
    pub fn resize(&mut self, c: HistoryColumn, delta: f64) -> bool {
        let Some(i) = Self::index(c) else { return false };
        let w = Self::MIN_COLUMN_WIDTH.max(self.widths[i] + delta);
        let changed = w != self.widths[i];
        self.widths[i] = w;
        changed
    }

    pub fn reset(&mut self) {
        for (i, c) in HistoryColumn::CONFIGURABLE.into_iter().enumerate() {
            (self.visible[i], self.widths[i]) = c.default_layout();
        }
    }

    /// `Load`: defaults, then each known column's saved state (a positive
    /// width, 40 at least); the file name shows if nothing would.
    pub fn load(settings: Option<&[HistoryColumnSettings]>) -> Self {
        let mut l = Self::default();
        if let Some(settings) = settings {
            for s in settings {
                let Some(i) = HistoryColumn::parse(&s.name).and_then(Self::index) else { continue };
                l.visible[i] = s.visible;
                if s.width > 0.0 {
                    l.widths[i] = Self::MIN_COLUMN_WIDTH.max(s.width);
                }
            }
            if l.visible_count() == 0 {
                l.visible[1] = true;
            }
        }
        l
    }

    /// `ToSettings`: every configurable column, widths rounded (half to even).
    pub fn to_settings(&self) -> Vec<HistoryColumnSettings> {
        HistoryColumn::CONFIGURABLE
            .into_iter()
            .enumerate()
            .map(|(i, c)| HistoryColumnSettings { name: c.name().into(), visible: self.visible[i], width: self.widths[i].round_ties_even() })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_json_matches_sharex_defaults() {
        let h = serde_json::to_value(HistorySettings::default()).unwrap();
        assert_eq!(h["HistoryListWidth"], 720.0);
        assert_eq!(h["Columns"], Value::Null);
        let i: ImageHistorySettings = serde_json::from_value(serde_json::json!({ "ThumbnailSize": "300, 200", "ThumbnailHeaderPosition": "Bottom" })).unwrap();
        assert_eq!((i.thumbnail_size(), i.max_item_count), ((300, 200), 500));
    }

    #[test]
    fn columns() {
        let mut l = HistoryColumnLayout::default();
        assert_eq!(l.fill_column(), Some(HistoryColumn::URL));
        assert!(l.set_visible(HistoryColumn::Tag, true));
        assert!(l.resize(HistoryColumn::Date, -100.0));
        assert_eq!(l.width(HistoryColumn::Date), 40.0);
        let saved = l.to_settings();
        assert_eq!(HistoryColumnLayout::load(Some(&saved)), l);
        let none = [HistoryColumnSettings { name: "URL".into(), visible: false, width: 0.0 }];
        let mut only = HistoryColumnLayout::load(Some(&none));
        assert!(only.is_visible(HistoryColumn::FileName));
        assert!(only.set_visible(HistoryColumn::Date, false) && !only.set_visible(HistoryColumn::FileName, false));
    }
}
