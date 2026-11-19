//! `ShareX/Presentation/DisplayDiagnostics`: `DisplayDiagnosticsWindow`,
//! its `MonitorLayoutView` and `DisplayDiagnosticsCollector`. A plain-text
//! report of the monitors as the capture code sees them, with findings,
//! the arrangement drawn above it, Copy, Save as... and Refresh.
//! 900x740, at least 640x540.
//!
//! On Windows the report is ShareX's `Collect` over Windows' own answers
//! ([`crate::display_native::snapshot`]: DPI awareness and compatibility
//! layers, display modes, devices and targets, DPI per monitor, taskbars,
//! graphics adapters, the cursor) and the capture windows' log
//! ([`crate::window_geometry_log`]); elsewhere `CollectAvaloniaOnly`.
//! Where ShareX names Avalonia, the report names winit, ShareXrs's toolkit.

use egui::{CornerRadius, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use sharex_capture::Rect as PxRect;
use sharex_strings::sharex as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, TextBox, wrap_lines};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, mono, text_width};
use sharex_ui::icons;

use super::{Cx, Dialog, Effect, Spec};
use crate::history_ui::common::{docked_toolbar, tool_panel};

/// `LabelWidth`.
const LABEL_WIDTH: usize = 30;

/// `DisplayDiagnosticsMonitor`.
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub number: usize,
    pub primary: bool,
    pub bounds: PxRect,
    pub working_area: PxRect,
    pub scale_percent: i32,
}

/// What a collection gives: the report, the monitors to draw, the cursor.
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub text: String,
    pub monitors: Vec<Monitor>,
    pub cursor: Option<(i32, i32)>,
}

fn line(report: &mut String, label: &str, value: &str) {
    report.push_str("  ");
    report.push_str(&format!("{:<width$}", format!("{label}:"), width = LABEL_WIDTH));
    report.push_str(value);
    report.push_str("\r\n");
}

fn format_rect(r: &PxRect) -> String {
    format!("X={} Y={} W={} H={}", r.x, r.y, r.w, r.h)
}

fn format_insets(bounds: &PxRect, work: &PxRect) -> String {
    format!("L={} T={} R={} B={}", work.x - bounds.x, work.y - bounds.y, bounds.right() - work.right(), bounds.bottom() - work.bottom())
}

fn yes_no(v: bool) -> &'static str {
    if v { "Yes" } else { "No" }
}

fn intersects(a: &PxRect, b: &PxRect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// `Touches`: within a pixel.
fn touches(a: &PxRect, b: &PxRect) -> bool {
    intersects(&PxRect::new(a.x - 1, a.y - 1, a.w + 2, a.h + 2), b)
}

/// `AppendArrangement`.
pub fn arrangement(report: &mut String, findings: &mut Vec<String>, monitors: &[Monitor]) {
    report.push_str("Arrangement\r\n");
    if monitors.len() < 2 {
        report.push_str("  Single monitor.\r\n\r\n");
        return;
    }
    for m in monitors {
        let a = m.bounds;
        let mut touches_any = false;
        for o in monitors.iter().filter(|o| o.number > m.number) {
            let b = o.bounds;
            let mut relation = None;
            let (mut start, mut end) = (0, 0);
            if a.right() == b.x || b.right() == a.x {
                if a.bottom().min(b.bottom()) > a.y.max(b.y) {
                    relation = Some(if a.right() == b.x { "left of" } else { "right of" });
                    start = b.y - a.y;
                    end = b.bottom() - a.bottom();
                }
            } else if (a.bottom() == b.y || b.bottom() == a.y) && a.right().min(b.right()) > a.x.max(b.x) {
                relation = Some(if a.bottom() == b.y { "above" } else { "below" });
                start = b.x - a.x;
                end = b.right() - a.right();
            }
            let Some(relation) = relation else {
                if intersects(&a, &b) {
                    report.push_str(&format!("  Monitor {} overlaps monitor {}.\r\n", m.number, o.number));
                    let i = a.intersect(&b);
                    findings.push(format!("Monitors {} and {} overlap ({}).", m.number, o.number, format_rect(&i)));
                    touches_any = true;
                }
                continue;
            };
            touches_any = true;
            let vertical = matches!(relation, "left of" | "right of");
            let edges = if start == 0 && end == 0 {
                "edges aligned".to_owned()
            } else if vertical {
                format!("top edges differ by {start} px, bottom edges by {end} px")
            } else {
                format!("left edges differ by {start} px, right edges by {end} px")
            };
            report.push_str(&format!("  Monitor {} is {relation} monitor {}: {edges}.\r\n", m.number, o.number));
            if (start != 0 || end != 0) && m.scale_percent != o.scale_percent {
                findings.push(format!(
                    "Monitors {} ({}%) and {} ({}%) have different scaling and are not aligned, so one overhangs the other. This arrangement is linked to region capture offsets.",
                    m.number, m.scale_percent, o.number, o.scale_percent
                ));
            }
        }
        if !touches_any && monitors.iter().filter(|x| *x != m).all(|x| !touches(&a, &x.bounds)) {
            report.push_str(&format!("  Monitor {} does not touch any other monitor.\r\n", m.number));
            findings.push(format!("Monitor {} does not touch any other monitor, leaving a gap in the desktop.", m.number));
        }
    }
    report.push_str("\r\n");
}

/// What `Collect` reads besides Windows' answers: the screens as ShareXrs
/// (winit) sees them, this window's scale, the settings, the time.
pub struct Inputs<'a> {
    pub screens: &'a [sharex_capture::Screen],
    /// This window's bounds and scale, from winit.
    pub window: Option<(PxRect, f32)>,
    pub settings: &'a sharex_settings::ApplicationConfig,
    pub now: chrono::DateTime<chrono::Local>,
    /// Windows' answers (`DisplayDiagnosticsNative`); none elsewhere.
    pub native: Option<&'a crate::display_native::Snapshot>,
    /// `WindowGeometryLog.Entries`, oldest first.
    pub geometry: &'a [crate::window_geometry_log::Entry],
}

fn header(report: &mut String, now: &chrono::DateTime<chrono::Local>) {
    // `AppendHeader`.
    report.push_str("ShareXrs display diagnostics\r\n");
    report.push_str(&"=".repeat(28));
    report.push_str("\r\n");
    line(report, "Generated", &now.format("%Y-%m-%d %H:%M:%S %:z").to_string());
    report.push_str("\r\n");
}

fn application(report: &mut String, elevated: Option<bool>) {
    // `AppendApplication`.
    report.push_str("ShareXrs\r\n");
    line(report, "Version", env!("CARGO_PKG_VERSION"));
    line(report, "Build", if cfg!(debug_assertions) { "Debug" } else { "Release" });
    if let Some(e) = elevated {
        line(report, "Elevated", yes_no(e));
    }
    line(report, "Process architecture", std::env::consts::ARCH);
    // The strings' language (ShareX: `CurrentUICulture.Name`); English is
    // the neutral one.
    let language = sharex_strings::language();
    line(report, "Language", if language.is_empty() { "en" } else { language });
    report.push_str("\r\n");
}

