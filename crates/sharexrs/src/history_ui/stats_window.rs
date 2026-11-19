//! `ShareX.HistoryLib/Views/StatsWindow` and `StatsCharts`: the page of
//! cards (total with the split bar and legend, today, past 7 days, streak,
//! busiest day), the contribution calendar with its span picker, the hour
//! chart with its filter, the weekday rows and the top apps, destinations
//! and file types, from `HistoryStats` (`sharex-history`), in the colours
//! of `StatsPalette`.

use std::sync::mpsc;

use egui::{Color32, CornerRadius, FontId, Id, Pos2, Rect, Ui, Vec2, pos2, vec2};
use sharex_helpers::date_format::{CultureFormat, format_custom as format_date};
use sharex_history::stats::{self as hs, CalendarSpan, HistoryStats, HistoryStatsEntry, HistoryStatsFilter, HistoryStatsKind, HistoryStatsTally};
use sharex_history::DateTime;
use sharex_strings::history_lib as s;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, ComboBox, ComboItem, content_font};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use crate::dialogs::render::Options;
use crate::dialogs::{Cx, Dialog, Effect, Spec};

pub const WIDTH: f32 = 1060.0;
pub const HEIGHT: f32 = 900.0;
pub const MIN_WIDTH: f32 = 720.0;
pub const MIN_HEIGHT: f32 = 480.0;

/// `StatsPalette`: images in the accent's indicator colour, recordings and
/// other files in two hues away from it, a peak shade of each, and the
/// calendar's ramp.
struct Palette {
    series: [Color32; 3],
    peak: [Color32; 3],
    ramp: [Color32; 5],
}

/// `Color.ToHsv()`'s hue, saturation and value.
fn hsv(c: Color32) -> (f64, f64, f64) {
    let (r, g, b) = (c.r() as f64 / 255.0, c.g() as f64 / 255.0, c.b() as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

/// `Mix`: per channel, `Math.Round` (half to even).
fn mix(from: Color32, to: Color32, amount: f64) -> Color32 {
    let m = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * amount).round_ties_even() as u8;
    Color32::from_rgb(m(from.r(), to.r()), m(from.g(), to.g()), m(from.b(), to.b()))
}

impl Palette {
    fn new(dark: bool, accent: Color32) -> Self {
        let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        let candidates: [Color32; 5] =
            if dark { [0xD95926, 0x199E70, 0x9085E9, 0xD55181, 0xC98500].map(hex) } else { [0xEB6834, 0x1BAF7A, 0x6250D6, 0xE87BA4, 0xEDA100].map(hex) };
        let (ah, asat, av) = hsv(accent);
        let has_hue = asat > 0.2 && av > 0.2;
        let distance = |a: f64, b: f64| {
            let d = (a - b).abs() % 360.0;
            if d > 180.0 { 360.0 - d } else { d }
        };
        let mut picked: Vec<Color32> = candidates.iter().copied().filter(|c| !has_hue || distance(hsv(*c).0, ah) >= 45.0).take(2).collect();
        if picked.len() < 2 {
            picked = candidates[..2].to_vec();
        }
        let peak = |c: Color32| if dark { mix(c, Color32::WHITE, 0.45) } else { mix(c, Color32::BLACK, 0.35) };
        let series = [accent, picked[0], picked[1]];
        let empty = if dark { hex(0x313131) } else { hex(0xEBEBEF) };
        Self {
            series,
            peak: series.map(peak),
            ramp: [empty, mix(empty, accent, if dark { 0.38 } else { 0.3 }), mix(empty, accent, if dark { 0.66 } else { 0.6 }), accent, peak(accent)],
        }
    }

    fn kind(kind: HistoryStatsKind) -> usize {
        match kind {
            HistoryStatsKind::Image => 0,
            HistoryStatsKind::Recording => 1,
            HistoryStatsKind::Other => 2,
        }
    }
}

/// `StatsFormat`'s formats in the current culture.
struct Fmt {
    culture: &'static CultureFormat,
}

impl Fmt {
    fn number(&self, v: i32) -> String {
        hs::format_number(v as i64, &self.culture.number_group_separator)
    }

    fn days(&self, n: i32) -> String {
        (if n == 1 { s::StatsWindow_OneDay() } else { s::StatsWindow_Days() }).replace("{0}", &self.number(n))
    }

    /// `GetShortLongDatePattern`.
    fn short_long_pattern(&self, with_day: bool) -> String {
        let pattern = self.culture.long_date_pattern.replace("MMMM", "MMM");
        if pattern.contains("dddd") {
            if with_day {
                pattern.replace("dddd", "ddd")
            } else {
                let re = regex_lite_dddd(&pattern);
                re.trim().to_string()
            }
        } else if with_day {
            format!("ddd {pattern}")
        } else {
            pattern
        }
    }

    fn date(&self, d: DateTime) -> String {
        format_date(d.naive(), &self.short_long_pattern(false), self.culture)
    }

    fn date_with_day(&self, d: DateTime) -> String {
        format_date(d.naive(), &self.short_long_pattern(true), self.culture)
    }

    /// `StatsFormat.LongDateWithDay`: the long date pattern, with the day's
    /// name in front when it has none.
    fn long_date_with_day(&self, d: DateTime) -> String {
        let pattern = &self.culture.long_date_pattern;
        let pattern = if pattern.contains("dddd") { pattern.clone() } else { format!("dddd {pattern}") };
        format_date(d.naive(), &pattern, self.culture)
    }

    /// `StatsFormat.TallyToolTip`: the title, then the total and each kind
    /// there is any of, a line each.
    fn tally_tooltip(&self, title: &str, t: HistoryStatsTally) -> String {
        let mut out = title.to_owned();
        if t.total() == 0 {
            out.push('\n');
            out.push_str(s::StatsWindow_NothingYet());
            return out;
        }
        out.push('\n');
        out.push_str(&s::StatsWindow_TotalCount().replace("{0}", &self.number(t.total())));
        for (kind, count) in [(HistoryStatsKind::Image, t.images), (HistoryStatsKind::Recording, t.recordings), (HistoryStatsKind::Other, t.other)] {
            if count > 0 {
                out.push('\n');
                out.push_str(&format!("{}: {}", Self::series_name(kind), self.number(count)));
            }
        }
        out
    }

    fn series_name(kind: HistoryStatsKind) -> &'static str {
        match kind {
            HistoryStatsKind::Image => s::StatsWindow_Images(),
            HistoryStatsKind::Recording => s::StatsWindow_Recordings(),
            HistoryStatsKind::Other => s::StatsWindow_OtherFiles(),
        }
    }
}

/// `Regex.Replace(pattern, @"[\s,،]*dddd[\s,،]*", " ")`.
fn regex_lite_dddd(pattern: &str) -> String {
    let sep = |c: char| c.is_whitespace() || c == ',' || c == '،';
    match pattern.find("dddd") {
        Some(i) => {
            let before = pattern[..i].trim_end_matches(sep);
            let after = pattern[i + 4..].trim_start_matches(sep);
            format!("{before} {after}")
        }
        None => pattern.to_string(),
    }
}

