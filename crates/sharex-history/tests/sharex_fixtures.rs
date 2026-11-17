//! Files ShareX's own HistoryLib wrote (tools/history-harness, run in GMT
//! Standard Time) and what ShareX read back from them (`fixtures/sharex.json`):
//! ShareXrs must read them the same, write the same items into an identical
//! database, and count the same stats. Dates are pinned to London's rules
//! (`RuleZone`), so this runs anywhere, CI included. The culture-dependent
//! queries (NLS sorting and matching) are only in the harness.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDateTime;
use rusqlite::Connection;
use serde_json::{Value, json};
use sharex_history::datetime::{DateTime, DateTimeKind, LocalZone, RuleZone};
use sharex_history::settings::{HistoryColumnLayout, HistoryColumnSettings};
use sharex_history::stats::{self, HistoryStats, HistoryStatsFilter};
use sharex_history::{HistoryItem, HistoryManagerSqlite, Tags, legacy};

const LONDON: RuleZone = RuleZone { standard: 0, eu_dst: true };

fn zone() -> Arc<dyn LocalZone + Send + Sync> {
    Arc::new(LONDON)
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn expected() -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixture("sharex.json")).unwrap()).unwrap()
}

fn cases() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/history-harness/cases.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// A scratch copy, so opening never touches the fixture.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sharex-history-fixtures-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let to = dir.join(format!("{}-{name}", std::thread::current().name().unwrap_or("t").replace("::", "-")));
    if fixture(name).exists() {
        std::fs::copy(fixture(name), &to).unwrap();
    } else {
        let _ = std::fs::remove_file(&to);
    }
    to
}

fn opt(s: &Option<String>) -> Value {
    s.as_ref().map_or(Value::Null, |s| Value::String(s.clone()))
}

fn opt_str(s: Option<&str>) -> Value {
    s.map_or(Value::Null, |s| Value::String(s.into()))
}

