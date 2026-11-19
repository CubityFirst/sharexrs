//! `ShareX.HelpersLib/NameParser/NameParser.cs`: name patterns (`%ra{10}`,
//! `%y-%mo`, `%pn_%ra{10}`, ...) into names, code for code in ShareX's
//! order (the order matters: `%y` is replaced before `%mo`, `%rna` before
//! `%rn`, ...).
//!
//! Differences: month and day names (`%mon`, `%w`) are in English for the
//! current culture too, and `%uln` is the user's domain from the
//! environment.

use chrono::{Datelike, Local, Timelike};

use crate::file_helpers::{sanitize_file_name, sanitize_path};
use crate::random::{self, ALPHANUMERIC, ALPHANUMERIC_INVERSE, BASE56, HEXADECIMAL, NUMBERS};

/// `NameParserType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NameParserType {
    #[default]
    Default,
    /// Allows new lines (`%n`).
    Text,
    FileName,
    FilePath,
    /// URL path encodes.
    Url,
}

/// `NameParser`, with its properties as fields.
#[derive(Clone, Debug, Default)]
pub struct NameParser {
    pub kind: NameParserType,
    pub max_name_length: usize,
    pub max_title_length: usize,
    /// `%i`, `%ia`, `%ib`, `%iAa`, `%ix`: incremented once per parse that
    /// uses one of them.
    pub auto_increment_number: i64,
    /// `%width`
    pub image_width: u32,
    /// `%height`
    pub image_height: u32,
    /// `%t`
    pub window_text: Option<String>,
    /// `%pn`
    pub process_name: Option<String>,
    /// `%rf` reads from the local file system and must only be enabled for
    /// trusted patterns.
    pub allow_file_read: bool,
    pub is_preview_mode: bool,
    /// `CustomTimeZone`: the time is `TimeZoneInfo.ConvertTime(DateTime.Now, zone)`.
    pub custom_time_zone: Option<crate::time_zone::TimeZoneInfo>,
}

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November",
    "December",
];
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

const TEXT_FILE_EXTENSIONS: &[&str] = &[
    "txt", "log", "nfo", "c", "cpp", "cc", "cxx", "h", "hpp", "hxx", "cs", "vb", "html", "htm", "xhtml", "xht", "xml",
    "css", "js", "php", "bat", "java", "lua", "py", "pl", "cfg", "ini", "dart", "go", "gohtml",
];

fn p(code: &str) -> String {
    format!("%{code}")
}

/// `Helpers.AddZeroes`.
fn zeroes(n: impl ToString, digits: usize) -> String {
    format!("{:0>digits$}", n.to_string())
}

/// `int.ToBase(radix, digits)`: no digits at all for 0, as ShareX.
fn to_base(value: i64, radix: usize, digits: &str) -> String {
    let digits: Vec<char> = digits.chars().collect();
    let radix = radix.clamp(2, digits.len()) as i64;
    let mut q = value.abs();
    let mut out = Vec::new();
    while q > 0 {
        out.push(digits[(q % radix) as usize]);
        q /= radix;
    }
    out.iter().rev().collect()
}

/// `string.ForEachBetween(front, back)`: each `front...back` and what's
/// between.
fn between(text: &str, front: &str, back: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut f = 0;
    while f < text.len() {
        let Some(start) = text[f..].find(front).map(|i| i + f) else { break };
        let Some(end) = text[start + front.len()..].find(back).map(|i| i + start + front.len()) else { break };
        let whole = &text[start..end + back.len()];
        out.push((whole.to_string(), text[start + front.len()..end].to_string()));
        f = start + front.len();
    }
    out
}

/// `ListEntryWithValues`: each `%code{a,b}` with its numbers (0 where not
/// a number), padded to `elements`.
fn values(text: &str, code: &str, elements: usize) -> Vec<(String, Vec<i64>)> {
    between(text, &format!("{code}{{"), "}")
        .into_iter()
        .map(|(whole, inside)| {
            let mut v: Vec<i64> = inside.split(',').map(|s| s.parse().unwrap_or(0)).collect();
            if v.len() < elements {
                v.resize(elements, 0);
            }
            (whole, v)
        })
        .collect()
}

fn value(text: &str, code: &str) -> Vec<(String, i64)> {
    values(text, code, 1).into_iter().map(|(w, v)| (w, v[0])).collect()
}

/// `string.ReplaceAll(search, replace)`: each occurrence, from the start,
/// with a new value.
fn replace_all(mut text: String, search: &str, mut replace: impl FnMut() -> String) -> String {
    if search.is_empty() {
        return text;
    }
    while let Some(at) = text.find(search) {
        text.replace_range(at..at + search.len(), &replace());
    }
    text
}