/// A `StatsRow`.
struct Row {
    label: String,
    label_width: f32,
    fraction: f64,
    value: String,
    peak: bool,
    /// `StatsRow.ToolTip`.
    tooltip: String,
}

/// What a chart's pointer is over (`HoverKey`): a split bar segment (its
/// kind), a calendar day (its ticks) or an hour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HoverKey {
    Split(usize),
    Day(i64),
    Hour(usize),
}

pub struct StatsWindow {
    stats: Option<HistoryStats>,
    error: Option<String>,
    loader: Option<mpsc::Receiver<Result<HistoryStats, String>>>,
    loading: bool,
    spans: Vec<CalendarSpan>,
    span: usize,
    filter: HistoryStatsFilter,
    scroll: f32,
    effects: Vec<Effect>,
    sync: bool,
    /// The chart element under the pointer, where the pointer was when its
    /// tooltip opened, and when (`UpdateHover`).
    hover: Option<(HoverKey, Pos2, f64)>,
    /// Share was clicked: the page goes out as an image this frame.
    share: bool,
    /// `ActionsPanel.Opacity = 0` while the snapshot renders.
    hide_actions: bool,
}

impl Default for StatsWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl StatsWindow {
    pub fn new() -> Self {
        Self {
            stats: None,
            error: None,
            loader: None,
            loading: false,
            spans: vec![CalendarSpan::PastYear],
            span: 0,
            filter: HistoryStatsFilter::All,
            scroll: 0.0,
            effects: Vec::new(),
            sync: false,
            hover: None,
            share: false,
            hide_actions: false,
        }
    }

    /// `HistoryStats.Calculate(items, DateTime.Now, FirstDayOfWeek)`.
    fn calculate() -> Result<HistoryStats, String> {
        let items = crate::history::load()?;
        Ok(HistoryStats::calculate(&items, DateTime::now(), CultureFormat::current().first_day_of_week))
    }

    /// `RefreshAsync`.
    fn refresh(&mut self, ctx: &egui::Context) {
        if self.loading {
            return;
        }
        if self.sync {
            let r = Self::calculate();
            self.show(r);
            return;
        }
        self.loading = true;
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Self::calculate());
            ctx.request_repaint();
        });
        self.loader = Some(rx);
    }

    /// `ShowStats` (the spans keep the chosen one when it's still there).
    fn show(&mut self, result: Result<HistoryStats, String>) {
        self.loading = false;
        match result {
            Ok(stats) => {
                let selected = self.spans.get(self.span).copied().unwrap_or(CalendarSpan::PastYear);
                self.spans = stats.calendar_spans();
                self.span = self.spans.iter().position(|s| *s == selected).unwrap_or(0);
                self.stats = Some(stats);
                self.error = None;
            }
            Err(e) => {
                self.stats = None;
                self.error = Some(e);
            }
        }
    }

    fn has_items(&self) -> bool {
        self.stats.as_ref().is_some_and(|s| s.total.total() > 0 && s.first_day.is_some())
    }
}

/// Drawing helpers on the page.
struct Page<'a> {
    ui: &'a Ui,
    pal: Palette,
    fmt: Fmt,
    /// The pointer (none while a menu or the snapshot has it).
    pointer: Option<Pos2>,
    /// The hovered chart element (`HoverKey`).
    hover: Option<HoverKey>,
    /// What the charts' `HitTest` found under the pointer this frame.
    hit: std::cell::Cell<Option<HoverKey>>,
}

const PAGE_PADDING: f32 = 24.0;
const CARD_SPACING: f32 = 8.0;

fn hint_font() -> FontId {
    font(12.0, 400)
}

fn title_font() -> FontId {
    font(15.0, 600)
}

/// A chart's `FormattedText`: the control's font, Inter.
fn chart_font(size: f32, weight: u16) -> FontId {
    content_font(size, weight)
}

impl Page<'_> {
    fn text(&self, pos: Pos2, text: &str, f: FontId, color: Color32, max_w: f32) -> f32 {
        let g = galley(self.ui, text, f, color, max_w);
        let w = g.size().x;
        self.ui.painter().galley(pos, g, color);
        w
    }

    /// `Border.section-card`: panel, 1px border, 6px corners.
    fn card(&self, rect: Rect) {
        let p = palette(self.ui.ctx());
        border::border(self.ui.painter(), rect, 6.0, 1.0, Some(Brush::Solid(p.background_panel)), Some(p.border));
    }

    fn lh(&self, f: &FontId) -> f32 {
        line_height(self.ui, f)
    }

    /// A rectangle with `radius` corners (Avalonia's `DrawRectangle`).
    fn rounded(&self, rect: Rect, radius: f32, color: Color32) {
        border::border(self.ui.painter(), rect, radius, 0.0, Some(Brush::Solid(color)), None);
    }

    fn muted(&self) -> Color32 {
        palette(self.ui.ctx()).text.gamma_multiply(0.62)
    }
}

