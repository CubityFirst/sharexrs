//! .NET's culture date formats as ShareX's windows use them
//! (`DateTimeFormatInfo` of `CultureInfo.CurrentCulture`, and
//! `DateTime.ToString(format)`). ShareX runs with `UseNls`, so on Windows the
//! culture's patterns and names are NLS's (`GetLocaleInfoEx` for the user's
//! locale, user overrides included, as `CultureInfo` reads them); elsewhere
//! (ShareX doesn't run there) en-US's.

use std::sync::OnceLock;

use chrono::{Datelike, NaiveDateTime, Timelike};

/// What `DateTimeFormatInfo` and `NumberFormatInfo` give the windows.
#[derive(Clone, Debug, PartialEq)]
pub struct CultureFormat {
    pub short_date_pattern: String,
    pub long_date_pattern: String,
    pub short_time_pattern: String,
    pub long_time_pattern: String,
    /// January first.
    pub month_names: [String; 12],
    pub abbreviated_month_names: [String; 12],
    /// Sunday first (`DayOfWeek` order).
    pub day_names: [String; 7],
    pub abbreviated_day_names: [String; 7],
    pub am_designator: String,
    pub pm_designator: String,
    /// `FirstDayOfWeek`: Sunday is 0.
    pub first_day_of_week: u32,
    pub number_group_separator: String,
    pub number_decimal_separator: String,
}

impl CultureFormat {
    /// en-US (the invariant culture's names, en-US's patterns).
    pub fn en_us() -> Self {
        let s = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let months = s(&["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"]);
        let abbr_months = s(&["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]);
        let days = s(&["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"]);
        let abbr_days = s(&["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]);
        Self {
            short_date_pattern: "M/d/yyyy".into(),
            long_date_pattern: "dddd, MMMM d, yyyy".into(),
            short_time_pattern: "h:mm tt".into(),
            long_time_pattern: "h:mm:ss tt".into(),
            month_names: months.try_into().unwrap(),
            abbreviated_month_names: abbr_months.try_into().unwrap(),
            day_names: days.try_into().unwrap(),
            abbreviated_day_names: abbr_days.try_into().unwrap(),
            am_designator: "AM".into(),
            pm_designator: "PM".into(),
            first_day_of_week: 0,
            number_group_separator: ",".into(),
            number_decimal_separator: ".".into(),
        }
    }

    /// en-GB as Windows 11's NLS has it (for tests that don't depend on the
    /// machine's culture).
    pub fn en_gb() -> Self {
        Self {
            short_date_pattern: "dd/MM/yyyy".into(),
            long_date_pattern: "dd MMMM yyyy".into(),
            short_time_pattern: "HH:mm".into(),
            long_time_pattern: "HH:mm:ss".into(),
            am_designator: "am".into(),
            pm_designator: "pm".into(),
            first_day_of_week: 1,
            ..Self::en_us()
        }
    }

    /// `CultureInfo.CurrentCulture`'s.
    pub fn current() -> &'static CultureFormat {
        static CURRENT: OnceLock<CultureFormat> = OnceLock::new();
        CURRENT.get_or_init(|| platform::current().unwrap_or_else(Self::en_us))
    }

    /// The `"g"` standard format: short date and short time.
    pub fn general_short_pattern(&self) -> String {
        format!("{} {}", self.short_date_pattern, self.short_time_pattern)
    }

    /// The `"G"` standard format: short date and long time.
    pub fn general_long_pattern(&self) -> String {
        format!("{} {}", self.short_date_pattern, self.long_time_pattern)
    }

    /// `DateTime.ToString("g")`.
    pub fn format_g(&self, t: NaiveDateTime) -> String {
        format_custom(t, &self.general_short_pattern(), self)
    }

    /// `DateTime.ToString("G")`.
    pub fn format_long_g(&self, t: NaiveDateTime) -> String {
        format_custom(t, &self.general_long_pattern(), self)
    }
}

/// `DateTime.ToString(pattern, culture)` for a custom pattern
/// (`DateTimeFormat.FormatCustomized`): `d`–`dddd`, `M`–`MMMM`, `y`–`yyyyy`,
/// `h`/`hh`, `H`/`HH`, `m`/`mm`, `s`/`ss`, `f`/`F` fractions, `t`/`tt`, `/`
/// and `:` (the culture's separators, which are `/` and `:` in the patterns
/// NLS gives), quoted literals and `\` escapes. `K`, `z` and `g` (time zone,
/// era) aren't in the patterns the windows use and are written as is.
pub fn format_custom(t: NaiveDateTime, pattern: &str, c: &CultureFormat) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let run = |i: usize, ch: char| chars[i..].iter().take_while(|&&x| x == ch).count();
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            'd' => {
                let n = run(i, ch);
                match n {
                    1 => out.push_str(&t.day().to_string()),
                    2 => out.push_str(&format!("{:02}", t.day())),
                    3 => out.push_str(&c.abbreviated_day_names[t.weekday().num_days_from_sunday() as usize]),
                    _ => out.push_str(&c.day_names[t.weekday().num_days_from_sunday() as usize]),
                }
                i += n;
            }
            'M' => {
                let n = run(i, ch);
                let m = t.month() as usize;
                match n {
                    1 => out.push_str(&m.to_string()),
                    2 => out.push_str(&format!("{m:02}")),
                    3 => out.push_str(&c.abbreviated_month_names[m - 1]),
                    _ => out.push_str(&c.month_names[m - 1]),
                }
                i += n;
            }
            'y' => {
                let n = run(i, ch);
                let y = t.year();
                if n <= 2 {
                    let v = y % 100;
                    out.push_str(&if n == 1 { v.to_string() } else { format!("{v:02}") });
                } else {
                    out.push_str(&format!("{y:0width$}", width = n));
                }
                i += n;
            }
            'h' => {
                let n = run(i, ch);
                let h = match t.hour() % 12 {
                    0 => 12,
                    h => h,
                };
                out.push_str(&if n == 1 { h.to_string() } else { format!("{h:02}") });
                i += n;
            }
            'H' | 'm' | 's' => {
                let n = run(i, ch);
                let v = match ch {
                    'H' => t.hour(),
                    'm' => t.minute(),
                    _ => t.second(),
                };
                out.push_str(&if n == 1 { v.to_string() } else { format!("{v:02}") });
                i += n;
            }
            'f' | 'F' => {
                let n = run(i, ch).min(7);
                let frac = format!("{:07}", t.and_utc().timestamp_subsec_nanos() / 100);
                let digits = &frac[..n];
                if ch == 'f' {
                    out.push_str(digits);
                } else {
                    out.push_str(digits.trim_end_matches('0'));
                }
                i += run(i, ch);
            }
            't' => {
                let n = run(i, ch);
                let d = if t.hour() < 12 { &c.am_designator } else { &c.pm_designator };
                if n == 1 {
                    out.extend(d.chars().next());
                } else {
                    out.push_str(d);
                }
                i += n;
            }
            '\'' | '"' => {
                i += 1;
                while i < chars.len() && chars[i] != ch {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                    }
                    out.push(chars[i]);
                    i += 1;
                }
                i += 1;
            }
            '\\' => {
                if i + 1 < chars.len() {
                    out.push(chars[i + 1]);
                }
                i += 2;
            }
            '%' => i += 1,
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    out
}

#[cfg(windows)]
mod platform {
    use super::CultureFormat;
    use windows_sys::Win32::Globalization::{
        GetLocaleInfoEx, LOCALE_IFIRSTDAYOFWEEK, LOCALE_S1159, LOCALE_S2359, LOCALE_SABBREVDAYNAME1, LOCALE_SABBREVMONTHNAME1,
        LOCALE_SDAYNAME1, LOCALE_SDECIMAL, LOCALE_SLONGDATE, LOCALE_SMONTHNAME1, LOCALE_SSHORTDATE, LOCALE_SSHORTTIME, LOCALE_STHOUSAND,
        LOCALE_STIMEFORMAT,
    };

