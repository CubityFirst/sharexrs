//! `System.DateTime` as ShareX's history keeps it: 100 ns ticks of wall
//! clock time since 0001-01-01 and a kind (`Unspecified`, `Utc`, `Local`,
//! plus .NET's hidden "local time in the ambiguous hour that is still
//! daylight time" bit), with the three ways history dates are read and the
//! one way they are written:
//!
//! - written with `ToString("o")` (`HistoryManagerSQLite`, the XML file):
//!   `yyyy-MM-ddTHH:mm:ss.fffffff` then `Z` (UTC), the local offset (local),
//!   or nothing (unspecified);
//! - read from History.db and History.xml with `DateTime.Parse`: a string
//!   with `Z` or an offset becomes local time (`Kind == Local`), one without
//!   stays unspecified;
//! - read from History.json by Json.NET (`DateTimeZoneHandling.RoundtripKind`):
//!   `Z` stays UTC, an offset becomes local time, none is unspecified.
//!
//! Comparisons (`==`, `<`, `.Date`, `.Hour`) look at the ticks only, as
//! .NET's do. The local time zone is behind [`LocalZone`] so tests can pin
//! one; [`SystemZone`] is the machine's (chrono's, which on Windows reads
//! the same dynamic time zone rules .NET's `TimeZoneInfo.Local` does).

use std::cmp::Ordering;
use std::fmt;

use chrono::{Datelike, NaiveDate, NaiveDateTime, TimeDelta, Timelike};

pub const TICKS_PER_SECOND: i64 = 10_000_000;
pub const TICKS_PER_MINUTE: i64 = 60 * TICKS_PER_SECOND;
pub const TICKS_PER_HOUR: i64 = 60 * TICKS_PER_MINUTE;
pub const TICKS_PER_DAY: i64 = 24 * TICKS_PER_HOUR;
/// `DateTime.MaxValue.Ticks`.
pub const MAX_TICKS: i64 = 3_155_378_975_999_999_999;

/// `DateTimeKind`, with the ambiguous-daylight bit .NET keeps for local times.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum DateTimeKind {
    #[default]
    Unspecified,
    Utc,
    Local,
}

/// A `System.DateTime`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DateTime {
    ticks: i64,
    kind: DateTimeKind,
    /// `KindLocalAmbiguousDst`: a local time in the repeated hour that is the
    /// first (daylight) occurrence. Only set by conversions from UTC.
    ambiguous_dst: bool,
}

/// `DateTime.MinValue`.
pub const MIN_VALUE: DateTime = DateTime { ticks: 0, kind: DateTimeKind::Unspecified, ambiguous_dst: false };

impl PartialEq for DateTime {
    /// `DateTime.op_Equality`: the ticks, not the kind.
    fn eq(&self, other: &Self) -> bool {
        self.ticks == other.ticks
    }
}
impl Eq for DateTime {}
impl std::hash::Hash for DateTime {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ticks.hash(state);
    }
}
impl PartialOrd for DateTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DateTime {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ticks.cmp(&other.ticks)
    }
}

/// The local time zone: the offset at a UTC instant, and the offsets a wall
/// clock time can have.
pub trait LocalZone {
    /// Seconds east of UTC at the UTC time `utc`.
    fn utc_offset(&self, utc: NaiveDateTime) -> i64;
    /// What `local` can be: one offset, two (the repeated hour, daylight
    /// first) or none (the skipped hour).
    fn local_offsets(&self, local: NaiveDateTime) -> LocalOffsets;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalOffsets {
    Single(i64),
    /// The repeated hour: `daylight` is the first occurrence's offset.
    Ambiguous { daylight: i64, standard: i64 },
    /// The skipped hour.
    Gap,
}

/// The machine's time zone, through chrono.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemZone;

/// chrono's Windows zone works with `SYSTEMTIME` years (1601..=30827);
/// outside the years history can sensibly hold, the same day in 2000 stands
/// in (.NET has no daylight rules for those years either).
fn zone_safe(t: NaiveDateTime) -> NaiveDateTime {
    if (1700..=9000).contains(&t.year()) {
        t
    } else {
        NaiveDate::from_ymd_opt(2000, t.month(), t.day().min(28)).unwrap_or_default().and_time(t.time())
    }
}

impl LocalZone for SystemZone {
    fn utc_offset(&self, utc: NaiveDateTime) -> i64 {
        use chrono::{Offset, TimeZone};
        chrono::Local.offset_from_utc_datetime(&zone_safe(utc)).fix().local_minus_utc() as i64
    }