fn capture_settings(report: &mut String, settings: &sharex_settings::ApplicationConfig) {
    // `AppendCaptureSettings`.
    report.push_str("Capture settings\r\n");
    let task = &settings.DefaultTaskSettings;
    let capture = task.CaptureSettings.clone().unwrap_or_default();
    let region = &capture.RegionCaptureOptions;
    line(report, "Show cursor", yes_no(capture.ShowCursor));
    line(report, "Capture transparent", yes_no(capture.CaptureTransparent));
    line(report, "HDR colour correction", yes_no(capture.HDRScreenshotColorCorrection));
    line(report, "Restrict to active monitor", yes_no(region.ActiveMonitorMode));
    line(report, "Detect windows", yes_no(region.DetectWindows));
    line(report, "Detect controls", yes_no(region.DetectControls));
    line(report, "Show magnifier", yes_no(region.ShowMagnifier));
    let recorder = if capture.ScreenRecordUseNative {
        "Native".to_owned()
    } else {
        format!("FFmpeg ({})", capture.FFmpegOptions.VideoSource)
    };
    line(report, "Screen recorder", &recorder);
    let general = task.GeneralSettings.clone().unwrap_or_default();
    let device = &general.ToastWindowMonitorDeviceName;
    line(report, "Toast monitor", &format!("{}{}", general.ToastWindowMonitor.name(), if device.is_empty() { String::new() } else { format!(" ({device})") }));
}

fn format_dpi(dpi: Option<(u32, u32)>) -> String {
    match dpi {
        Some((x, y)) if x == y => x.to_string(),
        Some((x, y)) => format!("{x} x {y}"),
        None => "Unknown".into(),
    }
}

fn percent(dpi: u32) -> String {
    format!("{}%", (dpi as f64 * 100.0 / 96.0).round())
}

fn orientation_name(o: i32) -> &'static str {
    match o {
        1 => "portrait (90°)",
        2 => "landscape flipped (180°)",
        3 => "portrait flipped (270°)",
        _ => "landscape",
    }
}

fn edge_name(e: i32) -> &'static str {
    match e {
        0 => "Left",
        1 => "Top",
        2 => "Right",
        _ => "Bottom",
    }
}

/// `"0.###"`.
fn scale_text(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `GetDisplayNumber`: `\\.\DISPLAY<n>`.
fn display_number(device: &str) -> Option<usize> {
    const PREFIX: &str = r"\\.\DISPLAY";
    if device.len() > PREFIX.len() && device[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        let rest = &device[PREFIX.len()..];
        if rest.bytes().all(|b| b.is_ascii_digit()) {
            return rest.parse().ok();
        }
    }
    None
}

/// `FindMonitorNumber`: the monitor the rectangle overlaps most.
fn find_monitor_number(monitors: &[(usize, &crate::display_native::NativeMonitor)], r: &PxRect) -> String {
    monitors
        .iter()
        .map(|(n, m)| (n, m.bounds.intersect(r)))
        .filter(|(_, i)| i.w > 0 && i.h > 0)
        .max_by_key(|(_, i)| i.w as i64 * i.h as i64)
        .map_or_else(|| "none".into(), |(n, _)| n.to_string())
}

/// `DisplayDiagnosticsCollector.Collect`: on Windows, everything it probes;
/// elsewhere `CollectAvaloniaOnly`.
pub fn collect(i: &Inputs) -> Diagnostics {
    match i.native {
        Some(native) => collect_windows(i, native),
        None => collect_screens_only(i),
    }
}

/// `CollectAvaloniaOnly`: what the toolkit knows of the screens.
fn collect_screens_only(i: &Inputs) -> Diagnostics {
    let mut report = String::new();
    header(&mut report, &i.now);
    application(&mut report, None);
    report.push_str("Operating system\r\n");
    line(&mut report, "Description", std::env::consts::OS);
    line(&mut report, "Session", &std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "Unknown".into()));
    line(&mut report, "Desktop", &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "Unknown".into()));
    report.push_str("\r\n");
    let mut monitors = Vec::new();
    for (k, s) in i.screens.iter().enumerate() {
        let number = k + 1;
        let scale = (s.scale * 100.0).round() as i32;
        monitors.push(Monitor { number, primary: s.primary, bounds: s.bounds, working_area: s.work_area, scale_percent: scale });
        report.push_str(&format!("Monitor {number}{}\r\n", if s.primary { " (primary)" } else { "" }));
        line(&mut report, "Name", "Unknown");
        line(&mut report, "Bounds", &format_rect(&s.bounds));
        line(&mut report, "Working area", &format_rect(&s.work_area));
        line(&mut report, "Scale", &format!("{scale}%"));
        report.push_str("\r\n");
    }
    Diagnostics { text: format!("{}\r\n", report.trim_end()), monitors, cursor: None }
}