impl StatsWindow {
    /// The page's content height at `width` (inside the padding).
    fn content(&mut self, ui: &mut Ui, origin: Pos2, width: f32, paint: bool) -> f32 {
        let p = palette(ui.ctx());
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        let pointer = if self.hide_actions { None } else { ui.input(|i| i.pointer.latest_pos()).filter(|pos| ui.clip_rect().contains(*pos)) };
        let page = Page {
            ui,
            pal: Palette::new(dark, p.accent_border),
            fmt: Fmt { culture: CultureFormat::current() },
            pointer,
            hover: self.hover.map(|(k, ..)| k),
            hit: std::cell::Cell::new(None),
        };
        let x0 = origin.x;
        let mut y = origin.y;
        // The title row: icon, "Stats", the subtitle, Refresh and Share.
        let title_f = font(24.0, 600);
        let row_h = page.lh(&title_f).max(40.0);
        if paint {
            let row = Rect::from_min_size(pos2(x0, y), vec2(width, row_h));
            icons::paint(ui.painter(), Rect::from_min_size(pos2(x0, center_y(row, 22.0)), Vec2::splat(22.0)), icon::BarChart, p.accent_icon);
            let tx = x0 + 22.0 + 9.0;
            let tw = page.text(pos2(tx, center_y(row, page.lh(&title_f))), s::StatsWindow_Stats(), title_f.clone(), p.accent_text, f32::INFINITY);
            let share = s::StatsWindow_Share();
            let refresh = s::StatsWindow_Refresh();
            let sw = super::common::icon_button_width(ui, share);
            let rw = super::common::icon_button_width(ui, refresh);
            let share_r = Rect::from_min_size(pos2(x0 + width - sw, center_y(row, 40.0)), vec2(sw, 40.0));
            let refresh_r = Rect::from_min_size(pos2(share_r.min.x - 6.0 - rw, share_r.min.y), vec2(rw, 40.0));
            // `ActionsPanel`, hidden from the snapshot.
            if !self.hide_actions {
                if super::common::icon_button(ui, refresh_r, Id::new("stats-refresh"), icon::Refresh, refresh, false, !self.loading).clicked() {
                    self.refresh(ui.ctx());
                }
                if super::common::icon_button(ui, share_r, Id::new("stats-share"), icon::Share, share, false, self.has_items()).clicked() {
                    self.share = true;
                }
                sharex_ui::tooltip::show(ui, share_r, Id::new("stats-share-tip"), s::StatsWindow_ShareToolTip());
            }
            if let Some(stats) = &self.stats
                && let Some(first) = stats.first_day
            {
                let sub = s::StatsWindow_CountingSince().replace("{0}", &page.fmt.date(first)).replace("{1}", &page.fmt.days(stats.days_counted()));
                let hf = hint_font();
                let sx = tx + tw + 9.0 + 6.0;
                let max_w = (refresh_r.min.x - 9.0 - sx).max(0.0);
                // `Margin="6,4,0,0"`, centred in the rest.
                let sy = row.min.y + 4.0 + ((row.height() - 4.0 - page.lh(&hf)) / 2.0).round_ties_even();
                page.text(pos2(sx, sy), &sub, hf, p.text_secondary, max_w);
            }
        }
        y += row_h + 14.0;

        if !self.has_items() {
            // `StatusPanel`: the status text centred in a card (`Padding="24"`).
            let text = if let Some(e) = &self.error {
                e.clone()
            } else if self.stats.is_none() {
                s::HistoryWindow_CalculatingStatistics().to_string()
            } else {
                s::StatsWindow_NoHistory().to_string()
            };
            let hf = hint_font();
            let lines = sharex_ui::controls::wrap_lines(ui, &text, &hf, width - 50.0);
            let lh = page.lh(&hf);
            let h = 26.0 + 24.0 + lh * lines.len() as f32;
            if paint {
                let r = Rect::from_min_size(pos2(x0, y), vec2(width, h));
                page.card(r);
                for (k, line) in lines.iter().enumerate() {
                    let w = text_width(ui, line, hf.clone());
                    let lx = x0 + ((width - w) / 2.0).round_ties_even();
                    page.text(pos2(lx, y + 25.0 + lh * k as f32), line, hf.clone(), p.text_secondary, f32::INFINITY);
                }
            }
            return y + h - origin.y;
        }
        let stats = self.stats.clone().unwrap();
        y += self.total_card(&page, pos2(x0, y), width, &stats, paint) + CARD_SPACING;
        y += self.highlights(&page, pos2(x0, y), width, &stats, paint) + CARD_SPACING;
        y += self.calendar_card(&page, pos2(x0, y), width, &stats, paint) + CARD_SPACING;
        y += self.hours_card(&page, pos2(x0, y), width, &stats, paint) + CARD_SPACING;
        let half = (width - CARD_SPACING) / 2.0;
        let fmt = &page.fmt;
        // `ShowWeekdays`.
        let culture = page.fmt.culture;
        let total: i32 = stats.weekdays.iter().map(HistoryStatsTally::total).sum();
        let max = stats.weekdays.iter().map(HistoryStatsTally::total).max().unwrap_or(0);
        let peak = stats.peak_weekday();
        let weekdays: Vec<Row> = (0..7)
            .map(|i| {
                let day = (stats.first_day_of_week + i) % 7;
                let t = stats.weekdays[day as usize];
                Row {
                    label: culture.abbreviated_day_names[day as usize].clone(),
                    label_width: 44.0,
                    fraction: if max > 0 { t.total() as f64 / max as f64 } else { 0.0 },
                    value: hs::format_share(t.total(), total),
                    peak: Some(day) == peak,
                    tooltip: page.fmt.tally_tooltip(&culture.day_names[day as usize], t),
                }
            })
            .collect();
        let weekday_note = peak.map(|d| s::StatsWindow_BusiestOn().replace("{0}", &culture.day_names[d as usize])).unwrap_or_default();
        let entries = |list: &[HistoryStatsEntry]| -> Vec<Row> {
            let total: i32 = list.iter().map(|e| e.count).sum();
            let max = list.iter().map(|e| e.count).max().unwrap_or(0);
            list.iter()
                .map(|e| {
                    let name = if e.name.is_empty() { s::StatsWindow_Other().to_string() } else { e.name.clone() };
                    let share = hs::format_share(e.count, total);
                    let tooltip = format!("{name}: {} ({share})", page.fmt.number(e.count));
                    Row {
                        label: name,
                        label_width: 150.0,
                        fraction: if max > 0 { e.count as f64 / max as f64 } else { 0.0 },
                        value: share,
                        peak: false,
                        tooltip,
                    }
                })
                .collect()
        };
        let _ = fmt;
        let pairs = [
            [(s::StatsWindow_DayOfWeek(), weekday_note, weekdays, false), (s::StatsWindow_TopApps(), String::new(), entries(&stats.apps), true)],
            [(s::StatsWindow_Destinations(), String::new(), entries(&stats.destinations), true), (s::StatsWindow_FileTypes(), String::new(), entries(&stats.file_types), true)],
        ];
        for (k, pair) in pairs.into_iter().enumerate() {
            let hs: Vec<f32> = pair.iter().map(|(_, _, rows, empty_text)| list_card_height(&page, rows, *empty_text)).collect();
            let h = hs[0].max(hs[1]);
            for (col, (title, note, rows, empty_text)) in pair.into_iter().enumerate() {
                let r = Rect::from_min_size(pos2(x0 + col as f32 * (half + CARD_SPACING), y), vec2(half, h));
                if paint {
                    list_card(&page, r, title, &note, &rows, empty_text);
                }
            }
            y += h;
            if k == 0 {
                y += CARD_SPACING;
            }
        }
        // `UpdateHover`: a new element under the pointer opens its tooltip
        // at once, where the pointer is.
        if paint && !self.hide_actions {
            let hit = page.hit.take();
            match (hit, self.hover) {
                (Some(key), Some((old, ..))) if key == old => {}
                (Some(key), _) => self.hover = Some((key, pointer.unwrap_or(origin), ui.input(|i| i.time))),
                (None, _) => self.hover = None,
            }
            if let Some((key, at, opened)) = self.hover
                && let Some(tip) = Self::hover_tip(&page, &stats, key)
            {
                sharex_ui::tooltip::show_now(ui, Id::new("stats-chart-tip"), at, opened, &tip);
            }
        }
        y - origin.y
    }