    fn local_offsets(&self, local: NaiveDateTime) -> LocalOffsets {
        use chrono::{LocalResult, Offset, TimeZone};
        match chrono::Local.offset_from_local_datetime(&zone_safe(local)) {
            LocalResult::Single(o) => LocalOffsets::Single(o.fix().local_minus_utc() as i64),
            LocalResult::Ambiguous(a, b) => {
                let (a, b) = (a.fix().local_minus_utc() as i64, b.fix().local_minus_utc() as i64);
                LocalOffsets::Ambiguous { daylight: a.max(b), standard: a.min(b) }
            }
            LocalResult::None => LocalOffsets::Gap,
        }
    }
}

/// A fixed-rule zone for tests: `standard` seconds east of UTC, plus an
/// hour of daylight time between the EU's switches (the last Sundays of
/// March and October, 01:00 UTC) when `eu_dst`. London is `(0, true)`.
#[derive(Clone, Copy, Debug)]
pub struct RuleZone {
    pub standard: i64,
    pub eu_dst: bool,
}

impl RuleZone {
    fn last_sunday(year: i32, month: u32) -> NaiveDateTime {
        let mut d = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - TimeDelta::days(1);
        while d.weekday() != chrono::Weekday::Sun {
            d -= TimeDelta::days(1);
        }
        d.and_hms_opt(1, 0, 0).unwrap()
    }

    fn is_dst_utc(&self, utc: NaiveDateTime) -> bool {
        self.eu_dst && utc >= Self::last_sunday(utc.year(), 3) && utc < Self::last_sunday(utc.year(), 10)
    }
}

impl LocalZone for RuleZone {
    fn utc_offset(&self, utc: NaiveDateTime) -> i64 {
        self.standard + if self.is_dst_utc(utc) { 3600 } else { 0 }
    }

    fn local_offsets(&self, local: NaiveDateTime) -> LocalOffsets {
        let candidates = [self.standard + 3600, self.standard];
        let fits: Vec<i64> = candidates
            .into_iter()
            .filter(|&o| self.utc_offset(local - TimeDelta::seconds(o)) == o)
            .collect();
        match fits.as_slice() {
            [o] => LocalOffsets::Single(*o),
            [d, s] => LocalOffsets::Ambiguous { daylight: *d, standard: *s },
            _ => LocalOffsets::Gap,
        }
    }
}

/// Why a date string didn't parse (`FormatException`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "String '{}' was not recognized as a valid DateTime.", self.0)
    }
}

impl std::error::Error for ParseError {}

impl DateTime {
    /// `new DateTime(ticks, kind)`.
    pub fn from_ticks(ticks: i64, kind: DateTimeKind) -> Self {
        Self { ticks: ticks.clamp(0, MAX_TICKS), kind, ambiguous_dst: false }
    }

    /// `new DateTime(year, month, day, hour, minute, second, kind)` plus ticks.
    pub fn new(date: NaiveDateTime, kind: DateTimeKind) -> Self {
        Self::from_ticks(naive_to_ticks(date), kind)
    }

    pub fn ticks(&self) -> i64 {
        self.ticks
    }

    pub fn kind(&self) -> DateTimeKind {
        self.kind
    }

    /// `IsAmbiguousDaylightSavingTime`'s fast path: .NET's hidden bit.
    pub fn is_ambiguous_dst(&self) -> bool {
        self.ambiguous_dst
    }

    pub fn is_min_value(&self) -> bool {
        self.ticks == 0
    }