fn collect_windows(i: &Inputs, n: &crate::display_native::Snapshot) -> Diagnostics {
    let mut report = String::new();
    let mut findings: Vec<String> = Vec::new();
    // `CollectMonitors`: numbered by their GDI name, in that order.
    let mut fallback = 100;
    let mut numbered: Vec<(usize, &crate::display_native::NativeMonitor)> = n
        .monitors
        .iter()
        .map(|m| {
            let number = display_number(&m.device_name).unwrap_or_else(|| {
                fallback += 1;
                fallback - 1
            });
            (number, m)
        })
        .collect();
    numbered.sort_by_key(|(k, _)| *k);
    // The screen ShareXrs sees for each monitor (`DesktopScreen.GetDeviceName` matched by bounds).
    let screen_of = |m: &crate::display_native::NativeMonitor| i.screens.iter().find(|s| s.bounds == m.bounds);
    let scale_of = |m: &crate::display_native::NativeMonitor| -> i32 {
        match (m.effective_dpi, screen_of(m)) {
            (Some((x, _)), _) => (x as f64 * 100.0 / 96.0).round() as i32,
            (None, Some(s)) => (s.scale * 100.0).round() as i32,
            _ => 100,
        }
    };
    let name_of = |m: &crate::display_native::NativeMonitor| -> String {
        if let Some(t) = m.target.as_ref().filter(|t| !t.friendly_name.trim().is_empty()) {
            return t.friendly_name.clone();
        }
        if let Some(d) = m.monitor_device.as_ref().filter(|d| !d.device_string.trim().is_empty()) {
            return d.device_string.clone();
        }
        "Unknown monitor".into()
    };

    header(&mut report, &i.now);
    let findings_at = report.len();
    application(&mut report, Some(n.elevated));
    // `AppendWindows`.
    report.push_str("Windows\r\n");
    line(&mut report, "Edition", &n.edition);
    line(&mut report, "Version", &n.version);
    line(&mut report, "Build", &n.build);
    line(&mut report, "OS architecture", std::env::consts::ARCH);
    report.push_str("\r\n");
    // `AppendDpi`.
    report.push_str("DPI awareness\r\n");
    line(&mut report, "Thread DPI awareness", &n.dpi_awareness);
    if !n.dpi_awareness.starts_with("Per-monitor v2") {
        findings.push(format!("ShareXrs is not running per-monitor v2 DPI aware ({}). Capture coordinates can be scaled by Windows.", n.dpi_awareness));
    }
    line(&mut report, "System DPI", &n.system_dpi.map_or("Unknown".into(), |d| format!("{d} ({})", percent(d))));
    let user = n.compat_user.clone().unwrap_or_default();
    let machine = n.compat_machine.clone().unwrap_or_default();
    line(&mut report, "Compatibility (user)", if user.trim().is_empty() { "None" } else { &user });
    line(&mut report, "Compatibility (all users)", if machine.trim().is_empty() { "None" } else { &machine });
    if format!("{user} {machine}").to_ascii_uppercase().contains("DPI") {
        findings.push(
            "A Windows compatibility setting overrides high DPI scaling for sharexrs.exe (Properties > Compatibility > Change high DPI settings). Turn it off and test again."
                .into(),
        );
    }
    report.push_str("\r\n");
    // `AppendVirtualScreen`.
    report.push_str("Virtual screen\r\n");
    let native_union = sharex_capture::geometry::combine(n.monitors.iter().map(|m| m.bounds));
    let screens_union = sharex_capture::geometry::combine(i.screens.iter().map(|s| s.bounds));
    let capture_bounds = sharex_capture::screenshot::screen_bounds(i.screens);
    line(&mut report, "GDI virtual screen", &format_rect(&n.gdi_virtual_screen));
    line(&mut report, "Monitor bounds combined", &format_rect(&native_union));
    line(&mut report, "winit screens combined", &format_rect(&screens_union));
    line(&mut report, "ShareXrs capture bounds", &format_rect(&capture_bounds));
    line(&mut report, "Monitor count", &format!("GDI {}, enumerated {}, winit {}", n.gdi_monitor_count, n.monitors.len(), i.screens.len()));
    line(&mut report, "Same display format", yes_no(n.same_display_format));
    if n.gdi_virtual_screen != native_union || screens_union != native_union || capture_bounds != native_union {
        findings.push(format!(
            "The desktop bounds disagree: GDI {}, monitors {}, winit {}, ShareXrs {}.",
            format_rect(&n.gdi_virtual_screen),
            format_rect(&native_union),
            format_rect(&screens_union),
            format_rect(&capture_bounds)
        ));
    }
    if i.screens.len() != n.monitors.len() {
        findings.push(format!("winit sees {} screens but Windows lists {} monitors.", i.screens.len(), n.monitors.len()));
    }
    if native_union.x < 0 || native_union.y < 0 {
        findings.push(format!("The desktop extends left of or above the primary monitor (origin {}, {}).", native_union.x, native_union.y));
    }
    report.push_str("\r\n");
    // `AppendMonitors`.
    for (number, m) in &numbered {
        report.push_str(&format!("Monitor {number}{}\r\n", if m.primary { " (primary)" } else { "" }));
        line(&mut report, "Name", &name_of(m));
        line(&mut report, "Device", &m.device_name);
        if let Some(t) = &m.target {
            line(&mut report, "Connection", &t.output_technology);
        }
        line(&mut report, "Graphics adapter", m.adapter.as_ref().map_or("Unknown", |a| a.device_string.as_str()));
        line(&mut report, "Bounds", &format_rect(&m.bounds));
        line(&mut report, "Working area", &format!("{}  insets {}", format_rect(&m.working_area), format_insets(&m.bounds, &m.working_area)));
        if let Some(mode) = &m.mode {
            line(
                &mut report,
                "Display mode",
                &format!("{} x {} at {}, {}, {} Hz, {}-bit, {}", mode.width, mode.height, mode.position.0, mode.position.1, mode.frequency, mode.bits_per_pixel, orientation_name(mode.orientation)),
            );
            if mode.width != m.bounds.w || mode.height != m.bounds.h {
                findings.push(format!(
                    "Monitor {number} reports {} x {} to ShareXrs but runs at {} x {}, so ShareXrs is being given scaled (virtualised) coordinates.",
                    m.bounds.w, m.bounds.h, mode.width, mode.height
                ));
            }
            if mode.orientation != 0 {
                findings.push(format!("Monitor {number} is rotated ({}).", orientation_name(mode.orientation)));
            }
        }
        let scale = scale_of(m);
        line(&mut report, "Scale", &format!("{scale}%"));
        line(&mut report, "Effective DPI", &format_dpi(m.effective_dpi));
        line(&mut report, "Angular DPI", &format_dpi(m.angular_dpi));
        line(&mut report, "Raw DPI", &format_dpi(m.raw_dpi));
        line(&mut report, "Windows scale factor", &m.windows_scale.map_or("Unknown".into(), |s| format!("{s}%")));
        let s = scale.max(1) as f64;
        line(&mut report, "Size in logical pixels", &format!("{} x {}", (m.bounds.w as f64 * 100.0 / s).round(), (m.bounds.h as f64 * 100.0 / s).round()));
        match screen_of(m) {
            Some(sc) => {
                line(&mut report, "winit", &format!("scaling {}, bounds {}, working area {}", scale_text(sc.scale), format_rect(&sc.bounds), format_rect(&sc.work_area)));
                if (sc.scale * 100.0 - scale as f64).abs() >= 1.0 {
                    findings.push(format!("winit scales monitor {number} by {:.0}% but Windows uses {scale}%.", sc.scale * 100.0));
                }
            }
            None => line(&mut report, "winit", "Not matched to a screen"),
        }
        if let Some(t) = &m.target {
            if t.refresh_rate > 0.0 {
                let r = format!("{:.2}", t.refresh_rate);
                line(&mut report, "Refresh rate", &format!("{} Hz", r.trim_end_matches('0').trim_end_matches('.')));
            }
            let mut hdr = if !t.advanced_color_supported {
                "Not supported".to_owned()
            } else if t.advanced_color_enabled {
                "On".to_owned()
            } else {
                "Off".to_owned()
            };
            if t.advanced_color_enabled && t.sdr_white_level_nits > 0 {
                hdr.push_str(&format!(", SDR content brightness {} nits", t.sdr_white_level_nits));
            }
            if t.bits_per_color_channel > 0 {
                hdr.push_str(&format!(", {} bits per channel", t.bits_per_color_channel));
            }
            line(&mut report, "HDR", &hdr);
        }
        report.push_str("\r\n");
    }
    let mut scales: Vec<i32> = numbered.iter().map(|(_, m)| scale_of(m)).collect();
    scales.sort_unstable();
    scales.dedup();
    if scales.len() > 1 {
        let list: Vec<String> = scales.iter().map(|s| format!("{s}%")).collect();
        findings.insert(0, format!("Monitors use different scaling ({}). Most region capture offset reports come from mixed scaling.", list.join(", ")));
    }
    // `AppendArrangement`.
    let monitors: Vec<Monitor> = numbered
        .iter()
        .map(|(k, m)| Monitor { number: *k, primary: m.primary, bounds: m.bounds, working_area: m.working_area, scale_percent: scale_of(m) })
        .collect();
    arrangement(&mut report, &mut findings, &monitors);
    // `AppendTaskbars`.
    report.push_str("Taskbars and app bars\r\n");
    let mut taskbars: Vec<PxRect> = Vec::new();
    match n.taskbar {
        Some((edge, bounds, auto_hide)) => {
            taskbars.push(bounds);
            line(
                &mut report,
                "Main taskbar",
                &format!("{}, {}{}, on monitor {}", edge_name(edge), format_rect(&bounds), if auto_hide { ", auto-hide" } else { "" }, find_monitor_number(&numbered, &bounds)),
            );
            if edge != 3 {
                findings.push(format!("The taskbar is on the {} edge. Side and top taskbars have caused overlay offsets.", edge_name(edge).to_lowercase()));
            }
        }
        None => line(&mut report, "Main taskbar", "Not found"),
    }
    for t in &n.secondary_taskbars {
        taskbars.push(*t);
        line(&mut report, "Secondary taskbar", &format!("{}, on monitor {}", format_rect(t), find_monitor_number(&numbered, t)));
    }
    for (number, m) in &numbered {
        let (area, work) = (m.bounds, m.working_area);
        let insets = [("left", work.x - area.x), ("top", work.y - area.y), ("right", area.right() - work.right()), ("bottom", area.bottom() - work.bottom())];
        for (edge, size) in insets.into_iter().filter(|(_, s)| *s > 0) {
            let strip = match edge {
                "left" => PxRect::new(area.x, area.y, size, area.h),
                "top" => PxRect::new(area.x, area.y, area.w, size),
                "right" => PxRect::new(area.right() - size, area.y, size, area.h),
                _ => PxRect::new(area.x, area.bottom() - size, area.w, size),
            };
            if !taskbars.iter().any(|t| intersects(t, &strip)) {
                line(&mut report, "Other app bar", &format!("monitor {number}, {edge} edge, {size} px"));
                findings.push(format!(
                    "Something other than the taskbar reserves {size} px at the {edge} of monitor {number} (an app bar such as yasb or a docked toolbar). App bars have shifted capture overlays."
                ));
            }
        }
    }
    report.push_str("\r\n");
    // `AppendGraphics`.
    report.push_str("Graphics adapters\r\n");
    for (a, driver) in &n.adapters {
        line(&mut report, &a.device_name, &if driver.is_empty() { a.device_string.clone() } else { format!("{}, driver {driver}", a.device_string) });
    }
    report.push_str("\r\n");
    // `AppendCursor`.
    report.push_str("Cursor\r\n");
    line(&mut report, "Position", &n.cursor.map_or("Unknown".into(), |(x, y)| format!("{x}, {y}")));
    line(&mut report, "Physical position", &n.physical_cursor.map_or("Unknown".into(), |(x, y)| format!("{x}, {y}")));
    if let Some((x, y)) = n.cursor {
        let on = numbered.iter().find(|(_, m)| Some(m.handle) == n.cursor_monitor).map(|(k, _)| k.to_string());
        line(&mut report, "On monitor", &on.unwrap_or_else(|| "None".into()));
        if let Some((px, py)) = n.physical_cursor
            && (px, py) != (x, y)
        {
            findings.push(format!("The cursor position differs between logical ({x}, {y}) and physical ({px}, {py}) coordinates."));
        }
    }
    report.push_str("\r\n");
    // `AppendThisWindow`.
    report.push_str("This window\r\n");
    let bounds = n.window_bounds.or(i.window.map(|w| w.0));
    line(&mut report, "Window bounds", &bounds.map_or("Unknown".into(), |b| format_rect(&b)));
    let dpi = n.window_dpi.or(i.window.map(|w| (w.1 * 96.0).round() as u32));
    line(&mut report, "Window DPI", &dpi.map_or("Unknown".into(), |d| format!("{d} ({})", percent(d))));
    line(&mut report, "winit render scaling", &i.window.map_or("Unknown".into(), |w| scale_text(w.1 as f64)));
    if let Some(b) = bounds {
        line(&mut report, "On monitor", &find_monitor_number(&numbered, &b));
    }
    report.push_str("\r\n");
    overlay_windows(&mut report, &mut findings, i.geometry);
    capture_settings(&mut report, i.settings);
    // The findings after the header.
    let mut f = String::from("Findings\r\n");
    if findings.is_empty() {
        f.push_str("  - Nothing unusual detected.\r\n");
    } else {
        for finding in &findings {
            f.push_str(&format!("  - {finding}\r\n"));
        }
    }
    f.push_str("\r\n");
    report.insert_str(findings_at, &f);
    Diagnostics { text: format!("{}\r\n", report.trim_end()), monitors, cursor: n.cursor }
}

