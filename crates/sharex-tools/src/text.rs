//! .NET's composite formatting (`string.Format`) for the format items ShareX's
//! tool strings use, with en-US's number and date conventions (`N0`, `0.##`,
//! `0.00`, custom date patterns).

use chrono::{Datelike, NaiveDateTime, Timelike};

/// A format argument.
#[derive(Debug, Clone)]
pub enum Arg {
    Str(String),
    I64(i64),
    F64(f64),
    Date(NaiveDateTime),
}

impl From<&str> for Arg {
    fn from(s: &str) -> Self {
        Arg::Str(s.to_string())
    }
}

impl From<String> for Arg {
    fn from(s: String) -> Self {
        Arg::Str(s)
    }
}

impl From<i64> for Arg {
    fn from(v: i64) -> Self {
        Arg::I64(v)
    }
}

impl From<i32> for Arg {
    fn from(v: i32) -> Self {
        Arg::I64(v as i64)
    }
}

/// `string.Format(format, args)`.
pub fn format_dotnet(format: &str, args: &[Arg]) -> String {
    let mut out = String::new();
    let chars: Vec<char> = format.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' {
            if chars.get(i + 1) == Some(&'{') {
                out.push('{');
                i += 2;
                continue;
            }
            let end = chars[i..].iter().position(|&c| c == '}').map(|p| p + i).unwrap_or(chars.len());
            let item: String = chars[i + 1..end].iter().collect();
            let (index, spec) = match item.split_once(':') {
                Some((a, b)) => (a, Some(b)),
                None => (item.as_str(), None),
            };
            let (index, align) = match index.split_once(',') {
                Some((a, b)) => (a, b.trim().parse::<i32>().ok()),
                None => (index, None),
            };
            let s = index.trim().parse::<usize>().ok().and_then(|n| args.get(n)).map(|a| format_arg(a, spec)).unwrap_or_default();
            match align {
                Some(w) if w > 0 => out.push_str(&format!("{s:>width$}", width = w as usize)),
                Some(w) if w < 0 => out.push_str(&format!("{s:<width$}", width = (-w) as usize)),
                _ => out.push_str(&s),
            }
            i = end + 1;
        } else if c == '}' && chars.get(i + 1) == Some(&'}') {
            out.push('}');
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn format_arg(a: &Arg, spec: Option<&str>) -> String {
    match a {
        Arg::Str(s) => s.clone(),
        Arg::I64(v) => match spec {
            None => v.to_string(),
            Some(s) => format_number(*v as f64, s, true),
        },
        Arg::F64(v) => match spec {
            None => format_double_g(*v),
            Some(s) => format_number(*v, s, false),
        },
        Arg::Date(d) => format_date(d, spec.unwrap_or("G")),
    }
}

/// `double.ToString()`: the shortest round-trip form (`R`), as .NET Core 3+.
pub fn format_double_g(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞".into() } else { "-∞".into() };
    }
    let s = format!("{v}");
    // Rust prints 1e21 as "1000000000000000000000"; .NET switches to E+ notation
    // past 15 digits of exponent. Values the tools format stay well inside.
    s
}