    /// The wall clock time.
    pub fn naive(&self) -> NaiveDateTime {
        ticks_to_naive(self.ticks)
    }

    /// `DateTime.Date` (keeps the kind, drops the ambiguous bit as .NET does).
    pub fn date(&self) -> DateTime {
        Self::from_ticks(self.ticks - self.ticks.rem_euclid(TICKS_PER_DAY), self.kind)
    }

    pub fn year(&self) -> i32 {
        self.naive().year()
    }

    pub fn hour(&self) -> u32 {
        ((self.ticks / TICKS_PER_HOUR) % 24) as u32
    }

    /// `DayOfWeek` as a number: Sunday is 0.
    pub fn day_of_week(&self) -> u32 {
        // 0001-01-01 was a Monday.
        ((self.ticks / TICKS_PER_DAY + 1) % 7) as u32
    }

    /// `AddTicks` (no time zone adjustment, as .NET).
    pub fn add_ticks(&self, ticks: i64) -> DateTime {
        Self::from_ticks(self.ticks + ticks, self.kind)
    }

    /// `AddDays(n)` for whole days.
    pub fn add_days(&self, days: i64) -> DateTime {
        self.add_ticks(days * TICKS_PER_DAY)
    }

    /// `AddYears(n)` (Feb 29 becomes Feb 28).
    pub fn add_years(&self, years: i32) -> DateTime {
        let n = self.naive();
        let y = n.year() + years;
        let date = NaiveDate::from_ymd_opt(y, n.month(), n.day())
            .or_else(|| NaiveDate::from_ymd_opt(y, n.month(), 28))
            .unwrap_or(n.date());
        let ticks_of_day = self.ticks.rem_euclid(TICKS_PER_DAY);
        Self::from_ticks(naive_to_ticks(date.and_hms_opt(0, 0, 0).unwrap()) + ticks_of_day, self.kind)
    }

    /// The local time `now` (`DateTime.Now`).
    pub fn now() -> DateTime {
        Self::now_in(&SystemZone)
    }

    pub fn now_in(zone: &dyn LocalZone) -> DateTime {
        Self::from_utc_to_local(naive_to_ticks(chrono::Utc::now().naive_utc()), zone)
    }

    /// A file's `File.GetLastWriteTime`: the UTC time stamp made local.
    pub fn from_system_time(time: std::time::SystemTime, zone: &dyn LocalZone) -> DateTime {
        let utc: chrono::DateTime<chrono::Utc> = time.into();
        Self::from_utc_to_local(naive_to_ticks(utc.naive_utc()), zone)
    }

    /// `new DateTime(utcTicks, Utc).ToLocalTime()`: the offset at that
    /// instant, and the ambiguous bit when it lands on the first occurrence
    /// of a repeated hour. Out of range clamps to Min/MaxValue (local).
    pub fn from_utc_to_local(utc_ticks: i64, zone: &dyn LocalZone) -> DateTime {
        let utc = ticks_to_naive(utc_ticks.clamp(0, MAX_TICKS));
        let offset = zone.utc_offset(utc);
        let ticks = utc_ticks + offset * TICKS_PER_SECOND;
        if ticks > MAX_TICKS {
            return Self { ticks: MAX_TICKS, kind: DateTimeKind::Local, ambiguous_dst: false };
        }
        if ticks < 0 {
            return Self { ticks: 0, kind: DateTimeKind::Local, ambiguous_dst: false };
        }
        let ambiguous_dst = matches!(
            zone.local_offsets(ticks_to_naive(ticks)),
            LocalOffsets::Ambiguous { daylight, standard } if offset == daylight && daylight != standard
        );
        Self { ticks, kind: DateTimeKind::Local, ambiguous_dst }
    }