    /// The hovered element's tooltip (`HitTest`'s text).
    fn hover_tip(page: &Page, stats: &HistoryStats, key: HoverKey) -> Option<String> {
        Some(match key {
            HoverKey::Split(k) => {
                let t = stats.total;
                let (kind, count) = [(HistoryStatsKind::Image, t.images), (HistoryStatsKind::Recording, t.recordings), (HistoryStatsKind::Other, t.other)][k];
                format!("{}: {} ({})", Fmt::series_name(kind), page.fmt.number(count), hs::format_share(count, t.total()))
            }
            HoverKey::Day(ticks) => {
                let day = DateTime::from_ticks(ticks, stats.today.kind());
                let title = page.fmt.long_date_with_day(day);
                if stats.first_day.is_some_and(|f| day < f) {
                    format!("{title}\n{}", s::StatsWindow_BeforeHistory())
                } else {
                    page.fmt.tally_tooltip(&title, stats.get_day(day))
                }
            }
            HoverKey::Hour(h) => page.fmt.tally_tooltip(&hs::format_hour_span(h as u32), stats.hours[h]),
        })
    }

    /// The Total card: the count, the rate, the split bar and its legend.
    fn total_card(&self, page: &Page, at: Pos2, width: f32, stats: &HistoryStats, paint: bool) -> f32 {
        let ui = page.ui;
        let p = palette(ui.ctx());
        let tf = title_font();
        let big = font(32.0, 600);
        let hf = hint_font();
        let total = stats.total;
        let (per_day, n) = hs::rate(total.total(), stats.days_counted());
        let rate = (if per_day { s::StatsWindow_AboutADay() } else { s::StatsWindow_AboutAWeek() }).replace("{0}", &n);
        let left_h = page.lh(&tf) + 3.0 + page.lh(&big) + page.lh(&hf);
        let h = 1.0 + 12.0 + left_h + 12.0 + 1.0;
        if !paint {
            return h;
        }
        let card = Rect::from_min_size(at, vec2(width, h));
        page.card(card);
        let inner = card.shrink(13.0);
        let mut y = inner.min.y;
        page.text(pos2(inner.min.x, y), s::StatsWindow_Total(), tf.clone(), p.text, f32::INFINITY);
        y += page.lh(&tf) + 3.0;
        let total_text = page.fmt.number(total.total());
        let w1 = text_width(ui, &total_text, big.clone());
        page.text(pos2(inner.min.x, y), &total_text, big.clone(), p.text, f32::INFINITY);
        y += page.lh(&big);
        let w2 = text_width(ui, &rate, hf.clone());
        page.text(pos2(inner.min.x, y), &rate, hf.clone(), p.text_secondary, f32::INFINITY);
        let left_w = w1.max(w2).max(text_width(ui, s::StatsWindow_Total(), tf)).ceil();
        // The right column: the bar, 12, the legend, centred vertically.
        let rx = inner.min.x + left_w + 32.0;
        let rw = inner.max.x - rx;
        let legend_f = font(13.0, 400);
        let lh = page.lh(&legend_f);
        let right_h = 10.0 + 12.0 + lh;
        let ry = center_y(inner, right_h);
        let bar = Rect::from_min_size(pos2(rx, ry), vec2(rw, 10.0));
        split_bar(page, bar, total);
        let mut x = rx;
        let ly = ry + 22.0;
        for (kind, count) in [(HistoryStatsKind::Image, total.images), (HistoryStatsKind::Recording, total.recordings), (HistoryStatsKind::Other, total.other)] {
            let row = Rect::from_min_size(pos2(x, ly), vec2(0.0, lh));
            page.rounded(Rect::from_min_size(pos2(x, center_y(row, 10.0)), Vec2::splat(10.0)), 2.0, page.pal.series[Palette::kind(kind)]);
            x += 10.0 + 7.0;
            x += page.text(pos2(x, ly), Fmt::series_name(kind), legend_f.clone(), p.text, f32::INFINITY).ceil() + 7.0;
            x += page.text(pos2(x, ly), &page.fmt.number(count), font(13.0, 600), p.text, f32::INFINITY).ceil() + 7.0;
            let share = hs::format_share(count, total.total());
            let hy = center_y(row, page.lh(&hf));
            x += page.text(pos2(x, hy), &share, hf.clone(), p.text_secondary, f32::INFINITY).ceil() + 22.0;
        }
        h
    }

    /// Today, Past 7 days, Streak, Busiest day.
    fn highlights(&self, page: &Page, at: Pos2, width: f32, stats: &HistoryStats, paint: bool) -> f32 {
        let ui = page.ui;
        let p = palette(ui.ctx());
        let tf = title_font();
        let vf = font(26.0, 600);
        let hf = hint_font();
        let h = 1.0 + 12.0 + page.lh(&tf) + 3.0 + page.lh(&vf) + page.lh(&hf) + 12.0 + 1.0;
        if !paint {
            return h;
        }
        let fmt = &page.fmt;
        let summary = |t: HistoryStatsTally| -> String {
            if t.total() == 0 {
                return s::StatsWindow_NothingYet().to_string();
            }
            [(HistoryStatsKind::Image, t.images), (HistoryStatsKind::Recording, t.recordings), (HistoryStatsKind::Other, t.other)]
                .iter()
                .filter(|(_, c)| *c > 0)
                .map(|(k, c)| format!("{} {}", Fmt::series_name(*k), fmt.number(*c)))
                .collect::<Vec<_>>()
                .join(" · ")
        };
        let streak_note = if stats.longest_streak <= 0 {
            String::new()
        } else if stats.streak >= stats.longest_streak {
            s::StatsWindow_LongestYet().to_string()
        } else {
            s::StatsWindow_LongestStreak().replace("{0}", &fmt.number(stats.longest_streak)).replace("{1}", &fmt.date(stats.longest_streak_end))
        };
        let (busiest, busiest_note) = match stats.busiest_day {
            Some(d) => (fmt.number(stats.busiest_day_count), fmt.date_with_day(d)),
            None => ("—".to_string(), s::StatsWindow_NothingYet().to_string()),
        };
        let week = hs::week_change(stats.past_7_days, stats.previous_7_days);
        let week_note = match &week {
            None if stats.past_7_days == 0 => s::StatsWindow_NonePrevious7DaysEither(),
            None => s::StatsWindow_NoneInPrevious7Days(),
            Some(_) => s::StatsWindow_VsPrevious7Days(),
        };
        let cards = [
            (s::StatsWindow_Today(), fmt.number(stats.today_tally.total()), None, summary(stats.today_tally)),
            (s::StatsWindow_Past7Days(), fmt.number(stats.past_7_days), week.clone(), week_note.to_string()),
            (s::StatsWindow_Streak(), fmt.days(stats.streak), None, streak_note),
            (s::StatsWindow_BusiestDay(), busiest, None, busiest_note),
        ];
        let cw = (width - 3.0 * CARD_SPACING) / 4.0;
        for (i, (title, value, change, note)) in cards.into_iter().enumerate() {
            let card = Rect::from_min_size(pos2(at.x + i as f32 * (cw + CARD_SPACING), at.y), vec2(cw, h));
            page.card(card);
            let inner = card.shrink(13.0);
            let mut y = inner.min.y;
            page.text(pos2(inner.min.x, y), title, tf.clone(), p.text, inner.width());
            y += page.lh(&tf) + 3.0;
            page.text(pos2(inner.min.x, y), &value, vf.clone(), p.text, inner.width());
            y += page.lh(&vf);
            let mut x = inner.min.x;
            if let Some((up, text)) = change {
                let color = if up { p.success } else { p.error };
                x += page.text(pos2(x, y), &text, font(12.0, 600), color, inner.width()).ceil() + 5.0;
            }
            page.text(pos2(x, y), &note, hf.clone(), p.text_secondary, (inner.max.x - x).max(0.0));
        }
        h
    }

