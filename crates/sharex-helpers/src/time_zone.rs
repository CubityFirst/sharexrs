//! .NET's `TimeZoneInfo` as ShareX uses it: Task settings › File naming ›
//! Time zone lists `TimeZoneInfo.GetSystemTimeZones()` and saves the chosen
//! zone in `TaskSettingsUpload.CustomTimeZone` as Newtonsoft writes an
//! `ISerializable` `TimeZoneInfo` (`Id`, `DisplayName`, ..., `AdjustmentRules`
//! with their `TransitionTime`s); `NameParser` converts the time with
//! `TimeZoneInfo.ConvertTime(DateTime.Now, CustomTimeZone)`.
//!
//! Times are .NET `DateTime` ticks (100 ns since 0001-01-01). The zone list
//! on Windows is read from the registry as `TimeZoneInfo.Win32.cs` reads it
//! (`TZI`, `Dynamic DST`, the `MUI_*` names through `tzres.dll`'s MUI file);
//! the conversion is `GetUtcOffsetFromUtc` with its adjustment rule search
//! and daylight saving checks. `tools/settings-harness --time-zones` records
//! .NET's own list, JSON and offsets for the tests.

use serde_json::{Map, Value, json};
use std::cmp::Ordering;

pub const TICKS_PER_MILLISECOND: i64 = 10_000;
pub const TICKS_PER_SECOND: i64 = 10_000_000;
pub const TICKS_PER_MINUTE: i64 = 600_000_000;
pub const TICKS_PER_HOUR: i64 = 36_000_000_000;
pub const TICKS_PER_DAY: i64 = 864_000_000_000;
/// `DateTime.MaxValue.Ticks`.
pub const MAX_TICKS: i64 = 3_155_378_975_999_999_999;
/// 1970-01-01.
pub const UNIX_EPOCH_TICKS: i64 = 621_355_968_000_000_000;

// ---------------------------------------------------------------------------
// DateTime arithmetic on ticks.

fn date(ticks: i64) -> chrono::NaiveDate {
    chrono::NaiveDate::from_num_days_from_ce_opt((ticks.div_euclid(TICKS_PER_DAY) + 1) as i32).unwrap_or(chrono::NaiveDate::MIN)
}

/// `new DateTime(year, month, day)`.
pub fn ymd(year: i32, month: u32, day: u32) -> i64 {
    chrono::NaiveDate::from_ymd_opt(year, month, day).map_or(0, |d| (chrono::Datelike::num_days_from_ce(&d) as i64 - 1) * TICKS_PER_DAY)
}

fn year(ticks: i64) -> i32 {
    use chrono::Datelike;
    date(ticks).year()
}

fn date_only(ticks: i64) -> i64 {
    ticks - ticks.rem_euclid(TICKS_PER_DAY)
}

fn time_of_day(ticks: i64) -> i64 {
    ticks.rem_euclid(TICKS_PER_DAY)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    chrono::NaiveDate::from_ymd_opt(ny, nm, 1).and_then(|d| d.pred_opt()).map_or(31, |p| chrono::Datelike::day(&p))
}

/// `(int)DateTime.DayOfWeek`: Sunday is 0.
fn day_of_week(ticks: i64) -> i64 {
    (ticks.div_euclid(TICKS_PER_DAY) + 1).rem_euclid(7)
}

/// A `DateTime`'s ticks as a date and time.
pub fn to_naive(ticks: i64) -> chrono::NaiveDateTime {
    let t = time_of_day(ticks);
    date(ticks).and_hms_nano_opt((t / TICKS_PER_HOUR) as u32, (t % TICKS_PER_HOUR / TICKS_PER_MINUTE) as u32, (t % TICKS_PER_MINUTE / TICKS_PER_SECOND) as u32, (t % TICKS_PER_SECOND * 100) as u32).unwrap_or_default()
}

/// The UTC ticks of a Unix time in milliseconds.
pub fn ticks_from_unix_millis(ms: i64) -> i64 {
    UNIX_EPOCH_TICKS + ms * TICKS_PER_MILLISECOND
}

// ---------------------------------------------------------------------------
// The JSON forms: TimeSpan's "c" format and Newtonsoft's ISO dates (as
// `DateTimeZoneHandling.Utc` writes them).

/// `TimeSpan.ToString()`: `[-][d.]hh:mm:ss[.fffffff]`.
pub fn format_time_span(ticks: i64) -> String {
    let neg = ticks < 0;
    let t = ticks.unsigned_abs();
    let days = t / TICKS_PER_DAY as u64;
    let rem = t % TICKS_PER_DAY as u64;
    let h = rem / TICKS_PER_HOUR as u64;
    let m = rem % TICKS_PER_HOUR as u64 / TICKS_PER_MINUTE as u64;
    let s = rem % TICKS_PER_MINUTE as u64 / TICKS_PER_SECOND as u64;
    let f = rem % TICKS_PER_SECOND as u64;
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if days > 0 {
        out.push_str(&format!("{days}."));
    }
    out.push_str(&format!("{h:02}:{m:02}:{s:02}"));
    if f > 0 {
        out.push_str(&format!(".{f:07}"));
    }
    out
}

/// `TimeSpan.Parse` of the "c" format.
pub fn parse_time_span(s: &str) -> Option<i64> {
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s),
    };
    let (days, rest) = match s.split_once('.') {
        Some((d, r)) if !d.contains(':') => (d.parse::<i64>().ok()?, r),
        _ => (0, s),
    };
    let (hms, frac) = match rest.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    let mut parts = hms.split(':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let sec: i64 = parts.next().map_or(Some(0), |x| x.parse().ok())?;
    let mut ticks = days * TICKS_PER_DAY + h * TICKS_PER_HOUR + m * TICKS_PER_MINUTE + sec * TICKS_PER_SECOND;
    if let Some(f) = frac {
        let f = format!("{f:0<7}");
        ticks += f.get(..7)?.parse::<i64>().ok()?;
    }
    Some(if neg { -ticks } else { ticks })
}

/// Newtonsoft's ISO date (`yyyy-MM-ddTHH:mm:ss.FFFFFFFK`) of a `DateTime`
/// written with `DateTimeZoneHandling.Utc` (an unspecified one gets `Z`).
pub fn format_date_time(ticks: i64) -> String {
    use chrono::Datelike;
    let d = date(ticks);
    let t = time_of_day(ticks);
    let mut out = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        d.year(),
        d.month(),
        d.day(),
        t / TICKS_PER_HOUR,
        t % TICKS_PER_HOUR / TICKS_PER_MINUTE,
        t % TICKS_PER_MINUTE / TICKS_PER_SECOND
    );
    let f = t % TICKS_PER_SECOND;
    if f > 0 {
        let s = format!("{f:07}");
        out.push('.');
        out.push_str(s.trim_end_matches('0'));
    }
    out.push('Z');
    out
}

/// Reads [`format_date_time`]'s form (an offset or none is taken as written).
pub fn parse_date_time(s: &str) -> Option<i64> {
    let s = s.trim_end_matches('Z');
    let (d, t) = s.split_once('T')?;
    let mut dp = d.split('-');
    let y: i32 = dp.next()?.parse().ok()?;
    let mo: u32 = dp.next()?.parse().ok()?;
    let da: u32 = dp.next()?.parse().ok()?;
    let t = t.split(['+']).next()?;
    let (hms, frac) = match t.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (t, None),
    };
    let mut tp = hms.split(':');
    let h: i64 = tp.next()?.parse().ok()?;
    let mi: i64 = tp.next()?.parse().ok()?;
    let se: i64 = tp.next().map_or(Some(0), |x| x.parse().ok())?;
    let mut ticks = ymd(y, mo, da) + h * TICKS_PER_HOUR + mi * TICKS_PER_MINUTE + se * TICKS_PER_SECOND;
    if let Some(f) = frac {
        let f = format!("{f:0<7}");
        ticks += f.get(..7)?.parse::<i64>().ok()?;
    }
    Some(ticks)
}