fn repeat(n: i64, mut f: impl FnMut() -> String) -> String {
    (0..n.max(0)).map(|_| f()).collect()
}

/// `Helpers.GetRandomLine`: a random line of `text` (trimmed first).
fn random_line(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.trim().split("\r\n").flat_map(|l| l.split('\n')).collect();
    random::pick(&lines).map(|s| s.to_string())
}

/// `TextInfo.ToTitleCase` (invariant culture), for the word lists.
fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) if w.chars().any(|c| c.is_lowercase()) => f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect(),
                Some(_) => w.to_string(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `Calendar.GetWeekOfYear(dt, CalendarWeekRule.FirstDay, DayOfWeek.Monday)`.
fn week_of_year(date: chrono::NaiveDate) -> u32 {
    let jan1 = chrono::NaiveDate::from_ymd_opt(date.year(), 1, 1).expect("1 January");
    let offset = jan1.weekday().num_days_from_monday();
    (date.ordinal0() + offset) / 7 + 1
}

/// `Environment.UserName`, `UserDomainName`, `MachineName`.
fn user_name() -> String {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default()
}

fn user_domain_name() -> String {
    std::env::var("USERDOMAIN").unwrap_or_else(|_| machine_name())
}

fn machine_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()))
        .unwrap_or_default()
}

fn guid() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("the OS random number generator");
    b[6] = (b[6] & 0x0F) | 0x40;
    b[8] = (b[8] & 0x3F) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// `Helpers.GetValidURL`: `HttpUtility.UrlPathEncode`.
fn url_path_encode(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c == ' ' {
            out.push_str("%20");
        } else if c.is_ascii() && !c.is_ascii_control() {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02x}"));
            }
        }
    }
    out
}

impl NameParser {
    pub fn new(kind: NameParserType) -> Self {
        Self { kind, allow_file_read: true, ..Default::default() }
    }

    fn sanitize_input(&self, input: &str) -> String {
        let input = input.trim().replace(' ', "_");
        match self.kind {
            NameParserType::FileName | NameParserType::FilePath => sanitize_file_name(&input),
            _ => input,
        }
    }

