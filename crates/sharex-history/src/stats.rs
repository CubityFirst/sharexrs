//! The stats: `HistoryStats` (the sidebar's Stats window, `StatsWindow`)
//! and `HistoryHelpers.OutputStats` (the text in the History and Image
//! history windows' Stats dialog), with the number formats and chart maths
//! of `StatsCharts.cs` that don't depend on drawing.
//!
//! Dates are wall clock times (.NET compares `DateTime` ticks, whatever the
//! kind), so `now` is the local time the window was refreshed.

use std::collections::{BTreeMap, HashMap};

use sharex_helpers::file_helpers::{get_file_name_extension, is_image_file, is_video_file};
use sharex_strings::history_lib as s;

use crate::culture::{ordinal_ignore_case_cmp, ordinal_upper};
use crate::datetime::{DateTime, TICKS_PER_DAY};
use crate::item::{HistoryItem, is_null_or_empty, is_null_or_white_space};

/// `HistoryStatsKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryStatsKind {
    Image,
    Recording,
    Other,
}

/// `HistoryStatsFilter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HistoryStatsFilter {
    #[default]
    All,
    Images,
    Recordings,
}

/// `HistoryStatsTally`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HistoryStatsTally {
    pub images: i32,
    pub recordings: i32,
    pub other: i32,
}

impl HistoryStatsTally {
    pub fn total(&self) -> i32 {
        self.images + self.recordings + self.other
    }

    pub fn add(&mut self, kind: HistoryStatsKind) {
        match kind {
            HistoryStatsKind::Image => self.images += 1,
            HistoryStatsKind::Recording => self.recordings += 1,
            HistoryStatsKind::Other => self.other += 1,
        }
    }

    pub fn get(&self, filter: HistoryStatsFilter) -> i32 {
        match filter {
            HistoryStatsFilter::Images => self.images,
            HistoryStatsFilter::Recordings => self.recordings,
            HistoryStatsFilter::All => self.total(),
        }
    }
}

/// `HistoryStatsEntry`: an empty name is the folded "other" entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryStatsEntry {
    pub name: String,
    pub count: i32,
}

const TOP_ENTRY_COUNT: usize = 6;

/// `HistoryStats`.
#[derive(Clone, Debug, Default)]
pub struct HistoryStats {
    /// `now.Date`.
    pub today: DateTime,
    /// `DayOfWeek` (Sunday 0).
    pub first_day_of_week: u32,
    pub total: HistoryStatsTally,
    /// By day (`DateTime.Date` ticks), in date order.
    pub daily: BTreeMap<i64, HistoryStatsTally>,
    pub hours: [HistoryStatsTally; 24],
    /// By `DayOfWeek` (Sunday 0).
    pub weekdays: [HistoryStatsTally; 7],
    pub first_day: Option<DateTime>,
    pub apps: Vec<HistoryStatsEntry>,
    pub destinations: Vec<HistoryStatsEntry>,
    pub file_types: Vec<HistoryStatsEntry>,
    pub today_tally: HistoryStatsTally,
    pub past_7_days: i32,
    pub previous_7_days: i32,
    pub streak: i32,
    pub longest_streak: i32,
    /// `MinValue` when there's no streak.
    pub longest_streak_end: DateTime,
    pub busiest_day: Option<DateTime>,
    pub busiest_day_count: i32,
}

/// `FileHelpers.GetFileNameExtension(name, checkSecondExtension: false)`.
fn last_extension(name: &str) -> &str {
    name.rfind('.').map_or("", |i| &name[i + 1..])
}

/// `char.IsLetterOrDigit` on UTF-16 code units: astral characters are
/// surrogates, never letters.
fn is_letter_or_digit(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric()
    } else {
        (c as u32) <= 0xFFFF && (c.is_alphabetic() || c.is_numeric())
    }
}

/// A `Dictionary<string, int>` with `StringComparer.OrdinalIgnoreCase`, in
/// insertion order, keeping the first spelling of each key.
#[derive(Default)]
struct Counts {
    order: Vec<(String, i32)>,
    index: HashMap<String, usize>,
}