    fn info(lctype: u32) -> Option<String> {
        let mut buf = [0u16; 256];
        // `LOCALE_NAME_USER_DEFAULT` is a null pointer.
        let n = unsafe { GetLocaleInfoEx(std::ptr::null(), lctype, buf.as_mut_ptr(), buf.len() as i32) };
        (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
    }

    pub fn current() -> Option<CultureFormat> {
        // NLS numbers days from Monday (SDAYNAME1); .NET's arrays start on Sunday.
        let day = |base: u32, dow: u32| info(base + (dow + 6) % 7);
        let days: Option<Vec<String>> = (0..7).map(|d| day(LOCALE_SDAYNAME1, d)).collect();
        let abbr_days: Option<Vec<String>> = (0..7).map(|d| day(LOCALE_SABBREVDAYNAME1, d)).collect();
        let months: Option<Vec<String>> = (0..12).map(|m| info(LOCALE_SMONTHNAME1 + m)).collect();
        let abbr_months: Option<Vec<String>> = (0..12).map(|m| info(LOCALE_SABBREVMONTHNAME1 + m)).collect();
        let first = info(LOCALE_IFIRSTDAYOFWEEK)?.parse::<u32>().ok()?;
        Some(CultureFormat {
            short_date_pattern: info(LOCALE_SSHORTDATE)?,
            long_date_pattern: info(LOCALE_SLONGDATE)?,
            short_time_pattern: info(LOCALE_SSHORTTIME)?,
            long_time_pattern: info(LOCALE_STIMEFORMAT)?,
            month_names: months?.try_into().ok()?,
            abbreviated_month_names: abbr_months?.try_into().ok()?,
            day_names: days?.try_into().ok()?,
            abbreviated_day_names: abbr_days?.try_into().ok()?,
            am_designator: info(LOCALE_S1159)?,
            pm_designator: info(LOCALE_S2359)?,
            first_day_of_week: (first + 1) % 7,
            number_group_separator: info(LOCALE_STHOUSAND)?,
            number_decimal_separator: info(LOCALE_SDECIMAL)?,
        })
    }
}

#[cfg(not(windows))]
mod platform {
    pub fn current() -> Option<super::CultureFormat> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, s).unwrap()
    }

    #[test]
    fn standard_formats() {
        let gb = CultureFormat::en_gb();
        let us = CultureFormat::en_us();
        let t = at(2026, 11, 7, 9, 5, 3);
        assert_eq!(gb.format_g(t), "07/11/2026 09:05");
        assert_eq!(gb.format_long_g(t), "07/11/2026 09:05:03");
        assert_eq!(us.format_g(t), "11/7/2026 9:05 AM");
        assert_eq!(format_custom(at(2026, 11, 17, 21, 0, 0), "ddd dd MMM yyyy", &gb), "Tue 17 Nov 2026");
        assert_eq!(format_custom(t, "dddd, MMMM d, yyyy 'at' h tt", &us), "Saturday, November 7, 2026 at 9 AM");
    }
}
