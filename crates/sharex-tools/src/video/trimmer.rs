//! Video trimmer: `VideoTrimmerService` (the FFmpeg export; the window's
//! preview and timeline use Windows Media Foundation and aren't engine code).

use std::io::{BufRead, BufReader};
use std::process::Stdio;

/// `VideoTrimmerService.Timestamp`: invariant, up to six decimals.
pub fn timestamp(seconds: f64) -> String {
    crate::text::format_number(seconds, "0.######", false)
}

/// `BuildTrimArguments`: a stream copy cut at keyframes, or a precise H.264/AAC
/// re-encode, with progress on stdout.
pub fn build_trim_arguments(input: &str, output: &str, start: f64, end: f64, precise: bool) -> Vec<String> {
    let mut a: Vec<String> = [
        "-v", "error", "-ss", &timestamp(start), "-i", input, "-t", &timestamp(end - start), "-map", "0:V:0", "-map", "0:a?", "-map_chapters", "-1",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let rest: &[&str] = if precise {
        &[
            "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k",
            "-movflags", "+faststart",
        ]
    } else {
        &["-map", "0:s?", "-c", "copy", "-avoid_negative_ts", "make_zero"]
    };
    a.extend(rest.iter().map(|s| s.to_string()));
    a.extend(["-progress", "pipe:1", "-nostats", "-n", output].iter().map(|s| s.to_string()));
    a
}

/// `VideoTrimmerViewModel.FormatTime`: `hh:mm:ss.fff` of
/// `TimeSpan.FromSeconds(max(0, seconds))` (ticks truncated); hours past 99
/// keep their digits.
pub fn format_time(seconds: f64) -> String {
    let s = if seconds.is_nan() { 0.0 } else { seconds.max(0.0) };
    // TimeSpan.FromSeconds throws past TimeSpan.MaxValue; the windows never get there.
    let ticks = (s * 10_000_000.0).min(i64::MAX as f64) as i64;
    let hours = ticks / 36_000_000_000;
    let minutes = ticks / 600_000_000 % 60;
    let secs = ticks / 10_000_000 % 60;
    let millis = ticks / 10_000 % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02}.{millis:03}")
}

/// `int.TryParse(s, NumberStyles.None, Invariant)`: ASCII digits only.
fn parse_int_none(s: &str) -> Option<i32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: i64 = s.trim_start_matches('0').parse().unwrap_or(0);
    // Leading zeros don't count towards overflow; a long run of digits does.
    if s.trim_start_matches('0').len() > 10 || v > i32::MAX as i64 {
        return None;
    }
    Some(v as i32)
}

/// `double.TryParse(s, NumberStyles.AllowDecimalPoint, Invariant)`: digits
/// with at most one `.`, at least one digit.
fn parse_double_decimal_point(s: &str) -> Option<f64> {
    let mut dots = 0;
    let mut digits = 0;
    for b in s.bytes() {
        match b {
            b'0'..=b'9' => digits += 1,
            b'.' => dots += 1,
            _ => return None,
        }
    }
    if digits == 0 || dots > 1 {
        return None;
    }
    let t = if s.starts_with('.') { format!("0{s}") } else { s.to_string() };
    t.trim_end_matches('.').parse().ok()
}

/// `VideoTrimmerViewModel.TryParseTime`: `h:mm:ss.fff` (hours any length,
/// minutes 0-59, seconds below 60 with decimals), surrounding white space
/// ignored.
pub fn try_parse_time(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours = parse_int_none(parts[0])?;
    let minutes = parse_int_none(parts[1])?;
    let seconds = parse_double_decimal_point(parts[2])?;
    if !(0..=59).contains(&minutes) || !(0.0..60.0).contains(&seconds) {
        return None;
    }
    let v = hours as f64 * 3600.0 + minutes as f64 * 60.0 + seconds;
    v.is_finite().then_some(v)
}

/// What `TrimAsync` checks before it runs FFmpeg.
pub fn validate(input: &str, output: &str, start: f64, end: f64, duration: f64, precise: bool) -> Result<(), String> {
    if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start || end > duration {
        return Err("start".into());
    }
    let full = |p: &str| std::path::absolute(p).map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
    if full(input) == full(output) {
        return Err(sharex_strings::tools::VideoTrimmer_SourceOverwrite().into());
    }
    let ext = if precise { ".mp4".to_string() } else { crate::dotnet::extension(input).to_string() };
    if !crate::dotnet::extension(output).eq_ignore_ascii_case(&ext) {
        return Err(crate::text::format_dotnet(sharex_strings::tools::VideoTrimmer_OutputExtension(), &[ext.into()]));
    }
    Ok(())
}

/// The progress FFmpeg's `out_time_us=` lines give, 0..100.
pub fn progress_from_line(line: &str, duration: f64) -> Option<f64> {
    let us: i64 = line.strip_prefix("out_time_us=")?.trim().parse().ok()?;
    Some((us as f64 / 1_000_000.0 / duration * 100.0).clamp(0.0, 100.0))
}

/// `TrimAsync`: runs FFmpeg (`-hide_banner -nostdin` + the arguments) into a
/// hidden temporary file beside the output, moved over it on success.
pub fn trim(
    ffmpeg: &std::path::Path,
    input: &str,
    output: &str,
    start: f64,
    end: f64,
    duration: f64,
    precise: bool,
    mut progress: impl FnMut(f64),
) -> Result<(), String> {
    validate(input, output, start, end, duration, precise)?;
    let ext = if precise { ".mp4".to_string() } else { crate::dotnet::extension(input).to_string() };
    let abs = std::path::absolute(output).map_err(|e| e.to_string())?;
    let dir = abs.parent().ok_or("no folder")?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let temporary = dir.join(format!(".sharex-trim-{nanos:032x}{ext}"));
    let args = build_trim_arguments(input, &temporary.to_string_lossy(), start, end, precise);
    let result = (|| -> Result<(), String> {
        let mut child = std::process::Command::new(ffmpeg)
            .args(["-hide_banner", "-nostdin"])
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stderr = child.stderr.take();
        let log = std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(e) = stderr {
                for line in BufReader::new(e).lines().map_while(Result::ok) {
                    s.push_str(&line);
                    s.push('\n');
                }
            }
            s
        });
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(p) = progress_from_line(&line, end - start) {
                    progress(p);
                }
            }
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        let log = log.join().unwrap_or_default();
        if !status.success() {
            return Err(log.trim().to_string());
        }
        if std::fs::metadata(&temporary).map(|m| m.len()).unwrap_or(0) == 0 {
            return Err(sharex_strings::tools::VideoTrimmer_EmptyOutput().into());
        }
        std::fs::rename(&temporary, output).map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments() {
        let a = build_trim_arguments("in.mkv", "out.mkv", 1.5, 4.25, false).join(" ");
        assert_eq!(
            a,
            "-v error -ss 1.5 -i in.mkv -t 2.75 -map 0:V:0 -map 0:a? -map_chapters -1 -map 0:s? -c copy -avoid_negative_ts make_zero -progress pipe:1 -nostats -n out.mkv"
        );
        assert_eq!(progress_from_line("out_time_us=1375000", 2.75), Some(50.0));
    }
}
