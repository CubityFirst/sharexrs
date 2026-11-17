//! Compares ShareXrs's history with ShareX's own HistoryLib (see
//! tools/history-harness/README.md):
//!
//!   cargo run -p sharex-history --example history_harness -- <HistoryHarness.exe> <cases.json> <work dir>
//!
//! Runs ShareX's side first (`HistoryHarness sharex`), then reads ShareX's
//! files with ShareXrs, writes the same items with ShareXrs and has ShareX
//! read them back, migrates the same legacy files, runs the same queries and
//! stats, and compares everything. Writes <work>/report.md; exits non-zero
//! on any difference.

use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::NaiveDateTime;
use rusqlite::Connection;
use serde_json::{Value, json};
use sharex_history::actions;
use sharex_history::datetime::{DateTime, DateTimeKind, SystemZone};
use sharex_history::query::{self, HistoryFilter, ImageHistoryFilter};
use sharex_history::settings::{HistoryColumnLayout, HistoryColumnSettings};
use sharex_history::stats::{self, CalendarSpan, HistoryStats, HistoryStatsFilter};
use sharex_history::{HistoryItem, HistoryManagerSqlite, Tags, legacy};

struct Report {
    lines: Vec<String>,
    failures: usize,
    checks: usize,
}

impl Report {
    fn check(&mut self, name: &str, sharex: &Value, rust: &Value) {
        self.checks += 1;
        if sharex == rust {
            self.lines.push(format!("- ✅ {name}"));
        } else {
            self.failures += 1;
            self.lines.push(format!("- ❌ {name}"));
            let mut diffs = Vec::new();
            diff("", sharex, rust, &mut diffs);
            for d in diffs.iter().take(12) {
                self.lines.push(format!("  - {d}"));
            }
            if diffs.len() > 12 {
                self.lines.push(format!("  - … {} more", diffs.len() - 12));
            }
        }
    }
}

fn diff(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                diff(&format!("{path}.{k}"), v, y.get(k).unwrap_or(&Value::Null), out);
            }
            for (k, v) in y {
                if !x.contains_key(k) {
                    diff(&format!("{path}.{k}"), &Value::Null, v, out);
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                diff(&format!("{path}[{i}]"), v, w, out);
            }
        }
        _ if a != b => out.push(format!("`{path}`: ShareX `{a}` / ShareXrs `{b}`")),
        _ => {}
    }
}

// ---- the cases' items --------------------------------------------------

fn spec_date(v: &Value) -> DateTime {
    let Some(o) = v.as_object() else { return DateTime::default() };
    let (kind, s) = o.iter().next().unwrap();
    let wall = NaiveDateTime::parse_from_str(s.as_str().unwrap(), "%Y-%m-%dT%H:%M:%S%.f").unwrap();
    match kind.as_str() {
        "local" => DateTime::new(wall, DateTimeKind::Local),
        "utc" => DateTime::new(wall, DateTimeKind::Utc),
        "unspecified" => DateTime::new(wall, DateTimeKind::Unspecified),
        "utc_to_local" => DateTime::from_utc_to_local(DateTime::new(wall, DateTimeKind::Utc).ticks(), &SystemZone),
        k => panic!("date kind {k}"),
    }
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

fn spec_tags(v: &Value) -> Tags {
    v.as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap(), p[1].as_str())).collect()
}

fn spec_item(v: &Value) -> HistoryItem {
    HistoryItem {
        id: 0,
        file_name: s(v, "FileName"),
        file_path: s(v, "FilePath"),
        date_time: spec_date(v.get("DateTime").unwrap_or(&Value::Null)),
        r#type: s(v, "Type"),
        host: s(v, "Host"),
        url: s(v, "URL"),
        thumbnail_url: s(v, "ThumbnailURL"),
        deletion_url: s(v, "DeletionURL"),
        shortened_url: s(v, "ShortenedURL"),
        tags: v.get("Tags").filter(|t| t.is_array()).map(spec_tags),
    }
}

