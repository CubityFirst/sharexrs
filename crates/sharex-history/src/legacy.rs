//! The history files before History.db, and the step that retires them.
//!
//! - History.json (`HistoryManagerJSON`, ShareX 11 to 16): the items as
//!   Json.NET objects (`DefaultValueHandling.Ignore`, indented) separated by
//!   `,\r\n`, without the surrounding brackets so new items can be appended.
//!   ShareX migrates it into History.db at startup (`MigrateHistoryFile`).
//! - History.xml (`HistoryManagerXML`, older): `<HistoryItem>` elements with
//!   `Filename`, `Filepath`, `DateTimeUtc`, `Type`, `Host`, `URL`,
//!   `ThumbnailURL`, `DeletionURL`, `ShortenedURL` children, no root. ShareX
//!   22.4 can still read it but never migrates it.

use std::path::Path;

use serde_json::Value;

use crate::datetime::{DateTime, DateTimeKind, LocalZone, TICKS_PER_SECOND};
use crate::error::HistoryError;
use crate::item::HistoryItem;
use crate::tags::Tags;

/// `File.ReadAllText(path, Encoding.UTF8)`: a byte order mark picks UTF-16
/// or UTF-8 (and is dropped); without one, UTF-8.
pub fn read_text(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let utf16 = |b: &[u8], le: bool| {
        let units: Vec<u16> =
            b.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    Ok(match bytes.as_slice() {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        b => String::from_utf8_lossy(b).into_owned(),
    })
}

/// `HistoryManagerJSON.Load`: nothing for a missing or empty file.
pub fn load_json(path: &Path, zone: &dyn LocalZone) -> Result<Vec<HistoryItem>, HistoryError> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let json = read_text(path)?;
    if json.is_empty() {
        return Ok(Vec::new());
    }
    parse_json(&json, zone)
}

/// The file's text as `JsonConvert.DeserializeObject<List<HistoryItem>>("[" + json + "]")`.
pub fn parse_json(json: &str, zone: &dyn LocalZone) -> Result<Vec<HistoryItem>, HistoryError> {
    let values: Vec<Value> = serde_json::from_str(&format!("[{json}]"))?;
    values.iter().map(|v| item_from_json(v, zone)).collect()
}

/// One object as Json.NET populates a `HistoryItem`: property names match
/// ignoring case, the `[JsonIgnore]` ones and unknown ones are skipped.
fn item_from_json(v: &Value, zone: &dyn LocalZone) -> Result<HistoryItem, HistoryError> {
    let obj = v.as_object().ok_or_else(|| HistoryError::Format(format!("not a history item: {v}")))?;
    let mut item = HistoryItem::default();
    for (key, value) in obj {
        let text = || -> Result<Option<String>, HistoryError> {
            match value {
                Value::Null => Ok(None),
                Value::String(s) => Ok(Some(s.clone())),
                Value::Bool(b) => Ok(Some(b.to_string())),
                Value::Number(n) => Ok(Some(n.to_string())),
                _ => Err(HistoryError::Format(format!("Unexpected value for {key}: {value}"))),
            }
        };
        match key.to_ascii_lowercase().as_str() {
            "filename" => item.file_name = text()?,
            "filepath" => item.file_path = text()?,
            "datetime" => item.date_time = json_date(value, zone)?,
            "type" => item.r#type = text()?,
            "host" => item.host = text()?,
            "url" => item.url = text()?,
            "thumbnailurl" => item.thumbnail_url = text()?,
            "deletionurl" => item.deletion_url = text()?,
            "shortenedurl" => item.shortened_url = text()?,
            "tags" => {
                item.tags = match value {
                    Value::Null => None,
                    Value::Object(_) => Tags::from_value(value),
                    _ => return Err(HistoryError::Format(format!("Unexpected value for Tags: {value}"))),
                }
            }
            _ => {}
        }
    }
    Ok(item)
}

/// A `DateTime` property: Json.NET's ISO 8601 (`RoundtripKind`) or
/// Microsoft `/Date(ms[±hhmm])/` forms.
fn json_date(value: &Value, zone: &dyn LocalZone) -> Result<DateTime, HistoryError> {
    let bad = || HistoryError::Format(format!("Could not convert string to DateTime: {value}."));
    let s = value.as_str().ok_or_else(bad)?;
    if let Some(inner) = s.strip_prefix("/Date(").and_then(|r| r.strip_suffix(")/")) {
        let split = inner[1..].find(['+', '-']).map(|i| i + 1);
        let (ms, offset) = match split {
            Some(i) => (&inner[..i], Some(&inner[i..])),
            None => (inner, None),
        };
        let ms: i64 = ms.parse().map_err(|_| bad())?;
        // 1970-01-01 in ticks.
        let utc = 621_355_968_000_000_000i64 + ms * (TICKS_PER_SECOND / 1000);
        return Ok(match offset {
            Some(_) => DateTime::from_utc_to_local(utc, zone),
            None => DateTime::from_ticks(utc, DateTimeKind::Utc),
        });
    }
    if s.len() >= 19 && s.len() <= 40 && s.as_bytes()[0].is_ascii_digit() && s.as_bytes()[10] == b'T' {
        return DateTime::parse_json_in(s, zone).ok_or_else(bad);
    }
    Err(bad())
}

/// `HistoryManagerXML.Load`: every `HistoryItem` element (not inside
/// another), its children by local name (the last wins), each value the
/// element's text without whitespace-only nodes (`IgnoreWhitespace`), the
/// date by `DateTime.TryParse` (left at `MinValue` when it fails).
pub fn load_xml(path: &Path, zone: &dyn LocalZone) -> Result<Vec<HistoryItem>, HistoryError> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    parse_xml(&read_text(path)?, zone)
}

