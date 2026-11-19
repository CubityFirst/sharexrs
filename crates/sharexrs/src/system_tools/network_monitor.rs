//! `ShareX.Tools/Tools/NetworkMonitor` (`NetworkMonitorWindow.axaml`,
//! `NetworkMonitorViewModel`, `Controls/NetworkMonitorChart`) with
//! `TaskHelpers.OpenNetworkMonitor`'s services: monitoring starts when the
//! window opens; each round probes the target off the UI thread (a failure
//! is probed again after a second before it counts), records the sample,
//! updates the connection and appends transitions to
//! `Logs/NetworkMonitor.log`, then waits the interval. The status, the
//! four summary cards, the latency chart over the chosen range, the
//! events, Copy all, Clear log and Open log. The probe is injectable
//! ([`Prober`]): tests and `--render` never send an echo.

use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use egui::{Color32, Id, Rect, Stroke, Ui, Vec2, pos2, vec2};
use sharex_strings::tools as s;
use sharex_tools::system::network_monitor::{self as nm, EventStatus, NetworkMonitor, ProbeResult, Target};
use sharex_ui::controls::{self, ComboBox, ComboItem, content_font};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use crate::dialogs::{Cx, Dialog, Effect, Spec};
use crate::image_tools as tools;

pub const WIDTH: f32 = 1080.0;
pub const HEIGHT: f32 = 760.0;

/// What probes a target (`INetworkMonitorProbe.ProbeAsync`).
pub type Prober = Arc<dyn Fn(&Target) -> ProbeResult + Send + Sync>;

/// `NetworkMonitorProbe` with ICMP echoes.
pub fn icmp_prober() -> Prober {
    Arc::new(|t: &Target| nm::probe(t, &super::native::IcmpPinger))
}

pub struct NetworkMonitorWindow {
    pub monitor: NetworkMonitor,
    targets: Vec<Target>,
    prober: Prober,
    log_path: Option<PathBuf>,
    in_flight: Option<mpsc::Receiver<ProbeResult>>,
    /// The second probe of a failure (`DisconnectConfirmationDelay`).
    confirming: bool,
    next_probe: Option<Instant>,
    /// `IsPingAnimating`: since when.
    ping_started: Option<Instant>,
    opened: bool,
    events_scroll: f32,
    /// The time the chart ends at (`DateTimeOffset.Now`; fixed for `--render`).
    pub now: Option<DateTime<Local>>,
    pub sync: bool,
}

impl NetworkMonitorWindow {
    pub fn new(prober: Prober, log_path: Option<PathBuf>) -> Self {
        Self {
            monitor: NetworkMonitor::new(),
            targets: nm::targets(),
            prober,
            log_path,
            in_flight: None,
            confirming: false,
            next_probe: None,
            ping_started: None,
            opened: false,
            events_scroll: 0.0,
            now: None,
            sync: false,
        }
    }

    /// Opened without monitoring (`--render`).
    pub fn set_offline(&mut self) {
        self.opened = true;
        self.sync = true;
    }

    fn now(&self) -> DateTime<Local> {
        self.now.unwrap_or_else(Local::now)
    }

    /// `Start`.
    pub fn start(&mut self) {
        if self.monitor.monitoring {
            return;
        }
        self.monitor.monitoring = true;
        self.confirming = false;
        self.next_probe = Some(Instant::now());
    }

    /// `StopMonitoring`.
    pub fn stop(&mut self) {
        if !self.monitor.monitoring {
            return;
        }
        self.monitor.stop();
        self.next_probe = None;
        self.in_flight = None;
        self.confirming = false;
    }

    /// A probe's result: `RunProbeAsync`'s sample, then the confirmation
    /// or `UpdateConnectionStateAsync`.
    pub fn take_result(&mut self, r: ProbeResult) {
        let now = self.now();
        self.monitor.record(&r, now);
        if !self.confirming && self.monitor.needs_confirmation(&r) {
            self.confirming = true;
            self.next_probe = Some(Instant::now() + nm::DISCONNECT_CONFIRMATION_DELAY);
            return;
        }
        self.confirming = false;
        if let Some(event) = self.monitor.update_connection(&r)
            && let Some(path) = &self.log_path
            && let Err(e) = nm::append_log(path, &event)
        {
            eprintln!("NetworkMonitorEventLog: Failed to write the network monitor event log. {e}");
        }
        self.next_probe = self.monitor.monitoring.then(|| Instant::now() + self.monitor.interval());
    }