    /// `NameParser.Parse`.
    pub fn parse(&mut self, pattern: &str) -> String {
        if pattern.is_empty() {
            return String::new();
        }
        let mut sb = pattern.to_string();
        if let Some(t) = &self.window_text {
            let mut text = self.sanitize_input(t);
            if self.max_title_length > 0 {
                text = text.chars().take(self.max_title_length).collect();
            }
            sb = sb.replace(&p("t"), &text);
        }
        if let Some(pn) = &self.process_name {
            sb = sb.replace(&p("pn"), &self.sanitize_input(pn));
        }
        let width = if self.image_width > 0 { self.image_width.to_string() } else { String::new() };
        let height = if self.image_height > 0 { self.image_height.to_string() } else { String::new() };
        sb = sb.replace(&p("width"), &width).replace(&p("height"), &height);

        let dt = match &self.custom_time_zone {
            Some(zone) => {
                let now = chrono::Utc::now();
                let ticks = crate::time_zone::UNIX_EPOCH_TICKS + now.timestamp_micros() * 10 + (now.timestamp_subsec_nanos() as i64 % 1000) / 100;
                crate::time_zone::to_naive(zone.convert_from_utc(ticks))
            }
            None => Local::now().naive_local(),
        };
        let month = MONTHS[dt.month0() as usize];
        sb = sb
            .replace(&p("mon2"), month)
            .replace(&p("mon"), month)
            .replace(&p("yy"), &zeroes(dt.year().rem_euclid(100), 2))
            .replace(&p("y"), &dt.year().to_string())
            .replace(&p("mo"), &zeroes(dt.month(), 2))
            .replace(&p("d"), &zeroes(dt.day(), 2));
        let hour = if sb.contains(&p("pm")) {
            // `Helpers.HourTo12`
            let h = dt.hour() % 12;
            zeroes(if h == 0 { 12 } else { h }, 2)
        } else {
            zeroes(dt.hour(), 2)
        };
        let day = DAYS[dt.weekday().num_days_from_sunday() as usize];
        sb = sb
            .replace(&p("h"), &hour)
            .replace(&p("mi"), &zeroes(dt.minute(), 2))
            .replace(&p("s"), &zeroes(dt.second(), 2))
            .replace(&p("ms"), &zeroes(dt.and_utc().timestamp_subsec_millis(), 3))
            .replace(&p("wy"), &week_of_year(dt.date()).to_string())
            .replace(&p("w2"), day)
            .replace(&p("w"), day)
            .replace(&p("pm"), if dt.hour() >= 12 { "PM" } else { "AM" });
        sb = sb.replace(&p("unix"), &chrono::Utc::now().timestamp().to_string());

        let increments = ["i", "ib", "iB", "iAa", "iaA", "ia", "iA", "ix", "iX"];
        if increments.iter().any(|c| sb.contains(&p(c))) {
            self.auto_increment_number += 1;
            let n = self.auto_increment_number;
            for (whole, v) in values(&sb, &p("ib"), 2) {
                sb = sb.replace(&whole, &zeroes(to_base(n, v[0] as usize, ALPHANUMERIC_INVERSE), v[1] as usize));
            }
            for (whole, v) in values(&sb, &p("iB"), 2) {
                sb = sb.replace(&whole, &zeroes(to_base(n, v[0] as usize, ALPHANUMERIC), v[1] as usize));
            }
            for (whole, d) in value(&sb, &p("iAa")) {
                sb = sb.replace(&whole, &zeroes(to_base(n, 62, ALPHANUMERIC), d as usize));
            }
            sb = sb.replace(&p("iAa"), &to_base(n, 62, ALPHANUMERIC));
            for (whole, d) in value(&sb, &p("iaA")) {
                sb = sb.replace(&whole, &zeroes(to_base(n, 62, ALPHANUMERIC_INVERSE), d as usize));
            }
            sb = sb.replace(&p("iaA"), &to_base(n, 62, ALPHANUMERIC_INVERSE));
            for (whole, d) in value(&sb, &p("ia")) {
                sb = sb.replace(&whole, &zeroes(to_base(n, 36, ALPHANUMERIC), d as usize).to_lowercase());
            }
            sb = sb.replace(&p("ia"), &to_base(n, 36, ALPHANUMERIC).to_lowercase());
            for (whole, d) in value(&sb, &p("iA")) {
                sb = sb.replace(&whole, &zeroes(to_base(n, 36, ALPHANUMERIC), d as usize).to_uppercase());
            }
            sb = sb.replace(&p("iA"), &to_base(n, 36, ALPHANUMERIC).to_uppercase());
            for (whole, d) in value(&sb, &p("ix")) {
                sb = sb.replace(&whole, &zeroes(format!("{n:x}"), d as usize));
            }
            sb = sb.replace(&p("ix"), &format!("{n:x}"));
            for (whole, d) in value(&sb, &p("iX")) {
                sb = sb.replace(&whole, &zeroes(format!("{n:X}"), d as usize));
            }
            sb = sb.replace(&p("iX"), &format!("{n:X}"));
            for (whole, d) in value(&sb, &p("i")) {
                sb = sb.replace(&whole, &zeroes(n, d as usize));
            }
            sb = sb.replace(&p("i"), &n.to_string());
        }

        sb = sb.replace(&p("un"), &user_name());
        sb = sb.replace(&p("uln"), &user_domain_name());
        sb = sb.replace(&p("cn"), &machine_name());
        if self.kind == NameParserType::Text {
            sb = sb.replace(&p("n"), if cfg!(windows) { "\r\n" } else { "\n" });
        }

        let mut result = sb;
        let adjectives = include_str!("../resources/adjectives.txt");
        let animals = include_str!("../resources/animals.txt");
        result = replace_all(result, &p("radjective"), || title_case(&random_line(adjectives).unwrap_or_default()));
        result = replace_all(result, &p("ranimal"), || title_case(&random_line(animals).unwrap_or_default()));

        for (whole, path) in between(&result, &format!("{}{{", p("rf")), "}") {
            if !self.allow_file_read {
                result = result.replacen(&whole, "", 1);
                continue;
            }
            let preview = self.is_preview_mode;
            result = replace_all(result, &whole, || {
                let ext = std::path::Path::new(&path)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let text = TEXT_FILE_EXTENSIONS
                    .contains(&ext.as_str())
                    .then(|| std::fs::read_to_string(&path).ok())
                    .flatten();
                match text.and_then(|t| random_line(&t)) {
                    Some(line) => line,
                    None if preview => "A valid text file is required.".to_string(),
                    None => String::new(),
                }
            });
        }

        for (whole, n) in value(&result, &p("rna")) {
            result = replace_all(result, &whole, || repeat(n, || random::char_from(BASE56).to_string()));
        }
        for (whole, n) in value(&result, &p("rn")) {
            result = replace_all(result, &whole, || repeat(n, || random::char_from(NUMBERS).to_string()));
        }
        for (whole, n) in value(&result, &p("ra")) {
            result = replace_all(result, &whole, || repeat(n, || random::char_from(ALPHANUMERIC).to_string()));
        }
        let hex_lower = HEXADECIMAL.to_lowercase();
        for (whole, n) in value(&result, &p("rx")) {
            result = replace_all(result, &whole, || repeat(n, || random::char_from(&hex_lower).to_string()));
        }
        for (whole, n) in value(&result, &p("rX")) {
            result = replace_all(result, &whole, || repeat(n, || random::char_from(HEXADECIMAL).to_string()));
        }
        for (whole, n) in value(&result, &p("remoji")) {
            result = replace_all(result, &whole, || {
                repeat(n, || random::pick(crate::emoji::EMOJIS).copied().unwrap_or_default().to_string())
            });
        }
        result = replace_all(result, &p("rna"), || random::char_from(BASE56).to_string());
        result = replace_all(result, &p("rn"), || random::char_from(NUMBERS).to_string());
        result = replace_all(result, &p("ra"), || random::char_from(ALPHANUMERIC).to_string());
        result = replace_all(result, &p("rx"), || random::char_from(&hex_lower).to_string());
        result = replace_all(result, &p("rX"), || random::char_from(HEXADECIMAL).to_string());
        result = replace_all(result, "%guid", guid);
        result = replace_all(result, "%GUID", || guid().to_uppercase());
        result = replace_all(result, &p("remoji"), || {
            random::pick(crate::emoji::EMOJIS).copied().unwrap_or_default().to_string()
        });

        result = match self.kind {
            NameParserType::FileName => sanitize_file_name(&result),
            NameParserType::FilePath => sanitize_path(&result),
            NameParserType::Url => url_path_encode(&result),
            _ => result,
        };
        if self.max_name_length > 0 && result.chars().count() > self.max_name_length {
            result = result.chars().take(self.max_name_length).collect();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_file_name_is_ten_random_alphanumerics() {
        let mut parser = NameParser::new(NameParserType::FileName);
        let name = parser.parse("%ra{10}");
        assert_eq!(name.len(), 10);
        assert!(name.chars().all(|c| ALPHANUMERIC.contains(c)));
        assert_ne!(name, parser.parse("%ra{10}"));
    }

    #[test]
    fn active_window_pattern_uses_the_process_name() {
        let mut parser = NameParser::new(NameParserType::FileName);
        parser.process_name = Some("my app".into());
        let name = parser.parse("%pn_%ra{10}");
        assert!(name.starts_with("my_app_"), "{name}");
        assert_eq!(name.len(), "my_app_".len() + 10);
    }

    #[test]
    fn sub_folder_pattern_is_year_and_month() {
        let now = Local::now();
        let folder = NameParser::new(NameParserType::FilePath).parse("%y-%mo");
        assert_eq!(folder, format!("{}-{:02}", now.year(), now.month()));
    }

    /// `CustomTimeZone`: the time in that zone (Nepal: UTC+05:45, no rules).
    #[test]
    fn custom_time_zone_converts_now() {
        use crate::time_zone::{TICKS_PER_MINUTE, TimeZoneInfo};
        let zone = TimeZoneInfo { id: "Nepal Standard Time".into(), base_utc_offset: 345 * TICKS_PER_MINUTE, ..TimeZoneInfo::utc() };
        let mut p = NameParser::new(NameParserType::Text);
        p.custom_time_zone = Some(zone);
        let expect = |t: chrono::DateTime<chrono::Utc>| (t + chrono::Duration::minutes(345)).format("%Y-%m-%d %H:%M").to_string();
        let before = expect(chrono::Utc::now());
        let got = p.parse("%y-%mo-%d %h:%mi");
        let after = expect(chrono::Utc::now());
        assert!(got == before || got == after, "{got} vs {before}");
    }

    #[test]
    fn increments_and_bases() {
        let mut parser = NameParser::new(NameParserType::Default);
        assert_eq!(parser.parse("%i{3}"), "001");
        assert_eq!(parser.parse("%ix"), "2");
        parser.auto_increment_number = 34;
        assert_eq!(parser.parse("%ia"), "z");
        assert_eq!(to_base(0, 10, NUMBERS), "");
    }

    #[test]
    fn weeks_start_on_monday_from_the_first_day() {
        let d = |y, m, day| chrono::NaiveDate::from_ymd_opt(y, m, day).unwrap();
        // 1 January 2026 is a Thursday: the 5th (Monday) starts week 2.
        assert_eq!(week_of_year(d(2026, 1, 1)), 1);
        assert_eq!(week_of_year(d(2026, 1, 4)), 1);
        assert_eq!(week_of_year(d(2026, 1, 5)), 2);
    }

    #[test]
    fn names_are_truncated_and_titles_too() {
        let mut parser = NameParser::new(NameParserType::FileName);
        parser.max_name_length = 5;
        assert_eq!(parser.parse("abcdefgh"), "abcde");
        let mut parser = NameParser::new(NameParserType::FileName);
        parser.window_text = Some("A long: title".into());
        parser.max_title_length = 6;
        assert_eq!(parser.parse("%t"), "A_long");
    }
}