const DAY_NAMES: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

// ---------------------------------------------------------------------------
// The types.

/// `TimeZoneInfo.TransitionTime`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransitionTime {
    /// `TimeOfDay`: a `DateTime` on 0001-01-01 (only its time of day counts).
    pub time_of_day: i64,
    pub month: u8,
    pub week: u8,
    pub day: u8,
    /// Sunday is 0.
    pub day_of_week: u8,
    pub is_fixed_date_rule: bool,
}

impl TransitionTime {
    /// `CreateFixedDateRule`.
    pub fn fixed(time_of_day: i64, month: u8, day: u8) -> Self {
        Self { time_of_day, month, week: 1, day, day_of_week: 0, is_fixed_date_rule: true }
    }

    /// `CreateFloatingDateRule`.
    pub fn floating(time_of_day: i64, month: u8, week: u8, day_of_week: u8) -> Self {
        Self { time_of_day, month, week, day: 1, day_of_week, is_fixed_date_rule: false }
    }

    fn to_json(self, w: &dyn Fn(i64) -> String) -> Value {
        json!({
            "TimeOfDay": w(self.time_of_day),
            "Month": self.month,
            "Week": self.week,
            "Day": self.day,
            "DayOfWeek": DAY_NAMES[self.day_of_week as usize % 7],
            "IsFixedDateRule": self.is_fixed_date_rule,
        })
    }

    fn from_json(v: &Value, local: Option<&TimeZoneInfo>) -> Option<Self> {
        let n = |k: &str| v.get(k).and_then(Value::as_u64).map(|x| x as u8);
        let dow = match v.get("DayOfWeek")? {
            Value::String(s) => DAY_NAMES.iter().position(|d| d.eq_ignore_ascii_case(s))? as u8,
            Value::Number(x) => x.as_u64()? as u8,
            _ => return None,
        };
        Some(Self {
            time_of_day: read_date(v.get("TimeOfDay")?, local)?,
            month: n("Month")?,
            week: n("Week")?,
            day: n("Day")?,
            day_of_week: dow,
            is_fixed_date_rule: v.get("IsFixedDateRule")?.as_bool()?,
        })
    }
}

/// `TimeZoneInfo.AdjustmentRule`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdjustmentRule {
    pub date_start: i64,
    pub date_end: i64,
    pub daylight_delta: i64,
    pub daylight_transition_start: TransitionTime,
    pub daylight_transition_end: TransitionTime,
    pub base_utc_offset_delta: i64,
    pub no_daylight_transitions: bool,
}

impl AdjustmentRule {
    /// `HasDaylightSaving`.
    fn has_daylight_saving(&self) -> bool {
        self.daylight_delta != 0
            || (self.daylight_transition_start != TransitionTime::default() && self.daylight_transition_start.time_of_day != 0)
            || (self.daylight_transition_end != TransitionTime::default() && self.daylight_transition_end.time_of_day != TICKS_PER_MILLISECOND)
    }

    /// `IsStartDateMarkerForBeginningOfYear`.
    fn is_start_marker(&self) -> bool {
        let t = self.daylight_transition_start;
        !self.no_daylight_transitions && t.month == 1 && t.day == 1 && time_of_day(t.time_of_day) < TICKS_PER_SECOND
    }

    /// `IsEndDateMarkerForEndOfYear`.
    fn is_end_marker(&self) -> bool {
        let t = self.daylight_transition_end;
        !self.no_daylight_transitions && t.month == 1 && t.day == 1 && time_of_day(t.time_of_day) < TICKS_PER_SECOND
    }

    fn to_json(self, w: &dyn Fn(i64) -> String) -> Value {
        json!({
            "DateStart": w(self.date_start),
            "DateEnd": w(self.date_end),
            "DaylightDelta": format_time_span(self.daylight_delta),
            "DaylightTransitionStart": self.daylight_transition_start.to_json(w),
            "DaylightTransitionEnd": self.daylight_transition_end.to_json(w),
            "BaseUtcOffsetDelta": format_time_span(self.base_utc_offset_delta),
            "NoDaylightTransitions": self.no_daylight_transitions,
        })
    }

    fn from_json(v: &Value, local: Option<&TimeZoneInfo>) -> Option<Self> {
        let span = |k: &str| v.get(k).and_then(Value::as_str).and_then(parse_time_span);
        Some(Self {
            date_start: read_date(v.get("DateStart")?, local)?,
            date_end: read_date(v.get("DateEnd")?, local)?,
            daylight_delta: span("DaylightDelta")?,
            daylight_transition_start: TransitionTime::from_json(v.get("DaylightTransitionStart")?, local)?,
            daylight_transition_end: TransitionTime::from_json(v.get("DaylightTransitionEnd")?, local)?,
            base_utc_offset_delta: span("BaseUtcOffsetDelta").unwrap_or(0),
            no_daylight_transitions: v.get("NoDaylightTransitions").and_then(Value::as_bool).unwrap_or(false),
        })
    }
}

/// A date of the JSON: as written, or (`local`) as `SettingsBase` loads it
/// with `DateTimeZoneHandling.Local` (`DateTime.ToLocalTime`).
fn read_date(v: &Value, local: Option<&TimeZoneInfo>) -> Option<i64> {
    let t = parse_date_time(v.as_str()?)?;
    Some(match local {
        Some(zone) => zone.to_local_time(t),
        None => t,
    })
}

/// `TimeZoneInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeZoneInfo {
    pub id: String,
    pub display_name: String,
    pub standard_name: String,
    pub daylight_name: String,
    pub base_utc_offset: i64,
    pub adjustment_rules: Option<Vec<AdjustmentRule>>,
    pub supports_daylight_saving_time: bool,
    /// Loaded as `SettingsBase` loads it: the dates are local times (kind
    /// `Local`) of this zone, the machine's, and are written back in UTC
    /// (`DateTime.ToUniversalTime`, as Newtonsoft's `DateTimeZoneHandling.Utc`
    /// does).
    pub loaded_with: Option<Box<TimeZoneInfo>>,
}

impl TimeZoneInfo {
    /// `TimeZoneInfo.Utc`.
    pub fn utc() -> Self {
        Self {
            id: "UTC".into(),
            display_name: "(UTC) Coordinated Universal Time".into(),
            standard_name: "Coordinated Universal Time".into(),
            daylight_name: "Coordinated Universal Time".into(),
            base_utc_offset: 0,
            adjustment_rules: None,
            supports_daylight_saving_time: false,
            loaded_with: None,
        }
    }