impl Counts {
    /// `Increment`: non-blank names, trimmed.
    fn increment(&mut self, name: Option<&str>) {
        if is_null_or_white_space(name) {
            return;
        }
        let name = name.unwrap().trim();
        let key = ordinal_upper(name);
        match self.index.get(&key) {
            Some(&i) => self.order[i].1 += 1,
            None => {
                self.index.insert(key, self.order.len());
                self.order.push((name.to_string(), 1));
            }
        }
    }

    /// `GetTopEntries`: by count, then name ignoring case; past the top six
    /// (when there are at least eight) the rest fold into a nameless entry.
    fn top_entries(mut self) -> Vec<HistoryStatsEntry> {
        self.order.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| ordinal_ignore_case_cmp(&a.0, &b.0)));
        let mut entries: Vec<HistoryStatsEntry> =
            self.order.into_iter().map(|(name, count)| HistoryStatsEntry { name, count }).collect();
        if entries.len() > TOP_ENTRY_COUNT + 1 {
            let rest = entries[TOP_ENTRY_COUNT..].iter().map(|e| e.count).sum();
            entries.truncate(TOP_ENTRY_COUNT);
            entries.push(HistoryStatsEntry { name: String::new(), count: rest });
        }
        entries
    }
}

fn kind_file_name(item: &HistoryItem) -> &str {
    if !is_null_or_empty(&item.file_name) { item.file_name.as_deref().unwrap() } else { item.file_path.as_deref().unwrap_or("") }
}