/// Groups an integer digit string with commas.
fn group(int_digits: &str) -> String {
    let mut out = String::new();
    let n = int_digits.len();
    for (i, c) in int_digits.chars().enumerate() {
        if i > 0 && (n - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Rounds `v` half away from zero to `places` decimals, from its 15 significant
/// digits (.NET's custom/fixed formats of a double).
fn round_digits(v: f64, places: usize) -> (bool, String, String) {
    let neg = v < 0.0;
    let a = v.abs();
    let s = format!("{:.*e}", 14, a);
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    // Digits with the decimal point after (exp + 1) of them.
    let point = exp + 1;
    let mut all: Vec<u8> = digits.bytes().map(|b| b - b'0').collect();
    let mut point = point;
    if point < 0 {
        let mut pad = vec![0u8; (-point) as usize];
        pad.extend(all);
        all = pad;
        point = 0;
    }
    let keep = point as usize + places;
    while all.len() < keep + 1 {
        all.push(0);
    }
    let round_up = all[keep] >= 5;
    all.truncate(keep);
    if round_up {
        let mut i = keep;
        loop {
            if i == 0 {
                all.insert(0, 1);
                point += 1;
                break;
            }
            i -= 1;
            if all[i] == 9 {
                all[i] = 0;
            } else {
                all[i] += 1;
                break;
            }
        }
    }
    let p = point as usize;
    let int: String = all[..p].iter().map(|d| (d + b'0') as char).collect();
    let frac: String = all[p..].iter().map(|d| (d + b'0') as char).collect();
    let int = int.trim_start_matches('0').to_string();
    (neg, if int.is_empty() { "0".into() } else { int }, frac)
}

/// The numeric format strings the tools use: `N<n>`/`n<n>`, `F<n>`, and custom
/// `0`, `0.0`, `0.00`, `0.##`, `0.######`, `#,0`.
pub fn format_number(v: f64, spec: &str, _integer: bool) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞".into() } else { "-∞".into() };
    }
    let upper = spec.to_ascii_uppercase();
    if let Some(rest) = upper.strip_prefix('N') {
        let places: usize = rest.parse().unwrap_or(2);
        let (neg, int, frac) = round_digits(v, places);
        let mut s = group(&int);
        if places > 0 {
            s.push('.');
            s.push_str(&frac);
        }
        return if neg && s.chars().any(|c| c.is_ascii_digit() && c != '0') { format!("-{s}") } else { s };
    }
    if let Some(rest) = upper.strip_prefix('F') {
        let places: usize = rest.parse().unwrap_or(2);
        let (neg, int, frac) = round_digits(v, places);
        let mut s = int;
        if places > 0 {
            s.push('.');
            s.push_str(&frac);
        }
        return if neg && s.chars().any(|c| c.is_ascii_digit() && c != '0') { format!("-{s}") } else { s };
    }
    // Custom: zeros are required digits, '#' optional ones.
    let grouped = spec.contains(',');
    let (int_spec, frac_spec) = spec.split_once('.').unwrap_or((spec, ""));
    let min_int = int_spec.chars().filter(|&c| c == '0').count();
    let required = frac_spec.chars().filter(|&c| c == '0').count();
    let optional = frac_spec.chars().filter(|&c| c == '#').count();
    let (neg, int, frac) = round_digits(v, required + optional);
    let mut frac = frac;
    while frac.len() > required && frac.ends_with('0') {
        frac.pop();
    }
    let int = if int.len() < min_int { format!("{}{int}", "0".repeat(min_int - int.len())) } else { int };
    let int = if min_int == 0 && int == "0" { String::new() } else { int };
    let mut s = if grouped { group(&int) } else { int };
    if !frac.is_empty() {
        s.push('.');
        s.push_str(&frac);
    }
    if neg && s.chars().any(|c| c.is_ascii_digit() && c != '0') { format!("-{s}") } else { s }
}

/// `DateTime.ToString(format)` for custom patterns (en-US), and `G`.
pub fn format_date(d: &NaiveDateTime, spec: &str) -> String {
    let spec = match spec {
        "G" => "M/d/yyyy h:mm:ss tt",
        "g" => "M/d/yyyy h:mm tt",
        "d" => "M/d/yyyy",
        "s" => "yyyy'-'MM'-'dd'T'HH':'mm':'ss",
        other => other,
    };
    let chars: Vec<char> = spec.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        match c {
            '\'' | '"' => {
                let end = chars[i + 1..].iter().position(|&x| x == c).map(|p| p + i + 1).unwrap_or(chars.len());
                out.extend(&chars[i + 1..end]);
                i = end + 1;
                continue;
            }
            '\\' => {
                if let Some(&x) = chars.get(i + 1) {
                    out.push(x);
                }
                i += 2;
                continue;
            }
            'y' => out.push_str(&match n {
                1 => (d.year() % 100).to_string(),
                2 => format!("{:02}", d.year() % 100),
                _ => format!("{:0width$}", d.year(), width = n),
            }),
            'M' => out.push_str(&match n {
                1 => d.month().to_string(),
                2 => format!("{:02}", d.month()),
                3 => MONTHS[d.month0() as usize][..3].to_string(),
                _ => MONTHS[d.month0() as usize].to_string(),
            }),
            'd' => out.push_str(&match n {
                1 => d.day().to_string(),
                2 => format!("{:02}", d.day()),
                3 => DAYS[d.weekday().num_days_from_sunday() as usize][..3].to_string(),
                _ => DAYS[d.weekday().num_days_from_sunday() as usize].to_string(),
            }),
            'H' => out.push_str(&if n == 1 { d.hour().to_string() } else { format!("{:02}", d.hour()) }),
            'h' => {
                let h = match d.hour() % 12 {
                    0 => 12,
                    h => h,
                };
                out.push_str(&if n == 1 { h.to_string() } else { format!("{h:02}") })
            }
            'm' => out.push_str(&if n == 1 { d.minute().to_string() } else { format!("{:02}", d.minute()) }),
            's' => out.push_str(&if n == 1 { d.second().to_string() } else { format!("{:02}", d.second()) }),
            'f' | 'F' => {
                let frac = format!("{:07}", d.nanosecond() / 100);
                let mut part = frac[..n.min(7)].to_string();
                if c == 'F' {
                    while part.ends_with('0') {
                        part.pop();
                    }
                }
                out.push_str(&part);
            }
            't' => {
                let ampm = if d.hour() < 12 { "AM" } else { "PM" };
                out.push_str(if n == 1 { &ampm[..1] } else { ampm });
            }
            _ => {
                for _ in 0..n {
                    out.push(c);
                }
            }
        }
        i += n;
    }
    out
}

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
];
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(format_number(1234567.0, "N0", true), "1,234,567");
        assert_eq!(format_number(99.995, "0.##", false), "100");
        assert_eq!(format_number(12.5, "0.##", false), "12.5");
        assert_eq!(format_number(0.125, "0.00", false), "0.13");
        assert_eq!(format_number(3.0, "0.######", false), "3");
        assert_eq!(format_number(1.5, "0.######", false), "1.5");
        assert_eq!(format_number(7.0, "00", false), "07");
        assert_eq!(format_number(5.25, "00.###", false), "05.25");
    }

    #[test]
    fn composite() {
        assert_eq!(
            format_dotnet("{0:N0} of {1:N0} pixels match.", &[Arg::I64(1000), Arg::I64(20000)]),
            "1,000 of 20,000 pixels match."
        );
        let d = chrono::NaiveDate::from_ymd_opt(2026, 11, 7).unwrap().and_hms_opt(5, 4, 3).unwrap();
        assert_eq!(format_dotnet("{0:yyyy-MM-dd 'at' HH:mm:ss 'UTC'}", &[Arg::Date(d)]), "2026-11-07 at 05:04:03 UTC");
        assert_eq!(format_date(&d, "G"), "11/7/2026 5:04:03 AM");
    }
}
