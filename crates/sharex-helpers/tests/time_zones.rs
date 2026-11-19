//! `sharex_helpers::time_zone` against .NET's own `TimeZoneInfo`, recorded by
//! `tools/settings-harness --time-zones` (see its README): every zone's JSON
//! as ShareX writes it, its UTC offsets 1970-2040, and what loading the JSON
//! as ShareX does gives on machines in other zones. The list itself is only
//! compared on the machine that recorded it (Windows build and UI language).

use serde_json::Value;
use sharex_helpers::time_zone::{TICKS_PER_DAY, TICKS_PER_MINUTE, TimeZoneInfo};

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/time_zones/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("json")
}

fn parse(s: &Value) -> Value {
    serde_json::from_str(s.as_str().expect("a json string")).expect("json")
}

/// The offsets match .NET's at each change (and the tick before it), half
/// way between changes, and every 7 days.
fn check_offsets(zone: &TimeZoneInfo, transitions: &Value, what: &str) -> Result<(), String> {
    let list: Vec<(i64, i64)> = transitions.as_array().expect("transitions").iter().map(|t| (t[0].as_i64().unwrap(), t[1].as_i64().unwrap())).collect();
    let offset = |t: i64| zone.utc_offset_from_utc(t) / TICKS_PER_MINUTE;
    let expected = |t: i64| list.iter().rev().find(|(s, _)| *s <= t).map_or(list[0].1, |(_, o)| *o);
    for (i, &(t, o)) in list.iter().enumerate() {
        if offset(t) != o {
            return Err(format!("{what}: offset at {t} is {} minutes, .NET's {o}", offset(t)));
        }
        if i > 0 && offset(t - 1) != list[i - 1].1 {
            return Err(format!("{what}: offset just before {t} is {}, .NET's {}", offset(t - 1), list[i - 1].1));
        }
        if let Some(&(next, _)) = list.get(i + 1) {
            let mid = t + (next - t) / 2;
            if offset(mid) != o {
                return Err(format!("{what}: offset at {mid} is {}, .NET's {o}", offset(mid)));
            }
        }
    }
    let (start, end) = (list[0].0, list[0].0 + 70 * 365 * TICKS_PER_DAY);
    let mut t = start;
    while t < end {
        if offset(t) != expected(t) {
            return Err(format!("{what}: offset at {t} is {}, .NET's {}", offset(t), expected(t)));
        }
        t += 7 * TICKS_PER_DAY + 36_000_000_007;
    }
    Ok(())
}

#[test]
fn zones_json_and_offsets_as_dotnet() {
    let base = fixture("time-zones-sharex.json");
    let zones = base["zones"].as_array().unwrap();
    assert!(zones.len() > 100);
    let mut failures = Vec::new();
    for z in zones {
        let id = z["id"].as_str().unwrap();
        let json = parse(&z["json"]);
        let zone = TimeZoneInfo::from_json(&json).unwrap_or_else(|| panic!("{id}: unreadable"));
        assert_eq!(zone.display_name, z["displayName"].as_str().unwrap());
        // The JSON ShareXrs writes for it is ShareX's, byte for byte.
        assert_eq!(serde_json::to_string(&zone.to_json()).unwrap(), z["json"].as_str().unwrap(), "{id}");
        if let Err(e) = check_offsets(&zone, &z["transitions"], id) {
            failures.push(e);
        }
    }
    assert_eq!(serde_json::to_string(&TimeZoneInfo::utc().to_json()).unwrap(), base["utc"].as_str().unwrap());
    assert!(failures.is_empty(), "{} of {} zones differ:\n{}", failures.len(), zones.len(), failures.join("\n"));
}

/// Loading the JSON as `SettingsBase` does (`DateTimeZoneHandling.Local`)
/// on a machine in Tokyo and one in California: the zone's JSON, its
/// offsets and the list item it selects, as ShareX has them after a restart.
#[test]
fn zones_loaded_on_other_machines_as_sharex() {
    let base = fixture("time-zones-sharex.json");
    let list: Vec<TimeZoneInfo> = base["zones"].as_array().unwrap().iter().map(|z| TimeZoneInfo::from_json(&parse(&z["json"])).unwrap()).collect();
    for name in ["time-zones-loaded-tokyo-standard-time.json", "time-zones-loaded-pacific-standard-time.json"] {
        let v = fixture(name);
        let local = TimeZoneInfo::from_json(&parse(&v["local"])).unwrap();
        let mut failures = Vec::new();
        for (z, b) in v["zones"].as_array().unwrap().iter().zip(base["zones"].as_array().unwrap()) {
            let id = z["id"].as_str().unwrap();
            assert_eq!(id, b["id"].as_str().unwrap());
            assert!(z.get("error").is_none(), "{name}: {id} fails to load in ShareX");
            let loaded = TimeZoneInfo::from_loaded_json(&parse(&b["json"]), &local).unwrap();
            let written = serde_json::to_string(&loaded.to_json()).unwrap();
            let expected = z.get("json").unwrap_or(&b["json"]).as_str().unwrap();
            if written != expected {
                failures.push(format!("{name}: {id}: JSON after loading\n  {written}\n  {expected}"));
                continue;
            }
            if let Err(e) = check_offsets(&loaded, z.get("transitions").unwrap_or(&b["transitions"]), &format!("{name}: {id}")) {
                failures.push(e);
            }
            let selects = list.iter().find(|x| x.equals(&loaded)).map(|x| x.id.as_str());
            if selects != z["selects"].as_str() {
                failures.push(format!("{name}: {id} selects {selects:?}, ShareX {:?}", z["selects"].as_str()));
            }
        }
        assert!(failures.is_empty(), "{} differences:\n{}", failures.len(), failures.join("\n"));
    }
}

/// The registry's list, read as .NET reads it, equals .NET's: on the
/// machine that recorded the fixture (elsewhere the list and its localized
/// names differ, and the test says so and stops).
#[cfg(windows)]
#[test]
fn system_list_as_dotnet_on_the_recording_machine() {
    let base = fixture("time-zones-sharex.json");
    let here = sharex_helpers::time_zone::machine_fingerprint();
    if here != base["machine"].as_str().unwrap() {
        eprintln!("skipped: the fixture was recorded on {}, this is {here}", base["machine"]);
        return;
    }
    let zones = sharex_helpers::time_zone::system_time_zones();
    let ours: Vec<String> = zones.iter().map(|z| serde_json::to_string(&z.to_json()).unwrap()).collect();
    let theirs: Vec<String> = base["zones"].as_array().unwrap().iter().map(|z| z["json"].as_str().unwrap().to_string()).collect();
    for (a, b) in ours.iter().zip(&theirs) {
        assert_eq!(a, b);
    }
    assert_eq!(ours.len(), theirs.len());
    let local = sharex_helpers::time_zone::local_time_zone();
    assert_eq!(serde_json::to_string(&local.to_json()).unwrap(), base["local"].as_str().unwrap());
}