impl HistoryStats {
    /// `HistoryStats.GetKind`: a video or GIF is a recording, an `Image`
    /// item or image file an image, the rest other.
    pub fn get_kind(item: &HistoryItem) -> HistoryStatsKind {
        let file_name = kind_file_name(item);
        let extension = last_extension(file_name);
        if is_video_file(file_name) || extension.eq_ignore_ascii_case("gif") {
            HistoryStatsKind::Recording
        } else if item.r#type.as_deref().is_some_and(|t| crate::culture::ordinal_ignore_case_eq(t, "Image")) || is_image_file(file_name) {
            HistoryStatsKind::Image
        } else {
            HistoryStatsKind::Other
        }
    }

    /// `GetFileType`: the last extension in capitals, if it's 1 to 6
    /// letters or digits.
    fn get_file_type(item: &HistoryItem) -> Option<String> {
        let extension = last_extension(kind_file_name(item));
        let len = extension.encode_utf16().count();
        if len == 0 || len > 6 || !extension.chars().all(is_letter_or_digit) {
            return None;
        }
        Some(ordinal_upper(extension))
    }

    /// `HistoryStats.Calculate(items, now, firstDayOfWeek)`.
    pub fn calculate<'a>(items: impl IntoIterator<Item = &'a HistoryItem>, now: DateTime, first_day_of_week: u32) -> Self {
        let mut stats = HistoryStats { today: now.date(), first_day_of_week, ..Default::default() };
        let (mut apps, mut destinations, mut file_types) = (Counts::default(), Counts::default(), Counts::default());
        let week_ago = now.add_days(-7);
        let two_weeks_ago = now.add_days(-14);
        for item in items {
            if item.date_time.is_min_value() {
                continue;
            }
            let kind = Self::get_kind(item);
            let day = item.date_time.date();
            stats.total.add(kind);
            stats.hours[item.date_time.hour() as usize].add(kind);
            stats.weekdays[item.date_time.day_of_week() as usize].add(kind);
            stats.daily.entry(day.ticks()).or_default().add(kind);
            if item.date_time <= now {
                if item.date_time > week_ago {
                    stats.past_7_days += 1;
                } else if item.date_time > two_weeks_ago {
                    stats.previous_7_days += 1;
                }
            }
            apps.increment(item.tags_process_name());
            destinations.increment(item.host.as_deref());
            file_types.increment(Self::get_file_type(item).as_deref());
        }
        stats.first_day = stats.daily.keys().next().map(|&t| DateTime::from_ticks(t, now.kind()));
        stats.apps = apps.top_entries();
        stats.destinations = destinations.top_entries();
        stats.file_types = file_types.top_entries();
        stats.calculate_highlights();
        stats
    }

    /// `DaysCounted`.
    pub fn days_counted(&self) -> i32 {
        match self.first_day {
            Some(first) => 0.max(((self.today.ticks() - first.ticks()) / TICKS_PER_DAY) as i32) + 1,
            None => 0,
        }
    }

    /// `GetDay`.
    pub fn get_day(&self, day: DateTime) -> HistoryStatsTally {
        self.daily.get(&day.date().ticks()).copied().unwrap_or_default()
    }

    /// `CountDays(from, to, predicate)`.
    pub fn count_days(&self, from: DateTime, to: DateTime, predicate: impl Fn(&HistoryStatsTally) -> bool) -> i32 {
        if from > to {
            return 0;
        }
        self.daily.range(from.ticks()..=to.ticks()).filter(|(_, t)| predicate(t)).count() as i32
    }

    /// `GetWeekStart`.
    pub fn get_week_start(&self, day: DateTime) -> DateTime {
        let offset = (day.day_of_week() as i64 - self.first_day_of_week as i64 + 7) % 7;
        day.date().add_days(-offset)
    }

    fn calculate_highlights(&mut self) {
        self.today_tally = self.get_day(self.today);
        let mut day = if self.today_tally.total() > 0 { self.today } else { self.today.add_days(-1) };
        while self.get_day(day).total() > 0 {
            self.streak += 1;
            day = day.add_days(-1);
        }
        let mut run = 0;
        let mut previous: Option<i64> = None;
        for (&key, tally) in &self.daily {
            let count = tally.total();
            if count <= 0 {
                continue;
            }
            run = if run > 0 && previous.is_some_and(|p| p + TICKS_PER_DAY == key) { run + 1 } else { 1 };
            previous = Some(key);
            if run > self.longest_streak {
                self.longest_streak = run;
                self.longest_streak_end = DateTime::from_ticks(key, self.today.kind());
            }
            if count > self.busiest_day_count {
                self.busiest_day_count = count;
                self.busiest_day = Some(DateTime::from_ticks(key, self.today.kind()));
            }
        }
    }

    /// `StatsHourChart.GetPeakHour`: the first busiest hour.
    pub fn peak_hour(&self, filter: HistoryStatsFilter) -> Option<u32> {
        let (mut peak, mut max) = (None, 0);
        for (hour, tally) in self.hours.iter().enumerate() {
            let count = tally.get(filter);
            if count > max {
                max = count;
                peak = Some(hour as u32);
            }
        }
        peak
    }

    /// `ShowWeekdays`' peak: the first busiest `DayOfWeek` (Sunday 0).
    pub fn peak_weekday(&self) -> Option<u32> {
        let max = self.weekdays.iter().map(HistoryStatsTally::total).max().unwrap_or(0);
        (max > 0).then(|| self.weekdays.iter().position(|t| t.total() == max).unwrap() as u32)
    }

    /// `StatsCalendar.GetRange(stats, span)`: the days the calendar shows.
    pub fn calendar_range(&self, span: CalendarSpan) -> Option<(DateTime, DateTime)> {
        let first = self.first_day?;
        let today = self.today;
        let (from, to) = match span {
            CalendarSpan::PastYear => (first.max(today.add_years(-1).add_days(1)), today),
            CalendarSpan::AllYears => (first, today),
            CalendarSpan::Year(y) => {
                let jan1 = DateTime::new(chrono::NaiveDate::from_ymd_opt(y, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap(), today.kind());
                let dec31 = DateTime::new(chrono::NaiveDate::from_ymd_opt(y, 12, 31).unwrap().and_hms_opt(0, 0, 0).unwrap(), today.kind());
                (first.max(jan1), today.min(dec31))
            }
        };
        Some((from, to))
    }

    /// `ShowCalendarSpans`: past year, then (when history goes back past
    /// this year) all years and each year from this one back.
    pub fn calendar_spans(&self) -> Vec<CalendarSpan> {
        let this_year = self.today.year();
        let first_year = self.first_day.map_or(this_year, |d| d.year());
        let mut spans = vec![CalendarSpan::PastYear];
        if first_year < this_year {
            spans.push(CalendarSpan::AllYears);
            spans.extend((first_year..=this_year).rev().map(CalendarSpan::Year));
        }
        spans
    }

    /// `StatsCalendar.CreateLayout`'s fill steps: the quartiles of the busy
    /// days within the ranges shown.
    pub fn calendar_steps(&self, ranges: &[(DateTime, DateTime)]) -> [i32; 3] {
        let mut busy: Vec<i32> = Vec::new();
        for (from, to) in ranges {
            for (&k, t) in &self.daily {
                if k >= from.ticks() && k <= to.ticks() && t.total() > 0 {
                    busy.push(t.total());
                }
            }
        }
        busy.sort();
        if busy.is_empty() {
            return [0, 0, 0];
        }
        let n = busy.len() - 1;
        [busy[n / 4], busy[n / 2], busy[n * 3 / 4]]
    }
}

