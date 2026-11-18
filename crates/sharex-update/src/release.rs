//! `GitHubUpdateChecker.GitHubRelease` / `GitHubAsset` as
//! `JsonConvert.DeserializeObject` fills them: properties matched by name
//! (exact, else ignoring case), unknown ones ignored, `string` properties
//! taking any primitive as .NET prints it, and the value-type properties
//! (`long`, `bool`, `DateTime`) failing the whole read on `null` or a value
//! that doesn't convert, which fails the update check.

use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde_json::Value;
use sharex_uploaders::json;

/// `GitHubRelease` (the members the update check uses).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GitHubRelease {
    pub url: Option<String>,
    pub html_url: Option<String>,
    pub id: i64,
    pub tag_name: Option<String>,
    pub name: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    /// .NET `DateTime` ticks as `CompareTo` sees them (the clock time:
    /// UTC for `Z`, local time for an offset, as written otherwise).
    pub created_at: NaiveDateTime,
    pub published_at: NaiveDateTime,
    /// `null` stays `None`; a `null` element is kept as `None` too.
    pub assets: Option<Vec<Option<GitHubAsset>>>,
    pub body: Option<String>,
}

/// `GitHubAsset`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GitHubAsset {
    /// The API URL (`/repos/{o}/{r}/releases/assets/{id}`), downloaded with
    /// `Accept: application/octet-stream`.
    pub url: Option<String>,
    pub id: i64,
    pub name: Option<String>,
    pub content_type: Option<String>,
    pub size: i64,
    pub browser_download_url: Option<String>,
}

/// `DateTime`'s default (`DateTime.MinValue`), for a missing property.
fn min_date() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(1, 1, 1).expect("date").and_time(NaiveTime::MIN)
}

/// `JsonConvert.DeserializeObject<GitHubRelease>(text)`: `None` for `null`
/// or an empty response.
pub fn parse_release(text: &str) -> Result<Option<GitHubRelease>, String> {
    let Some(value) = json::deserialize(text)? else { return Ok(None) };
    match &value {
        Value::Object(_) => release(&value).map(Some),
        other => Err(format!("Cannot deserialize the current JSON {} into type 'GitHubRelease'.", kind(other))),
    }
}

/// `JsonConvert.DeserializeObject<List<GitHubRelease>>(text)`.
pub fn parse_releases(text: &str) -> Result<Option<Vec<Option<GitHubRelease>>>, String> {
    let Some(value) = json::deserialize(text)? else { return Ok(None) };
    match &value {
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Null => Ok(None),
                Value::Object(_) => release(item).map(Some),
                other => Err(format!("Error converting value {} to type 'GitHubRelease'.", other)),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        other => Err(format!("Cannot deserialize the current JSON {} into type 'List<GitHubRelease>'.", kind(other))),
    }
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        _ => "value",
    }
}

fn release(v: &Value) -> Result<GitHubRelease, String> {
    Ok(GitHubRelease {
        url: string(v, "url")?,
        html_url: string(v, "html_url")?,
        id: long(v, "id")?,
        tag_name: string(v, "tag_name")?,
        name: string(v, "name")?,
        draft: boolean(v, "draft")?,
        prerelease: boolean(v, "prerelease")?,
        created_at: date(v, "created_at")?,
        published_at: date(v, "published_at")?,
        assets: match get(v, "assets") {
            None | Some(Value::Null) => None,
            Some(Value::Array(items)) => Some(
                items
                    .iter()
                    .map(|item| match item {
                        Value::Null => Ok(None),
                        Value::Object(_) => asset(item).map(Some),
                        other => Err(format!("Error converting value {other} to type 'GitHubAsset'.")),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Some(_) => return Err("Cannot deserialize the current JSON object into type 'GitHubAsset[]'.".to_owned()),
        },
        body: string(v, "body")?,
    })
}

fn asset(v: &Value) -> Result<GitHubAsset, String> {
    // The other members are read (and must convert) even though nothing uses them.
    long(v, "download_count")?;
    date(v, "created_at")?;
    date(v, "updated_at")?;
    for name in ["node_id", "label", "state"] {
        string(v, name)?;
    }
    Ok(GitHubAsset {
        url: string(v, "url")?,
        id: long(v, "id")?,
        name: string(v, "name")?,
        content_type: string(v, "content_type")?,
        size: long(v, "size")?,
        browser_download_url: string(v, "browser_download_url")?,
    })
}

/// A property as Newtonsoft fills it: the last one whose name matches,
/// exactly or ignoring case (it sets the property each time it reads one).
fn get<'a>(v: &'a Value, name: &str) -> Option<&'a Value> {
    v.as_object()?.iter().filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v).next_back()
}