/// `AppendOverlayWindows`: the recent capture windows, newest first.
fn overlay_windows(report: &mut String, findings: &mut Vec<String>, entries: &[crate::window_geometry_log::Entry]) {
    report.push_str("Recent capture windows\r\n");
    if entries.is_empty() {
        report.push_str("  None yet. Start a region capture, close it, then press Refresh to add its measurements.\r\n\r\n");
        return;
    }
    for e in entries.iter().rev() {
        report.push_str(&format!("  {} {}\r\n", e.time, e.source));
        let position_only = e.expected.w == 0 && e.expected.h == 0;
        line(report, "  Expected", &if position_only { format!("position {}, {}", e.expected.x, e.expected.y) } else { format_rect(&e.expected) });
        line(report, "  Actual window", &format_rect(&e.actual));
        line(report, "  Client origin", &format!("{}, {}", e.client_origin.0, e.client_origin.1));
        line(report, "  Window DPI", &if e.window_dpi > 0 { format!("{} ({})", e.window_dpi, percent(e.window_dpi)) } else { "Unknown".into() });
        line(report, "  winit render scaling", &scale_text(e.render_scaling));
        let (cw, ch) = ((e.client_size.0 * e.render_scaling).round(), (e.client_size.1 * e.render_scaling).round());
        line(report, "  Client size x scaling", &format!("{cw} x {ch}"));
        if e.image_size != (0, 0) {
            line(report, "  Screenshot size", &format!("{} x {}", e.image_size.0, e.image_size.1));
        }
        line(report, "  Monitor", &e.monitor_device_name);
        let source = e.source.to_lowercase();
        if e.window_dpi > 0 && (e.window_dpi as f64 / 96.0 - e.render_scaling).abs() > 0.01 {
            findings.push(format!(
                "The {source} window runs at {} in Windows but winit renders it at {:.0}%. Toolbar, magnifier and drawing positions can be scaled twice or not at all.",
                percent(e.window_dpi),
                e.render_scaling * 100.0
            ));
        }
        if let (Some(c), Some(a)) = (e.cursor_screen, e.cursor_client_pixels) {
            let expected = (c.0 - e.client_origin.0, c.1 - e.client_origin.1);
            line(report, "  Cursor in window", &format!("Windows {}, {}; winit {}, {}", expected.0, expected.1, a.0.round(), a.1.round()));
            if (a.0 - expected.0 as f64).abs() > 2.0 || (a.1 - expected.1 as f64).abs() > 2.0 {
                findings.push(format!(
                    "In the {source} window winit places the cursor at {}, {} but Windows has it at {}, {}, so the magnifier and selections use the wrong pixels.",
                    a.0.round(),
                    a.1.round(),
                    expected.0,
                    expected.1
                ));
            }
        }
        let position_wrong = e.actual.x != e.expected.x || e.actual.y != e.expected.y;
        let size_wrong = !position_only && (e.actual.w != e.expected.w || e.actual.h != e.expected.h);
        if position_wrong || size_wrong {
            findings.push(format!(
                "The {source} window opened at {} instead of {}.",
                format_rect(&e.actual),
                if position_only { format!("{}, {}", e.expected.x, e.expected.y) } else { format_rect(&e.expected) }
            ));
        }
        if e.image_size != (0, 0) && ((cw - e.image_size.0 as f64).abs() > 1.0 || (ch - e.image_size.1 as f64).abs() > 1.0) {
            findings.push(format!(
                "The {source} window covers {cw} x {ch} pixels but its screenshot is {} x {}, so selections will not line up.",
                e.image_size.0, e.image_size.1
            ));
        }
    }
    report.push_str("\r\n");
}