/// `StatsCalendar.GetLevel`: 0 (none) to 4.
pub fn calendar_level(count: i32, steps: [i32; 3]) -> u8 {
    match count {
        c if c <= 0 => 0,
        c if c <= steps[0] => 1,
        c if c <= steps[1] => 2,
        c if c <= steps[2] => 3,
        _ => 4,
    }
}

/// `StatsCalendar.PastYear` (0), `AllYears` (-1) or a year.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarSpan {
    PastYear,
    AllYears,
    Year(i32),
}

/// `StatsHourChart.GetScale`: a 1, 2 or 5 × 10ⁿ grid step for about two
/// lines, and the top of the scale.
pub fn hour_chart_scale(max: i32) -> (f64, f64) {
    if max <= 0 {
        return (1.0, 1.0);
    }
    let rough = max as f64 / 2.0;
    let magnitude = 10f64.powf(rough.log10().floor());
    let normalized = rough / magnitude;
    let step = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    } * magnitude;
    let step = step.max(1.0);
    (step, (max as f64 / step).ceil() * step)
}

/// .NET's custom numeric format digits for a double (15 significant
/// digits, then rounded half away from zero to `decimals`), with optional
/// decimals as in `0.#`: `decimals` is how many `#`.
pub fn format_custom(value: f64, decimals: usize) -> String {
    let fifteen = format!("{:.*e}", 14, value);
    let (mantissa, exp) = fifteen.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let negative = mantissa.starts_with('-');
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|d| d - b'0').collect();
    // Value = 0.d1d2... × 10^(exp+1); keep digits up to `decimals` after the point.
    let point = exp + 1;
    let keep = point + decimals as i32;
    let mut kept: Vec<u8> = digits.iter().take(keep.max(0) as usize).copied().collect();
    let next = if keep >= 0 { digits.get(keep as usize).copied().unwrap_or(0) } else { 0 };
    while (kept.len() as i32) < keep {
        kept.push(0);
    }
    if next >= 5 {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, 1);
                break;
            }
            i -= 1;
            if kept[i] == 9 {
                kept[i] = 0;
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let int_len = (kept.len() as i32 - decimals as i32).max(0) as usize;
    let int_part: String = kept[..int_len].iter().map(|d| (d + b'0') as char).collect();
    let mut frac: String = kept[int_len..].iter().map(|d| (d + b'0') as char).collect();
    while frac.ends_with('0') {
        frac.pop();
    }
    let int_part = if int_part.is_empty() { "0".to_string() } else { int_part.trim_start_matches('0').to_string() };
    let int_part = if int_part.is_empty() { "0".to_string() } else { int_part };
    let body = if frac.is_empty() { int_part } else { format!("{int_part}.{frac}") };
    if negative && body.chars().any(|c| c.is_ascii_digit() && c != '0') { format!("-{body}") } else { body }
}

/// `StatsFormat.Number`: `N0` with the culture's group separator.
pub fn format_number(value: i64, group_separator: &str) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(group_separator);
        }
        out.push(c);
    }
    if value < 0 { format!("-{out}") } else { out }
}

/// `StatsFormat.Share`: a decimal for shares under 10% and between 99 and 100%.
pub fn format_share(value: i32, total: i32) -> String {
    if total <= 0 {
        return "0%".to_string();
    }
    let share = value as f64 * 100.0 / total as f64;
    let decimals = usize::from((share > 0.0 && share < 10.0) || (share > 99.0 && share < 100.0));
    format!("{}%", format_custom(share, decimals))
}