    /// The offset `ToString("o")` writes for a local time
    /// (`TimeZoneInfo.Local.GetUtcOffset`): the repeated hour is standard
    /// time unless the ambiguous bit says otherwise, and the skipped hour
    /// takes the standard offset.
    pub fn local_offset(&self, zone: &dyn LocalZone) -> i64 {
        let local = self.naive();
        match zone.local_offsets(local) {
            LocalOffsets::Single(o) => o,
            LocalOffsets::Ambiguous { daylight, standard } => {
                if self.ambiguous_dst {
                    daylight
                } else {
                    standard
                }
            }
            // The offset before the jump forward is the standard one.
            LocalOffsets::Gap => zone.utc_offset(local - TimeDelta::days(1)),
        }
    }

    /// `ToString("o")` in the machine's zone.
    pub fn to_string_o(&self) -> String {
        self.to_string_o_in(&SystemZone)
    }

    /// `ToString("o")`: `yyyy-MM-ddTHH:mm:ss.fffffff` and `Z`, `+HH:mm` or nothing.
    pub fn to_string_o_in(&self, zone: &dyn LocalZone) -> String {
        let n = self.naive();
        let fraction = self.ticks % TICKS_PER_SECOND;
        let mut s = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:07}",
            n.year(),
            n.month(),
            n.day(),
            n.hour(),
            n.minute(),
            n.second(),
            fraction
        );
        match self.kind {
            DateTimeKind::Utc => s.push('Z'),
            DateTimeKind::Local => s.push_str(&format_offset(self.local_offset(zone))),
            DateTimeKind::Unspecified => {}
        }
        s
    }

    /// `DateTime.Parse(s)` (current culture) for the ISO 8601 forms history
    /// files hold, in the machine's zone.
    pub fn parse(s: &str) -> Result<DateTime, ParseError> {
        Self::parse_in(s, &SystemZone)
    }

    /// `DateTime.Parse(s)`: a time with `Z` or an offset is converted to
    /// local time (`AdjustTimeZoneToLocal`); without one it is unspecified.
    /// The fraction is summed as a double and rounded half to even, as
    /// `DateTimeParse.ParseFraction` does.
    pub fn parse_in(s: &str, zone: &dyn LocalZone) -> Result<DateTime, ParseError> {
        let iso = parse_iso(s.trim(), true).ok_or_else(|| ParseError(s.to_string()))?;
        let mut fraction = 0.0f64;
        let mut base = 0.1f64;
        for d in iso.fraction_digits.bytes() {
            fraction += (d - b'0') as f64 * base;
            base *= 0.1;
        }
        let ticks = iso.ticks + (fraction * TICKS_PER_SECOND as f64).round_ties_even() as i64;
        match iso.zone {
            None => Ok(Self::from_ticks(ticks, DateTimeKind::Unspecified)),
            Some(offset) => {
                let utc = ticks - offset * TICKS_PER_SECOND;
                if !(0..=MAX_TICKS).contains(&utc) {
                    return Err(ParseError(s.to_string()));
                }
                Ok(Self::from_utc_to_local(utc, zone))
            }
        }
    }

    /// `DateTime.TryParse(s, out d)`: `MinValue` when it fails.
    pub fn try_parse_in(s: &str, zone: &dyn LocalZone) -> Option<DateTime> {
        Self::parse_in(s, zone).ok()
    }

    /// Json.NET's `DateTimeUtils.TryParseDateTimeIso` with
    /// `DateTimeZoneHandling.RoundtripKind`: `Z` stays UTC, an offset is
    /// converted to local time, no zone is unspecified; the fraction is
    /// kept to seven digits, an eighth of 5 or more rounding up.
    pub fn parse_json_in(s: &str, zone: &dyn LocalZone) -> Option<DateTime> {
        let iso = parse_iso(s, false)?;
        let digits = &iso.fraction_digits[..iso.fraction_digits.len().min(7)];
        let mut fraction: i64 = digits.bytes().fold(0, |a, d| a * 10 + (d - b'0') as i64);
        for _ in digits.len()..7 {
            fraction *= 10;
        }
        if iso.fraction_digits.as_bytes().get(7).is_some_and(|&d| d >= b'5') {
            fraction += 1;
        }
        let ticks = iso.ticks + fraction;
        match iso.zone {
            None => Some(Self::from_ticks(ticks, DateTimeKind::Unspecified)),
            Some(0) if iso.zulu => Some(Self::from_ticks(ticks, DateTimeKind::Utc)),
            Some(offset) => {
                let utc = ticks - offset * TICKS_PER_SECOND;
                if utc > MAX_TICKS {
                    // Json.NET: too late for UTC, so the local offset is added instead.
                    let local = ticks + Self::from_ticks(ticks, DateTimeKind::Local).local_offset(zone) * TICKS_PER_SECOND;
                    return Some(Self::from_ticks(local.min(MAX_TICKS), DateTimeKind::Local));
                }
                Some(Self::from_utc_to_local(utc.max(0), zone))
            }
        }
    }
}