    /// Every day: the header (note, ramp legend, span picker) and the calendar.
    fn calendar_card(&mut self, page: &Page, at: Pos2, width: f32, stats: &HistoryStats, paint: bool) -> f32 {
        let ui = page.ui;
        let p = palette(ui.ctx());
        let inner_w = width - 26.0;
        let span = self.spans.get(self.span).copied().unwrap_or(CalendarSpan::PastYear);
        let layout = calendar_layout(stats, span, inner_w);
        let cal_h = layout.as_ref().map_or(0.0, |l| l.runs.len() as f32 * (18.0 + 7.0 * l.step - 3.0) + (l.runs.len() as f32 - 1.0) * 14.0);
        let header_h = 32.0;
        let h = 1.0 + 12.0 + header_h + 10.0 + cal_h + 12.0 + 1.0;
        if !paint {
            return h;
        }
        let card = Rect::from_min_size(at, vec2(width, h));
        page.card(card);
        let inner = card.shrink(13.0);
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), header_h));
        // The span picker (`ComboBox.form-control`), when there's a choice.
        let names: Vec<ComboItem> = self
            .spans
            .iter()
            .map(|s| {
                ComboItem::text(match s {
                    CalendarSpan::PastYear => s::StatsWindow_PastYear().to_string(),
                    CalendarSpan::AllYears => s::StatsWindow_AllYears().to_string(),
                    CalendarSpan::Year(y) => y.to_string(),
                })
            })
            .collect();
        let combo = ComboBox { padding: Thickness::hv(8.0, 4.0), enabled: true };
        let mut right = header.max.x;
        if self.spans.len() > 1 {
            let cw = combo.content_width(ui, &names[self.span..=self.span]).max(140.0);
            let cr = Rect::from_min_size(pos2(right - cw, center_y(header, 32.0)), vec2(cw, 32.0));
            if let Some(i) = combo.show(ui, cr, Id::new("stats-span"), &names, Some(self.span)) {
                self.span = i;
            }
            right = cr.min.x - 10.0;
        }
        // Less, the ramp, More.
        let lf = font(11.0, 400);
        let more_w = text_width(ui, s::StatsWindow_More(), lf.clone());
        let less_w = text_width(ui, s::StatsWindow_Less(), lf.clone());
        let ramp_w = 5.0 * 10.0 + 4.0 * 3.0;
        let mut x = right - more_w - 6.0 - ramp_w - 6.0 - less_w;
        let lh = page.lh(&lf);
        page.text(pos2(x, center_y(header, lh)), s::StatsWindow_Less(), lf.clone(), p.text_secondary, f32::INFINITY);
        x += less_w + 6.0;
        for (level, c) in page.pal.ramp.iter().enumerate() {
            page.rounded(Rect::from_min_size(pos2(x + level as f32 * 13.0, center_y(header, 10.0)), Vec2::splat(10.0)), 2.0, *c);
        }
        x += ramp_w + 6.0;
        page.text(pos2(x, center_y(header, lh)), s::StatsWindow_More(), lf, p.text_secondary, f32::INFINITY);
        // The title and the active days note, at the top.
        let tf = title_font();
        let tw = page.text(inner.min, s::StatsWindow_EveryDay(), tf.clone(), p.text, f32::INFINITY);
        if let Some((from, to)) = stats.calendar_range(span) {
            let days = 0.max(((to.ticks() - from.ticks()) / 864_000_000_000) as i32) + 1;
            let active = stats.count_days(from, to, |t| t.total() > 0);
            let note = s::StatsWindow_ActiveDays().replace("{0}", &page.fmt.number(active)).replace("{1}", &page.fmt.days(days));
            let hf = hint_font();
            let title_row = Rect::from_min_size(inner.min, vec2(0.0, page.lh(&tf)));
            page.text(pos2(inner.min.x + tw.ceil() + 10.0, center_y(title_row, page.lh(&hf))), &note, hf, p.text_secondary, f32::INFINITY);
        }
        if let Some(l) = layout {
            paint_calendar(page, pos2(inner.min.x, header.max.y + 10.0), inner_w, stats, &l);
        }
        h
    }

    /// Time of day: the header with the filter buttons, and the hour chart.
    fn hours_card(&mut self, page: &Page, at: Pos2, width: f32, stats: &HistoryStats, paint: bool) -> f32 {
        let h = 1.0 + 12.0 + 32.0 + 10.0 + 230.0 + 12.0 + 1.0;
        if !paint {
            return h;
        }
        let ui = page.ui;
        let p = palette(ui.ctx());
        let card = Rect::from_min_size(at, vec2(width, h));
        page.card(card);
        let inner = card.shrink(13.0);
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), 32.0));
        let filters = [(HistoryStatsFilter::All, s::StatsWindow_All()), (HistoryStatsFilter::Images, s::StatsWindow_Images()), (HistoryStatsFilter::Recordings, s::StatsWindow_Recordings())];
        let b = Button::compact(Thickness::hv(12.0, 4.0));
        let widths: Vec<f32> = filters.iter().map(|(_, t)| b.width(ui, t)).collect();
        let mut x = header.max.x - widths.iter().sum::<f32>() - 12.0;
        for (k, (f, text)) in filters.into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, center_y(header, 32.0)), vec2(widths[k], 32.0));
            if filter_button(ui, r, Id::new(("stats-filter", k)), text, self.filter == f).clicked() {
                self.filter = f;
            }
            x += widths[k] + 6.0;
        }
        let tf = title_font();
        let tw = page.text(inner.min, s::StatsWindow_TimeOfDay(), tf.clone(), p.text, f32::INFINITY);
        if let Some(hour) = stats.peak_hour(self.filter) {
            let hf = hint_font();
            let note = s::StatsWindow_BusiestTime().replace("{0}", &hs::format_hour_span(hour));
            let title_row = Rect::from_min_size(inner.min, vec2(0.0, page.lh(&tf)));
            page.text(pos2(inner.min.x + tw.ceil() + 10.0, center_y(title_row, page.lh(&hf))), &note, hf, p.text_secondary, f32::INFINITY);
        }
        let chart = Rect::from_min_size(pos2(inner.min.x, header.max.y + 10.0), vec2(inner.width(), 230.0));
        hour_chart(page, chart, stats, self.filter);
        h
    }
}