/// `StatsFormat.Hour`.
pub fn format_hour(hour: u32) -> String {
    format!("{:02}:00", hour % 24)
}

/// `StatsFormat.HourSpan`.
pub fn format_hour_span(hour: u32) -> String {
    format!("{}–{}", format_hour(hour), format_hour(hour + 1))
}

/// `StatsFormat.Compact`: 1.2k, 3.4M.
pub fn format_compact(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("{}M", format_custom(value / 1_000_000.0, 1))
    } else if value >= 1_000.0 {
        format!("{}k", format_custom(value / 1_000.0, 1))
    } else {
        format_custom(value, 0)
    }
}

/// `ToString("N0")` of a double: the exact value correctly rounded, an
/// exact half to even (.NET's Dragon4), with group separators.
pub fn format_n0(value: f64, group_separator: &str) -> String {
    format_number(value.round_ties_even() as i64, group_separator)
}

/// `StatsWindow.Round`: `N0` from 10, else `0.#`.
pub fn format_rate_number(value: f64) -> String {
    if value >= 10.0 { format_n0(value, ",") } else { format_custom(value, 1) }
}

/// `StatsWindow.GetRate`: per day when at least one a day, else per week
/// (the number for `StatsWindow_AboutADay` / `StatsWindow_AboutAWeek`).
pub fn rate(total: i32, days: i32) -> (bool, String) {
    let per_day = total as f64 / days.max(1) as f64;
    if per_day >= 1.0 { (true, format_rate_number(per_day)) } else { (false, format_rate_number(per_day * 7.0)) }
}

/// `UpdateWeekChange`: `▲ 25%` / `▼ 10%`, or `None` without a previous week.
pub fn week_change(past: i32, previous: i32) -> Option<(bool, String)> {
    if previous == 0 {
        return None;
    }
    let change = (past - previous) as f64 * 100.0 / previous as f64;
    let up = change >= 0.0;
    Some((up, format!("{}{}%", if up { "▲ " } else { "▼ " }, format_custom(change.abs(), 0))))
}

/// `{2:N0}` of a float percentage (`count / (float)total * 100`).
fn n0_percent(count: usize, total: usize) -> String {
    let p = (count as f32 / total as f32) * 100f32;
    format_n0(p as f64, ",")
}

/// `Environment.NewLine`.
pub const NEW_LINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// Groups in order of first appearance (LINQ `GroupBy`), sorted by count
/// descending (stable, like `OrderByDescending`).
fn group_counts<K: Eq + std::hash::Hash + Clone>(keys: impl Iterator<Item = K>) -> Vec<(K, usize)> {
    let mut order: Vec<(K, usize)> = Vec::new();
    let mut index: HashMap<K, usize> = HashMap::new();
    for k in keys {
        match index.get(&k) {
            Some(&i) => order[i].1 += 1,
            None => {
                index.insert(k.clone(), order.len());
                order.push((k, 1));
            }
        }
    }
    order.sort_by_key(|x| std::cmp::Reverse(x.1));
    order
}