/// `+HH:mm` / `-HH:mm`.
pub fn format_offset(seconds: i64) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let m = seconds.abs() / 60;
    format!("{sign}{:02}:{:02}", m / 60, m % 60)
}

pub fn naive_to_ticks(t: NaiveDateTime) -> i64 {
    let epoch = NaiveDate::from_ymd_opt(1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
    let d = t - epoch;
    d.num_seconds() * TICKS_PER_SECOND + (d.subsec_nanos() as i64) / 100
}

pub fn ticks_to_naive(ticks: i64) -> NaiveDateTime {
    let epoch = NaiveDate::from_ymd_opt(1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
    epoch + TimeDelta::seconds(ticks.div_euclid(TICKS_PER_SECOND)) + TimeDelta::nanoseconds(ticks.rem_euclid(TICKS_PER_SECOND) * 100)
}

struct Iso<'a> {
    /// Up to whole seconds.
    ticks: i64,
    fraction_digits: &'a str,
    /// Seconds east of UTC.
    zone: Option<i64>,
    zulu: bool,
}

/// `yyyy-MM-dd`, then optionally `T` (or a space for `DateTime.Parse`)
/// `HH:mm[:ss[.f…]]` and `Z` / `±HH[:mm]` / `±HHmm`. Json.NET wants the
/// seconds and a `T`.
fn parse_iso(s: &str, lenient: bool) -> Option<Iso<'_>> {
    let b = s.as_bytes();
    let num = |from: usize, len: usize| -> Option<u32> {
        let part = b.get(from..from + len)?;
        part.iter().all(u8::is_ascii_digit).then(|| part.iter().fold(0u32, |a, d| a * 10 + (d - b'0') as u32))
    };
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    if b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year as i32, month, day)?;
    let mut i = 10;
    let (mut h, mut m, mut sec) = (0, 0, 0);
    let mut fraction_digits = "";
    if i < b.len() {
        match b[i] {
            b'T' => {}
            b' ' if lenient => {}
            _ => return None,
        }
        h = num(i + 1, 2)?;
        if b.get(i + 3) != Some(&b':') {
            return None;
        }
        m = num(i + 4, 2)?;
        i += 6;
        if b.get(i) == Some(&b':') {
            sec = num(i + 1, 2)?;
            i += 3;
            if b.get(i) == Some(&b'.') {
                let start = i + 1;
                let mut end = start;
                while end < b.len() && b[end].is_ascii_digit() {
                    end += 1;
                }
                if end == start {
                    return None;
                }
                fraction_digits = &s[start..end];
                i = end;
            }
        } else if !lenient {
            return None;
        }
    } else if !lenient && b.len() != 10 {
        return None;
    }
    if h > 23 || m > 59 || sec > 59 {
        return None;
    }
    let mut zone = None;
    let mut zulu = false;
    if i < b.len() {
        match b[i] {
            b'Z' | b'z' if i + 1 == b.len() => {
                zone = Some(0);
                zulu = true;
            }
            c @ (b'+' | b'-') => {
                let zh = num(i + 1, 2)? as i64;
                let rest = &b[i + 3..];
                let zm = match rest {
                    [] => 0,
                    [b':', _, _] => num(i + 4, 2)? as i64,
                    [_, _] => num(i + 3, 2)? as i64,
                    _ => return None,
                };
                if zh > 14 || zm > 59 {
                    return None;
                }
                let off = zh * 3600 + zm * 60;
                zone = Some(if c == b'-' { -off } else { off });
            }
            _ => return None,
        }
    }
    let base = naive_to_ticks(date.and_hms_opt(h, m, sec)?);
    Some(Iso { ticks: base, fraction_digits, zone, zulu })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONDON: RuleZone = RuleZone { standard: 0, eu_dst: true };
    const TOKYO: RuleZone = RuleZone { standard: 9 * 3600, eu_dst: false };

    fn local(s: &str) -> DateTime {
        DateTime::new(NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f").unwrap(), DateTimeKind::Local)
    }

    #[test]
    fn round_trip_format() {
        let d = local("2026-07-01 12:34:56.1234567");
        assert_eq!(d.to_string_o_in(&LONDON), "2026-07-01T12:34:56.1234567+01:00");
        assert_eq!(d.to_string_o_in(&TOKYO), "2026-07-01T12:34:56.1234567+09:00");
        let w = local("2026-01-01 00:00:00");
        assert_eq!(w.to_string_o_in(&LONDON), "2026-01-01T00:00:00.0000000+00:00");
        assert_eq!(DateTime::from_ticks(w.ticks(), DateTimeKind::Utc).to_string_o(), "2026-01-01T00:00:00.0000000Z");
        assert_eq!(MIN_VALUE.to_string_o(), "0001-01-01T00:00:00.0000000");
    }

    #[test]
    fn parse_converts_offsets_to_local() {
        let d = DateTime::parse_in("2026-07-01T12:00:00.0000000+09:00", &LONDON).unwrap();
        assert_eq!((d.kind(), d.to_string_o_in(&LONDON).as_str()), (DateTimeKind::Local, "2026-07-01T04:00:00.0000000+01:00"));
        let z = DateTime::parse_in("2026-01-05T10:00:00Z", &LONDON).unwrap();
        assert_eq!(z.kind(), DateTimeKind::Local);
        let u = DateTime::parse_in("2026-01-05T10:00:00.5", &LONDON).unwrap();
        assert_eq!((u.kind(), u.ticks() % TICKS_PER_SECOND), (DateTimeKind::Unspecified, 5_000_000));
        let j = DateTime::parse_json_in("2026-01-05T10:00:00Z", &LONDON).unwrap();
        assert_eq!(j.kind(), DateTimeKind::Utc);
        assert!(DateTime::parse_in("", &LONDON).is_err());
    }

    #[test]
    fn the_repeated_hour_keeps_which_occurrence_it_was() {
        // 2025-10-26: 01:30 London happens at 00:30Z (BST) and 01:30Z (GMT).
        let first = DateTime::parse_in("2025-10-26T00:30:00Z", &LONDON).unwrap();
        let second = DateTime::parse_in("2025-10-26T01:30:00Z", &LONDON).unwrap();
        assert_eq!(first.ticks(), second.ticks());
        assert!(first.is_ambiguous_dst() && !second.is_ambiguous_dst());
        assert_eq!(first.to_string_o_in(&LONDON), "2025-10-26T01:30:00.0000000+01:00");
        assert_eq!(second.to_string_o_in(&LONDON), "2025-10-26T01:30:00.0000000+00:00");
        // Without the bit, standard time; the skipped hour is standard too.
        assert_eq!(local("2025-10-26 01:30:00").to_string_o_in(&LONDON), "2025-10-26T01:30:00.0000000+00:00");
        assert_eq!(local("2025-03-30 01:30:00").to_string_o_in(&LONDON), "2025-03-30T01:30:00.0000000+00:00");
    }

    #[test]
    fn calendar_parts() {
        let d = local("2026-11-17 13:05:00");
        assert_eq!((d.day_of_week(), d.hour(), d.year()), (2, 13, 2026));
        assert_eq!(d.date().naive().to_string(), "2026-11-17 00:00:00");
        assert_eq!(local("2024-02-29 00:00:00").add_years(-1).naive().to_string(), "2023-02-28 00:00:00");
    }
}