/// `Button.filter-button` (`.active`: the accent fill and its foreground).
fn filter_button(ui: &Ui, rect: Rect, id: Id, text: &str, active: bool) -> egui::Response {
    if !active {
        return Button::compact(Thickness::hv(12.0, 4.0)).show(ui, rect, id, text, true);
    }
    let p = palette(ui.ctx());
    let r = ui.interact(rect, id, egui::Sense::click());
    border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::accent(ui.ctx())), Some(p.border));
    let f = content_font(13.0, 400);
    let w = text_width(ui, text, f.clone());
    let inner = rect.shrink(1.0);
    let x = inner.min.x + ((inner.width() - w) / 2.0).round_ties_even();
    ui.painter().galley(pos2(x, center_y(inner, line_height(ui, &f))), galley(ui, text, f, p.accent_foreground, f32::INFINITY), p.accent_foreground);
    r
}

/// `StatsSplitBar`: a segment per kind there is any of, 2 apart, each at
/// least 4 wide; the hovered one in its peak colour. `HitTest`: the
/// segment within the gap of the pointer's x.
fn split_bar(page: &Page, bar: Rect, t: HistoryStatsTally) {
    if t.total() == 0 {
        page.rounded(bar, 3.0, page.pal.ramp[0]);
        return;
    }
    let parts: Vec<(usize, i32)> = [(0, t.images), (1, t.recordings), (2, t.other)].into_iter().filter(|(_, c)| *c > 0).collect();
    let available = bar.width() as f64 - 2.0 * (parts.len() as f64 - 1.0);
    let minimum = 4f64.min(available / parts.len() as f64);
    let scalable = available - minimum * parts.len() as f64;
    let mut x = bar.min.x as f64;
    let mut hit = None;
    for (k, count) in parts {
        let w = minimum + scalable * count as f64 / t.total() as f64;
        let seg = Rect::from_min_size(pos2(x as f32, bar.min.y), vec2(w as f32, bar.height()));
        let color = if page.hover == Some(HoverKey::Split(k)) { page.pal.peak[k] } else { page.pal.series[k] };
        page.rounded(seg, 3.0, color);
        if hit.is_none()
            && let Some(p) = page.pointer.filter(|p| bar.contains(*p))
            && p.x >= seg.min.x - 2.0
            && p.x <= seg.max.x + 2.0
        {
            hit = Some(k);
        }
        x += w + 2.0;
    }
    if let Some(k) = hit {
        page.hit.set(Some(HoverKey::Split(k)));
    }
}

/// A run of the calendar: its label, the week its first column starts,
/// and its days.
struct Run {
    label: Option<String>,
    week_start: DateTime,
    from: DateTime,
    to: DateTime,
}

struct CalLayout {
    runs: Vec<Run>,
    left: f32,
    step: f32,
    steps: [i32; 3],
}

fn ymd(y: i32, m: u32, d: u32, like: DateTime) -> DateTime {
    DateTime::new(chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(0, 0, 0).unwrap(), like.kind())
}

/// `StatsCalendar.CreateLayoutCore`.
fn calendar_layout(stats: &HistoryStats, span: CalendarSpan, width: f32) -> Option<CalLayout> {
    let first = stats.first_day?;
    if width <= 0.0 {
        return None;
    }
    let today = stats.today;
    let left: f64 = if span == CalendarSpan::AllYears { 40.0 } else { 32.0 };
    let room = width as f64 - left + 3.0;
    let mut runs = Vec::new();
    let step = if span == CalendarSpan::PastYear {
        let weeks = ((room / 11.0) as i32).clamp(1, 53);
        let step = 20f64.min(room / weeks as f64);
        let week_start = stats.get_week_start(today).add_days(-7 * (weeks as i64 - 1));
        runs.push(Run { label: None, week_start, from: week_start, to: today });
        step
    } else {
        let step = (room / 54.0).clamp(6.0, 20.0);
        let years: Vec<i32> = match span {
            CalendarSpan::AllYears => (first.year()..=today.year()).rev().collect(),
            CalendarSpan::Year(y) => vec![y],
            CalendarSpan::PastYear => unreachable!(),
        };
        for year in years {
            let from = ymd(year, 1, 1, today);
            let to = ymd(year, 12, 31, today).min(today);
            runs.push(Run { label: (span == CalendarSpan::AllYears).then(|| year.to_string()), week_start: stats.get_week_start(from), from, to });
        }
        step
    };
    let ranges: Vec<(DateTime, DateTime)> = runs.iter().map(|r| (r.from, r.to)).collect();
    let steps = stats.calendar_steps(&ranges);
    Some(CalLayout { runs, left: left as f32, step: step as f32, steps })
}

const TICKS_PER_DAY: i64 = 864_000_000_000;

fn days_between(a: DateTime, b: DateTime) -> i64 {
    (b.ticks() - a.ticks()) / TICKS_PER_DAY
}