    /// The probe loop, one step a frame.
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.in_flight {
            match rx.try_recv() {
                Ok(r) => {
                    self.in_flight = None;
                    self.take_result(r);
                }
                Err(mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
                Err(mpsc::TryRecvError::Disconnected) => self.in_flight = None,
            }
        }
        if !self.monitor.monitoring || self.in_flight.is_some() {
            return;
        }
        let Some(at) = self.next_probe else { return };
        let now = Instant::now();
        if now < at {
            ctx.request_repaint_after(at - now);
            return;
        }
        self.next_probe = None;
        self.ping_started = Some(now);
        let target = self.targets[self.monitor.target].clone();
        let prober = self.prober.clone();
        let (tx, rx) = mpsc::channel();
        if self.sync {
            let _ = tx.send(prober(&target));
        } else {
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(prober(&target));
                ctx.request_repaint();
            });
        }
        self.in_flight = Some(rx);
        ctx.request_repaint_after(Duration::from_millis(50));
    }

    /// A button with an icon and text at `Spacing="7"`: its width.
    fn button_width(ui: &Ui, text: &str) -> f32 {
        1.0 + 10.0 + 13.0 + 7.0 + text_width(ui, text, font(13.0, 400)) + 10.0 + 1.0
    }

    fn button(ui: &Ui, rect: Rect, id: Id, key: &str, text: &str, enabled: bool) -> bool {
        let p = palette(ui.ctx());
        let r = controls::Button::default().show(ui, rect, id, "", enabled);
        let color = if enabled { p.text } else { sharex_ui::border::group_content(p.text, p.control_background, 0.35, p.background_panel) };
        let inner = rect.shrink2(vec2(11.0, 7.0));
        sharex_ui::icons::paint(ui.painter(), Rect::from_min_size(pos2(inner.min.x, center_y(inner, 13.0)), Vec2::splat(13.0)), key, color);
        let f = font(13.0, 400);
        tools::text_at(ui, pos2(inner.min.x + 13.0 + 7.0, center_y(inner, line_height(ui, &f))), text, f, color);
        r.clicked()
    }

    fn items(names: impl Iterator<Item = String>) -> Vec<ComboItem> {
        names.map(ComboItem::normal).collect()
    }

    /// The status row.
    fn status_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let row = rect.shrink(13.0);
        let nf = font(13.0, 400);
        let label_h = line_height(ui, &nf);
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        // Start / Pause, bottom-aligned.
        let (key, text) = if self.monitor.monitoring { (icon::Pause, s::NetworkMonitorWindow_Pause()) } else { (icon::Play, s::NetworkMonitorWindow_Start()) };
        let bw = Self::button_width(ui, text);
        let br = Rect::from_min_size(pos2(row.max.x - bw, row.max.y - 40.0), vec2(bw, 40.0));
        if Self::button(ui, br, Id::new(("nm-start", self.monitor.monitoring)), key, text, true) {
            if self.monitor.monitoring {
                self.stop();
            } else {
                self.start();
            }
        }
        let ix = br.min.x - 8.0 - 78.0;
        tools::text_at(ui, pos2(ix, row.min.y), s::NetworkMonitorWindow_Check_every(), nf.clone(), p.text_secondary);
        let items = Self::items(nm::INTERVALS.iter().map(|(n, _)| n.to_string()));
        if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(ix, row.min.y + label_h + 3.0), vec2(78.0, 40.0)), Id::new("nm-interval"), &items, Some(self.monitor.interval)) {
            self.monitor.interval = i;
        }
        let tx = ix - 8.0 - 235.0;
        tools::text_at(ui, pos2(tx, row.min.y), s::NetworkMonitorWindow_Target(), nf.clone(), p.text_secondary);
        let items = Self::items(self.targets.iter().map(|t| t.display_name.clone()));
        if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(tx, row.min.y + label_h + 3.0), vec2(235.0, 40.0)), Id::new("nm-target"), &items, Some(self.monitor.target)) {
            self.monitor.target = i;
        }
        // The dot (`Margin="8,0,6,0"`, 11px) and the status (18px SemiBold).
        let dot = Rect::from_min_size(pos2(row.min.x + 8.0, center_y(row, 11.0)), Vec2::splat(11.0));
        let mut color = match self.monitor.connected {
            Some(true) => p.success,
            Some(false) => p.error,
            None => p.text_secondary,
        };
        if let Some(t) = self.ping_started {
            // The 0.6 s pulse: 1 → 0.2 at 35% → 1 at 70%.
            let k = t.elapsed().as_secs_f32() / 0.6;
            if k < 1.0 {
                let o = if k < 0.35 { 1.0 - 0.8 * k / 0.35 } else if k < 0.7 { 0.2 + 0.8 * (k - 0.35) / 0.35 } else { 1.0 };
                color = color.gamma_multiply(o);
                ui.ctx().request_repaint();
            } else {
                self.ping_started = None;
            }
        }
        ui.painter().circle_filled(dot.center(), 5.5, color);
        let sf = font(18.0, 600);
        tools::text_at(ui, pos2(dot.max.x + 6.0 + 14.0, center_y(row, line_height(ui, &sf))), &self.monitor.status_text, sf, p.text);
    }

    /// The four summary cards.
    fn cards(&self, ui: &Ui, rect: Rect) {
        let p = palette(ui.ctx());
        let w = (rect.width() - 30.0) / 4.0;
        let cards = [
            (s::NetworkMonitorWindow_Current_latency(), &self.monitor.current_latency_text),
            (s::NetworkMonitorWindow_Average_latency(), &self.monitor.average_latency_text),
            (s::NetworkMonitorWindow_Availability(), &self.monitor.availability_text),
            (s::NetworkMonitorWindow_Disconnects(), &self.monitor.disconnect_count_text),
        ];
        let nf = font(13.0, 400);
        let vf = font(21.0, 600);
        for (k, (label, value)) in cards.into_iter().enumerate() {
            let x = rect.min.x + (w + 10.0) * k as f32;
            let card = Rect::from_min_max(pos2(x.round(), rect.min.y), pos2((x + w).round(), rect.max.y));
            sharex_ui::border::border(ui.painter(), card, 6.0, 1.0, Some(sharex_ui::border::Brush::Solid(p.background_panel)), Some(p.border));
            let inner = Rect::from_min_max(pos2(card.min.x + 15.0, card.min.y + 12.0), pos2(card.max.x - 15.0, card.max.y - 12.0));
            tools::text_at(ui, inner.min, label, nf.clone(), p.text_secondary);
            tools::text_at(ui, pos2(inner.min.x, inner.min.y + line_height(ui, &nf) + 2.0), value, vf.clone(), p.text);
        }
    }

    fn cards_height(ui: &Ui) -> f32 {
        24.0 + line_height(ui, &font(13.0, 400)) + 2.0 + line_height(ui, &font(21.0, 600))
    }

    /// A section header: the accent icon, the heading.
    fn section_title(ui: &Ui, rect: Rect, key: &str, text: &str) {
        let p = palette(ui.ctx());
        sharex_ui::icons::paint(ui.painter(), Rect::from_min_size(pos2(rect.min.x, center_y(rect, 13.0)), Vec2::splat(13.0)), key, p.accent_icon);
        tools::heading(ui, Rect::from_min_max(pos2(rect.min.x + 13.0 + 8.0, rect.min.y), rect.max), text);
    }

    /// The latency history panel.
    fn chart_panel(&mut self, ui: &mut Ui, rect: Rect) {
        let p = palette(ui.ctx());
        tools::panel(ui, rect);
        let inner = rect.shrink(1.0);
        let head = Rect::from_min_size(pos2(inner.min.x + 12.0, inner.min.y + 10.0), vec2(inner.width() - 24.0, 40.0));
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        let cr = Rect::from_min_size(pos2(head.max.x - 150.0, head.min.y), vec2(150.0, 40.0));
        let ranges = nm::time_ranges();
        let items = Self::items(ranges.iter().map(|(n, _)| n.clone()));
        if let Some(i) = combo.show(ui, cr, Id::new("nm-range"), &items, Some(self.monitor.time_range)) {
            let now = self.now();
            self.monitor.set_time_range(i, now);
        }
        let nf = font(13.0, 400);
        let rw = text_width(ui, s::NetworkMonitorWindow_Range(), nf.clone());
        tools::text_at(ui, pos2(cr.min.x - 8.0 - rw, center_y(head, line_height(ui, &nf))), s::NetworkMonitorWindow_Range(), nf, p.text_secondary);
        Self::section_title(ui, head, icon::LineChart, s::NetworkMonitorWindow_Latency_history());
        let chart = Rect::from_min_max(pos2(inner.min.x + 10.0, head.max.y + 10.0), pos2(inner.max.x - 10.0, inner.max.y - 10.0));
        self.draw_chart(ui, chart);
    }

    /// `NetworkMonitorChart.Render`.
    fn draw_chart(&self, ui: &Ui, bounds: Rect) {
        if bounds.width() < 120.0 || bounds.height() < 80.0 {
            return;
        }
        let text_color = Color32::from_rgb(142, 148, 158);
        let grid = Color32::from_rgba_unmultiplied(142, 148, 158, 55);
        let success = Color32::from_rgb(48, 187, 108);
        let failure = Color32::from_rgb(235, 78, 78);
        let plot = Rect::from_min_size(pos2(bounds.min.x + 52.0, bounds.min.y + 12.0), vec2((bounds.width() - 64.0).max(1.0), (bounds.height() - 42.0).max(1.0)));
        let now = self.now();
        let mut all = self.monitor.samples.clone();
        all.sort_by_key(|s| s.timestamp);
        let mut start = match self.monitor.chart_range() {
            Some(d) => now - d,
            None => all.first().map_or(now - chrono::Duration::minutes(1), |s| s.timestamp),
        };
        let visible: Vec<_> = all.iter().filter(|s| s.timestamp >= start && s.timestamp <= now).collect();
        let painter = ui.painter().with_clip_rect(bounds.intersect(ui.clip_rect()));
        if visible.is_empty() {
            let f = content_font(12.0, 400);
            let g = galley(ui, s::NetworkMonitorWindow_No_samples(), f, text_color, f32::INFINITY);
            let size = g.size();
            painter.galley(pos2(plot.center().x - size.x / 2.0, plot.center().y - size.y / 2.0), g, text_color);
            return;
        }
        let end = now;
        if end - start < chrono::Duration::seconds(1) {
            start = end - chrono::Duration::minutes(1);
        }
        let observed = visible.iter().filter(|s| s.success).filter_map(|s| s.latency_ms).fold(0.0, f64::max);
        let maximum = (observed / 25.0).ceil().mul_add(25.0, 0.0).max(50.0);
        let small = content_font(11.0, 400);
        for i in 0..=4 {
            let ratio = i as f32 / 4.0;
            let y = plot.max.y - plot.height() * ratio;
            painter.line_segment([pos2(plot.min.x, y), pos2(plot.max.x, y)], Stroke::new(1.0, grid));
            let label = sharex_tools::text::format_dotnet(s::NetworkMonitorViewModel_Milliseconds(), &[sharex_tools::text::Arg::F64(maximum * ratio as f64)]);
            painter.galley(pos2(bounds.min.x + 2.0, y - 7.0), galley(ui, &label, small.clone(), text_color, f32::INFINITY), text_color);
        }
        let c = sharex_helpers::date_format::CultureFormat::current();
        let time = |t: DateTime<Local>| sharex_helpers::date_format::format_custom(t.naive_local(), &c.short_time_pattern, c);
        let span = (end - start).num_milliseconds().max(1) as f64;
        let labels = [(start, plot.min.x, 0.0), (start + (end - start) / 2, plot.center().x, 0.5), (end, plot.max.x, 1.0)];
        for (t, x, align) in labels {
            let g = galley(ui, &time(t), small.clone(), text_color, f32::INFINITY);
            let w = g.size().x;
            painter.galley(pos2(x - w * align, plot.max.y + 8.0), g, text_color);
        }
        let mut previous: Option<(egui::Pos2, bool)> = None;
        for sample in visible {
            let xr = ((sample.timestamp - start).num_milliseconds() as f64 / span).clamp(0.0, 1.0) as f32;
            let x = plot.min.x + plot.width() * xr;
            let ok = sample.success && sample.latency_ms.is_some();
            let y = if ok { plot.max.y - plot.height() * (sample.latency_ms.unwrap_or(0.0) / maximum).clamp(0.0, 1.0) as f32 } else { plot.max.y };
            let point = pos2(x, y);
            if let Some((prev, prev_ok)) = previous {
                painter.line_segment([prev, point], Stroke::new(2.0, if prev_ok && ok { success } else { failure }));
            }
            previous = Some((point, ok));
        }
    }

    /// The connection events panel.
    fn events_panel(&mut self, ui: &mut Ui, rect: Rect) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let mut out = Vec::new();
        tools::panel(ui, rect);
        let inner = rect.shrink(1.0);
        let head = Rect::from_min_size(pos2(inner.min.x + 12.0, inner.min.y + 10.0), vec2(inner.width() - 24.0, 40.0));
        let has = self.monitor.has_events();
        let buttons = [
            (icon::FileText, s::NetworkMonitorWindow_Open_log(), self.log_path.is_some()),
            (icon::Trash, s::NetworkMonitorWindow_Clear_log(), has),
            (icon::Copy, s::NetworkMonitorWindow_Copy_all(), has),
        ];
        let mut x = head.max.x;
        let mut clicked = [false; 3];
        for (k, (key, text, enabled)) in buttons.into_iter().enumerate() {
            let w = Self::button_width(ui, text);
            x -= w;
            clicked[k] = Self::button(ui, Rect::from_min_size(pos2(x, head.min.y), vec2(w, 40.0)), Id::new(("nm-events", k)), key, text, enabled);
            x -= 8.0;
        }
        Self::section_title(ui, head, icon::ListRestart, s::NetworkMonitorWindow_Connection_events());
        // The column header (`Background.Toolbar`, 1px top and bottom borders).
        let nf = font(13.0, 400);
        let lh = line_height(ui, &nf);
        let bar = Rect::from_min_size(pos2(inner.min.x, head.max.y + 10.0), vec2(inner.width(), 1.0 + 7.0 + lh + 7.0 + 1.0));
        ui.painter().rect_filled(bar, 0.0, p.background_toolbar);
        ui.painter().rect_filled(Rect::from_min_size(bar.min, vec2(bar.width(), 1.0)), 0.0, p.border);
        ui.painter().rect_filled(Rect::from_min_size(pos2(bar.min.x, bar.max.y - 1.0), vec2(bar.width(), 1.0)), 0.0, p.border);
        let cy = bar.min.y + 1.0 + 7.0;
        tools::text_at(ui, pos2(bar.min.x + 10.0, cy), s::NetworkMonitorWindow_Date(), nf.clone(), p.text_secondary);
        tools::text_at(ui, pos2(bar.min.x + 10.0 + 165.0 + 12.0, cy), s::NetworkMonitorWindow_Status(), nf.clone(), p.text_secondary);
        let list = Rect::from_min_max(pos2(inner.min.x, bar.max.y), inner.max);
        if !has {
            tools::empty_state(ui, list, icon::Activity, 34.0, 7.0, &[(s::NetworkMonitorWindow_No_events(), nf, p.text_secondary)]);
        } else {
            let sf = font(13.0, 600);
            let row_h = 8.0 + lh.max(line_height(ui, &sf)) + 8.0 + 1.0;
            let content = row_h * self.monitor.events.len() as f32;
            let max_scroll = (content - list.height()).max(0.0);
            if ui.rect_contains_pointer(list) {
                self.events_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
            }
            self.events_scroll = self.events_scroll.clamp(0.0, max_scroll);
            let painter = ui.painter().with_clip_rect(list.intersect(ui.clip_rect()));
            for (i, e) in self.monitor.events.iter().enumerate() {
                let row = Rect::from_min_size(pos2(list.min.x, list.min.y + row_h * i as f32 - self.events_scroll), vec2(list.width(), row_h));
                if !row.intersects(list) {
                    continue;
                }
                painter.rect_filled(Rect::from_min_size(pos2(row.min.x, row.max.y - 1.0), vec2(row.width(), 1.0)), 0.0, p.border);
                let ty = row.min.y + 8.0;
                painter.galley(pos2(row.min.x + 10.0, ty), galley(ui, &e.timestamp_text(), nf.clone(), p.text, f32::INFINITY), p.text);
                let color = if e.status == EventStatus::Connected { p.success } else { p.error };
                painter.galley(pos2(row.min.x + 10.0 + 165.0 + 12.0, ty), galley(ui, e.status_text(), sf.clone(), color, f32::INFINITY), color);
            }
            if max_scroll > 0.0 {
                crate::dialogs::layout::scroll_bar(ui, list, content, self.events_scroll, 0.0);
            }
        }
        if clicked[2] && has {
            out.push(Effect::CopyText(self.monitor.copy_all_text()));
        }
        if clicked[1] && has {
            // `ClearLogAsync`: the file emptied, then the events.
            let cleared = match &self.log_path {
                Some(path) => path.parent().is_none_or(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(path, b"").is_ok(),
                None => true,
            };
            if cleared {
                self.monitor.clear_events();
            }
        }
        if clicked[0]
            && let Some(path) = &self.log_path
        {
            // `OpenLogFile`: created when missing, then opened.
            if !path.exists() {
                let _ = path.parent().map(std::fs::create_dir_all);
                let _ = std::fs::write(path, b"");
            }
            out.push(Effect::OpenFile(path.clone()));
        }
        out
    }
}