/// A Windows answer for synthetic `screens` (`--render`, tests): every
/// monitor at its screen's scale, a DisplayPort target, a bottom taskbar
/// on the first.
pub fn sample_snapshot(screens: &[sharex_capture::Screen]) -> crate::display_native::Snapshot {
    use crate::display_native::{DisplayDevice, DisplayMode, DisplayTarget, NativeMonitor, Snapshot};
    let monitors: Vec<NativeMonitor> = screens
        .iter()
        .enumerate()
        .map(|(k, s)| {
            let dpi = (s.scale * 96.0).round() as u32;
            let device_name = format!(r"\\.\DISPLAY{}", k + 1);
            NativeMonitor {
                handle: k as isize + 1,
                device_name: device_name.clone(),
                primary: s.primary,
                bounds: s.bounds,
                working_area: s.work_area,
                effective_dpi: Some((dpi, dpi)),
                angular_dpi: Some((dpi, dpi)),
                raw_dpi: Some((96, 96)),
                windows_scale: Some((s.scale * 100.0).round() as i32),
                mode: Some(DisplayMode { width: s.bounds.w, height: s.bounds.h, position: (s.bounds.x, s.bounds.y), bits_per_pixel: 32, frequency: 60, orientation: 0 }),
                adapter: Some(DisplayDevice { device_name: device_name.clone(), device_string: "Example Graphics".into(), ..Default::default() }),
                monitor_device: Some(DisplayDevice { device_string: "Generic PnP Monitor".into(), ..Default::default() }),
                target: Some(DisplayTarget {
                    source_device_name: device_name,
                    friendly_name: format!("Monitor {}", k + 1),
                    output_technology: "DisplayPort".into(),
                    refresh_rate: 60.0,
                    ..Default::default()
                }),
            }
        })
        .collect();
    let first = screens.first().map(|s| s.bounds).unwrap_or_default();
    Snapshot {
        dpi_awareness: "Per-monitor v2 (process: per-monitor aware)".into(),
        system_dpi: Some(96),
        edition: "Windows 11 Pro".into(),
        version: "24H2".into(),
        build: "26100.1".into(),
        gdi_virtual_screen: sharex_capture::geometry::combine(screens.iter().map(|s| s.bounds)),
        gdi_monitor_count: screens.len() as i32,
        same_display_format: true,
        taskbar: Some((3, PxRect::new(first.x, first.bottom() - 48, first.w, 48), false)),
        adapters: vec![(DisplayDevice { device_name: r"\\.\DISPLAY1".into(), device_string: "Example Graphics".into(), ..Default::default() }, "31.0.1 (1-1-2026)".into())],
        cursor: Some((640, 480)),
        physical_cursor: Some((640, 480)),
        cursor_monitor: Some(1),
        monitors,
        ..Default::default()
    }
}

pub struct DisplayDiagnosticsWindow {
    pub data: Diagnostics,
    /// The report as the text box shows it (`\n` lines).
    text: String,
    status: Option<(String, std::time::Instant)>,
    /// Collect on the next frame (`Opened`, Refresh).
    pending: bool,
    scrolled: bool,
}

impl Default for DisplayDiagnosticsWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayDiagnosticsWindow {
    pub fn new() -> Self {
        Self { data: Diagnostics::default(), text: String::new(), status: None, pending: true, scrolled: false }
    }

    /// For `--render`: a report already collected.
    pub fn with_data(data: Diagnostics) -> Self {
        let text = data.text.replace("\r\n", "\n");
        Self { data, text, status: None, pending: false, scrolled: false }
    }

    fn refresh(&mut self, ui: &egui::Ui, settings: &sharex_settings::ApplicationConfig) {
        let screens = sharex_capture::screenshot::screens();
        let ppp = ui.ctx().pixels_per_point();
        let window = ui.ctx().input(|i| i.viewport().outer_rect).map(|r| {
            (PxRect::new((r.min.x * ppp).round() as i32, (r.min.y * ppp).round() as i32, (r.width() * ppp).round() as i32, (r.height() * ppp).round() as i32), ppp)
        });
        let native = crate::display_native::snapshot(None);
        let geometry = crate::window_geometry_log::entries();
        self.data = collect(&Inputs { screens: &screens, window, settings, now: chrono::Local::now(), native: native.as_ref(), geometry: &geometry });
        self.text = self.data.text.replace("\r\n", "\n");
        self.scrolled = false;
    }