/// `StatsCalendar.Render`, and its `HitTest` (`width`: the control's).
fn paint_calendar(page: &Page, origin: Pos2, width: f32, stats: &HistoryStats, l: &CalLayout) {
    // `HitTest`: the day under the pointer, in a run's grid and range.
    if let Some(p) = page.pointer
        && p.x >= origin.x + l.left
        && p.x < origin.x + width
    {
        let mut top = origin.y;
        for run in &l.runs {
            let grid_top = top + 18.0;
            let grid_bottom = grid_top + 7.0 * l.step - 3.0;
            if p.y >= grid_top && p.y < grid_bottom {
                let column = ((p.x - origin.x - l.left) / l.step) as i64;
                let row = (((p.y - grid_top) / l.step) as i64).min(6);
                let day = run.week_start.add_days(column * 7 + row);
                if day >= run.from && day <= run.to {
                    page.hit.set(Some(HoverKey::Day(day.ticks())));
                }
                break;
            }
            top = grid_bottom + 14.0;
        }
    }
    let ui = page.ui;
    let p = palette(ui.ctx());
    let culture = page.fmt.culture;
    let muted = page.muted();
    let size = l.step - 3.0;
    let mut top = origin.y;
    let small = chart_font(10.0, 400);
    let small_h = galley(ui, "Mg", small.clone(), muted, f32::INFINITY).size().y;
    for run in &l.runs {
        let grid_top = top + 18.0;
        if let Some(label) = &run.label {
            page.text(pos2(origin.x, top), label, chart_font(11.0, 600), p.text, f32::INFINITY);
        }
        for row in (0..7).step_by(2) {
            let name = &culture.abbreviated_day_names[((stats.first_day_of_week + row) % 7) as usize];
            page.text(pos2(origin.x, grid_top + row as f32 * l.step + (size - small_h) / 2.0), name, small.clone(), muted, f32::INFINITY);
        }
        let mut last_x = f32::NEG_INFINITY;
        let fd = run.from.naive();
        use chrono::Datelike;
        let (mut y, mut m) = (fd.year(), fd.month());
        loop {
            let month = ymd(y, m, 1, run.from);
            if month > run.to {
                break;
            }
            if month >= run.from {
                let x = origin.x + l.left + (days_between(run.week_start, month) / 7) as f32 * l.step;
                if x - last_x >= 3.0 * l.step {
                    page.text(pos2(x, top), &culture.abbreviated_month_names[m as usize - 1], small.clone(), muted, f32::INFINITY);
                    last_x = x;
                }
            }
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
        let first = stats.first_day.unwrap();
        let mut day = run.from;
        while day <= run.to {
            let index = days_between(run.week_start, day);
            let cell = Rect::from_min_size(pos2(origin.x + l.left + (index / 7) as f32 * l.step, grid_top + (index % 7) as f32 * l.step), Vec2::splat(size));
            if day < first {
                border::border(ui.painter(), cell, 3.0, 1.0, None, Some(page.pal.ramp[0]));
                day = day.add_days(1);
                continue;
            }
            let level = hs::calendar_level(stats.get_day(day).total(), l.steps);
            page.rounded(cell, 3.0, page.pal.ramp[level as usize]);
            if page.hover == Some(HoverKey::Day(day.ticks())) {
                // `new Pen(text, 1.5)` round the cell, inflated 1, corners 4.
                border::border(ui.painter(), cell.expand(1.0), 4.0, 1.5, None, Some(p.text));
            }
            day = day.add_days(1);
        }
        top = grid_top + 7.0 * l.step - 3.0 + 14.0;
    }
}

/// `StatsHourChart.Render`.
fn hour_chart(page: &Page, rect: Rect, stats: &HistoryStats, filter: HistoryStatsFilter) {
    let ui = page.ui;
    let p = palette(ui.ctx());
    let kind = if filter == HistoryStatsFilter::Recordings { 1 } else { 0 };
    let muted = page.muted();
    let values: Vec<i32> = stats.hours.iter().map(|t| t.get(filter)).collect();
    let peak = stats.peak_hour(filter);
    let (step, top) = hs::hour_chart_scale(values.iter().copied().max().unwrap_or(0));
    let small = chart_font(10.0, 400);
    let label_w = text_width(ui, &hs::format_compact(top), small.clone());
    let plot_left = rect.min.x + label_w + 12.0;
    let plot_right = rect.max.x;
    let plot_top = rect.min.y + 8.0;
    let plot_bottom = rect.max.y - 20.0 - 34.0;
    let slot = (plot_right - plot_left) / 24.0;
    let mut value = 0.0;
    while value <= top + step / 2.0 {
        let y = plot_bottom - (plot_bottom - plot_top) * (value / top) as f32;
        let ly = y.round();
        ui.painter().rect_filled(Rect::from_min_max(pos2(plot_left, ly), pos2(plot_right, ly + 1.0)), 0.0, p.separator);
        let text = hs::format_compact(value);
        let g = galley(ui, &text, small.clone(), muted, f32::INFINITY);
        let size = g.size();
        ui.painter().galley(pos2(plot_left - 8.0 - size.x, y - size.y / 2.0), g, muted);
        value += step;
    }
    // `HitTest`: the hour column under the pointer, down to the axis labels.
    if let Some(p) = page.pointer.filter(|p| rect.contains(*p))
        && p.x >= plot_left
        && p.x < plot_right
        && p.y <= plot_bottom + 20.0
    {
        let hour = (((p.x - plot_left) / slot) as i32).clamp(0, 23) as usize;
        page.hit.set(Some(HoverKey::Hour(hour)));
    }
    for (hour, &v) in values.iter().enumerate() {
        let col_left = plot_left + hour as f32 * slot;
        if page.hover == Some(HoverKey::Hour(hour)) {
            let column = Rect::from_min_max(pos2(col_left, plot_top), pos2(col_left + slot, plot_bottom));
            page.rounded(column, 3.0, p.control_background_hover);
        }
        if v > 0 {
            let gap = (slot * 0.22).max(2.0);
            let height = ((plot_bottom - plot_top) * v as f32 / top as f32).max(2.0);
            let bar = Rect::from_min_max(pos2(col_left + gap / 2.0, plot_bottom - height), pos2(col_left + slot - gap / 2.0, plot_bottom));
            let fill = if Some(hour as u32) == peak { page.pal.peak[kind] } else { page.pal.series[kind] };
            // `DataEndRoundedRect`: rounded at the top only.
            let r = 4f32.min(bar.width() / 2.0).min(bar.height());
            let cr = CornerRadius { nw: r.round() as u8, ne: r.round() as u8, sw: 0, se: 0 };
            ui.painter().rect_filled(bar, cr, fill);
        }
        if hour % 3 == 0 {
            let text = hs::format_hour(hour as u32);
            let g = galley(ui, &text, small.clone(), muted, f32::INFINITY);
            let w = g.size().x;
            ui.painter().galley(pos2(col_left + slot / 2.0 - w / 2.0, plot_bottom + 4.0), g, muted);
        }
    }
    let parts = [s::StatsWindow_Night(), s::StatsWindow_Morning(), s::StatsWindow_Afternoon(), s::StatsWindow_Evening()];
    let total: i32 = values.iter().sum();
    let quarter = (plot_right - plot_left) / 4.0;
    let parts_top = plot_bottom + 20.0 + 6.0;
    let name_f = chart_font(11.0, 400);
    let share_f = chart_font(11.0, 600);
    for (i, part) in parts.iter().enumerate() {
        let x0 = plot_left + i as f32 * quarter + 2.0;
        let x1 = plot_left + (i + 1) as f32 * quarter - 2.0;
        ui.painter().rect_filled(Rect::from_min_max(pos2(x0, parts_top - 1.0), pos2(x1, parts_top + 1.0)), 0.0, p.separator);
        let share = hs::format_share(values[i * 6..i * 6 + 6].iter().sum(), total);
        let nw = text_width(ui, part, name_f.clone());
        let sw = text_width(ui, &share, share_f.clone());
        let x = (x0 + x1 - nw - 4.0 - sw) / 2.0;
        page.text(pos2(x, parts_top + 6.0), part, name_f.clone(), muted, f32::INFINITY);
        page.text(pos2(x + nw + 4.0, parts_top + 6.0), &share, share_f.clone(), p.text, f32::INFINITY);
    }
}

/// A breakdown card's height: its header, its rows (26 each) or "No data".
fn list_card_height(page: &Page, rows: &[Row], empty_text: bool) -> f32 {
    let header = page.lh(&title_font());
    let body = if rows.is_empty() && empty_text { page.lh(&hint_font()) } else { 26.0 * rows.len() as f32 };
    1.0 + 12.0 + header + 10.0 + body + 12.0 + 1.0
}

/// A breakdown card: the title (and note), then a `StatsRow` each.
fn list_card(page: &Page, rect: Rect, title: &str, note: &str, rows: &[Row], empty_text: bool) {
    let ui = page.ui;
    let p = palette(ui.ctx());
    page.card(rect);
    let inner = rect.shrink(13.0);
    let tf = title_font();
    let tw = page.text(inner.min, title, tf.clone(), p.text, f32::INFINITY);
    let hf = hint_font();
    let header = Rect::from_min_size(inner.min, vec2(inner.width(), page.lh(&tf)));
    if !note.is_empty() {
        page.text(pos2(inner.min.x + tw.ceil() + 10.0, center_y(header, page.lh(&hf))), note, hf.clone(), p.text_secondary, f32::INFINITY);
    }
    let mut y = header.max.y + 10.0;
    if rows.is_empty() && empty_text {
        page.text(pos2(inner.min.x, y), s::StatsWindow_NoData(), hf, p.text_secondary, f32::INFINITY);
        return;
    }
    for (k, row) in rows.iter().enumerate() {
        let r = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner.width(), 26.0));
        if page.pointer.is_some_and(|p| r.contains(p)) {
            ui.painter().rect_filled(r, CornerRadius::same(4), p.control_background_hover);
        }
        sharex_ui::tooltip::show(ui, r, Id::new(("stats-row-tip", title, k)), &row.tooltip);
        let c = Rect::from_min_max(pos2(r.min.x + 4.0, r.min.y), pos2(r.max.x - 4.0, r.max.y));
        let lf = font(13.0, if row.peak { 600 } else { 400 });
        page.text(pos2(c.min.x, center_y(c, page.lh(&lf))), &row.label, lf, p.text, row.label_width);
        let vf = font(12.0, 400);
        let vw = text_width(ui, &row.value, vf.clone());
        let value_w = vw.ceil().max(44.0);
        page.text(pos2(c.max.x - vw, center_y(c, page.lh(&vf))), &row.value, vf, p.text_secondary, f32::INFINITY);
        // `StatsMeter`: the track, filled to the fraction (half its height at least).
        let mx0 = c.min.x + row.label_width + 10.0;
        let mx1 = c.max.x - value_w - 10.0;
        let meter = Rect::from_min_max(pos2(mx0, center_y(c, 10.0)), pos2(mx1, center_y(c, 10.0) + 10.0));
        page.rounded(meter, 3.0, page.pal.ramp[0]);
        let f = row.fraction.clamp(0.0, 1.0) as f32;
        if f > 0.0 {
            let fill = Rect::from_min_size(meter.min, vec2((meter.width() * f).max(5.0), 10.0));
            page.rounded(fill, 3.0, if row.peak { page.pal.peak[0] } else { page.pal.series[0] });
        }
        y += 26.0;
    }
}

