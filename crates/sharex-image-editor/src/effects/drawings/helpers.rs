//! `Core/ImageEffects/Drawings/DrawingEffectHelpers.cs`.

#![allow(dead_code)]

use super::enums::{DrawingImageRotateFlipType, DrawingInterpolationMode, DrawingPlacement};
use crate::effects::prelude::*;

/// `GetPosition`: where an object of `object` size goes on `background`.
pub(crate) fn get_position(
    placement: DrawingPlacement,
    offset: PointI,
    background: SizeI,
    object: SizeI,
) -> PointI {
    let mid_x =
        (background.width as f32 / 2.0 - object.width as f32 / 2.0).round_ties_even() as i32;
    let mid_y =
        (background.height as f32 / 2.0 - object.height as f32 / 2.0).round_ties_even() as i32;
    let right = background.width - object.width;
    let bottom = background.height - object.height;
    match placement {
        DrawingPlacement::TopCenter => PointI::new(mid_x, offset.y),
        DrawingPlacement::TopRight => PointI::new(right - offset.x, offset.y),
        DrawingPlacement::MiddleLeft => PointI::new(offset.x, mid_y),
        DrawingPlacement::MiddleCenter => PointI::new(mid_x, mid_y),
        DrawingPlacement::MiddleRight => PointI::new(right - offset.x, mid_y),
        DrawingPlacement::BottomLeft => PointI::new(offset.x, bottom - offset.y),
        DrawingPlacement::BottomCenter => PointI::new(mid_x, bottom - offset.y),
        DrawingPlacement::BottomRight => PointI::new(right - offset.x, bottom - offset.y),
        DrawingPlacement::TopLeft => offset,
    }
}

/// `ApplyAspectRatio`: a 0 dimension follows the bitmap's aspect ratio.
pub(crate) fn apply_aspect_ratio(width: i32, height: i32, bitmap: &Bitmap) -> SizeI {
    let (bw, bh) = (bitmap.width() as f32, bitmap.height() as f32);
    if width == 0 {
        SizeI::new((height as f32 / bh * bw).round_ties_even() as i32, height)
    } else if height == 0 {
        SizeI::new(width, (width as f32 / bw * bh).round_ties_even() as i32)
    } else {
        SizeI::new(width, height)
    }
}

/// `GetSamplingOptions`. Catmull-Rom is the default (`HighQualityBicubic`).
pub(crate) fn sampling(mode: DrawingInterpolationMode) -> Sampling {
    match mode {
        DrawingInterpolationMode::Bicubic | DrawingInterpolationMode::HighQualityBilinear => {
            Sampling::Mitchell
        }
        DrawingInterpolationMode::Bilinear => Sampling::Linear,
        DrawingInterpolationMode::NearestNeighbor => Sampling::Nearest,
        DrawingInterpolationMode::HighQualityBicubic => Sampling::CatmullRom,
    }
}

/// `RotateFlip`.
pub(crate) fn rotate_flip(source: &Bitmap, mode: DrawingImageRotateFlipType) -> Bitmap {
    use DrawingImageRotateFlipType as R;
    match mode {
        R::Rotate90 => rotate(source, 90),
        R::Rotate180 => rotate(source, 180),
        R::Rotate270 => rotate(source, 270),
        R::FlipX => flip(source, true),
        R::FlipY => flip(source, false),
        R::Rotate90FlipX => flip(&rotate(source, 90), true),
        R::Rotate90FlipY => flip(&rotate(source, 90), false),
        R::None => source.clone(),
    }
}

fn rotate(source: &Bitmap, degrees: i32) -> Bitmap {
    let (w, h) = (source.width(), source.height());
    let (nw, nh, tx, ty) = match degrees {
        90 => (h, w, h as f32, 0.0),
        180 => (w, h, w as f32, h as f32),
        _ => (h, w, 0.0, w as f32),
    };
    let mut out = Bitmap::new(nw, nh);
    let mut canvas = Canvas::new(&mut out);
    canvas.translate(tx, ty);
    canvas.rotate(degrees as f32);
    canvas.draw_bitmap(source, 0.0, 0.0, None);
    drop(canvas);
    out
}

