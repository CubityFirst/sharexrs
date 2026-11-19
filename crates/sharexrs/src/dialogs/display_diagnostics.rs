//! `ShareX/Presentation/DisplayDiagnostics`: `DisplayDiagnosticsWindow`,
//! its `MonitorLayoutView` and `DisplayDiagnosticsCollector`. A plain-text
//! report of the monitors as the capture code sees them, with findings,
//! the arrangement drawn above it, Copy, Save as... and Refresh.
//! 900x740, at least 640x540.
//!
//! The report's sections that need ShareX's Windows-only probes (DPI
//! awareness and compatibility layers, display modes and devices, DPI per
//! monitor, taskbars, graphics adapters, the region capture's window log)
//! aren't ported: the report has what every platform knows (ShareX's
//! `CollectAvaloniaOnly` and the sections built from the monitors' bounds).

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

/// `DisplayDiagnosticsCollector.Collect` for what ShareXrs knows of the
/// monitors (`screens`), the pointer and this window (`window`: its bounds
/// and scale), with the default task's capture settings.
pub fn collect(screens: &[sharex_capture::Screen], cursor: Option<(i32, i32)>, window: Option<(PxRect, f32)>, settings: &sharex_settings::ApplicationConfig, now: chrono::DateTime<chrono::Local>) -> Diagnostics {
    let mut report = String::new();
    let mut findings = Vec::new();
    // `AppendHeader`.
    report.push_str("ShareXrs display diagnostics\r\n");
    report.push_str(&"=".repeat(28));
    report.push_str("\r\n");
    line(&mut report, "Generated", &now.format("%Y-%m-%d %H:%M:%S %:z").to_string());
    report.push_str("\r\n");
    let findings_at = report.len();
    // `AppendApplication`.
    report.push_str("ShareXrs\r\n");
    line(&mut report, "Version", env!("CARGO_PKG_VERSION"));
    line(&mut report, "Build", if cfg!(debug_assertions) { "Debug" } else { "Release" });
    line(&mut report, "Process architecture", std::env::consts::ARCH);
    report.push_str("\r\n");
    report.push_str("Operating system\r\n");
    line(&mut report, "Description", std::env::consts::OS);
    if cfg!(all(unix, not(target_os = "macos"))) {
        line(&mut report, "Session", &std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "Unknown".into()));
        line(&mut report, "Desktop", &std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "Unknown".into()));
    }
    report.push_str("\r\n");
    // The monitors, numbered from the primary.
    let mut ordered: Vec<&sharex_capture::Screen> = screens.iter().collect();
    ordered.sort_by_key(|s| (!s.primary, s.bounds.x, s.bounds.y));
    let monitors: Vec<Monitor> = ordered
        .iter()
        .enumerate()
        .map(|(i, s)| Monitor { number: i + 1, primary: s.primary, bounds: s.bounds, working_area: s.work_area, scale_percent: (s.scale * 100.0).round() as i32 })
        .collect();
    // `AppendVirtualScreen`.
    report.push_str("Virtual screen\r\n");
    let union = sharex_capture::geometry::combine(monitors.iter().map(|m| m.bounds));
    line(&mut report, "Monitor bounds combined", &format_rect(&union));
    line(&mut report, "Monitor count", &monitors.len().to_string());
    report.push_str("\r\n");
    if let Some(primary) = monitors.iter().find(|m| m.primary)
        && (union.x < primary.bounds.x || union.y < primary.bounds.y)
    {
        findings.push(format!("The desktop extends left of or above the primary monitor (origin {}, {}).", union.x, union.y));
    }
    // `AppendMonitors`.
    for m in &monitors {
        report.push_str(&format!("Monitor {}{}\r\n", m.number, if m.primary { " (primary)" } else { "" }));
        line(&mut report, "Bounds", &format_rect(&m.bounds));
        line(&mut report, "Working area", &format!("{}  insets {}", format_rect(&m.working_area), format_insets(&m.bounds, &m.working_area)));
        line(&mut report, "Scale", &format!("{}%", m.scale_percent));
        let scale = m.scale_percent.max(1) as f64;
        line(&mut report, "Size in logical pixels", &format!("{} x {}", (m.bounds.w as f64 * 100.0 / scale).round(), (m.bounds.h as f64 * 100.0 / scale).round()));
        report.push_str("\r\n");
    }
    arrangement(&mut report, &mut findings, &monitors);
    // `AppendCursor`.
    report.push_str("Cursor\r\n");
    line(&mut report, "Position", &cursor.map_or("Unknown".into(), |(x, y)| format!("{x}, {y}")));
    if let Some((x, y)) = cursor {
        let on = monitors.iter().find(|m| m.bounds.contains(x, y));
        line(&mut report, "On monitor", &on.map_or("None".into(), |m| m.number.to_string()));
    }
    report.push_str("\r\n");
    // `AppendThisWindow`.
    report.push_str("This window\r\n");
    match window {
        Some((bounds, scale)) => {
            line(&mut report, "Window bounds", &format_rect(&bounds));
            line(&mut report, "Render scaling", &format!("{}", (scale * 1000.0).round() / 1000.0));
            let best = monitors
                .iter()
                .map(|m| (m, m.bounds.intersect(&bounds)))
                .filter(|(_, i)| i.w > 0 && i.h > 0)
                .max_by_key(|(_, i)| i.w as i64 * i.h as i64)
                .map(|(m, _)| m.number.to_string())
                .unwrap_or_else(|| "none".into());
            line(&mut report, "On monitor", &best);
        }
        None => line(&mut report, "Window bounds", "Unknown"),
    }
    report.push_str("\r\n");
    // `AppendCaptureSettings`.
    report.push_str("Capture settings\r\n");
    let task = &settings.DefaultTaskSettings;
    let capture = task.CaptureSettings.clone().unwrap_or_default();
    let region = &capture.RegionCaptureOptions;
    line(&mut report, "Show cursor", yes_no(capture.ShowCursor));
    line(&mut report, "Capture transparent", yes_no(capture.CaptureTransparent));
    line(&mut report, "HDR colour correction", yes_no(capture.HDRScreenshotColorCorrection));
    line(&mut report, "Restrict to active monitor", yes_no(region.ActiveMonitorMode));
    line(&mut report, "Detect windows", yes_no(region.DetectWindows));
    line(&mut report, "Detect controls", yes_no(region.DetectControls));
    line(&mut report, "Show magnifier", yes_no(region.ShowMagnifier));
    let general = task.GeneralSettings.clone().unwrap_or_default();
    let device = &general.ToastWindowMonitorDeviceName;
    line(&mut report, "Toast monitor", &format!("{}{}", general.ToastWindowMonitor.name(), if device.is_empty() { String::new() } else { format!(" ({device})") }));
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
    Diagnostics { text: format!("{}\r\n", report.trim_end()), monitors, cursor }
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
        let cursor = sharex_capture::screenshot::cursor_position();
        let ppp = ui.ctx().pixels_per_point();
        let window = ui.ctx().input(|i| i.viewport().outer_rect).map(|r| {
            (PxRect::new((r.min.x * ppp).round() as i32, (r.min.y * ppp).round() as i32, (r.width() * ppp).round() as i32, (r.height() * ppp).round() as i32), ppp)
        });
        self.data = collect(&screens, cursor, window, settings, chrono::Local::now());
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
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
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

    #[test]
    fn report_lines_are_padded() {
        let mut r = String::new();
        line(&mut r, "Scale", "100%");
        assert_eq!(r, format!("  {:<30}100%\r\n", "Scale:"));
    }
}