    /// The zone as ShareX's settings file has it (`GetObjectData`'s values).
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("Id".into(), json!(self.id));
        m.insert("DisplayName".into(), json!(self.display_name));
        m.insert("StandardName".into(), json!(self.standard_name));
        m.insert("DaylightName".into(), json!(self.daylight_name));
        m.insert("BaseUtcOffset".into(), json!(format_time_span(self.base_utc_offset)));
        let w = |t: i64| format_date_time(self.loaded_with.as_ref().map_or(t, |local| local.to_universal_time(t)));
        m.insert("AdjustmentRules".into(), self.adjustment_rules.as_ref().map_or(Value::Null, |r| Value::Array(r.iter().map(|x| x.to_json(&w)).collect())));
        m.insert("SupportsDaylightSavingTime".into(), json!(self.supports_daylight_saving_time));
        Value::Object(m)
    }

    /// Reads the settings' JSON as written (the zone ShareX had in memory
    /// when it saved).
    pub fn from_json(v: &Value) -> Option<Self> {
        Self::read(v, None)
    }

    /// Reads the settings' JSON as `SettingsBase.LoadInternal` does: every
    /// date goes through `DateTime.ToLocalTime` with `local`, the machine's
    /// zone. Where the local offset isn't 0 at those dates (anywhere but
    /// UTC or a GMT-like zone in January) the rules' dates and transition
    /// times move, as they do in ShareX after a restart.
    pub fn from_loaded_json(v: &Value, local: &TimeZoneInfo) -> Option<Self> {
        Self::read(v, Some(local))
    }

    fn read(v: &Value, local: Option<&TimeZoneInfo>) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let rules = match v.get("AdjustmentRules") {
            Some(Value::Array(a)) => Some(a.iter().map(|r| AdjustmentRule::from_json(r, local)).collect::<Option<Vec<_>>>()?),
            _ => None,
        };
        Some(Self {
            id: v.get("Id")?.as_str()?.to_string(),
            display_name: s("DisplayName"),
            standard_name: s("StandardName"),
            daylight_name: s("DaylightName"),
            base_utc_offset: v.get("BaseUtcOffset").and_then(Value::as_str).and_then(parse_time_span).unwrap_or(0),
            adjustment_rules: rules,
            supports_daylight_saving_time: v.get("SupportsDaylightSavingTime").and_then(Value::as_bool).unwrap_or(false),
            loaded_with: local.map(|z| Box::new(z.clone())),
        })
    }

    /// `IEquatable<TimeZoneInfo>.Equals`: the same id (ignoring case) and
    /// `HasSameRules` (what `ObjectCombo` selects by).
    pub fn equals(&self, other: &TimeZoneInfo) -> bool {
        self.id.eq_ignore_ascii_case(&other.id) && self.has_same_rules(other)
    }

    /// `HasSameRules`.
    pub fn has_same_rules(&self, other: &TimeZoneInfo) -> bool {
        if self.base_utc_offset != other.base_utc_offset || self.supports_daylight_saving_time != other.supports_daylight_saving_time {
            return false;
        }
        let a = self.adjustment_rules.as_deref().unwrap_or_default();
        let b = other.adjustment_rules.as_deref().unwrap_or_default();
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| rule_equals(x, y))
    }

    fn rules(&self) -> &[AdjustmentRule] {
        self.adjustment_rules.as_deref().unwrap_or_default()
    }

    // -- conversion: .NET 11's `GetOffsetForUtcDate` and the per-year
    // transition table it builds (`CacheTransitionsForYear`), transcribed.

    /// `TransitionTimeToDateTime`.
    fn transition_to_date_time(year: i32, t: TransitionTime) -> i64 {
        let tod = time_of_day(t.time_of_day);
        let month = (t.month as u32).clamp(1, 12);
        if t.is_fixed_date_rule {
            let mut day = t.day as u32;
            if day > 28 {
                day = day.min(days_in_month(year, month));
            }
            ymd(year, month, day.max(1)) + tod
        } else if t.week <= 4 {
            let mut value = ymd(year, month, 1) + tod;
            let mut delta = t.day_of_week as i64 - day_of_week(value);
            if delta < 0 {
                delta += 7;
            }
            delta += 7 * (t.week as i64 - 1);
            if delta > 0 {
                value += delta * TICKS_PER_DAY;
            }
            value
        } else {
            let mut value = ymd(year, month, days_in_month(year, month)) + tod;
            let mut delta = day_of_week(value) - t.day_of_week as i64;
            if delta < 0 {
                delta += 7;
            }
            if delta > 0 {
                value -= delta * TICKS_PER_DAY;
            }
            value
        }
    }

    /// `StartTransitionTimeToDateTime`.
    fn start_time(year: i32, rule: &AdjustmentRule) -> i64 {
        if rule.is_start_marker() { ymd(year, 1, 1) } else { Self::transition_to_date_time(year, rule.daylight_transition_start) }
    }

    /// `EndTransitionTimeToDateTime`.
    fn end_time(year: i32, rule: &AdjustmentRule) -> i64 {
        if !rule.is_end_marker() {
            Self::transition_to_date_time(year, rule.daylight_transition_end)
        } else if year < 9999 {
            ymd(year + 1, 1, 1)
        } else {
            MAX_TICKS
        }
    }

    /// `GetTransitionUtcOffsetTicks`.
    fn tr_offset(&self, rule: &AdjustmentRule, daylight: bool) -> i64 {
        self.base_utc_offset + rule.base_utc_offset_delta + if daylight { rule.daylight_delta } else { 0 }
    }

    /// `GetUtcDateTimeFromLocalTicks`.
    fn utc_from_local(&self, local: i64, rule: &AdjustmentRule, daylight: bool, adjust: i64) -> i64 {
        safe(local - self.tr_offset(rule, daylight) + adjust)
    }

    /// `FindRuleForYear`.
    fn find_rule_for_year(&self, year_: i32) -> Option<usize> {
        let rules = self.rules();
        let (mut lo, mut hi) = (0i64, rules.len() as i64 - 1);
        while lo <= hi {
            let mut mid = (lo + hi) / 2;
            let r = &rules[mid as usize];
            if year_ < year(r.date_start) {
                hi = mid - 1;
                continue;
            }
            if year_ > year(r.date_end) {
                lo = mid + 1;
                continue;
            }
            while mid > 0 && {
                let p = &rules[mid as usize - 1];
                year(p.date_end) >= year_ || year(p.date_end + (self.base_utc_offset + p.base_utc_offset_delta + p.daylight_delta)) >= year_
            } {
                mid -= 1;
            }
            return Some(mid as usize);
        }
        None
    }

    /// `CacheTransitionsForYear`: the year's transitions (UTC start, end,
    /// offset from the base, daylight saving).
    fn transitions_for_year(&self, year_: i32) -> Vec<Transition> {
        let rules = self.rules();
        let Some(mut num) = self.find_rule_for_year(year_) else { return Vec::new() };
        let mut t: Vec<Transition> = Vec::new();
        let r = rules[num];
        let jan1 = ymd(year_, 1, 1);
        let last_end = |t: &Vec<Transition>| t.last().map_or(0, |x| x.1);
        if r.no_daylight_transitions {
            let num2 = self.base_utc_offset + r.date_start + r.base_utc_offset_delta;
            if r.date_start > jan1 || num2 > jan1 {
                let prev = if num > 0 { Some(rules[num - 1]) } else { None };
                match prev {
                    Some(p) if year(p.date_end) >= year_ - 1 => {
                        if p.no_daylight_transitions {
                            let ds = safe(p.date_end + 1);
                            if ds <= r.date_start - 1 {
                                add(&mut t, Transition(ds, safe(r.date_start - 1), p.base_utc_offset_delta, false));
                            } else {
                                add(&mut t, Transition(p.date_start, p.date_end, full(&p), p.has_daylight_saving()));
                            }
                        } else {
                            let d2 = Self::start_time(year_ - 1, &p);
                            let d3 = Self::end_time(year_ - 1, &p);
                            if d2 < d3 {
                                let u = self.utc_from_local(d3, &p, true, -1);
                                if u < r.date_start - 1 {
                                    add(&mut t, Transition(u, safe(r.date_start - 1), p.base_utc_offset_delta, false));
                                } else {
                                    add(&mut t, Transition(self.utc_from_local(d2, &p, false, 0), safe(r.date_start - 1), full(&p), p.has_daylight_saving()));
                                }
                            } else {
                                let de = safe(jan1 - self.tr_offset(&p, true) - 1);
                                if de < r.date_start - 1 {
                                    add(&mut t, Transition(safe(de + 1), safe(r.date_start - 1), r.base_utc_offset_delta, false));
                                }
                                let u2 = self.utc_from_local(d2, &p, false, 0);
                                add(&mut t, Transition(u2, de, full(&p), p.has_daylight_saving()));
                            }
                        }
                    }
                    _ => {
                        let d4 = safe(ymd(if year_ <= 1 { 1 } else { year_ - 1 }, 1, 1) - self.base_utc_offset);
                        if d4 < r.date_start {
                            add(&mut t, Transition(d4, safe(r.date_start - 1), 0, false));
                        }
                    }
                }
            }
            add(&mut t, Transition(r.date_start, r.date_end, full(&r), r.has_daylight_saving()));
            let mut d5 = if year_ < 9999 { ymd(year_ + 1, 1, 1) - 1 } else { MAX_TICKS };
            loop {
                let last = *t.last().expect("added above");
                if !(last.1 < d5 || last.1 + last.2 + self.base_utc_offset < d5) {
                    break;
                }
                num += 1;
                if num < rules.len() && year(rules[num].date_start) <= year(last.1) {
                    let r3 = rules[num];
                    if r3.no_daylight_transitions {
                        if last.1 < r3.date_start - 1 {
                            add(&mut t, Transition(last.1 + 1, r3.date_start - 1, r3.base_utc_offset_delta, false));
                        }
                        add(&mut t, Transition(r3.date_start, r3.date_end, full(&r3), r3.has_daylight_saving()));
                        continue;
                    }
                    let year2 = (year_ + 1).min(year(r3.date_start));
                    let d6 = Self::start_time(year2, &r3);
                    let d7 = Self::end_time(year2, &r3);
                    if d6 < d7 {
                        let u3 = self.utc_from_local(d6, &r3, false, 0);
                        if last_end(&t) < u3 - 1 {
                            let e = last_end(&t);
                            add(&mut t, Transition(e + 1, u3 - 1, r3.base_utc_offset_delta, false));
                        }
                        add(&mut t, Transition(u3, self.utc_from_local(d7, &r3, true, -1), full(&r3), r3.has_daylight_saving()));
                    } else {
                        let u4 = self.utc_from_local(d7, &r3, true, -1);
                        let u5 = self.utc_from_local(ymd(year_ + 1, 1, 1), &r3, true, 0);
                        if last_end(&t) < u5 - 1 {
                            let e = last_end(&t);
                            add(&mut t, Transition(e + 1, u5 - 1, full(&r), r.has_daylight_saving()));
                        }
                        if last_end(&t) < u4 {
                            let e = last_end(&t);
                            add(&mut t, Transition(e + 1, u4, full(&r3), r3.has_daylight_saving()));
                        }
                    }
                    if last_end(&t) < d5 {
                        if r3.base_utc_offset_delta > 0 {
                            d5 = safe(d5 + r3.base_utc_offset_delta);
                        }
                        let e = last_end(&t);
                        add(&mut t, Transition(e + 1, d5, r3.base_utc_offset_delta, false));
                    }
                    break;
                }
                let d8 = if year_ + 1 < 9999 { ymd(year_ + 2, 1, 1) - 1 } else { MAX_TICKS };
                if last_end(&t) < d8 {
                    let e = last_end(&t);
                    add(&mut t, Transition(e + 1, d8, 0, false));
                }
                break;
            }
        } else {
            let d9 = Self::start_time(year_, &r);
            let d10 = Self::end_time(year_, &r);
            let flag = r.is_start_marker();
            let u6 = self.utc_from_local(d9, &r, flag, 0);
            let u7 = self.utc_from_local(d10, &r, true, -1);
            let d11 = safe(jan1 - self.tr_offset(&r, flag || d9 > d10));
            let d12 = jan1;
            if d9 < d10 {
                if u6 > d12 || d9 > d12 {
                    let r4 = if year(r.date_start) < year_ {
                        Some(r)
                    } else if num > 0 && year(rules[num - 1].date_start) <= year_ - 1 && year(rules[num - 1].date_end) >= year_ - 1 {
                        Some(rules[num - 1])
                    } else {
                        None
                    };
                    if let Some(r4) = r4 {
                        if r4.no_daylight_transitions {
                            let ds2 = safe(r4.date_end + 1);
                            if ds2 <= d11 - 1 {
                                add(&mut t, Transition(ds2, safe(d11 - 1), r4.base_utc_offset_delta, false));
                            } else {
                                add(&mut t, Transition(r4.date_start, r4.date_end, full(&r4), r4.has_daylight_saving()));
                            }
                        } else {
                            let d13 = Self::start_time(year_ - 1, &r4);
                            let d14 = Self::end_time(year_ - 1, &r4);
                            if d13 < d14 {
                                let u8 = self.utc_from_local(d14, &r4, true, 0);
                                add(&mut t, Transition(self.utc_from_local(d13, &r4, false, 0), safe(u8 - 1), full(&r4), r4.has_daylight_saving()));
                                if u8 < d11 - 1 {
                                    add(&mut t, Transition(u8, d11 - 1, r4.base_utc_offset_delta, false));
                                }
                            } else {
                                let u9 = self.utc_from_local(d13, &r4, false, 0);
                                add(&mut t, Transition(u9, safe(d11 - 1), full(&r4), r4.has_daylight_saving()));
                            }
                        }
                    } else {
                        let d15 = ymd(if year_ <= 1 { 1 } else { year_ - 1 }, 1, 1);
                        if d15 < d11 {
                            add(&mut t, Transition(d15, d11 - 1, 0, false));
                        }
                    }
                    if d11 < u6 - 1 {
                        add(&mut t, Transition(d11, u6 - 1, r.base_utc_offset_delta, false));
                    }
                }
                add(&mut t, Transition(u6, u7, full(&r), r.has_daylight_saving()));
                let d16 = if year_ < 9999 { ymd(year_ + 1, 1, 1) - 1 } else { MAX_TICKS };
                let d17 = safe(d16 - self.tr_offset(&r, r.is_end_marker()));
                if u7 < d17 {
                    add(&mut t, Transition(u7 + 1, d17, r.base_utc_offset_delta, false));
                }
                if d17 < d16 {
                    let r5 = if year(r.date_start) <= year_ + 1 && year(r.date_end) >= year_ + 1 {
                        Some(r)
                    } else if num + 1 < rules.len() && year(rules[num + 1].date_start) <= year_ + 1 && year(rules[num + 1].date_end) >= year_ + 1 {
                        Some(rules[num + 1])
                    } else {
                        None
                    };
                    match r5 {
                        Some(r5) if r5.no_daylight_transitions => {
                            if d17 < r5.date_start - 1 {
                                add(&mut t, Transition(d17 + 1, r5.date_start - 1, r5.base_utc_offset_delta, false));
                            }
                            add(&mut t, Transition(r5.date_start, r5.date_end, full(&r5), r5.has_daylight_saving()));
                        }
                        Some(r5) => {
                            let d18 = Self::start_time(year_ + 1, &r5);
                            let d19 = Self::end_time(year_ + 1, &r5);
                            let u10 = self.utc_from_local(d18, &r5, r5.is_start_marker(), 0);
                            let u11 = self.utc_from_local(d19, &r5, true, -1);
                            if d18 < d19 {
                                if d17 < u10 - 1 {
                                    add(&mut t, Transition(d17 + 1, u10 - 1, r5.base_utc_offset_delta, false));
                                }
                                add(&mut t, Transition(u10, u11, full(&r5), r5.has_daylight_saving()));
                            } else if d17 < u11 {
                                add(&mut t, Transition(d17 + 1, u11, full(&r5), r5.has_daylight_saving()));
                            }
                        }
                        None => add(&mut t, Transition(d17 + 1, d16, 0, false)),
                    }
                }
            } else {
                if d11 > d12 {
                    let r6 = if year(r.date_start) < year_ {
                        Some(r)
                    } else if num > 0 && year(rules[num - 1].date_start) <= year_ - 1 && year(rules[num - 1].date_end) >= year_ - 1 {
                        Some(rules[num - 1])
                    } else {
                        None
                    };
                    match r6 {
                        Some(r6) if r6.no_daylight_transitions => {
                            if d11 > r6.date_end - 1 {
                                add(&mut t, Transition(r6.date_end + 1, d11 - 1, r.base_utc_offset_delta, false));
                            }
                            add(&mut t, Transition(r6.date_start, r6.date_end, full(&r6), r6.has_daylight_saving()));
                        }
                        Some(r6) => {
                            let d20 = Self::start_time(year_ - 1, &r6);
                            let d21 = Self::end_time(year_ - 1, &r6);
                            let u12 = self.utc_from_local(d20, &r6, r6.is_start_marker(), 0);
                            if d20 < d21 {
                                let u13 = self.utc_from_local(d21, &r6, true, -1);
                                if u13 < d11 - 1 {
                                    add(&mut t, Transition(u13 + 1, d11 - 1, r6.base_utc_offset_delta, false));
                                } else {
                                    add(&mut t, Transition(u12, u13, full(&r6), r6.has_daylight_saving()));
                                }
                            } else if u12 < d11 {
                                add(&mut t, Transition(u12, d11 - 1, full(&r6), r6.has_daylight_saving()));
                            }
                        }
                        None => {
                            if d12 < d11 {
                                add(&mut t, Transition(d12, d11 - 1, 0, false));
                            }
                        }
                    }
                }
                let d22 = if year_ < 9999 { ymd(year_ + 1, 1, 1) - 1 } else { MAX_TICKS };
                let d23 = safe(d22 - self.tr_offset(&r, true));
                add(&mut t, Transition(d11, u7, full(&r), r.has_daylight_saving()));
                add(&mut t, Transition(safe(u7 + 1), safe(u6 - 1), r.base_utc_offset_delta, false));
                add(&mut t, Transition(u6, d23, full(&r), r.has_daylight_saving()));
                if d23 < d22 {
                    let r7 = if year(r.date_end) > year_ {
                        Some(r)
                    } else if num + 1 < rules.len() && year(rules[num + 1].date_start) <= year_ + 1 && year(rules[num + 1].date_end) >= year_ + 1 {
                        Some(rules[num + 1])
                    } else {
                        None
                    };
                    match r7 {
                        Some(r7) if r7.no_daylight_transitions => {
                            if d23 < r7.date_start {
                                add(&mut t, Transition(d23 + 1, r7.date_start - 1, r7.base_utc_offset_delta, false));
                            } else if d23 < r7.date_end {
                                add(&mut t, Transition(d23 + 1, r7.date_end, full(&r7), r7.has_daylight_saving()));
                            }
                        }
                        Some(r7) => {
                            let d24 = Self::start_time(year_ + 1, &r7);
                            let d25 = Self::end_time(year_ + 1, &r7);
                            let u14 = self.utc_from_local(d24, &r7, r7.is_start_marker(), 0);
                            let u15 = self.utc_from_local(d25, &r7, true, -1);
                            if d24 < d25 {
                                if d23 < u14 - 1 {
                                    add(&mut t, Transition(d23 + 1, u14 - 1, r7.base_utc_offset_delta, false));
                                }
                                add(&mut t, Transition(u14, u15, full(&r7), r7.has_daylight_saving()));
                            } else if d23 < u15 {
                                add(&mut t, Transition(d23 + 1, u15, full(&r7), r7.has_daylight_saving()));
                            }
                        }
                        None => add(&mut t, Transition(d23 + 1, d22, 0, false)),
                    }
                }
            }
        }
        t
    }

    /// `GetUtcOffsetFromUtc(time, zone)` (`GetOffsetForUtcDate`): the zone's
    /// offset at UTC `ticks`.
    pub fn utc_offset_from_utc(&self, ticks: i64) -> i64 {
        if !self.supports_daylight_saving_time || self.rules().is_empty() {
            return self.base_utc_offset;
        }
        for tr in self.transitions_for_year(year(ticks)) {
            if ticks >= tr.0 && ticks <= tr.1 {
                return self.base_utc_offset + tr.2;
            }
        }
        self.base_utc_offset
    }

    /// `TimeZoneInfo.ConvertTimeFromUtc(utc, zone)`: the zone's time at UTC
    /// `ticks` (what `ConvertTime(DateTime.Now, zone)` gives NameParser).
    pub fn convert_from_utc(&self, ticks: i64) -> i64 {
        safe(ticks + self.utc_offset_from_utc(ticks))
    }

    /// `DateTime.ToLocalTime()` of a UTC time with this zone as the local
    /// one: clamped to `DateTime`'s range.
    pub fn to_local_time(&self, utc: i64) -> i64 {
        safe(utc + self.utc_offset_from_utc(utc))
    }

    /// `TryGetUtcOffset(localDateTime)`: the offset of a local time, and
    /// whether the time exists (not in a skipped hour). The local time
    /// isn't marked ambiguous (`IsAmbiguousDaylightSavingTime`).
    fn try_get_utc_offset(&self, local: i64) -> (bool, i64) {
        if !self.supports_daylight_saving_time || self.rules().is_empty() {
            return (true, self.base_utc_offset);
        }
        let list = self.transitions_for_year(year(local));
        if list.is_empty() {
            return (true, self.base_utc_offset);
        }
        for (i, tr) in list.iter().enumerate() {
            let mut offset = self.base_utc_offset + tr.2;
            let ticks = safe(local - offset);
            if ticks < tr.0 || ticks > tr.1 {
                continue;
            }
            if i + 1 < list.len() && tr.3 {
                let next = list[i + 1];
                let o2 = self.base_utc_offset + next.2;
                let t2 = safe(local - o2);
                if t2 >= next.0 && t2 <= next.1 {
                    offset = o2;
                }
            }
            return (true, offset);
        }
        (false, self.base_utc_offset)
    }

    /// `FindRuleIndexForLocalTime` (rules with unspecified dates: by the
    /// local date).
    fn rule_index_for_local_time(&self, local: i64) -> Option<usize> {
        let rules = self.rules();
        let date = date_only(local);
        let (mut lo, mut hi) = (0i64, rules.len() as i64 - 1);
        while lo <= hi {
            let mid = lo + (hi - lo) / 2;
            let r = &rules[mid as usize];
            if date < r.date_start {
                hi = mid - 1;
            } else if date <= r.date_end {
                return Some(mid as usize);
            } else {
                lo = mid + 1;
            }
        }
        None
    }

    /// `DateTime.ToUniversalTime()` of a local time of this zone
    /// (`ConvertTime(dateTime, Local, Utc, NoThrowOnInvalidTime)`: a time
    /// in a skipped hour takes the standard offset).
    pub fn to_universal_time(&self, local: i64) -> i64 {
        let (valid, offset) = self.try_get_utc_offset(local);
        let utc = if valid {
            local - offset
        } else {
            let standard = self.rule_index_for_local_time(local).map_or(self.base_utc_offset, |i| self.tr_offset(&self.rules()[i], false));
            local - standard
        };
        safe(utc)
    }
}