fn flip(source: &Bitmap, horizontal: bool) -> Bitmap {
    let (w, h) = (source.width(), source.height());
    let mut out = Bitmap::new(w, h);
    let mut canvas = Canvas::new(&mut out);
    if horizontal {
        canvas.translate(w as f32, 0.0);
        canvas.scale(-1.0, 1.0);
    } else {
        canvas.translate(0.0, h as f32);
        canvas.scale(1.0, -1.0);
    }
    canvas.draw_bitmap(source, 0.0, 0.0, None);
    drop(canvas);
    out
}

/// `string.Replace(token, value, OrdinalIgnoreCase)`.
fn replace_ignore_case(s: &str, token: &str, value: &str) -> String {
    let lower = s.to_lowercase();
    let t = token.to_lowercase();
    if t.is_empty() || lower.len() != s.len() {
        return s.to_string();
    }
    let mut out = String::new();
    let mut i = 0;
    while let Some(p) = lower[i..].find(&t) {
        out.push_str(&s[i..i + p]);
        out.push_str(value);
        i += p + t.len();
    }
    out.push_str(&s[i..]);
    out
}

/// The local date and time: year, month, day, hour, minute, second,
/// millisecond, day of week (0 = Sunday).
fn local_now() -> (i32, u32, u32, u32, u32, u32, u32, u32) {
    #[cfg(windows)]
    {
        let mut st = windows_sys::Win32::Foundation::SYSTEMTIME {
            wYear: 0,
            wMonth: 0,
            wDayOfWeek: 0,
            wDay: 0,
            wHour: 0,
            wMinute: 0,
            wSecond: 0,
            wMilliseconds: 0,
        };
        unsafe { windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut st) };
        (
            st.wYear as i32,
            st.wMonth as u32,
            st.wDay as u32,
            st.wHour as u32,
            st.wMinute as u32,
            st.wSecond as u32,
            st.wMilliseconds as u32,
            st.wDayOfWeek as u32,
        )
    }
    #[cfg(not(windows))]
    {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let days = ms.div_euclid(86_400_000);
        let rem = ms.rem_euclid(86_400_000);
        // Civil from days (Howard Hinnant).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
        let dow = ((days + 4).rem_euclid(7)) as u32;
        (
            y,
            m,
            d,
            (rem / 3_600_000) as u32,
            (rem / 60_000 % 60) as u32,
            (rem / 1000 % 60) as u32,
            (rem % 1000) as u32,
            dow,
        )
    }
}

/// `ExpandTextVariables`: environment variables, then ShareX's `%` tokens
/// (date and time, image size, user and machine names), ignoring case.
pub(crate) fn expand_text_variables(text: &str, size: SizeI) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut s = crate::effects::helpers::expand_environment_variables(text);
    let (year, month, day, hour, minute, second, ms, dow) = local_now();
    let twelve = s.to_lowercase().contains("%pm");
    let mut h12 = hour % 12;
    if h12 == 0 {
        h12 = 12;
    }
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    const DAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let month_name = MONTHS[(month as usize).clamp(1, 12) - 1];
    let day_name = DAYS[(dow as usize) % 7];
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let env = |names: &[&str]| {
        names
            .iter()
            .find_map(|n| std::env::var(n).ok())
            .unwrap_or_default()
    };
    let user = env(&["USERNAME", "USER"]);
    let machine = env(&["COMPUTERNAME", "HOSTNAME"]);
    let domain = std::env::var("USERDOMAIN").unwrap_or_else(|_| machine.clone());
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    let replacements: [(&str, String); 20] = [
        ("%width", size.width.to_string()),
        ("%height", size.height.to_string()),
        ("%unix", unix.to_string()),
        ("%mon2", month_name.to_string()),
        ("%mon", month_name.to_string()),
        ("%yy", format!("{:02}", year.rem_euclid(100))),
        ("%y", year.to_string()),
        ("%mo", format!("{month:02}")),
        ("%mi", format!("{minute:02}")),
        ("%ms", format!("{ms:03}")),
        ("%pm", if hour >= 12 { "PM" } else { "AM" }.to_string()),
        ("%w2", day_name.to_string()),
        ("%w", day_name.to_string()),
        ("%d", format!("{day:02}")),
        ("%h", format!("{:02}", if twelve { h12 } else { hour })),
        ("%s", format!("{second:02}")),
        ("%un", user),
        ("%uln", domain),
        ("%cn", machine),
        ("%n", newline.to_string()),
    ];
    for (token, value) in &replacements {
        s = replace_ignore_case(&s, token, value);
    }
    s
}