fn string(v: &Value, name: &str) -> Result<Option<String>, String> {
    match get(v, name) {
        None => Ok(None),
        Some(value) => json::to_dotnet_string(value).map_err(|_| format!("Unexpected character encountered while parsing value. Path '{name}'.")),
    }
}

fn null_error(name: &str, type_name: &str) -> String {
    format!("Error converting value {{null}} to type '{type_name}'. Path '{name}'.")
}

fn long(v: &Value, name: &str) -> Result<i64, String> {
    match get(v, name) {
        None => Ok(0),
        Some(Value::Null) => Err(null_error(name, "System.Int64")),
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().filter(|f| f.is_finite() && f.abs() < 9.2e18).map(|f| f.round_ties_even() as i64))
            .ok_or_else(|| format!("JSON integer {n} is too large or small for an Int64. Path '{name}'.")),
        Some(Value::String(s)) => {
            s.trim().parse().map_err(|_| format!("Could not convert string to integer: {s}. Path '{name}'."))
        }
        Some(other) => Err(format!("Error converting value {other} to type 'System.Int64'. Path '{name}'.")),
    }
}

fn boolean(v: &Value, name: &str) -> Result<bool, String> {
    match get(v, name) {
        None => Ok(false),
        Some(Value::Null) => Err(null_error(name, "System.Boolean")),
        Some(Value::Bool(b)) => Ok(*b),
        Some(Value::Number(n)) if n.is_i64() || n.is_u64() => Ok(n.as_f64() != Some(0.0)),
        // `bool.TryParse`: "true"/"false" in any case, white space and NULs around it.
        Some(Value::String(s)) => {
            let t = s.trim_matches(|c: char| c.is_whitespace() || c == '\0');
            if t.eq_ignore_ascii_case("true") {
                Ok(true)
            } else if t.eq_ignore_ascii_case("false") {
                Ok(false)
            } else {
                Err(format!("Could not convert string to boolean: {s}. Path '{name}'."))
            }
        }
        Some(other) => Err(format!("Error reading boolean. Unexpected token: {other}. Path '{name}'.")),
    }
}

fn date(v: &Value, name: &str) -> Result<NaiveDateTime, String> {
    match get(v, name) {
        None => Ok(min_date()),
        Some(Value::Null) => Err(null_error(name, "System.DateTime")),
        Some(Value::String(s)) => {
            parse_iso_date(s).ok_or_else(|| format!("Could not convert string to DateTime: {s}. Path '{name}'."))
        }
        Some(other) => Err(format!("Unexpected token parsing date. Expected String, got {other}. Path '{name}'.")),
    }
}