impl StatsWindow {
    /// `RenderSnapshot`: `SnapshotPanel` (the page on the window's
    /// background, its padding, everything scrolled out of view too)
    /// without the title's buttons, at the window's width and scale.
    fn snapshot(&mut self, width: f32, ppp: f32, dark: bool) -> Result<image::RgbaImage, String> {
        let inner = width - 2.0 * PAGE_PADDING;
        let mut probe = sharex_ui::offscreen::Offscreen::new([1, 1], ppp, dark)?;
        let mut h = 0.0;
        probe.run(Vec::new(), &mut |ui| h = self.content(ui, pos2(0.0, 0.0), inner, false));
        drop(probe);
        let height = h + 2.0 * PAGE_PADDING;
        let size = [(width * ppp).round().max(1.0) as u32, (height * ppp).round().max(1.0) as u32];
        let mut screen = sharex_ui::offscreen::Offscreen::new(size, ppp, dark)?;
        self.hide_actions = true;
        let image = screen.render(|ui| {
            let p = palette(ui.ctx());
            ui.painter().rect_filled(ui.max_rect(), 0.0, p.background_main);
            self.content(ui, pos2(PAGE_PADDING, PAGE_PADDING), inner, true);
        });
        self.hide_actions = false;
        Ok(image)
    }
}

impl Dialog for StatsWindow {
    fn spec(&self) -> Spec {
        Spec::sized(crate::rs_title(s::StatsWindow_Title()), WIDTH, HEIGHT, MIN_WIDTH, MIN_HEIGHT)
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Cx) -> Vec<Effect> {
        if self.stats.is_none() && self.error.is_none() && !self.loading && !self.sync {
            self.refresh(ui.ctx());
        }
        if let Some(rx) = &self.loader
            && let Ok(r) = rx.try_recv()
        {
            self.loader = None;
            self.show(r);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ComboBox::is_open(ui.ctx(), Id::new("stats-span")) {
            self.effects.push(Effect::Close);
        }
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let width = full.width() - 2.0 * PAGE_PADDING;
        let content_h = self.content(ui, pos2(0.0, 0.0), width, false) + 2.0 * PAGE_PADDING;
        if ui.rect_contains_pointer(full) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, (content_h - full.height()).max(0.0));
        let origin = pos2(full.min.x + PAGE_PADDING, full.min.y + PAGE_PADDING - self.scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(full));
        child.set_clip_rect(full.intersect(ui.clip_rect()));
        self.content(&mut child, origin, width, true);
        crate::dialogs::layout::scroll_bar(ui, full, content_h, self.scroll, 16.0);
        if std::mem::take(&mut self.share) {
            // `OnShareClick`: the page as an image (`RenderSnapshot`) through
            // `UploadManager.RunImageTask(new TaskMetadata(image) { ProcessName
            // = "ShareX" }, TaskSettings.GetDefaultTaskSettings())`.
            let dark = ui.ctx().theme() == egui::Theme::Dark;
            match self.snapshot(full.width(), ui.ctx().pixels_per_point(), dark) {
                Ok(image) => {
                    let task = sharex_settings::TaskSettings::default_task_settings(&cx.settings.DefaultTaskSettings);
                    self.effects.push(Effect::RunImageTaskAs(image, Box::new(task), "ShareX"));
                }
                Err(e) => eprintln!("Stats › Share: {e}"),
            }
        }
        std::mem::take(&mut self.effects)
    }

    fn single(&self) -> Option<&'static str> {
        Some("stats")
    }
}

/// `--render stats`: the page over `--db`, `--scroll`ed, the hour chart's
/// `--state images|recordings`, the calendar's span `--select N`.
pub fn render_dialog(_name: &str, o: &Options) -> Result<Box<dyn Dialog>, String> {
    let mut w = StatsWindow::new();
    w.sync = true;
    w.refresh(&egui::Context::default());
    w.scroll = o.scroll;
    w.filter = match o.state.as_deref() {
        Some("images") => HistoryStatsFilter::Images,
        Some("recordings") => HistoryStatsFilter::Recordings,
        _ => HistoryStatsFilter::All,
    };
    if let Some(&i) = o.select.first() {
        w.span = i.min(w.spans.len().saturating_sub(1));
    }
    Ok(Box::new(w))
}