// ---- dumps in the .NET harness's shape -----------------------------------

fn opt(s: &Option<String>) -> Value {
    s.as_ref().map_or(Value::Null, |s| Value::String(s.clone()))
}

fn opt_str(s: Option<&str>) -> Value {
    s.map_or(Value::Null, |s| Value::String(s.into()))
}

fn dump_date(d: &DateTime) -> Value {
    let kind = match d.kind() {
        DateTimeKind::Local => "Local",
        DateTimeKind::Utc => "Utc",
        DateTimeKind::Unspecified => "Unspecified",
    };
    json!({ "Ticks": d.ticks(), "Kind": kind, "O": d.to_string_o() })
}

fn dump_item(i: &HistoryItem) -> Value {
    let tags = i.tags.as_ref().map_or(Value::Null, |t| Value::Array(t.iter().map(|(k, v)| json!([k, opt_str(v)])).collect()));
    json!({
        "Id": i.id,
        "FileName": opt(&i.file_name),
        "FilePath": opt(&i.file_path),
        "DateTime": dump_date(&i.date_time),
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

fn dump_items(items: &[HistoryItem]) -> Value {
    Value::Array(items.iter().map(dump_item).collect())
}

/// ShareX's `ReadDb`: the items, or the kind of error (compared by presence).
fn read_db(path: &Path) -> Value {
    match HistoryManagerSqlite::open(path).and_then(|m| m.load()) {
        Ok(items) => dump_items(&items),
        Err(e) => json!({ "Error": format!("{e}") }),
    }
}

fn error_shape(v: &Value) -> Value {
    if v.get("Error").is_some() { json!("error") } else { v.clone() }
}

/// Every stored value with its SQLite type, the schema and the sequence.
fn raw_db(path: &Path) -> Value {
    let c = Connection::open(path).unwrap();
    let mut out = serde_json::Map::new();
    let q = |sql: &str| -> Vec<Value> {
        let mut stmt = c.prepare(sql).unwrap();
        let n = stmt.column_count();
        stmt.query_map([], |r| {
            Ok(Value::Array(
                (0..n)
                    .map(|i| match r.get_ref(i).unwrap() {
                        rusqlite::types::ValueRef::Null => json!(["null"]),
                        rusqlite::types::ValueRef::Integer(v) => json!(["integer", v]),
                        rusqlite::types::ValueRef::Real(v) => json!(["real", v]),
                        rusqlite::types::ValueRef::Text(t) => json!(["text", String::from_utf8_lossy(t)]),
                        rusqlite::types::ValueRef::Blob(b) => json!(["blob", format!("{b:02x?}")]),
                    })
                    .collect(),
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
    };
    out.insert("schema".into(), Value::Array(q("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY name")));
    out.insert("sequence".into(), Value::Array(q("SELECT * FROM sqlite_sequence")));
    out.insert("rows".into(), Value::Array(q("SELECT * FROM History ORDER BY Id")));
    for pragma in ["encoding", "page_size", "user_version", "application_id", "auto_vacuum", "journal_mode"] {
        out.insert(pragma.into(), Value::Array(q(&format!("PRAGMA {pragma}"))));
    }
    Value::Object(out)
}

// ---- queries and stats ---------------------------------------------------

fn ids(items: &[&HistoryItem]) -> Value {
    Value::Array(items.iter().map(|i| json!(i.id)).collect())
}

fn tally(t: &stats::HistoryStatsTally) -> Value {
    json!({ "Images": t.images, "Recordings": t.recordings, "Other": t.other })
}

fn entries(e: &[stats::HistoryStatsEntry]) -> Value {
    Value::Array(e.iter().map(|x| json!([x.name, x.count])).collect())
}

fn dump_stats(st: &HistoryStats) -> Value {
    let ranges: Vec<Value> = st
        .calendar_spans()
        .into_iter()
        .map(|span| {
            let n = match span {
                CalendarSpan::PastYear => 0,
                CalendarSpan::AllYears => -1,
                CalendarSpan::Year(y) => y,
            };
            match st.calendar_range(span) {
                Some((from, to)) => json!([n, [from.ticks(), to.ticks(), st.count_days(from, to, |t| t.total() > 0)]]),
                None => json!([n, null]),
            }
        })
        .collect();
    let peaks = [HistoryStatsFilter::All, HistoryStatsFilter::Images, HistoryStatsFilter::Recordings].map(|f| st.peak_hour(f));
    json!({
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
        "Ranges": ranges,
        "WeekStart": st.get_week_start(st.today).ticks(),
    })
}

fn queries(cases: &Value, items: &[HistoryItem]) -> Value {
    let filters: Vec<Value> = cases["filters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let b = |k: &str, d: bool| f.get(k).and_then(Value::as_bool).unwrap_or(d);
            let filter = HistoryFilter {
                filename: s(f, "Filename").unwrap_or_default(),
                url: s(f, "URL").unwrap_or_default(),
                filter_date: b("FilterDate", false),
                from_date: f.get("FromDate").map(spec_date).unwrap_or_default(),
                to_date: f.get("ToDate").map(spec_date).unwrap_or_default(),
                filter_type: b("FilterType", false),
                r#type: s(f, "Type"),
                filter_host: b("FilterHost", false),
                host: s(f, "Host").unwrap_or_default(),
                filter_favorites: b("FilterFavorites", false),
                max_item_count: f.get("MaxItemCount").and_then(Value::as_i64).unwrap_or(0) as i32,
                search_in_tags: b("SearchInTags", true),
            };
            ids(&filter.apply(items))
        })
        .collect();
    let existing: Vec<&str> = cases["existing_files"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    let image_filters: Vec<Value> = cases["image_filters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let filter = ImageHistoryFilter {
                search: s(f, "search").unwrap_or_default(),
                favorites: f["favorites"].as_bool().unwrap(),
                media_only: f["media_only"].as_bool().unwrap(),
                filter_missing: f["filter_missing"].as_bool().unwrap(),
            };
            ids(&filter.apply(items, |p| existing.contains(&p)))
        })
        .collect();
    let stats: Vec<Value> = cases["stats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| dump_stats(&HistoryStats::calculate(items, spec_date(&c["now"]), c["first_day"].as_u64().unwrap() as u32)))
        .collect();
    json!({
        "Types": query::distinct_types(items),
        "Hosts": query::distinct_hosts(items),
        "ProcessNames": query::process_names(items),
        "Filters": filters,
        "ImageFilters": image_filters,
        "OutputStats": stats::output_stats(items),
        "Stats": stats,
    })
}

fn formats(cases: &Value) -> Value {
    use sharex_strings::history_lib as hs;
    let f = &cases["formats"];
    let arr = |k: &str| f[k].as_array().unwrap().clone();
    let pair = |v: &Value| (v[0].as_i64().unwrap() as i32, v[1].as_i64().unwrap() as i32);
    json!({
        "Share": arr("share").iter().map(|p| { let (a, b) = pair(p); stats::format_share(a, b) }).collect::<Vec<_>>(),
        "Number": arr("number").iter().map(|n| stats::format_number(n.as_i64().unwrap(), ",")).collect::<Vec<_>>(),
        "Compact": arr("compact").iter().map(|n| stats::format_compact(n.as_f64().unwrap())).collect::<Vec<_>>(),
        "Hour": (0..25).map(stats::format_hour_span).collect::<Vec<_>>(),
        "Rate": arr("rate").iter().map(|p| {
            let (total, days) = pair(p);
            let (per_day, n) = stats::rate(total, days);
            (if per_day { hs::StatsWindow_AboutADay() } else { hs::StatsWindow_AboutAWeek() }).replace("{0}", &n)
        }).collect::<Vec<_>>(),
        "Scale": arr("scale").iter().map(|n| { let (a, b) = stats::hour_chart_scale(n.as_i64().unwrap() as i32); json!([a, b]) }).collect::<Vec<_>>(),
        "Level": arr("level").iter().map(|p| {
            let steps: Vec<i32> = p[1].as_array().unwrap().iter().map(|x| x.as_i64().unwrap() as i32).collect();
            stats::calendar_level(p[0].as_i64().unwrap() as i32, [steps[0], steps[1], steps[2]]) as i32
        }).collect::<Vec<_>>(),
        "WeekChange": arr("week_change").iter().map(|p| { let (a, b) = pair(p); stats::week_change(a, b).map(|(_, t)| t) }).collect::<Vec<_>>(),
    })
}

fn columns(cases: &Value) -> Value {
    Value::Array(
        cases["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let settings: Option<Vec<HistoryColumnSettings>> = serde_json::from_value(c.clone()).unwrap();
                serde_json::to_value(HistoryColumnLayout::load(settings.as_deref()).to_settings()).unwrap()
            })
            .collect(),
    )
}

// ---- the run -----------------------------------------------------------------

fn sharex(exe: &Path, args: &[&str]) {
    let status = Command::new(exe).args(args).status().expect("run HistoryHarness");
    assert!(status.success(), "HistoryHarness {args:?} failed");
}

/// Each app appending while the other holds a write transaction for 2 s:
/// the append waits (busy timeout, then the command's retries) and lands.
fn concurrency(exe: &Path, work: &Path, r: &mut Report) {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let item = json!({ "FileName": "late.png", "FilePath": "C:\\late.png", "DateTime": { "local": "2026-11-17T12:00:01.0000000" }, "Type": "Image" });

    // ShareX holds, ShareXrs appends.
    let db = work.join("concurrent-1.db");
    let mut hold = Command::new(exe).args(["hold", db.to_str().unwrap(), "2000"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(hold.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let start = Instant::now();
    let mut m = HistoryManagerSqlite::open(&db).unwrap();
    let mut it = spec_item(&item);
    let ok = m.append_history_item(&mut it);
    let waited = start.elapsed();
    drop(m);
    hold.wait().unwrap();
    let rows = raw_db(&db)["rows"].as_array().unwrap().len();
    r.lines.push(format!("ShareX held the database for 2 s: ShareXrs's append waited {} ms.", waited.as_millis()));
    r.check("ShareXrs appends while ShareX writes", &json!([true, true, 2, 2]), &json!([ok, waited >= Duration::from_millis(1500), it.id, rows]));

    // ShareXrs holds, ShareX appends.
    let db = work.join("concurrent-2.db");
    drop(HistoryManagerSqlite::open(&db).unwrap());
    let c = Connection::open(&db).unwrap();
    c.execute_batch("BEGIN IMMEDIATE; INSERT INTO History (FileName, FilePath, DateTime) VALUES ('held.png', 'C:\\held.png', '2026-11-17T12:00:00.0000000+00:00');").unwrap();
    let append = Command::new(exe).args(["append", db.to_str().unwrap(), &item.to_string()]).stdout(Stdio::piped()).spawn().unwrap();
    std::thread::sleep(Duration::from_millis(2000));
    c.execute_batch("COMMIT;").unwrap();
    drop(c);
    let out = append.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let (status, ms) = out.split_once(' ').unwrap_or((&out, "0"));
    let rows = raw_db(&db)["rows"].as_array().unwrap().len();
    r.lines.push(format!("ShareXrs held the database for 2 s: ShareX's append took {ms} ms."));
    r.check(
        "ShareX appends while ShareXrs writes",
        &json!(["ok", true, 2]),
        &json!([status, ms.parse::<u64>().unwrap_or(0) >= 1500, rows]),
    );
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (exe, cases_path, work) = (PathBuf::from(&args[1]), PathBuf::from(&args[2]), PathBuf::from(&args[3]));
    std::fs::create_dir_all(&work).unwrap();
    for e in std::fs::read_dir(&work).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::remove_file(e.path()).unwrap();
        }
    }
    let cases = read_json(&cases_path);
    sharex(&exe, &["sharex", cases_path.to_str().unwrap(), work.to_str().unwrap()]);
    let sx = read_json(&work.join("sharex.json"));
    let w = |n: &str| work.join(n);
    let mut r = Report { lines: Vec::new(), failures: 0, checks: 0 };

    r.lines.push(format!(
        "SQLite: ShareX {} (e_sqlite3), ShareXrs {} (rusqlite bundled). ShareX's BeginTransaction: second writer `{}`, journal `{}`.\n",
        sx["SqliteVersion"].as_str().unwrap(),
        rusqlite::version(),
        sx["Transactions"]["SecondWriterDuringBeginTransaction"].as_str().unwrap(),
        sx["Transactions"]["JournalMode"].as_str().unwrap()
    ));

    r.lines.push("## ShareXrs reads ShareX's files".into());
    r.check("History.db written and edited by ShareX", &sx["Read"], &read_db(&w("sharex.db")));
    r.check("History.db with odd stored values", &sx["ReadRaw"], &read_db(&w("sharex-raw.db")));
    for (name, v) in sx["RawErrors"].as_object().unwrap() {
        let rust = read_db(&w(&format!("sharex-raw-{name}.db")));
        r.check(&format!("load fails on {name}"), &error_shape(v), &error_shape(&rust));
    }
    let load_json = |p: &str| dump_items(&legacy::load_json(&w(p), &SystemZone).unwrap());
    let load_xml = |p: &str| dump_items(&legacy::load_xml(&w(p), &SystemZone).unwrap());
    r.check("History.json written by ShareX", &sx["JsonRead"], &load_json("sharex-History.json"));
    r.check("History.json, hand-written legacy forms", &sx["JsonRawRead"], &load_json("sharex-raw-History.json"));
    r.check("History.xml written by ShareX", &sx["XmlRead"], &load_xml("sharex-History.xml"));
    r.check("History.xml, hand-written", &sx["XmlRawRead"], &load_xml("sharex-raw-History.xml"));

    r.lines.push("\n## ShareXrs writes, ShareX reads".into());
    let rust_db = w("rust.db");
    {
        let mut m = HistoryManagerSqlite::open(&rust_db).unwrap();
        let mut items: Vec<HistoryItem> = cases["write"].as_array().unwrap().iter().map(spec_item).collect();
        m.append_history_items(&mut items);
        r.check("ids given by AppendHistoryItems", &sx["AppendedIds"], &json!(items.iter().map(|i| i.id).collect::<Vec<_>>()));
        for op in cases["ops"].as_array().unwrap() {
            let loaded = m.load().unwrap();
            let pick = |id: &Value| loaded.iter().find(|i| i.id == id.as_i64().unwrap()).cloned();
            let mut item = op.get("id").and_then(pick);
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
                    it.file_name = s(op, "FileName");
                    it.url = s(op, "URL");
                    it.host = s(op, "Host");
                    m.edit(it).unwrap();
                }
                "delete" => {
                    let ids: Vec<i64> = op["ids"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
                    let doomed: Vec<HistoryItem> = loaded.into_iter().filter(|i| ids.contains(&i.id)).collect();
                    m.delete(&doomed).unwrap();
                }
                "append" => {
                    let mut more: Vec<HistoryItem> = op["items"].as_array().unwrap().iter().map(spec_item).collect();
                    m.append_history_items(&mut more);
                }
                o => panic!("op {o}"),
            }
        }
    }
    r.check("stored values, schema and sequence (ShareXrs's History.db vs ShareX's)", &raw_db(&w("sharex.db")), &raw_db(&rust_db));
    sharex(&exe, &["read", rust_db.to_str().unwrap(), w("sharex-reads-rust.json").to_str().unwrap()]);
    r.check("ShareX reading ShareXrs's History.db", &sx["Read"], &read_json(&w("sharex-reads-rust.json")));

    r.lines.push("\n## Migration from History.json".into());
    for (json_file, sharex_db, sharex_read) in [
        ("sharex-History.json", "sharex-migrated.db", "JsonMigrated"),
        ("sharex-raw-History.json", "sharex-migrated-raw.db", "JsonRawMigrated"),
    ] {
        let rust_db = w(&format!("rust-{sharex_db}"));
        HistoryManagerSqlite::open(&rust_db).unwrap().migrate_from_json(&w(json_file)).unwrap();
        r.check(&format!("{json_file}: stored values"), &raw_db(&w(sharex_db)), &raw_db(&rust_db));
        r.check(&format!("{json_file}: read back"), &sx[sharex_read], &read_db(&rust_db));
    }
    // SettingManager.MigrateHistoryFile in a scratch personal folder.
    let personal = w("personal");
    let _ = std::fs::remove_dir_all(&personal);
    std::fs::create_dir_all(&personal).unwrap();
    std::fs::copy(w("sharex-History.json"), personal.join("History.json")).unwrap();
    let migrated = sharex_history::migrate_history_file(
        Some(&personal.join("History.db")),
        Some(&personal.join("History.json")),
        &personal.join("Backup"),
        std::sync::Arc::new(SystemZone),
    )
    .unwrap();
    r.check(
        "MigrateHistoryFile: History.db made, History.json moved to Backup",
        &json!([true, true, false, true]),
        &json!([
            migrated,
            personal.join("History.db").is_file(),
            personal.join("History.json").exists(),
            personal.join("Backup").join("History.json").is_file()
        ]),
    );
    r.check("MigrateHistoryFile: items", &raw_db(&w("sharex-migrated.db"))["rows"], &raw_db(&personal.join("History.db"))["rows"]);

    r.lines.push("\n## Queries and stats".into());
    let items = HistoryManagerSqlite::open(w("sharex.db")).unwrap().load_newest_first().unwrap();
    let rq = queries(&cases, &items);
    for key in ["Types", "Hosts", "ProcessNames", "Filters", "ImageFilters", "OutputStats", "Stats"] {
        r.check(&format!("{key} (History.db)"), &sx["Queries"][key], &rq[key]);
    }
    let raw_items = HistoryManagerSqlite::open(w("sharex-raw.db")).unwrap().load_newest_first().unwrap();
    let rq = queries(&cases, &raw_items);
    for key in ["Types", "Hosts", "ProcessNames", "Filters", "ImageFilters", "OutputStats", "Stats"] {
        r.check(&format!("{key} (odd values)"), &sx["QueriesRaw"][key], &rq[key]);
    }
    let rf = formats(&cases);
    for key in ["Share", "Number", "Compact", "Hour", "Rate", "Scale", "Level", "WeekChange"] {
        r.check(&format!("stats format {key}"), &sx["Formats"][key], &rf[key]);
    }
    r.check("history column layouts", &sx["Columns"], &columns(&cases));
    let _ = actions::MenuEntry::Other;

    r.lines.push("\n## Concurrent access".into());
    concurrency(&exe, &work, &mut r);

    if let Some(reference) = args.get(4) {
        r.lines.push("\n## A real History.db (a copy)".into());
        let copy = w("reference.db");
        std::fs::copy(reference, &copy).unwrap();
        sharex(&exe, &["read", copy.to_str().unwrap(), w("sharex-reads-reference.json").to_str().unwrap()]);
        let sharex_read = read_json(&w("sharex-reads-reference.json"));
        r.lines.push(format!("{} items.", sharex_read.as_array().map_or(0, Vec::len)));
        r.check("ShareX and ShareXrs read the reference build's History.db alike", &sharex_read, &read_db(&copy));
    }

    let summary = format!("\n**{} of {} checks identical.**", r.checks - r.failures, r.checks);
    r.lines.push(summary.clone());
    std::fs::write(w("report.md"), format!("# History harness\n\n{}\n", r.lines.join("\n"))).unwrap();
    println!("{}", r.lines.join("\n"));
    if r.failures > 0 {
        std::process::exit(1);
    }
}