/// Newtonsoft's ISO 8601 reader (`DateTimeParser`, `RoundtripKind`):
/// `yyyy-MM-ddTHH:mm:ss[.fffffff][Z|±HH[:]mm]`. `Z` keeps the UTC clock
/// time, an offset is converted to local time, no zone is taken as is.
pub fn parse_iso_date(text: &str) -> Option<NaiveDateTime> {
    let s = text.as_bytes();
    let two = |i: usize| -> Option<u32> {
        let a = (*s.get(i)? as char).to_digit(10)?;
        let b = (*s.get(i + 1)? as char).to_digit(10)?;
        Some(a * 10 + b)
    };
    if s.len() < 19 || s[4] != b'-' || s[7] != b'-' || s[10] != b'T' || s[13] != b':' || s[16] != b':' {
        return None;
    }
    let year = two(0)? * 100 + two(2)?;
    let (month, day, hour, minute, second) = (two(5)?, two(8)?, two(11)?, two(14)?, two(17)?);
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let mut i = 19;
    let mut ticks = 0u32;
    if s.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while i < s.len() && s[i].is_ascii_digit() {
            if i - start < 7 {
                ticks = ticks * 10 + (s[i] - b'0') as u32;
            }
            i += 1;
        }
        let n = i - start;
        if n == 0 {
            return None;
        }
        for _ in n..7 {
            ticks *= 10;
        }
    }
    let naive = NaiveDate::from_ymd_opt(year as i32, month, day)?
        .and_time(NaiveTime::from_hms_nano_opt(hour, minute, second, ticks * 100)?);
    match s.get(i) {
        None => Some(naive),
        Some(b'Z') if i + 1 == s.len() => Some(naive),
        Some(&sign @ (b'+' | b'-')) => {
            let h = two(i + 1)? as i64;
            let mut j = i + 3;
            let mut m = 0;
            if s.get(j) == Some(&b':') {
                j += 1;
            }
            if j < s.len() {
                m = two(j)? as i64;
                j += 2;
            }
            if j != s.len() {
                return None;
            }
            let offset = if sign == b'-' { -(h * 60 + m) } else { h * 60 + m };
            let utc = naive - chrono::Duration::minutes(offset);
            Some(Utc.from_utc_datetime(&utc).with_timezone(&Local).naive_local())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_releases_as_newtonsoft() {
        let r = parse_release(
            r#"{"TAG_NAME":"v1.2.3","prerelease":"True","id":"5","assets":[null,{"name":"a.zip","size":1.0,"url":"u"}],"published_at":"2024-05-01T10:00:00Z","extra":{}}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.tag_name.as_deref(), Some("v1.2.3"));
        assert!(r.prerelease);
        assert_eq!(r.id, 5);
        let assets = r.assets.unwrap();
        assert_eq!(assets[0], None);
        assert_eq!(assets[1].as_ref().unwrap().size, 1);
        assert_eq!(r.published_at.to_string(), "2024-05-01 10:00:00");
        assert_eq!(parse_release("null").unwrap(), None);
        assert_eq!(parse_release("  ").unwrap(), None);
        assert!(parse_release("[]").is_err());
        assert!(parse_release(r#"{"published_at":null}"#).is_err());
        assert!(parse_release(r#"{"id":null}"#).is_err());
        assert!(parse_release(r#"{"draft":"yes"}"#).is_err());
        assert!(parse_release(r#"{"tag_name":{}}"#).is_err());
        assert!(parse_release(r#"{"assets":[{"size":null}]}"#).is_err());
        assert!(parse_release(r#"{"assets":{}}"#).is_err());
        assert!(parse_release("{").is_err());
        assert_eq!(parse_releases("[null]").unwrap().unwrap(), vec![None]);
        assert!(parse_releases("{}").is_err());
        assert_eq!(parse_release(r#"{"tag_name":12}"#).unwrap().unwrap().tag_name.as_deref(), Some("12"));
    }

    #[test]
    fn reads_iso_dates() {
        assert_eq!(parse_iso_date("2024-05-01T10:00:00.1234567Z").unwrap().to_string(), "2024-05-01 10:00:00.123456700");
        assert_eq!(parse_iso_date("2024-05-01T10:00:00.5").unwrap().to_string(), "2024-05-01 10:00:00.500");
        assert!(parse_iso_date("2024-05-01").is_none());
        assert!(parse_iso_date("2024-05-01T10:00:00Q").is_none());
        assert!(parse_iso_date("2024-13-01T10:00:00Z").is_none());
    }
}