/// `TimeTransition`: UTC start and end (both inclusive), the offset from
/// the base, daylight saving.
#[derive(Clone, Copy, Debug)]
struct Transition(i64, i64, i64, bool);

/// `AddTransition`: one that continues the last (same offset and daylight
/// saving) extends it.
fn add(list: &mut Vec<Transition>, t: Transition) {
    if let Some(last) = list.last_mut()
        && last.2 == t.2
        && last.3 == t.3
    {
        last.1 = t.1;
        return;
    }
    list.push(t);
}

/// `SafeCreateDateTimeFromTicks`.
fn safe(ticks: i64) -> i64 {
    if ticks as u64 > MAX_TICKS as u64 {
        if ticks >= 0 { MAX_TICKS } else { 0 }
    } else {
        ticks
    }
}

/// `GetRuleFullUtcOffset`.
fn full(rule: &AdjustmentRule) -> i64 {
    rule.base_utc_offset_delta + if rule.has_daylight_saving() { rule.daylight_delta } else { 0 }
}

/// `AdjustmentRule.Equals`.
fn rule_equals(a: &AdjustmentRule, b: &AdjustmentRule) -> bool {
    a.date_start == b.date_start
        && a.date_end == b.date_end
        && a.daylight_delta == b.daylight_delta
        && a.base_utc_offset_delta == b.base_utc_offset_delta
        && a.daylight_transition_end == b.daylight_transition_end
        && a.daylight_transition_start == b.daylight_transition_start
}