    fn tool_button(ui: &egui::Ui, rect: Rect, id: egui::Id, key: &str, text: &str) -> egui::Response {
        let p = palette(ui.ctx());
        let b = Button { padding: Thickness::hv(10.0, 4.0), ..Button::default() };
        let r = b.show(ui, rect, id, "", true);
        let inner = rect.shrink2(vec2(11.0, 5.0));
        let bf = font(13.0, 400);
        let content = 13.0 + 7.0 + text_width(ui, text, bf.clone());
        let x = inner.min.x + ((inner.width() - content) / 2.0).round_ties_even().max(0.0);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(x, center_y(inner, 13.0)), Vec2::splat(13.0)), key, b.foreground(ui, &r, true, p.background_panel));
        ui.painter().galley(pos2(x + 20.0, center_y(inner, line_height(ui, &bf))), galley(ui, text, bf, p.text_secondary, f32::INFINITY), p.text_secondary);
        r
    }

    /// `MonitorLayoutView.Render`.
    fn layout_view(&self, ui: &egui::Ui, rect: Rect) {
        let p = palette(ui.ctx());
        let monitors = &self.data.monitors;
        if monitors.is_empty() || rect.width() <= 16.0 || rect.height() <= 16.0 {
            return;
        }
        let desktop = sharex_capture::geometry::combine(monitors.iter().map(|m| m.bounds));
        if desktop.w <= 0 || desktop.h <= 0 {
            return;
        }
        let scale = ((rect.width() - 16.0) / desktop.w as f32).min((rect.height() - 16.0) / desktop.h as f32);
        let ox = rect.min.x + (rect.width() - desktop.w as f32 * scale) / 2.0;
        let oy = rect.min.y + (rect.height() - desktop.h as f32 * scale) / 2.0;
        let to_view = |r: &PxRect| Rect::from_min_size(pos2(ox + (r.x - desktop.x) as f32 * scale, oy + (r.y - desktop.y) as f32 * scale), vec2(r.w as f32 * scale, r.h as f32 * scale));
        let painter = ui.painter();
        let border_c = p.text.gamma_multiply(0.28);
        let reserved = p.text.gamma_multiply(0.3);
        let accent = sharex_ui::theme::palette(ui.ctx()).accent_start;
        for m in monitors {
            let area = to_view(&m.bounds).shrink(1.5);
            if area.width() <= 0.0 || area.height() <= 0.0 {
                continue;
            }
            let radius = 6.0_f32.min(area.width().min(area.height()) / 4.0);
            painter.rect_filled(area, CornerRadius::same(radius as u8), p.control_background_hover);
            // The reserved strips (taskbars), at least 4px thick.
            let work = to_view(&m.working_area);
            let clip = painter.with_clip_rect(area);
            let thick = |v: f32| v.max(4.0);
            if work.min.x > area.min.x {
                clip.rect_filled(Rect::from_min_size(area.min, vec2(thick(work.min.x - area.min.x), area.height())), CornerRadius::same(radius as u8), reserved);
            }
            if work.min.y > area.min.y {
                clip.rect_filled(Rect::from_min_size(area.min, vec2(area.width(), thick(work.min.y - area.min.y))), CornerRadius::same(radius as u8), reserved);
            }
            if work.max.x < area.max.x {
                let s = thick(area.max.x - work.max.x);
                clip.rect_filled(Rect::from_min_max(pos2(area.max.x - s, area.min.y), area.max), CornerRadius::same(radius as u8), reserved);
            }
            if work.max.y < area.max.y {
                let s = thick(area.max.y - work.max.y);
                clip.rect_filled(Rect::from_min_max(pos2(area.min.x, area.max.y - s), area.max), CornerRadius::same(radius as u8), reserved);
            }
            let bt = if m.primary { 2.0 } else { 1.0 };
            painter.rect_stroke(area, CornerRadius::same(radius as u8), Stroke::new(bt, if m.primary { accent } else { border_c }), StrokeKind::Inside);
            // The labels: the number, and the size and scale under it.
            let nsize = (area.width().min(area.height()) * 0.28).clamp(11.0, 30.0);
            let nf = font(nsize, 600);
            let cf = font(11.0, 400);
            let number = m.number.to_string();
            let caption = format!("{} × {}  ·  {}%", m.bounds.w, m.bounds.h, m.scale_percent);
            let (nw, nh) = (text_width(ui, &number, nf.clone()), line_height(ui, &nf));
            let (cw, ch) = (text_width(ui, &caption, cf.clone()), line_height(ui, &cf));
            let show_caption = cw + 12.0 <= area.width() && nh + ch + 8.0 <= area.height();
            let total = nh + if show_caption { ch + 2.0 } else { 0.0 };
            let top = area.center().y - total / 2.0;
            if nw <= area.width() && nh <= area.height() {
                painter.galley(pos2(area.center().x - nw / 2.0, top), galley(ui, &number, nf, p.text, f32::INFINITY), p.text);
            }
            if show_caption {
                painter.galley(pos2(area.center().x - cw / 2.0, top + nh + 2.0), galley(ui, &caption, cf, p.text_secondary, f32::INFINITY), p.text_secondary);
            }
        }
        if let Some((x, y)) = self.data.cursor
            && desktop.contains(x, y)
        {
            let c = pos2(ox + (x - desktop.x) as f32 * scale, oy + (y - desktop.y) as f32 * scale);
            painter.circle(c, 4.0, accent, Stroke::new(2.0, p.background_panel));
        }
    }
}