fn dump_item(i: &HistoryItem) -> Value {
    let kind = match i.date_time.kind() {
        DateTimeKind::Local => "Local",
        DateTimeKind::Utc => "Utc",
        DateTimeKind::Unspecified => "Unspecified",
    };
    let tags = i.tags.as_ref().map_or(Value::Null, |t| Value::Array(t.iter().map(|(k, v)| json!([k, opt_str(v)])).collect()));
    json!({
        "Id": i.id,
        "FileName": opt(&i.file_name),
        "FilePath": opt(&i.file_path),
        "DateTime": { "Ticks": i.date_time.ticks(), "Kind": kind, "O": i.date_time.to_string_o_in(&LONDON) },
        "Type": opt(&i.r#type),
        "Host": opt(&i.host),
        "URL": opt(&i.url),
        "ThumbnailURL": opt(&i.thumbnail_url),
        "DeletionURL": opt(&i.deletion_url),
        "ShortenedURL": opt(&i.shortened_url),
        "Tags": tags,
        "Favorite": i.favorite(),
        "Tag": opt_str(i.tag_text()),
        "TagsWindowTitle": opt_str(i.tags_window_title()),
        "TagsProcessName": opt_str(i.tags_process_name()),
    })
}

fn dump(items: &[HistoryItem]) -> Value {
    Value::Array(items.iter().map(dump_item).collect())
}

fn load(name: &str) -> Vec<HistoryItem> {
    HistoryManagerSqlite::open_with_zone(scratch(name), zone()).unwrap().load().unwrap()
}

fn raw_rows(path: &Path) -> Value {
    let c = Connection::open(path).unwrap();
    let mut out = Vec::new();
    for sql in ["SELECT type, name, sql FROM sqlite_master ORDER BY name", "SELECT * FROM sqlite_sequence", "SELECT * FROM History ORDER BY Id"] {
        let mut stmt = c.prepare(sql).unwrap();
        let n = stmt.column_count();
        let rows: Vec<Value> = stmt
            .query_map([], |r| {
                Ok(Value::Array(
                    (0..n)
                        .map(|i| match r.get_ref(i).unwrap() {
                            rusqlite::types::ValueRef::Null => json!(null),
                            rusqlite::types::ValueRef::Integer(v) => json!(v),
                            rusqlite::types::ValueRef::Real(v) => json!(v),
                            rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            rusqlite::types::ValueRef::Blob(b) => json!(format!("{b:02x?}")),
                        })
                        .collect(),
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        out.push(Value::Array(rows));
    }
    Value::Array(out)
}

fn spec_date(v: &Value) -> DateTime {
    let Some(o) = v.as_object() else { return DateTime::default() };
    let (kind, s) = o.iter().next().unwrap();
    let wall = NaiveDateTime::parse_from_str(s.as_str().unwrap(), "%Y-%m-%dT%H:%M:%S%.f").unwrap();
    match kind.as_str() {
        "local" => DateTime::new(wall, DateTimeKind::Local),
        "utc" => DateTime::new(wall, DateTimeKind::Utc),
        "unspecified" => DateTime::new(wall, DateTimeKind::Unspecified),
        "utc_to_local" => DateTime::from_utc_to_local(DateTime::new(wall, DateTimeKind::Utc).ticks(), &LONDON),
        k => panic!("{k}"),
    }
}

fn spec_tags(v: &Value) -> Tags {
    v.as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap(), p[1].as_str())).collect()
}

fn spec_item(v: &Value) -> HistoryItem {
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    HistoryItem {
        id: 0,
        file_name: s("FileName"),
        file_path: s("FilePath"),
        date_time: spec_date(v.get("DateTime").unwrap_or(&Value::Null)),
        r#type: s("Type"),
        host: s("Host"),
        url: s("URL"),
        thumbnail_url: s("ThumbnailURL"),
        deletion_url: s("DeletionURL"),
        shortened_url: s("ShortenedURL"),
        tags: v.get("Tags").filter(|t| t.is_array()).map(spec_tags),
    }
}

#[test]
fn reads_what_sharex_wrote() {
    let e = expected();
    assert_eq!(dump(&load("sharex.db")), e["Read"]);
    assert_eq!(dump(&load("sharex-raw.db")), e["ReadRaw"]);
    assert_eq!(dump(&load("sharex-migrated.db")), e["JsonMigrated"]);
    assert_eq!(dump(&load("sharex-migrated-raw.db")), e["JsonRawMigrated"]);
    assert_eq!(dump(&legacy::load_json(&fixture("sharex-History.json"), &LONDON).unwrap()), e["JsonRead"]);
    assert_eq!(dump(&legacy::load_json(&fixture("sharex-raw-History.json"), &LONDON).unwrap()), e["JsonRawRead"]);
    assert_eq!(dump(&legacy::load_xml(&fixture("sharex-History.xml"), &LONDON).unwrap()), e["XmlRead"]);
    assert_eq!(dump(&legacy::load_xml(&fixture("sharex-raw-History.xml"), &LONDON).unwrap()), e["XmlRawRead"]);
}

#[test]
fn writes_what_sharex_writes() {
    let cases = cases();
    let db = scratch("written.db");
    let mut m = HistoryManagerSqlite::open_with_zone(&db, zone()).unwrap();
    let mut items: Vec<HistoryItem> = cases["write"].as_array().unwrap().iter().map(spec_item).collect();
    assert!(m.append_history_items(&mut items));
    assert_eq!(json!(items.iter().map(|i| i.id).collect::<Vec<_>>()), expected()["AppendedIds"]);
    for op in cases["ops"].as_array().unwrap() {
        let loaded = m.load().unwrap();
        let mut item = op.get("id").and_then(|id| loaded.iter().find(|i| i.id == id.as_i64().unwrap()).cloned());
        match op["op"].as_str().unwrap() {
            "favorite" => {
                let it = item.as_mut().unwrap();
                it.set_favorite(!it.favorite());
                m.edit(it).unwrap();
            }
            "tag" => {
                let it = item.as_mut().unwrap();
                it.set_tag(op["value"].as_str());
                m.edit(it).unwrap();
            }
            "set_tags" => {
                let it = item.as_mut().unwrap();
                it.tags = Some(spec_tags(&op["tags"]));
                m.edit(it).unwrap();
            }
            "edit" => {
                let it = item.as_mut().unwrap();
                let s = |k: &str| op.get(k).and_then(Value::as_str).map(str::to_string);
                (it.file_name, it.url, it.host) = (s("FileName"), s("URL"), s("Host"));
                m.edit(it).unwrap();
            }
            "delete" => {
                let ids: Vec<i64> = op["ids"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
                m.delete(&loaded.into_iter().filter(|i| ids.contains(&i.id)).collect::<Vec<_>>()).unwrap();
            }
            "append" => {
                let mut more: Vec<HistoryItem> = op["items"].as_array().unwrap().iter().map(spec_item).collect();
                m.append_history_items(&mut more);
            }
            o => panic!("{o}"),
        }
    }
    drop(m);
    assert_eq!(raw_rows(&db), raw_rows(&fixture("sharex.db")));

    for (json_file, db_file) in [("sharex-History.json", "sharex-migrated.db"), ("sharex-raw-History.json", "sharex-migrated-raw.db")] {
        let db = scratch(&format!("migrated-{db_file}"));
        HistoryManagerSqlite::open_with_zone(&db, zone()).unwrap().migrate_from_json(&fixture(json_file)).unwrap();
        assert_eq!(raw_rows(&db), raw_rows(&fixture(db_file)), "{json_file}");
    }
}

fn tally(t: &stats::HistoryStatsTally) -> Value {
    json!({ "Images": t.images, "Recordings": t.recordings, "Other": t.other })
}

#[test]
fn counts_what_sharex_counts() {
    let e = expected();
    let cases = cases();
    for (db, key) in [("sharex.db", "Stats"), ("sharex-raw.db", "StatsRaw")] {
        let mut items = load(db);
        items.reverse();
        for (case, want) in cases["stats"].as_array().unwrap().iter().zip(e[key].as_array().unwrap()) {
            let st = HistoryStats::calculate(&items, spec_date(&case["now"]), case["first_day"].as_u64().unwrap() as u32);
            let entries = |v: &[stats::HistoryStatsEntry]| Value::Array(v.iter().map(|x| json!([x.name, x.count])).collect());
            let peaks = [HistoryStatsFilter::All, HistoryStatsFilter::Images, HistoryStatsFilter::Recordings].map(|f| st.peak_hour(f));
            let got = json!({
                "Today": st.today.ticks(),
                "Total": tally(&st.total),
                "Daily": st.daily.iter().map(|(k, t)| json!([k, tally(t)])).collect::<Vec<_>>(),
                "Hours": st.hours.iter().map(tally).collect::<Vec<_>>(),
                "Weekdays": st.weekdays.iter().map(tally).collect::<Vec<_>>(),
                "FirstDay": st.first_day.map(|d| d.ticks()),
                "Apps": entries(&st.apps),
                "Destinations": entries(&st.destinations),
                "FileTypes": entries(&st.file_types),
                "TodayTally": tally(&st.today_tally),
                "Past7Days": st.past_7_days,
                "Previous7Days": st.previous_7_days,
                "Streak": st.streak,
                "LongestStreak": st.longest_streak,
                "LongestStreakEnd": st.longest_streak_end.ticks(),
                "BusiestDay": st.busiest_day.map(|d| d.ticks()),
                "BusiestDayCount": st.busiest_day_count,
                "DaysCounted": st.days_counted(),
                "PeakHour": peaks,
                "WeekStart": st.get_week_start(st.today).ticks(),
            });
            let mut want = want.clone();
            want.as_object_mut().unwrap().remove("Ranges");
            assert_eq!(got, want, "{db} {case}");
        }
        if db == "sharex.db" {
            assert_eq!(stats::output_stats(&items).replace("\r\n", "\n"), e["OutputStats"].as_str().unwrap().replace("\r\n", "\n"));
        }
    }
}

#[test]
fn formats_and_columns() {
    let e = expected();
    let cases = cases();
    let f = &cases["formats"];
    let pair = |v: &Value| (v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32);
    let share: Vec<String> = f["share"].as_array().unwrap().iter().map(|p| { let (a, b) = pair(p); stats::format_share(a, b) }).collect();
    assert_eq!(json!(share), e["Formats"]["Share"]);
    let compact: Vec<String> = f["compact"].as_array().unwrap().iter().map(|n| stats::format_compact(n.as_f64().unwrap())).collect();
    assert_eq!(json!(compact), e["Formats"]["Compact"]);
    let scale: Vec<Value> = f["scale"].as_array().unwrap().iter().map(|n| { let (a, b) = stats::hour_chart_scale(n.as_i64().unwrap() as i32); json!([a, b]) }).collect();
    assert_eq!(json!(scale), e["Formats"]["Scale"]);
    let change: Vec<Option<String>> = f["week_change"].as_array().unwrap().iter().map(|p| { let (a, b) = pair(p); stats::week_change(a, b).map(|(_, t)| t) }).collect();
    assert_eq!(json!(change), e["Formats"]["WeekChange"]);
    let columns: Vec<Value> = cases["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let s: Option<Vec<HistoryColumnSettings>> = serde_json::from_value(c.clone()).unwrap();
            serde_json::to_value(HistoryColumnLayout::load(s.as_deref()).to_settings()).unwrap()
        })
        .collect();
    assert_eq!(json!(columns), e["Columns"]);
}