/// `GetSystemTimeZones`' order: by base offset, then the display name
/// (ordinal).
pub fn sort_zones(zones: &mut [TimeZoneInfo]) {
    zones.sort_by(|a, b| match a.base_utc_offset.cmp(&b.base_utc_offset) {
        Ordering::Equal => a.display_name.encode_utf16().cmp(b.display_name.encode_utf16()),
        o => o,
    });
}

// ---------------------------------------------------------------------------
// The system's zones (Windows registry, as TimeZoneInfo.Win32.cs reads it).

/// `REG_TZI_FORMAT`: biases in minutes and the two `SYSTEMTIME`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegTzi {
    pub bias: i32,
    pub standard_bias: i32,
    pub daylight_bias: i32,
    /// `wYear, wMonth, wDayOfWeek, wDay, wHour, wMinute, wSecond, wMilliseconds`.
    pub standard_date: [u16; 8],
    pub daylight_date: [u16; 8],
}

impl RegTzi {
    /// The registry value's 44 bytes.
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != 44 {
            return None;
        }
        let i = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let st = |o: usize| std::array::from_fn(|k| u16::from_le_bytes([b[o + 2 * k], b[o + 2 * k + 1]]));
        Some(Self { bias: i(0), standard_bias: i(4), daylight_bias: i(8), standard_date: st(12), daylight_date: st(28) })
    }
}