impl Dialog for DisplayDiagnosticsWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::DisplayDiagnosticsWindow_Title()), 900.0, 740.0, 640.0, 540.0)
    }

    fn single(&self) -> Option<&'static str> {
        Some("display-diagnostics")
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx) -> Vec<Effect> {
        if self.pending {
            self.pending = false;
            self.refresh(ui, cx.settings);
        }
        let p = palette(ui.ctx());
        let full = super::layout::background(ui);
        let mut out = Vec::new();
        let left = full.min.x + 12.0;
        let right = full.max.x - 12.0;
        // The toolbar.
        let bar = Rect::from_min_max(pos2(left, full.min.y), pos2(right, full.min.y + 8.0 + 34.0 + 8.0 + 1.0));
        docked_toolbar(ui, bar);
        let top = bar.min.y + 8.0;
        let bw = |t: &str| 2.0 + 20.0 + 13.0 + 7.0 + text_width(ui, t, font(13.0, 400));
        let (copy, save, refresh) = (s::DisplayDiagnosticsWindow_CopyToClipboard(), s::DisplayDiagnosticsWindow_SaveAs(), s::DisplayDiagnosticsWindow_Refresh());
        let copy_rect = Rect::from_min_size(pos2(bar.min.x + 9.0, top), vec2(bw(copy), 34.0));
        let save_rect = Rect::from_min_size(pos2(copy_rect.max.x + 6.0, top), vec2(bw(save), 34.0));
        let refresh_rect = Rect::from_min_size(pos2(bar.max.x - 9.0 - bw(refresh), top), vec2(bw(refresh), 34.0));
        if Self::tool_button(ui, copy_rect, ui.id().with("copy"), icon::ClipboardCopy, copy).clicked() && !self.data.text.is_empty() {
            out.push(Effect::CopyText(self.data.text.clone()));
            self.status = Some((s::DisplayDiagnosticsWindow_Copied().to_owned(), std::time::Instant::now()));
        }
        if Self::tool_button(ui, save_rect, ui.id().with("save"), icon::SaveAs, save).clicked() && !self.data.text.is_empty() {
            let name = format!("ShareXrs-display-diagnostics-{}.txt", chrono::Local::now().format("%Y-%m-%d-%H%M%S"));
            if let Some(path) = rfd::FileDialog::new().set_title(s::DisplayDiagnosticsWindow_SaveDialogTitle()).set_file_name(&name).add_filter(s::DisplayDiagnosticsWindow_TextFile(), &["txt"]).save_file() {
                match std::fs::write(&path, &self.data.text) {
                    Ok(()) => self.status = Some((s::DisplayDiagnosticsWindow_Saved0().replace("{0}", &path.to_string_lossy()), std::time::Instant::now())),
                    Err(e) => eprintln!("Failed to save the display diagnostics report. {e}"),
                }
            }
        }
        if Self::tool_button(ui, refresh_rect, ui.id().with("refresh"), icon::Refresh, refresh).clicked() {
            self.refresh(ui, cx.settings);
        }
        // The layout panel (`Padding="16,14"`).
        let hf = font(15.0, 600);
        let df = font(13.0, 400);
        let text_x = left + 1.0 + 16.0 + 20.0 + 10.0;
        let desc = wrap_lines(ui, s::DisplayDiagnosticsWindow_Description(), &df, right - 17.0 - text_x);
        let head_h = line_height(ui, &hf) + 2.0 + line_height(ui, &df) * desc.len() as f32;
        let lf = font(12.0, 400);
        let legend_h = line_height(ui, &lf).max(11.0);
        let panel = Rect::from_min_size(pos2(left, bar.max.y + 8.0), vec2(right - left, 2.0 + 28.0 + head_h + 10.0 + 200.0 + 10.0 + legend_h));
        tool_panel(ui, panel);
        let inner = Rect::from_min_max(pos2(panel.min.x + 17.0, panel.min.y + 15.0), pos2(panel.max.x - 17.0, panel.max.y - 15.0));
        icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, inner.min.y + 1.0), Vec2::splat(20.0)), icon::Monitor, p.accent_icon);
        ui.painter().galley(pos2(text_x, inner.min.y), galley(ui, s::DisplayDiagnosticsWindow_DisplayLayout(), hf.clone(), p.text, f32::INFINITY), p.text);
        let mut y = inner.min.y + line_height(ui, &hf) + 2.0;
        for l in &desc {
            ui.painter().galley(pos2(text_x, y), galley(ui, l, df.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            y += line_height(ui, &df);
        }
        let view = Rect::from_min_size(pos2(inner.min.x, inner.min.y + head_h + 10.0), vec2(inner.width(), 200.0));
        self.layout_view(ui, view);
        // The legend.
        let items = [s::DisplayDiagnosticsWindow_PrimaryMonitor(), s::DisplayDiagnosticsWindow_ReservedArea(), s::DisplayDiagnosticsWindow_MouseCursor()];
        let widths: Vec<f32> = items.iter().enumerate().map(|(i, t)| if i == 2 { 8.0 } else { 16.0 } + 7.0 + text_width(ui, t, lf.clone())).collect();
        let total = widths.iter().sum::<f32>() + 18.0 * 2.0;
        let mut x = inner.min.x + ((inner.width() - total) / 2.0).round_ties_even();
        let row = Rect::from_min_size(pos2(inner.min.x, view.max.y + 10.0), vec2(inner.width(), legend_h));
        for (i, t) in items.iter().enumerate() {
            let sw = if i == 2 { 8.0 } else { 16.0 };
            let sh = if i == 2 { 8.0 } else { 11.0 };
            let swatch = Rect::from_min_size(pos2(x, center_y(row, sh)), vec2(sw, sh));
            match i {
                0 => border::border(ui.painter(), swatch, 2.0, 2.0, Some(Brush::Solid(p.control_background_hover)), Some(p.accent_start)),
                1 => {
                    border::border(ui.painter(), swatch, 2.0, 0.0, Some(Brush::Solid(p.control_background_hover)), None);
                    ui.painter().rect_filled(Rect::from_min_max(pos2(swatch.min.x, swatch.max.y - 3.0), swatch.max), CornerRadius { sw: 2, se: 2, ..Default::default() }, p.text.gamma_multiply(0.3));
                    border::border(ui.painter(), swatch, 2.0, 1.0, None, Some(p.text.gamma_multiply(0.28)));
                }
                _ => {
                    ui.painter().circle_filled(swatch.center(), 4.0, p.accent_start);
                }
            }
            ui.painter().galley(pos2(x + sw + 7.0, center_y(row, line_height(ui, &lf))), galley(ui, t, lf.clone(), p.text_secondary, f32::INFINITY), p.text_secondary);
            x += widths[i] + 18.0;
        }
        // The status line.
        let status_h = line_height(ui, &df).max(13.0);
        let status = Rect::from_min_max(pos2(left, full.max.y - 12.0 - status_h), pos2(right, full.max.y - 12.0));
        if let Some((text, at)) = &self.status {
            let elapsed = at.elapsed().as_secs_f32();
            // `StatusDuration`, then the 0.15 s fade.
            let opacity = if elapsed < 3.0 { (elapsed / 0.15).min(1.0) } else { (1.0 - (elapsed - 3.0) / 0.15).max(0.0) };
            if opacity <= 0.0 {
                self.status = None;
            } else {
                let mut painter = ui.painter().clone();
                painter.set_opacity(opacity);
                icons::paint(&painter, Rect::from_min_size(pos2(status.min.x, center_y(status, 13.0)), Vec2::splat(13.0)), icon::CircleCheck, p.success);
                painter.galley(pos2(status.min.x + 20.0, center_y(status, line_height(ui, &df))), galley(ui, text, df.clone(), p.text_secondary, status.width() - 20.0), p.text_secondary);
                ui.ctx().request_repaint();
            }
        }
        // The report.
        let report = Rect::from_min_max(pos2(left, panel.max.y + 8.0), pos2(right, status.min.y - 8.0));
        tool_panel(ui, report);
        let box_rect = report.shrink(2.0);
        let fill = if ui.rect_contains_pointer(box_rect) { p.control_background_hover } else { p.control_background };
        border::border(ui.painter(), box_rect, 4.0, 0.0, Some(Brush::Solid(fill)), None);
        let view = box_rect.shrink(9.0);
        let id = ui.id().with("report");
        let tb = TextBox { padding: Thickness::all(0.0), multiline: true, read_only: true, plain: true, font: Some(mono(12.0)), ..TextBox::default() };
        let content = ui.fonts_mut(|f| f.layout_no_wrap(self.text.clone(), mono(12.0), p.text)).size();
        let content = vec2(content.x.ceil(), content.y.ceil());
        let mut scroll_ui = ui.new_child(egui::UiBuilder::new().max_rect(view));
        scroll_ui.set_clip_rect(view.intersect(ui.clip_rect()));
        let mut area = egui::ScrollArea::both().id_salt(id.with("scroll")).auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
        if !self.scrolled {
            // `CaretIndex = 0`.
            self.scrolled = true;
            area = area.scroll_offset(Vec2::ZERO);
        }
        let shown = area.show(&mut scroll_ui, |ui| {
            let r = Rect::from_min_size(ui.min_rect().min, content.max(view.size()));
            ui.allocate_rect(r, Sense::hover());
            tb.show(ui, r, id, &mut self.text)
        });
        super::debug_log::overlay_scroll_bars(ui, view, content, shown.state.offset);
        if sharex_ui::popup::escape_pressed(ui.ctx()) {
            out.push(Effect::Close);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(number: usize, x: i32, y: i32, w: i32, h: i32, scale: i32) -> Monitor {
        Monitor { number, primary: number == 1, bounds: PxRect::new(x, y, w, h), working_area: PxRect::new(x, y, w, h - 40), scale_percent: scale }
    }

    #[test]
    fn arrangements() {
        let (mut r, mut f) = (String::new(), Vec::new());
        arrangement(&mut r, &mut f, &[m(1, 0, 0, 1920, 1080, 100), m(2, 1920, 0, 1920, 1080, 100)]);
        assert!(r.contains("Monitor 1 is left of monitor 2: edges aligned."));
        assert!(f.is_empty());
        let (mut r, mut f) = (String::new(), Vec::new());
        arrangement(&mut r, &mut f, &[m(1, 0, 0, 1920, 1080, 100), m(2, 1920, 200, 2560, 1440, 150)]);
        assert!(r.contains("top edges differ by 200 px, bottom edges by 560 px"));
        assert_eq!(f.len(), 1);
        let (mut r, mut f) = (String::new(), Vec::new());
        arrangement(&mut r, &mut f, &[m(1, 0, 0, 100, 100, 100), m(2, 500, 0, 100, 100, 100)]);
        assert!(r.contains("Monitor 1 does not touch any other monitor."));
        assert_eq!(f.len(), 2);
        let (mut r, mut f) = (String::new(), Vec::new());
        arrangement(&mut r, &mut f, &[m(1, 0, 0, 100, 100, 100), m(2, 50, 50, 100, 100, 100)]);
        assert!(r.contains("Monitor 1 overlaps monitor 2."));
        assert_eq!(f[0], "Monitors 1 and 2 overlap (X=50 Y=50 W=50 H=50).");
    }

    fn screens() -> Vec<sharex_capture::Screen> {
        vec![
            sharex_capture::Screen { bounds: PxRect::new(0, 0, 1920, 1080), work_area: PxRect::new(0, 0, 1920, 1032), scale: 1.0, primary: true },
            sharex_capture::Screen { bounds: PxRect::new(1920, 0, 2560, 1440), work_area: PxRect::new(1920, 0, 2560, 1440), scale: 1.5, primary: false },
        ]
    }

    fn report(native: Option<&crate::display_native::Snapshot>, geometry: &[crate::window_geometry_log::Entry]) -> Diagnostics {
        let screens = screens();
        let config = sharex_settings::ApplicationConfig::default();
        collect(&Inputs { screens: &screens, window: Some((PxRect::new(10, 10, 900, 740), 1.0)), settings: &config, now: chrono::Local::now(), native, geometry })
    }

    #[test]
    fn the_windows_report_has_every_probe() {
        let screens = screens();
        let native = sample_snapshot(&screens);
        let d = report(Some(&native), &[]);
        for section in ["Windows\r\n", "DPI awareness\r\n", "Virtual screen\r\n", "Monitor 1 (primary)\r\n", "Monitor 2\r\n", "Arrangement\r\n", "Taskbars and app bars\r\n", "Graphics adapters\r\n", "Cursor\r\n", "This window\r\n", "Recent capture windows\r\n", "Capture settings\r\n"] {
            assert!(d.text.contains(section), "{section}");
        }
        assert!(d.text.contains(&format!("  {:<30}{}", "Display mode:", "2560 x 1440 at 1920, 0, 60 Hz, 32-bit, landscape")));
        assert!(d.text.contains(&format!("  {:<30}{}", "Effective DPI:", "144")));
        assert!(d.text.contains(&format!("  {:<30}{}", "Connection:", "DisplayPort")));
        assert!(d.text.contains(&format!("  {:<30}{}", "Main taskbar:", "Bottom, X=0 Y=1032 W=1920 H=48, on monitor 1")));
        // Mixed scaling comes first among the findings.
        let findings = &d.text[d.text.find("Findings").unwrap()..];
        assert!(findings.starts_with("Findings\r\n  - Monitors use different scaling (100%, 150%)."));
        assert_eq!(d.monitors.iter().map(|m| m.scale_percent).collect::<Vec<_>>(), [100, 150]);
    }

    #[test]
    fn findings_flag_virtualised_modes_side_taskbars_and_app_bars() {
        let screens = screens();
        let mut native = sample_snapshot(&screens);
        native.monitors[1].mode.as_mut().unwrap().width = 3840;
        native.taskbar = Some((0, PxRect::new(0, 0, 48, 1032), true));
        native.monitors[1].working_area = PxRect::new(1920, 30, 2560, 1410);
        native.dpi_awareness = "System aware".into();
        let d = report(Some(&native), &[]);
        assert!(d.text.contains("Monitor 2 reports 2560 x 1440 to ShareXrs but runs at 3840 x 1440"));
        assert!(d.text.contains("The taskbar is on the left edge."));
        assert!(d.text.contains("Something other than the taskbar reserves 30 px at the top of monitor 2"));
        assert!(d.text.contains("ShareXrs is not running per-monitor v2 DPI aware (System aware)."));
    }

    #[test]
    fn capture_windows_are_listed_newest_first() {
        let screens = screens();
        let native = sample_snapshot(&screens);
        let entry = |source: &str, x| crate::window_geometry_log::Entry {
            time: "12:00:00".into(),
            source: source.into(),
            expected: PxRect::new(0, 0, 1920, 1080),
            actual: PxRect::new(x, 0, 1920, 1080),
            window_dpi: 96,
            render_scaling: 1.0,
            client_size: (1920.0, 1080.0),
            image_size: (1920, 1080),
            monitor_device_name: r"\\.\DISPLAY1".into(),
            ..Default::default()
        };
        let d = report(Some(&native), &[entry("Region capture", 0), entry("Region selection (transparent)", 8)]);
        let a = d.text.find("Region selection (transparent)").unwrap();
        let b = d.text.find("12:00:00 Region capture").unwrap();
        assert!(a < b);
        assert!(d.text.contains("The region selection (transparent) window opened at X=8 Y=0 W=1920 H=1080 instead of X=0 Y=0 W=1920 H=1080."));
    }

    #[test]
    fn other_platforms_report_the_screens_only() {
        let d = report(None, &[]);
        assert!(d.text.contains("Operating system\r\n"));
        assert!(d.text.contains("Monitor 2\r\n"));
        assert!(!d.text.contains("Findings"));
        assert!(!d.text.contains("Arrangement"));
    }

    #[test]
    fn report_lines_are_padded() {
        let mut r = String::new();
        line(&mut r, "Scale", "100%");
        assert_eq!(r, format!("  {:<30}100%\r\n", "Scale:"));
    }
}
