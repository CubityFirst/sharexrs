//! The number texts the downloader shows: `NumberExtensions.ToSizeString`
//! and `double.ToString("0.0")`, in the invariant (English) format.

/// `double.ToString("0.<decimals zeros>")`: .NET takes 15 significant
/// digits, then rounds half away from zero to the format's decimals.
pub fn format_fixed(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return if value.is_nan() { "NaN".into() } else if value > 0.0 { "∞".into() } else { "-∞".into() };
    }
    let sci = format!("{:.14e}", value.abs());
    let (mantissa, exp) = sci.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent");
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    // `digits` is d.ddd… × 10^exp; keep the integer digits and `decimals` more.
    let keep = exp + 1 + decimals as i32;
    let mut int_len = exp + 1;
    if keep < 0 {
        digits = vec![0];
        int_len = 1;
    } else {
        let keep = keep as usize;
        let round_up = digits.get(keep).is_some_and(|&d| d >= 5);
        digits.truncate(keep);
        while digits.len() < keep {
            digits.push(0);
        }
        if round_up {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    int_len += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    // Pad leading zeros so there's at least one integer digit.
    while int_len < 1 {
        digits.insert(0, 0);
        int_len += 1;
    }
    let int_len = int_len as usize;
    let mut out: String = digits[..int_len].iter().map(|d| (b'0' + d) as char).collect();
    let int_part = out.trim_start_matches('0');
    out = if int_part.is_empty() { "0".into() } else { int_part.into() };
    if decimals > 0 {
        out.push('.');
        out.extend(digits[int_len..int_len + decimals].iter().map(|d| (b'0' + d) as char));
    }
    let negative = value < 0.0 && out.bytes().any(|b| (b'1'..=b'9').contains(&b));
    if negative { format!("-{out}") } else { out }
}

/// `long.ToSizeString(binary = false, decimalPlaces = 2)`.
pub fn to_size_string(size: i64) -> String {
    const BYTES: f64 = 1000.0;
    const SUFFIXES: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    if size < 1000 {
        return format!("{} B", size.max(0));
    }
    let place = ((size as f64).ln() / BYTES.ln()).floor() as i32;
    let num = size as f64 / BYTES.powi(place);
    format!("{} {}", format_fixed(num, 2), SUFFIXES[place as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_dotnet() {
        assert_eq!(format_fixed(1.005, 2), "1.01");
        assert_eq!(format_fixed(0.05, 1), "0.1");
        assert_eq!(format_fixed(99.96, 1), "100.0");
        assert_eq!(format_fixed(0.0, 1), "0.0");
        assert_eq!(format_fixed(12.3456, 0), "12");
        assert_eq!(format_fixed(-0.04, 1), "0.0");
        assert_eq!(format_fixed(0.004, 2), "0.00");
        assert_eq!(format_fixed(100.0, 1), "100.0");
    }

    #[test]
    fn sizes() {
        assert_eq!(to_size_string(-1), "0 B");
        assert_eq!(to_size_string(999), "999 B");
        assert_eq!(to_size_string(1000), "1.00 KB");
        assert_eq!(to_size_string(1_536_000), "1.54 MB");
        assert_eq!(to_size_string(52_428_800), "52.43 MB");
    }
}