/// `TransitionTimeFromTimeZoneInformation`.
fn transition_from_tzi(tzi: &RegTzi, start: bool) -> Option<TransitionTime> {
    if tzi.standard_date[1] == 0 {
        return None;
    }
    let d = if start { tzi.daylight_date } else { tzi.standard_date };
    let [y, month, dow, day, h, m, s, ms] = d.map(i64::from);
    if !(0..24).contains(&h) || !(0..60).contains(&m) || !(0..60).contains(&s) || !(0..1000).contains(&ms) || !(1..=12).contains(&month) {
        return None;
    }
    let tod = h * TICKS_PER_HOUR + m * TICKS_PER_MINUTE + s * TICKS_PER_SECOND + ms * TICKS_PER_MILLISECOND;
    if y == 0 {
        if !(1..=5).contains(&day) || !(0..7).contains(&dow) {
            return None;
        }
        Some(TransitionTime::floating(tod, month as u8, day as u8, dow as u8))
    } else {
        if !(1..=31).contains(&day) {
            return None;
        }
        Some(TransitionTime::fixed(tod, month as u8, day as u8))
    }
}

/// `CreateAdjustmentRuleFromTimeZoneInformation`. `Err` where .NET's
/// `CreateAdjustmentRule` would throw (the zone is then left out).
fn rule_from_tzi(tzi: &RegTzi, start: i64, end: i64, default_bias: i32) -> Result<Option<AdjustmentRule>, ()> {
    let supports_dst = tzi.standard_date[1] != 0;
    if !supports_dst {
        if tzi.bias == default_bias {
            return Ok(None);
        }
        return Ok(Some(AdjustmentRule {
            date_start: start,
            date_end: end,
            daylight_delta: 0,
            daylight_transition_start: TransitionTime::fixed(0, 1, 1),
            daylight_transition_end: TransitionTime::fixed(TICKS_PER_MILLISECOND, 1, 1),
            base_utc_offset_delta: (default_bias - tzi.bias) as i64 * TICKS_PER_MINUTE,
            no_daylight_transitions: false,
        }));
    }
    let (Some(ts), Some(te)) = (transition_from_tzi(tzi, true), transition_from_tzi(tzi, false)) else { return Err(()) };
    if ts == te {
        return Ok(None);
    }
    Ok(Some(AdjustmentRule {
        date_start: start,
        date_end: end,
        daylight_delta: -(tzi.daylight_bias as i64) * TICKS_PER_MINUTE,
        daylight_transition_start: ts,
        daylight_transition_end: te,
        base_utc_offset_delta: (default_bias - tzi.bias) as i64 * TICKS_PER_MINUTE,
        no_daylight_transitions: false,
    }))
}

/// What a zone's registry key holds (`TZI`, the names, `Dynamic DST`).
#[derive(Clone, Debug, Default)]
pub struct RegistryZone {
    pub id: String,
    pub tzi: Option<RegTzi>,
    pub display: String,
    pub standard: String,
    pub daylight: String,
    /// `FirstEntry`, `LastEntry` and each year's `TZI` (`None`: no key).
    pub dynamic: Option<(i32, i32, Vec<(i32, Option<RegTzi>)>)>,
}

/// `TryCreateAdjustmentRules`.
fn rules_from_registry(z: &RegistryZone, tzi: &RegTzi) -> Option<Option<Vec<AdjustmentRule>>> {
    let min_date = 0;
    let max_date = date_only(MAX_TICKS);
    let one = |r: Result<Option<AdjustmentRule>, ()>| r.map(|r| r.map(|x| vec![x]));
    let Some((first, last, years)) = &z.dynamic else {
        return one(rule_from_tzi(tzi, min_date, max_date, tzi.bias)).ok();
    };
    let (first, last) = (*first, *last);
    if first == -1 || last == -1 || first > last {
        return None;
    }
    let get = |y: i32| years.iter().find(|(k, _)| *k == y).and_then(|(_, t)| *t);
    let first_tzi = get(first)?;
    if first == last {
        return one(rule_from_tzi(&first_tzi, min_date, max_date, tzi.bias)).ok();
    }
    let mut rules = Vec::new();
    if let Some(r) = rule_from_tzi(&first_tzi, min_date, ymd(first, 12, 31), tzi.bias).ok()? {
        rules.push(r);
    }
    for y in first + 1..last {
        let t = get(y)?;
        if let Some(r) = rule_from_tzi(&t, ymd(y, 1, 1), ymd(y, 12, 31), tzi.bias).ok()? {
            rules.push(r);
        }
    }
    let t = get(last)?;
    if let Some(r) = rule_from_tzi(&t, ymd(last, 1, 1), max_date, tzi.bias).ok()? {
        rules.push(r);
    }
    Some(if rules.is_empty() { None } else { Some(rules) })
}