/// `HistoryHelpers.OutputStats`: item counts by type and year, file
/// extensions, hosts and process names.
pub fn output_stats(items: &[HistoryItem]) -> String {
    let empty = s::HistoryHelpers_Empty();
    let nl = NEW_LINE;
    let total = items.len();
    let mut sb = String::new();
    let line = |sb: &mut String, text: &str| {
        sb.push_str(text);
        sb.push_str(nl);
    };
    line(&mut sb, s::HistoryHelpers_HistoryItemCounts());
    line(&mut sb, &format!("{} {}", s::HistoryHelpers_Total(), total));
    let types: Vec<String> = group_counts(items.iter().map(|x| x.r#type.clone()))
        .into_iter()
        .map(|(k, c)| format!("{}: {} ({}%)", k.unwrap_or_default(), c, n0_percent(c, total)))
        .collect();
    line(&mut sb, &types.join(nl));
    line(&mut sb, "");
    line(&mut sb, s::HistoryHelpers_YearlyUsages());
    let mut years = group_counts(items.iter().map(|x| x.date_time.year()));
    years.sort_by_key(|x| std::cmp::Reverse(x.0));
    let years: Vec<String> = years.into_iter().map(|(y, c)| format!("{y}: {c} ({}%)", n0_percent(c, total))).collect();
    line(&mut sb, &years.join(nl));
    line(&mut sb, "");
    line(&mut sb, s::HistoryHelpers_FileExtensions());
    let extensions: Vec<String> = group_counts(
        items
            .iter()
            .filter_map(|x| x.file_name.as_deref().filter(|n| !n.is_empty() && !n.ends_with(')')))
            .map(get_file_name_extension)
            .map(|e| if is_null_or_white_space(Some(&e)) { empty.to_string() } else { e }),
    )
    .into_iter()
    .map(|(k, c)| format!("[{c}] {k}"))
    .collect();
    line(&mut sb, &extensions.join(nl));
    line(&mut sb, "");
    line(&mut sb, s::HistoryHelpers_Hosts());
    let hosts: Vec<String> = group_counts(
        items.iter().map(|x| if is_null_or_white_space(x.host.as_deref()) { empty.to_string() } else { x.host.clone().unwrap() }),
    )
    .into_iter()
    .map(|(k, c)| format!("[{c}] {k}"))
    .collect();
    line(&mut sb, &hosts.join(nl));
    line(&mut sb, "");
    line(&mut sb, s::HistoryHelpers_ProcessNames());
    let processes: Vec<String> = group_counts(items.iter().map(|x| match x.tags_process_name() {
        p if is_null_or_white_space(p) => empty.to_string(),
        p => p.unwrap().to_string(),
    }))
    .into_iter()
    .map(|(k, c)| format!("[{c}] {k}"))
    .collect();
    sb.push_str(&processes.join(nl));
    sb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datetime::DateTimeKind;
    use chrono::NaiveDateTime;

    fn at(s: &str) -> DateTime {
        DateTime::new(NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap(), DateTimeKind::Local)
    }

    fn item(name: &str, when: &str) -> HistoryItem {
        HistoryItem { file_name: Some(name.into()), date_time: at(when), r#type: Some("File".into()), ..Default::default() }
    }

    #[test]
    fn stats() {
        let items = [
            item("a.png", "2026-11-17 09:00"),
            item("b.mp4", "2026-11-16 22:00"),
            item("c.gif", "2026-11-15 22:30"),
            item("d.txt", "2026-11-10 08:00"),
            item("e", "2026-11-01 08:00"),
        ];
        let st = HistoryStats::calculate(&items, at("2026-11-17 12:00"), 1);
        assert_eq!((st.total.images, st.total.recordings, st.total.other), (1, 2, 2));
        assert_eq!((st.streak, st.longest_streak, st.past_7_days, st.previous_7_days), (3, 3, 3, 1));
        assert_eq!(st.days_counted(), 17);
        assert_eq!(st.peak_hour(HistoryStatsFilter::All), Some(8));
        assert_eq!(st.file_types.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["GIF", "MP4", "PNG", "TXT"]);
        assert_eq!(st.get_week_start(at("2026-11-17 12:00")).naive().to_string(), "2026-11-16 00:00:00");
    }

    #[test]
    fn formats() {
        assert_eq!(format_share(1, 3), "33%");
        assert_eq!(format_share(1, 40), "2.5%");
        assert_eq!(format_share(999, 1000), "99.9%");
        assert_eq!(format_custom(12.25, 1), "12.3");
        assert_eq!(format_custom(0.05, 1), "0.1");
        assert_eq!(format_custom(2.5, 0), "3");
        assert_eq!(format_compact(1250.0), "1.3k");
        assert_eq!(format_number(1234567, ","), "1,234,567");
        assert_eq!(hour_chart_scale(7), (5.0, 10.0));
        assert_eq!(hour_chart_scale(3), (2.0, 4.0));
        assert_eq!(week_change(5, 4), Some((true, "▲ 25%".into())));
        assert_eq!(format_hour_span(23), "23:00–00:00");
    }
}