impl Dialog for NetworkMonitorWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::NetworkMonitorWindow_ShareX_Network_Monitor()), WIDTH, HEIGHT, 840.0, 620.0)
    }

    fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx) -> Vec<Effect> {
        let ctx = ui.ctx().clone();
        if !self.opened {
            // `OnOpened` → `Start`.
            self.opened = true;
            self.start();
        }
        self.poll(&ctx);
        let c = tools::background(ui);
        let label_h = line_height(ui, &font(13.0, 400));
        let status_h = 26.0 + label_h + 3.0 + 40.0;
        let status = Rect::from_min_size(c.min, vec2(c.width(), status_h));
        self.status_panel(ui, status);
        let ch = Self::cards_height(ui);
        let cards = Rect::from_min_size(pos2(c.min.x, status.max.y + 12.0), vec2(c.width(), ch));
        self.cards(ui, cards);
        // `RowDefinitions="Auto,Auto,*,1.15*"`.
        let rest = Rect::from_min_max(pos2(c.min.x, cards.max.y + 12.0), c.max);
        let star = (rest.height() - 12.0) / 2.15;
        let chart = Rect::from_min_size(rest.min, vec2(rest.width(), star.round()));
        self.chart_panel(ui, chart);
        let events = Rect::from_min_max(pos2(rest.min.x, chart.max.y + 12.0), rest.max);
        self.events_panel(ui, events)
    }

    fn closed(&mut self, _cx: &mut Cx) -> Vec<Effect> {
        self.stop();
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Answers from a script (no echo is sent).
    fn scripted(answers: Vec<bool>) -> Prober {
        let answers = Arc::new(Mutex::new(answers.into_iter()));
        Arc::new(move |t: &Target| {
            let ok = answers.lock().unwrap().next().unwrap_or(true);
            let replies = vec![if ok { nm::EchoReply::Success { latency_ms: 12.0 } } else { nm::EchoReply::TimedOut }; t.endpoints.len()];
            nm::probe_result(Local::now(), t, true, &replies)
        })
    }

    #[test]
    fn connects_confirms_a_disconnect_and_logs_it() {
        let dir = std::env::temp_dir().join(format!("sharexrs-nm-{}", std::process::id()));
        let log = dir.join("NetworkMonitor.log");
        let _ = std::fs::remove_file(&log);
        let mut w = NetworkMonitorWindow::new(scripted(vec![true, false, false]), Some(log.clone()));
        w.sync = true;
        w.opened = true;
        w.start();
        let ctx = egui::Context::default();
        let step = |w: &mut NetworkMonitorWindow| {
            w.next_probe = Some(Instant::now());
            w.poll(&ctx);
            w.poll(&ctx);
        };
        step(&mut w);
        assert_eq!(w.monitor.connected, Some(true));
        assert_eq!(w.monitor.status_text, s::NetworkMonitorViewModel_Connected());
        // A failure is probed again before it counts.
        step(&mut w);
        assert!(w.confirming);
        assert_eq!(w.monitor.connected, Some(true));
        step(&mut w);
        assert_eq!(w.monitor.connected, Some(false));
        assert_eq!(w.monitor.disconnect_count_text, "1");
        let text = std::fs::read_to_string(&log).unwrap();
        assert!(text.contains("Connected.") && text.contains("Disconnected."), "{text}");
        assert_eq!(w.monitor.samples.len(), 3);
        w.stop();
        assert!(!w.monitor.monitoring);
    }
}
