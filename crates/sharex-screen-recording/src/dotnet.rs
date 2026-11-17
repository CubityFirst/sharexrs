//! The bits of .NET the recorder's strings depend on, ported so ShareXrs
//! builds the same command lines: `Path.ChangeExtension`,
//! `float.ToString("0.0", InvariantCulture)`, `string.Replace(..,
//! OrdinalIgnoreCase)`, `TimeSpan.TryParse`, `Helpers.EscapeCLIText`,
//! `FileHelpers.AppendTextToFileName` and how .NET splits
//! `ProcessStartInfo.Arguments` into argv off Windows.

/// `Helpers.EscapeCLIText`: backslashes doubled, quotes escaped, all in
/// quotes.
pub fn escape_cli_text(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `Path.ChangeExtension(path, extension)`: the part after the last `.` of
/// the file name replaced (or a `.extension` added). `None` stays `None`,
/// as .NET returns null for a null path.
pub fn change_extension(path: Option<&str>, extension: &str) -> Option<String> {
    let path = path?;
    if path.is_empty() {
        return Some(String::new());
    }
    let mut sub = path.len();
    for (i, c) in path.char_indices().rev() {
        if c == '.' {
            sub = i;
            break;
        }
        if is_directory_separator(c) || is_volume_separator(c) {
            break;
        }
    }
    let base = &path[..sub];
    Some(if extension.starts_with('.') { format!("{base}{extension}") } else { format!("{base}.{extension}") })
}

#[cfg(windows)]
fn is_directory_separator(c: char) -> bool {
    c == '\\' || c == '/'
}

#[cfg(not(windows))]
fn is_directory_separator(c: char) -> bool {
    c == '/'
}

#[cfg(windows)]
fn is_volume_separator(c: char) -> bool {
    c == ':'
}

#[cfg(not(windows))]
fn is_volume_separator(c: char) -> bool {
    c == '/'
}

/// `FileHelpers.AppendTextToFileName`: `text` inserted before the last `.`
/// of the whole path (the folder's, when the file has none), or appended.
pub fn append_text_to_file_name(path: &str, text: &str) -> String {
    match path.rfind('.') {
        Some(pos) if !path.is_empty() => format!("{}{text}{}", &path[..pos], &path[pos..]),
        _ => format!("{path}{text}"),
    }
}

/// `value.ToString("0.0", CultureInfo.InvariantCulture)` for a `float`.
///
/// .NET formats a `float` for a custom format from its 7 most significant
/// digits (`SinglePrecisionCustomFormat`), then rounds that digit string
/// half away from zero to one decimal: 2.25f is "2.3" (Rust's `{:.1}`
/// says "2.2"), 0.05f is "0.1".
pub fn format_float_0_0(value: f32) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let negative = value.is_sign_negative() && value != 0.0;
    // 7 significant digits, d.dddddde±x.
    let sci = format!("{:.6e}", f64::from(value.abs()));
    let (mantissa, exp) = sci.split_once('e').expect("scientific");
    let exp: i32 = exp.parse().expect("exponent");
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    // The decimal point sits after digit `exp + 1`; keep one decimal.
    let mut point = exp + 1;
    if value == 0.0 {
        digits = vec![0];
        point = 1;
    }
    let keep = point + 1; // digits kept: the integer part and one decimal
    let mut int_and_frac: Vec<u8> = if keep <= 0 {
        // Everything is below the kept decimal: only the rounding is left.
        let round_up = keep == 0 && digits[0] >= 5;
        point = 1;
        if round_up { vec![0, 1] } else { vec![0, 0] }
    } else {
        let keep = keep as usize;
        let mut kept: Vec<u8> = digits.iter().copied().chain(std::iter::repeat(0)).take(keep).collect();
        if digits.get(keep).is_some_and(|&d| d >= 5) {
            let mut i = kept.len();
            loop {
                if i == 0 {
                    kept.insert(0, 1);
                    point += 1;
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
        if point <= 0 {
            // 0.0x: pad the integer part.
            let pad = (1 - point) as usize;
            let mut v = vec![0; pad];
            v.extend(kept);
            point = 1;
            v
        } else {
            kept
        }
    };
    while int_and_frac.len() < (point + 1) as usize {
        int_and_frac.push(0);
    }
    let point = point as usize;
    let int: String = int_and_frac[..point].iter().map(|d| char::from(b'0' + d)).collect();
    let int = int.trim_start_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let frac = char::from(b'0' + int_and_frac[point]);
    let zero = int == "0" && frac == '0';
    format!("{}{int}.{frac}", if negative && !zero { "-" } else { "" })
}

/// `text.Replace(from, to, StringComparison.OrdinalIgnoreCase)` for an
/// ASCII `from` (ShareX's `$fps$`-style placeholders): left to right, no
/// overlaps, replaced text isn't searched again. Ordinal case-insensitive
/// matching upper-cases both sides; no non-ASCII character upper-cases to
/// an ASCII letter in .NET's ordinal casing, so ASCII matching is exact.
pub fn replace_ordinal_ignore_case(text: &str, from: &str, to: &str) -> String {
    debug_assert!(from.is_ascii() && !from.is_empty());
    let hay = text.as_bytes();
    let needle = from.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            out.push_str(&text[last..i]);
            out.push_str(to);
            i += needle.len();
            last = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[last..]);
    out
}

/// .NET's 100 ns ticks.
pub const TICKS_PER_SECOND: i64 = 10_000_000;

/// `TimeSpan.TryParse` for the `h:m:s.f` text FFmpeg prints (the only form
/// ShareX's regexes hand it): hours 0-23, minutes and seconds 0-59, 1-7
/// fraction digits; in ticks.
pub fn parse_timespan(text: &str) -> Option<i64> {
    let (hms, frac) = text.split_once('.')?;
    let mut parts = hms.split(':');
    let mut field = |max: i64| -> Option<i64> {
        let p = parts.next()?;
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let v: i64 = p.parse().ok()?;
        (v <= max).then_some(v)
    };
    let (h, m, s) = (field(23)?, field(59)?, field(59)?);
    if parts.next().is_some() || frac.is_empty() || frac.len() > 7 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let f: i64 = format!("{frac:0<7}").parse().ok()?;
    Some(((h * 60 + m) * 60 + s) * TICKS_PER_SECOND + f)
}

/// How .NET turns `ProcessStartInfo.Arguments` into argv off Windows
/// (`Process.Unix.cs`, `ParseArgumentsIntoList`): the MSVCRT rules, so the
/// one string ShareX builds means the same everywhere. On Windows the
/// string goes to `CreateProcess` as it is.
pub fn split_arguments(arguments: &str) -> Vec<String> {
    let chars: Vec<char> = arguments.chars().collect();
    let mut results = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }
        if i == chars.len() {
            break;
        }
        results.push(next_argument(&chars, &mut i));
        i += 1;
    }
    results
}