pub fn parse_xml(text: &str, zone: &dyn LocalZone) -> Result<Vec<HistoryItem>, HistoryError> {
    let mut body = text.trim_start();
    if body.starts_with("<?xml")
        && let Some(end) = body.find("?>")
    {
        body = &body[end + 2..];
    }
    let wrapped = format!("<_sharexrs_root>{body}</_sharexrs_root>");
    let doc = roxmltree::Document::parse(&wrapped).map_err(|e| HistoryError::Xml(e.to_string()))?;
    let mut items = Vec::new();
    collect_xml(doc.root_element(), zone, &mut items);
    Ok(items)
}

fn collect_xml(node: roxmltree::Node<'_, '_>, zone: &dyn LocalZone, items: &mut Vec<HistoryItem>) {
    for child in node.children().filter(roxmltree::Node::is_element) {
        if child.tag_name().name() == "HistoryItem" && child.tag_name().namespace().is_none() {
            items.push(xml_item(child, zone));
        } else {
            collect_xml(child, zone, items);
        }
    }
}

/// `XElement.Value` with insignificant whitespace dropped.
fn xml_value(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .filter(|t| !t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')))
        .collect()
}

fn xml_item(element: roxmltree::Node<'_, '_>, zone: &dyn LocalZone) -> HistoryItem {
    let mut hi = HistoryItem::default();
    for child in element.children().filter(roxmltree::Node::is_element) {
        let value = xml_value(child);
        match child.tag_name().name() {
            "Filename" => hi.file_name = Some(value),
            "Filepath" => hi.file_path = Some(value),
            "DateTimeUtc" => {
                if let Some(d) = DateTime::try_parse_in(&value, zone) {
                    hi.date_time = d;
                }
            }
            "Type" => hi.r#type = Some(value),
            "Host" => hi.host = Some(value),
            "URL" => hi.url = Some(value),
            "ThumbnailURL" => hi.thumbnail_url = Some(value),
            "DeletionURL" => hi.deletion_url = Some(value),
            "ShortenedURL" => hi.shortened_url = Some(value),
            _ => {}
        }
    }
    hi
}

/// `FileHelpers.MoveFile(filePath, destinationFolder)`: into the folder
/// under the same name, replacing a file already there.
pub fn move_file(file_path: &Path, destination_folder: &Path) -> std::io::Result<Option<std::path::PathBuf>> {
    let Some(name) = file_path.file_name().filter(|_| file_path.is_file()) else { return Ok(None) };
    std::fs::create_dir_all(destination_folder)?;
    let destination = destination_folder.join(name);
    if destination.exists() {
        std::fs::remove_file(&destination)?;
    }
    std::fs::rename(file_path, &destination)?;
    Ok(Some(destination))
}

/// `SettingManager.MigrateHistoryFile`, run at startup before the history
/// is opened: if History.json is there, its items go into a new History.db
/// (only when there's no History.db yet), then History.json moves into the
/// backup folder either way. `None` paths are sandbox mode (no history).
/// Returns whether a migration ran.
pub fn migrate_history_file(
    history_file: Option<&Path>,
    history_file_old: Option<&Path>,
    backup_folder: &Path,
    zone: std::sync::Arc<dyn LocalZone + Send + Sync>,
) -> Result<bool, HistoryError> {
    let (Some(db), Some(old)) = (history_file, history_file_old) else { return Ok(false) };
    if !old.is_file() {
        return Ok(false);
    }
    let mut migrated = false;
    if !db.exists() {
        eprintln!("Migrating JSON history file \"{}\" to SQLite history file \"{}\"", old.display(), db.display());
        let mut manager = crate::sqlite::HistoryManagerSqlite::open_with_zone(db, zone)?;
        manager.migrate_from_json(old)?;
        migrated = true;
    }
    move_file(old, backup_folder)?;
    Ok(migrated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datetime::RuleZone;

    const LONDON: RuleZone = RuleZone { standard: 0, eu_dst: true };

    #[test]
    fn json_items() {
        let json = "{\r\n  \"FileName\": \"a.png\",\r\n  \"DateTime\": \"2016-05-10T13:33:56.5556426+03:00\",\r\n  \"type\": \"Image\",\r\n  \"Tags\": {\r\n    \"ProcessName\": \"chrome\"\r\n  }\r\n},\r\n{\r\n  \"FileName\": \"b.txt\",\r\n  \"DateTime\": \"2016-05-10T13:33:56Z\"\r\n}";
        let items = parse_json(json, &LONDON).unwrap();
        assert_eq!(items[0].r#type.as_deref(), Some("Image"));
        assert_eq!(items[0].date_time.to_string_o_in(&LONDON), "2016-05-10T11:33:56.5556426+01:00");
        assert_eq!(items[0].tags_process_name(), Some("chrome"));
        assert_eq!(items[1].date_time.kind(), DateTimeKind::Utc);
        assert_eq!(items[1].url, None);
    }

    #[test]
    fn xml_items() {
        let xml = "\u{feff}<HistoryItem>\r\n    <Filename>a.png</Filename>\r\n    <DateTimeUtc>2012-01-02T03:04:05.0000000Z</DateTimeUtc>\r\n    <URL>http://x/a.png</URL>\r\n</HistoryItem>\r\n<HistoryItem><Filename>b &amp; c</Filename><DateTimeUtc>nope</DateTimeUtc></HistoryItem>";
        let items = parse_xml(xml.trim_start_matches('\u{feff}'), &LONDON).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].date_time.to_string_o_in(&LONDON), "2012-01-02T03:04:05.0000000+00:00");
        assert_eq!(items[1].file_name.as_deref(), Some("b & c"));
        assert!(items[1].date_time.is_min_value());
        assert_eq!(items[1].file_path, None);
    }
}