/// `TryGetTimeZoneFromLocalMachine`: the zone a registry key describes, or
/// `None` where .NET's would fail (it's then left out of the list).
pub fn zone_from_registry(z: &RegistryZone) -> Option<TimeZoneInfo> {
    let tzi = z.tzi?;
    let rules = rules_from_registry(z, &tzi)?;
    let base = -(tzi.bias as i64) * TICKS_PER_MINUTE;
    // `ValidateTimeZoneInfo`: offsets within ±14 hours.
    let max = 14 * TICKS_PER_HOUR;
    if base.abs() > max {
        return None;
    }
    if let Some(r) = &rules {
        for w in r.windows(2) {
            if w[1].date_start <= w[0].date_end {
                return None;
            }
        }
        for x in r {
            let o = base + x.base_utc_offset_delta;
            if o.abs() > max || (o + x.daylight_delta).abs() > max {
                return None;
            }
        }
    }
    let supports = rules.as_ref().is_some_and(|r| !r.is_empty());
    Some(TimeZoneInfo {
        id: z.id.clone(),
        display_name: z.display.clone(),
        standard_name: z.standard.clone(),
        daylight_name: z.daylight.clone(),
        base_utc_offset: base,
        adjustment_rules: rules,
        supports_daylight_saving_time: supports,
        loaded_with: None,
    })
}