fn next_argument(chars: &[char], i: &mut usize) -> String {
    let mut arg = String::new();
    let mut in_quotes = false;
    while *i < chars.len() {
        let mut backslashes = 0;
        while *i < chars.len() && chars[*i] == '\\' {
            *i += 1;
            backslashes += 1;
        }
        if backslashes > 0 {
            if *i >= chars.len() || chars[*i] != '"' {
                arg.extend(std::iter::repeat_n('\\', backslashes));
            } else {
                arg.extend(std::iter::repeat_n('\\', backslashes / 2));
                if backslashes % 2 != 0 {
                    arg.push('"');
                    *i += 1;
                }
            }
            continue;
        }
        let c = chars[*i];
        if c == '"' {
            if in_quotes && *i + 1 < chars.len() && chars[*i + 1] == '"' {
                arg.push('"');
                *i += 1;
            } else {
                in_quotes = !in_quotes;
            }
            *i += 1;
            continue;
        }
        if (c == ' ' || c == '\t') && !in_quotes {
            break;
        }
        arg.push(c);
        *i += 1;
    }
    arg
}

/// `Environment.NewLine`, which `StringBuilder.AppendLine` and
/// `StreamWriter.WriteLine` end lines with.
pub const NEW_LINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_format_as_dotnet() {
        let cases = [
            (3.0, "3.0"),
            (2.25, "2.3"),
            (0.05, "0.1"),
            (0.04, "0.0"),
            (0.0, "0.0"),
            (1.45, "1.5"),
            (9.96, "10.0"),
            (99.95, "100.0"),
            (0.96, "1.0"),
            (12345.67, "12345.7"),
            (1e10, "10000000000.0"),
            (0.25, "0.3"),
            (0.35, "0.4"),
            (-2.25, "-2.3"),
        ];
        for (v, want) in cases {
            assert_eq!(format_float_0_0(v), want, "{v}");
        }
    }

    #[test]
    fn changes_extensions_as_dotnet() {
        assert_eq!(change_extension(Some("a.mp4"), "gif").as_deref(), Some("a.gif"));
        assert_eq!(change_extension(Some("a"), "gif").as_deref(), Some("a.gif"));
        assert_eq!(change_extension(Some("a."), "gif").as_deref(), Some("a.gif"));
        assert_eq!(change_extension(Some("a.b.c"), ".gif").as_deref(), Some("a.b.gif"));
        assert_eq!(change_extension(Some(""), "gif").as_deref(), Some(""));
        assert_eq!(change_extension(None, "gif"), None);
        #[cfg(windows)]
        {
            assert_eq!(change_extension(Some(r"C:\a.b\file"), "mp4").as_deref(), Some(r"C:\a.b\file.mp4"));
            assert_eq!(change_extension(Some("C:file"), "mp4").as_deref(), Some("C:file.mp4"));
        }
    }

    #[test]
    fn appends_text_like_sharex() {
        assert_eq!(append_text_to_file_name("C:/x/a.mp4", "-concat"), "C:/x/a-concat.mp4");
        assert_eq!(append_text_to_file_name("C:/x.y/a", "-temp"), "C:/x-temp.y/a");
        assert_eq!(append_text_to_file_name("a", "-temp"), "a-temp");
    }

    #[test]
    fn replaces_ignoring_case() {
        assert_eq!(replace_ordinal_ignore_case("-r $FPS$ $fps$x$Fps", "$fps$", "30"), "-r 30 30x$Fps");
        assert_eq!(replace_ordinal_ignore_case("é$fps$é", "$fps$", "1"), "é1é");
    }

    #[test]
    fn parses_timespans() {
        assert_eq!(parse_timespan("00:00:15.32"), Some(153_200_000));
        assert_eq!(parse_timespan("01:02:03.1234567"), Some(37_231_234_567));
        assert_eq!(parse_timespan("24:00:00.00"), None);
        assert_eq!(parse_timespan("00:60:00.00"), None);
        assert_eq!(parse_timespan("00:00:00.12345678"), None);
    }

    #[test]
    fn splits_like_dotnet() {
        assert_eq!(split_arguments(r#"-i "a b.mp4" -y"#), ["-i", "a b.mp4", "-y"]);
        assert_eq!(split_arguments(r#"-i audio="Mic \"x\"" "#), ["-i", r#"audio=Mic "x""#]);
        assert_eq!(split_arguments(r#"a\\"b c" d\e"#), [r"a\b c", r"d\e"]);
        assert_eq!(split_arguments(r#""a""b""#), [r#"a"b"#]);
        assert_eq!(split_arguments("  "), Vec::<String>::new());
    }

    #[test]
    fn escapes_cli_text() {
        assert_eq!(escape_cli_text(r#"Mic "1" \ x"#), r#""Mic \"1\" \\ x""#);
    }
}