/// `TimeZoneInfo.GetSystemTimeZones()`: every zone the registry describes,
/// sorted. Empty where there's no registry (macOS, Linux: not ported).
pub fn system_time_zones() -> Vec<TimeZoneInfo> {
    #[cfg(windows)]
    {
        // `PopulateAllSystemTimeZones`: the cache starts with `TimeZoneInfo.Utc`
        // under "UTC", so the registry's own UTC key is never read.
        let mut zones: Vec<TimeZoneInfo> = vec![utc_time_zone()];
        zones.extend(win::registry_zones().iter().filter(|z| !z.id.eq_ignore_ascii_case("UTC")).filter_map(zone_from_registry));
        sort_zones(&mut zones);
        zones
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

/// `TimeZoneInfo.Utc` as .NET creates it (`CreateUtcTimeZone`): named
/// "Coordinated Universal Time" in English (and invariant) UIs; otherwise
/// the registry UTC key's localized standard name (`GetUtcStandardDisplayName`).
pub fn utc_time_zone() -> TimeZoneInfo {
    let standard = {
        #[cfg(windows)]
        {
            win::utc_standard_display_name()
        }
        #[cfg(not(windows))]
        {
            None::<String>
        }
    };
    match standard {
        Some(name) => TimeZoneInfo { display_name: format!("(UTC) {name}"), standard_name: name.clone(), daylight_name: name, ..TimeZoneInfo::utc() },
        None => TimeZoneInfo::utc(),
    }
}

/// The zones picked in Task settings this session (as their JSON): ShareX
/// holds those as the list's own `TimeZoneInfo`, while one read from a
/// settings file went through `DateTimeZoneHandling.Local`.
static CHOSEN: std::sync::Mutex<Vec<Value>> = std::sync::Mutex::new(Vec::new());

/// Task settings picked this zone (`upload.CustomTimeZone = value`).
pub fn note_chosen(zone: &Value) {
    if let Ok(mut list) = CHOSEN.lock()
        && !list.contains(zone)
    {
        list.push(zone.clone());
    }
}

/// `TaskSettingsUpload.CustomTimeZone` as ShareX has it in memory: a zone
/// picked this session as it was listed, else as loading the settings file
/// left it ([`TimeZoneInfo::from_loaded_json`] with the machine's zone).
pub fn from_settings(zone: &Value) -> Option<TimeZoneInfo> {
    if CHOSEN.lock().is_ok_and(|list| list.contains(zone)) {
        return TimeZoneInfo::from_json(zone);
    }
    static LOCAL: std::sync::OnceLock<TimeZoneInfo> = std::sync::OnceLock::new();
    TimeZoneInfo::from_loaded_json(zone, LOCAL.get_or_init(local_time_zone))
}

/// `TimeZoneInfo.Local`: the machine's zone (the registry's zone named by
/// `GetDynamicTimeZoneInformation`), or UTC where it can't be read.
pub fn local_time_zone() -> TimeZoneInfo {
    #[cfg(windows)]
    {
        if let Some(id) = win::local_zone_id()
            && let Some(z) = win::read_zone(&id).as_ref().and_then(zone_from_registry)
        {
            return z;
        }
    }
    TimeZoneInfo::utc()
}

/// The machine `tools/settings-harness` records with its zone list: the
/// Windows build (`CurrentBuild.UBR`) and the UI language. Its list and
/// names only hold on the same machine.
pub fn machine_fingerprint() -> String {
    #[cfg(windows)]
    {
        win::fingerprint()
    }
    #[cfg(not(windows))]
    {
        String::new()
    }
}

#[cfg(windows)]
mod win {
    use super::{RegTzi, RegistryZone};
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, FreeLibrary};
    use windows_sys::Win32::Globalization::{GetFileMUIPath, GetUserDefaultUILanguage, LCIDToLocaleName, MUI_USER_PREFERRED_UI_LANGUAGES};
    use windows_sys::Win32::System::LibraryLoader::{LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE, LoadLibraryExW};
    use windows_sys::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_ANY, RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW};
    use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
    use windows_sys::Win32::System::Time::{DYNAMIC_TIME_ZONE_INFORMATION, GetDynamicTimeZoneInformation};
    use windows_sys::Win32::UI::WindowsAndMessaging::LoadStringW;

    const HIVE: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Time Zones";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: a key this module opened.
            unsafe { RegCloseKey(self.0) };
        }
    }

    fn open(path: &str) -> Option<Key> {
        let mut h: HKEY = std::ptr::null_mut();
        let p = wide(path);
        // SAFETY: valid strings and out pointer.
        (unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, p.as_ptr(), 0, KEY_READ, &mut h) } == ERROR_SUCCESS).then_some(Key(h))
    }

    /// A value's type and bytes.
    fn value(key: &Key, name: &str) -> Option<(u32, Vec<u8>)> {
        let n = wide(name);
        let mut ty = 0u32;
        let mut len = 0u32;
        // SAFETY: size query, then a buffer of that size.
        unsafe {
            if RegGetValueW(key.0, std::ptr::null(), n.as_ptr(), RRF_RT_ANY, &mut ty, std::ptr::null_mut(), &mut len) != ERROR_SUCCESS {
                return None;
            }
            let mut buf = vec![0u8; len as usize];
            if RegGetValueW(key.0, std::ptr::null(), n.as_ptr(), RRF_RT_ANY, &mut ty, buf.as_mut_ptr().cast(), &mut len) != ERROR_SUCCESS {
                return None;
            }
            buf.truncate(len as usize);
            Some((ty, buf))
        }
    }

    fn string(key: &Key, name: &str) -> String {
        match value(key, name) {
            // REG_SZ, REG_EXPAND_SZ.
            Some((1 | 2, b)) => {
                let w: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                String::from_utf16_lossy(&w).trim_end_matches('\0').to_string()
            }
            _ => String::new(),
        }
    }

    fn dword(key: &Key, name: &str) -> Option<i32> {
        match value(key, name) {
            Some((4, b)) if b.len() == 4 => Some(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            _ => None,
        }
    }

    fn tzi(key: &Key, name: &str) -> Option<RegTzi> {
        match value(key, name) {
            Some((3, b)) => RegTzi::from_bytes(&b),
            _ => None,
        }
    }

    fn subkeys(key: &Key) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0.. {
            let mut buf = [0u16; 256];
            let mut len = buf.len() as u32;
            // SAFETY: a buffer of `len` characters.
            let r = unsafe { RegEnumKeyExW(key.0, i, buf.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
            if r != ERROR_SUCCESS {
                break;
            }
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        out
    }

    /// `GetLocalizedNameByMuiNativeResource("@tzres.dll,-100")`: the string
    /// from the file's MUI resource in the preferred UI languages.
    fn mui_string(resource: &str) -> String {
        let parts: Vec<&str> = resource.split(',').collect();
        if parts.len() != 2 {
            return String::new();
        }
        let Ok(id) = parts[1].trim().parse::<i32>() else { return String::new() };
        let id = -id;
        let mut sys = [0u16; 260];
        // SAFETY: a MAX_PATH buffer.
        let n = unsafe { GetSystemDirectoryW(sys.as_mut_ptr(), sys.len() as u32) } as usize;
        let path = format!("{}\\{}", String::from_utf16_lossy(&sys[..n]), parts[0].trim_start_matches('@'));
        let path = wide(&path);
        let mut mui = [0u16; 260];
        let mut mui_len = mui.len() as u32;
        let mut lang_len = 0u32;
        let mut enumerator = 0u64;
        // SAFETY: buffers sized as passed; no language buffer.
        let ok = unsafe { GetFileMUIPath(MUI_USER_PREFERRED_UI_LANGUAGES, path.as_ptr(), std::ptr::null_mut(), &mut lang_len, mui.as_mut_ptr(), &mut mui_len, &mut enumerator) };
        if ok == 0 {
            return String::new();
        }
        let end = mui.iter().position(|&c| c == 0).unwrap_or(mui.len());
        let file: Vec<u16> = mui[..end].iter().copied().chain(std::iter::once(0)).collect();
        // SAFETY: loaded as a data file only; freed below.
        unsafe {
            let module = LoadLibraryExW(file.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE);
            if module.is_null() {
                return String::new();
            }
            let mut buf = [0u16; 500];
            let len = LoadStringW(module, id as u32, buf.as_mut_ptr(), buf.len() as i32);
            FreeLibrary(module);
            if len > 0 { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() }
        }
    }

    /// `GetLocalizedNamesByRegistryKey`: the `MUI_*` strings, else the plain values.
    fn name(key: &Key, mui: &str, plain: &str) -> String {
        let r = string(key, mui);
        let s = if r.is_empty() { String::new() } else { mui_string(&r) };
        if s.is_empty() { string(key, plain) } else { s }
    }

    /// The user's UI language (`CultureInfo.CurrentUICulture.Name`).
    fn ui_language() -> String {
        // SAFETY: plain queries into a stack buffer.
        unsafe {
            let mut buf = [0u16; 85];
            let n = LCIDToLocaleName(GetUserDefaultUILanguage() as u32, buf.as_mut_ptr(), buf.len() as i32, 0);
            if n > 1 { String::from_utf16_lossy(&buf[..n as usize - 1]) } else { String::new() }
        }
    }

    /// `GetUtcStandardDisplayName`: `None` for an English or invariant UI
    /// (the built-in "Coordinated Universal Time"), else the registry UTC
    /// key's `MUI_Std` (or `Std`); "GMT", "UTC" or nothing fall back too.
    pub fn utc_standard_display_name() -> Option<String> {
        let lang = ui_language();
        if lang.is_empty() || lang.split('-').next().is_some_and(|l| l.eq_ignore_ascii_case("en")) {
            return None;
        }
        let key = open(&format!("{HIVE}\\UTC"))?;
        let mui = string(&key, "MUI_Std");
        let mut text = if mui.is_empty() { String::new() } else { mui_string(&mui) };
        if text.is_empty() {
            text = string(&key, "Std");
        }
        (!matches!(text.as_str(), "" | "GMT" | "UTC")).then_some(text)
    }

    pub fn read_zone(id: &str) -> Option<RegistryZone> {
        let key = open(&format!("{HIVE}\\{id}"))?;
        let dynamic = open(&format!("{HIVE}\\{id}\\Dynamic DST")).map(|d| {
            let first = dword(&d, "FirstEntry").unwrap_or(-1);
            let last = dword(&d, "LastEntry").unwrap_or(-1);
            let years = if first != -1 && last != -1 && first <= last { (first..=last).map(|y| (y, tzi(&d, &y.to_string()))).collect() } else { Vec::new() };
            (first, last, years)
        });
        Some(RegistryZone {
            id: id.to_string(),
            tzi: tzi(&key, "TZI"),
            display: name(&key, "MUI_Display", "Display"),
            standard: name(&key, "MUI_Std", "Std"),
            daylight: name(&key, "MUI_Dlt", "Dlt"),
            dynamic,
        })
    }

    pub fn registry_zones() -> Vec<RegistryZone> {
        let Some(hive) = open(HIVE) else { return Vec::new() };
        subkeys(&hive).iter().filter_map(|id| read_zone(id)).collect()
    }

    pub fn local_zone_id() -> Option<String> {
        // SAFETY: a zeroed out structure filled by the call.
        let mut info: DYNAMIC_TIME_ZONE_INFORMATION = unsafe { std::mem::zeroed() };
        let r = unsafe { GetDynamicTimeZoneInformation(&mut info) };
        if r == u32::MAX {
            return None;
        }
        let end = info.TimeZoneKeyName.iter().position(|&c| c == 0).unwrap_or(info.TimeZoneKeyName.len());
        let id = String::from_utf16_lossy(&info.TimeZoneKeyName[..end]);
        (!id.is_empty()).then_some(id)
    }

    pub fn fingerprint() -> String {
        let cv = open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
        let (build, ubr) = cv.as_ref().map_or((String::new(), None), |k| (string(k, "CurrentBuild"), dword(k, "UBR")));
        format!("{build}.{} {}", ubr.unwrap_or(0), ui_language())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_and_dates_as_dotnet_writes_them() {
        assert_eq!(format_time_span(0), "00:00:00");
        assert_eq!(format_time_span(-10 * TICKS_PER_HOUR), "-10:00:00");
        assert_eq!(format_time_span(5 * TICKS_PER_HOUR + 45 * TICKS_PER_MINUTE), "05:45:00");
        assert_eq!(parse_time_span("-03:30:00"), Some(-(3 * TICKS_PER_HOUR + 30 * TICKS_PER_MINUTE)));
        assert_eq!(format_date_time(0), "0001-01-01T00:00:00Z");
        assert_eq!(format_date_time(date_only(MAX_TICKS)), "9999-12-31T00:00:00Z");
        let t = 23 * TICKS_PER_HOUR + 59 * TICKS_PER_MINUTE + 59 * TICKS_PER_SECOND + 999 * TICKS_PER_MILLISECOND;
        assert_eq!(format_date_time(t), "0001-01-01T23:59:59.999Z");
        assert_eq!(parse_date_time("0001-01-01T23:59:59.999Z"), Some(t));
        assert_eq!(day_of_week(0), 1, "0001-01-01 is a Monday");
    }

    #[test]
    fn utc_is_sharex_default() {
        let utc = TimeZoneInfo::utc();
        assert_eq!(
            utc.to_json().to_string(),
            r#"{"Id":"UTC","DisplayName":"(UTC) Coordinated Universal Time","StandardName":"Coordinated Universal Time","DaylightName":"Coordinated Universal Time","BaseUtcOffset":"00:00:00","AdjustmentRules":null,"SupportsDaylightSavingTime":false}"#
        );
        assert_eq!(TimeZoneInfo::from_json(&utc.to_json()), Some(utc));
    }
}
