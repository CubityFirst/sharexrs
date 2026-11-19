//! `ShareXClickerControl`: the About window's hidden clicker. Clicking the
//! logo (or Enter/Space on it) earns logos: the about texts hide, the stats
//! show under the logo (count, rate, the next goal and its progress),
//! floating "+n" texts and logo particles fly; at 10 lifetime logos the
//! store replaces the links (upgrades and generators, bought x1, x10 or as
//! many as affordable), and generators earn on a 33 ms timer.

use std::time::Instant;

use egui::{Color32, Id, Pos2, Rect, Vec2, pos2, vec2};
use sharex_ui::border::{Brush, border};
use sharex_ui::controls::{Button, wrap_lines};
use sharex_ui::icons;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};

use crate::clicker::{self, BUILDING_MILESTONES, Building, Game, Upgrade};

const MAX_PARTICLES: usize = 200;
const MAX_PASSIVE_PARTICLES_PER_SECOND: f64 = 140.0;
const PASSIVE_ANIMATION_INTERVAL: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuyMode {
    One,
    Ten,
    Max,
}

/// Which canvas a floating text is on (`_overlay` in the logo grid, or the
/// window-wide `_particleOverlay`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Canvas {
    Logo,
    Window,
}

struct Floating {
    canvas: Canvas,
    text: String,
    size: f32,
    left: f32,
    start_top: f32,
    lifetime: f64,
    age: f64,
}

struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rotation: f64,
    lifetime: f64,
    age: f64,
}

/// A tiny xorshift for the effects' randomness (`Random`).
struct Rng(u64);

impl Rng {
    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `Next(min, max)`: `min..max`.
    fn range(&mut self, min: i32, max: i32) -> i32 {
        min + (self.next_f64() * (max - min) as f64) as i32
    }
}

pub struct Clicker {
    pub game: Game,
    pub activated: bool,
    pub store_visible: bool,
    buy_mode: BuyMode,
    press_until: Option<Instant>,
    floating: Vec<Floating>,
    particles: Vec<Particle>,
    last_tick: Instant,
    passive_visual_logos: f64,
    passive_cooldown: f64,
    passive_carry: f64,
    empire_celebrated: bool,
    rng: Rng,
    store_scroll: f32,
    logo_texture: Option<egui::TextureHandle>,
    /// Sizes from the last frame (`Bounds` of the canvases).
    window: Rect,
    logo_canvas: Rect,
}

impl Default for Clicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Clicker {
    pub fn new() -> Self {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x2545F491) | 1;
        Self {
            game: Game::new(clicker::State::default()),
            activated: false,
            store_visible: false,
            buy_mode: BuyMode::One,
            press_until: None,
            floating: Vec::new(),
            particles: Vec::new(),
            last_tick: Instant::now(),
            passive_visual_logos: 0.0,
            passive_cooldown: 0.0,
            passive_carry: 0.0,
            empire_celebrated: false,
            rng: Rng(seed),
            store_scroll: 0.0,
            logo_texture: None,
            window: Rect::NOTHING,
            logo_canvas: Rect::NOTHING,
        }
    }

    fn texture(&mut self, ctx: &egui::Context) -> egui::TextureId {
        self.logo_texture
            .get_or_insert_with(|| {
                let ci = egui::ColorImage::from_rgba_unmultiplied([64, 64], &crate::app_icon::rgba64());
                ctx.load_texture("about-clicker-logo", ci, egui::TextureOptions::LINEAR)
            })
            .id()
    }

    /// The logo scaled while pressed (`ScaleTransform(0.93)` for 0.12 s).
    pub fn logo_rect(&self, rect: Rect) -> Rect {
        if self.press_until.is_some_and(|t| Instant::now() < t) { Rect::from_center_size(rect.center(), rect.size() * 0.93) } else { rect }
    }

    /// `ClickLogo`: at `pos` (window points).
    pub fn click_logo(&mut self, pos: Pos2) {
        let gain = self.game.click();
        crate::sound::play_action_completed();
        if !self.activated {
            self.activated = true;
            self.passive_cooldown = PASSIVE_ANIMATION_INTERVAL;
            self.last_tick = Instant::now();
        }
        self.press_until = Some(Instant::now() + std::time::Duration::from_millis(120));
        let logo_pos = pos - self.logo_canvas.min.to_vec2();
        self.add_floating(Canvas::Logo, logo_pos, format!("+{}", clicker::format_number(gain)), 24.0, 0.65);
        self.add_particles(gain.ceil() as usize, Some(pos));
        if self.game.state.store_discovered && !self.store_visible {
            self.store_visible = true;
        }
    }

    /// `OnTimerTick`: the generators, the effects; returns when to tick next.
    pub fn tick(&mut self) {
        if !self.activated {
            return;
        }
        let now = Instant::now();
        let mut elapsed = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;
        if elapsed <= 0.0 || elapsed > 2.0 {
            elapsed = 0.0;
        }
        let earned = self.game.tick(elapsed);
        self.update_effects(elapsed);
        self.add_passive(earned, elapsed);
        if self.game.state.store_discovered && !self.store_visible {
            self.store_visible = true;
        }
        // `UpdateGoal`'s celebration.
        if self.game.state.lifetime_logos >= clicker::EMPIRE_GOAL_AT && !self.empire_celebrated {
            self.empire_celebrated = true;
            let c = self.window.center();
            self.add_particles(80, Some(c));
            self.add_floating(Canvas::Window, c - self.window.min.to_vec2(), "Empire complete!".into(), 24.0, 1.5);
        }
    }

    fn add_floating(&mut self, canvas: Canvas, pos: Pos2, text: String, size: f32, lifetime: f64) {
        let area = match canvas {
            Canvas::Logo => self.logo_canvas,
            Canvas::Window => self.window,
        };
        let estimated = (text.chars().count() as f32 * size * 0.55).max(40.0);
        let left = (pos.x - estimated / 2.0 + self.rng.range(-8, 9) as f32).clamp(0.0, (area.width() - estimated).max(0.0));
        let top = (pos.y - 16.0).clamp(8.0, (area.height() - 40.0).max(8.0));
        self.floating.push(Floating { canvas, text, size, left, start_top: top, lifetime, age: 0.0 });
    }

    fn add_particles(&mut self, count: usize, origin: Option<Pos2>) {
        for _ in 0..count.min(MAX_PARTICLES) {
            if self.particles.len() >= MAX_PARTICLES {
                self.particles.remove(0);
            }
            let start = origin.unwrap_or_else(|| self.random_logo_point());
            let lifetime = 2.6 + self.rng.next_f64() * 0.6;
            let p = Particle {
                x: (start.x - self.window.min.x) as f64,
                y: (start.y - self.window.min.y) as f64,
                vx: self.rng.next_f64() * 140.0 - 70.0,
                vy: -40.0 - self.rng.next_f64() * 75.0,
                rotation: self.rng.next_f64() * 200.0 - 100.0,
                lifetime,
                age: 0.0,
            };
            self.particles.push(p);
        }
    }

    /// `RandomLogoPoint`: within 65 of the logo's centre (window points).
    fn random_logo_point(&mut self) -> Pos2 {
        let origin = self.logo_canvas.min + vec2((self.logo_canvas.width() - 160.0) / 2.0, 50.0);
        let angle = self.rng.next_f64() * std::f64::consts::TAU;
        let distance = self.rng.next_f64().sqrt();
        pos2(origin.x + 80.0 + (angle.cos() * distance * 65.0) as f32, origin.y + 80.0 + (angle.sin() * distance * 65.0) as f32)
    }

    fn add_passive(&mut self, earned: f64, elapsed: f64) {
        if earned <= 0.0 {
            return;
        }
        self.passive_carry += earned.min(MAX_PASSIVE_PARTICLES_PER_SECOND * elapsed);
        let n = self.passive_carry.floor();
        if n > 0.0 {
            self.passive_carry -= n;
            self.add_particles(n as usize, None);
        }
        self.passive_visual_logos += earned;
        self.passive_cooldown = (self.passive_cooldown - elapsed).max(0.0);
        if self.passive_visual_logos < 1.0 || self.passive_cooldown > 0.0 {
            return;
        }
        let shown = self.passive_visual_logos.floor();
        self.passive_visual_logos -= shown;
        self.passive_cooldown = PASSIVE_ANIMATION_INTERVAL;
        let at = pos2(120.0 + self.rng.range(-35, 36) as f32, 135.0 + self.rng.range(-20, 21) as f32);
        self.add_floating(Canvas::Logo, at, format!("+{}", clicker::format_number(shown)), 24.0, 0.65);
    }

    fn update_effects(&mut self, elapsed: f64) {
        for f in &mut self.floating {
            f.age += elapsed;
        }
        self.floating.retain(|f| f.age < f.lifetime);
        let (w, h) = (self.window.width() as f64, self.window.height() as f64);
        for p in &mut self.particles {
            p.age += elapsed;
            p.x += p.vx * elapsed;
            p.y += p.vy * elapsed;
            p.vy += 500.0 * elapsed;
        }
        self.particles.retain(|p| !(p.age >= p.lifetime || p.y >= h - 15.0 || p.x < -20.0 || p.x > w + 20.0));
    }

    /// The stats under the logo (`_statsPanel` at 5,225 in the logo grid, 230 wide).
    pub fn stats(&mut self, ui: &egui::Ui, logo_box: Rect) {
        self.logo_canvas = logo_box;
        if !self.activated {
            return;
        }
        let p = palette(ui.ctx());
        let x0 = logo_box.min.x + 5.0;
        let mut y = logo_box.min.y + 225.0;
        let w = 230.0;
        let g = &self.game;
        let lines: [(String, egui::FontId, Color32); 3] = [
            (format!("{} logos", clicker::format_number(g.state.logos)), font(24.0, 700), p.accent_text),
            (format!("+{}/sec  •  {} per click", clicker::format_number(g.logos_per_second()), clicker::format_number(g.click_value())), font(12.0, 600), p.text_secondary.gamma_multiply(0.75)),
            (g.goal().0, font(11.0, 600), p.text_secondary.gamma_multiply(0.75)),
        ];
        let top = y;
        for (text, f, color) in lines {
            let lh = line_height(ui, &f);
            for line in wrap_lines(ui, &text, &f, w) {
                let tw = text_width(ui, &line, f.clone());
                ui.painter().galley(pos2(x0 + ((w - tw) / 2.0).round_ties_even(), y), galley(ui, &line, f.clone(), color, f32::INFINITY), color);
                y += lh;
            }
            y += 4.0;
        }
        // `ProgressBar Height="4"`: Fluent's track and the accent.
        let bar = Rect::from_min_size(pos2(x0, y), vec2(w, 4.0));
        let f = sharex_ui::theme::fluent(ui.ctx());
        border(ui.painter(), bar, 2.0, 0.0, Some(Brush::Solid(f.base_low)), None);
        let done = (w * g.goal().1 as f32).round();
        if done > 0.0 {
            border(ui.painter(), Rect::from_min_size(bar.min, vec2(done, 4.0)), 2.0, 0.0, Some(Brush::Solid(p.accent_start)), None);
        }
        let tip = format!(
            "{} lifetime logos • {} clicks • {} generators",
            clicker::format_number(g.state.lifetime_logos),
            clicker::group(g.state.manual_clicks),
            clicker::group(g.total_buildings())
        );
        sharex_ui::tooltip::show(ui, Rect::from_min_max(pos2(x0, top), pos2(x0 + w, y + 4.0)), Id::new("clicker-stats-tip"), &tip);
        // The logo's floating texts.
        self.paint_floating(ui, Canvas::Logo, logo_box);
    }

    fn paint_floating(&self, ui: &egui::Ui, canvas: Canvas, area: Rect) {
        let p = palette(ui.ctx());
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new(("clicker-floating", canvas as u8))));
        for f in self.floating.iter().filter(|f| f.canvas == canvas) {
            let opacity = (1.0 - f.age / f.lifetime).max(0.0) as f32;
            let top = f.start_top - (f.age * 34.0) as f32;
            let color = p.accent_text.gamma_multiply(opacity);
            painter.galley(area.min + vec2(f.left, top), galley(ui, &f.text, font(f.size, 600), color, f32::INFINITY), color);
        }
    }

    /// The window-wide overlay: particles and the window's floating texts.
    pub fn overlay(&mut self, ui: &egui::Ui, window: Rect) {
        self.window = window;
        if !self.activated {
            return;
        }
        let tex = self.texture(ui.ctx());
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("clicker-particles")));
        for p in &self.particles {
            let opacity = if p.age < p.lifetime - 0.35 { 1.0 } else { ((p.lifetime - p.age) / 0.35).max(0.0) };
            let rect = Rect::from_min_size(window.min + vec2(p.x as f32, p.y as f32), Vec2::splat(15.0));
            let mut mesh = egui::Mesh::with_texture(tex);
            mesh.add_rect_with_uv(rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE.gamma_multiply(opacity as f32));
            mesh.rotate(egui::emath::Rot2::from_angle((p.rotation * p.age).to_radians() as f32), rect.center());
            painter.add(mesh);
        }
        self.paint_floating(ui, Canvas::Window, window);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }

    fn buy_quantity(&self, b: &Building) -> i32 {
        match self.buy_mode {
            BuyMode::One => 1,
            BuyMode::Ten => 10,
            BuyMode::Max => self.game.max_affordable(b),
        }
    }

    fn celebrate(&mut self, at: Pos2, message: String, particles: usize) {
        self.add_particles(particles, Some(at));
        self.add_floating(Canvas::Window, at - self.window.min.to_vec2(), message, 17.0, 1.1);
        crate::sound::play_action_completed();
    }

    /// The store (`ClickerHost`), in place of the links: `area` is column 1.
    pub fn store(&mut self, ui: &mut egui::Ui, area: Rect) {
        let p = palette(ui.ctx());
        // `Border Padding="18,12,8,12"`, the `ScrollViewer`, a `StackPanel`
        // `Spacing="10" Margin="4,0,4,0"`.
        let view = Rect::from_min_max(pos2(area.min.x + 18.0, area.min.y + 12.0), pos2(area.max.x - 8.0, area.max.y - 12.0));
        let content_w = view.width() - 8.0;
        let x0 = view.min.x + 4.0;
        let ups = self.game.upgrades_to_display();
        let blds = self.game.buildings_to_display();
        let row_h = 48.0;
        let heading_h = 6.0 + line_height(ui, &font(16.0, 600));
        let header_h = line_height(ui, &font(18.0, 600)).max(18.0) + 2.0 + line_height(ui, &font(12.0, 600));
        let total = header_h + 10.0 + heading_h + 10.0 + (row_h + 10.0) * ups.len() as f32 + heading_h + (row_h + 10.0) * blds.len() as f32;
        if ui.rect_contains_pointer(view) {
            self.store_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.store_scroll = self.store_scroll.clamp(0.0, (total - view.height()).max(0.0));
        let clip = ui.clip_rect();
        ui.set_clip_rect(view.intersect(clip));
        let mut y = view.min.y - self.store_scroll;
        // The header: the title and subtitle, the buy mode on the right.
        let tf = font(18.0, 600);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(x0, y + ((line_height(ui, &tf) - 18.0) / 2.0).round()), Vec2::splat(18.0)), icon::Landmark, p.accent_icon);
        ui.painter().galley(pos2(x0 + 18.0 + 7.0, y), galley(ui, "ShareX Clicker", tf.clone(), p.accent_text, f32::INFINITY), p.accent_text);
        let sf = font(12.0, 600);
        let sub_y = y + line_height(ui, &tf) + 2.0;
        let dim = p.text_secondary.gamma_multiply(0.7);
        ui.painter().galley(pos2(x0, sub_y), galley(ui, "Build a tiny screenshot empire.", sf, dim, f32::INFINITY), dim);
        let gf = font(11.0, 600);
        let gw = text_width(ui, "Generator amount", gf.clone());
        let right = x0 + content_w;
        ui.painter().galley(pos2(right - gw, y), galley(ui, "Generator amount", gf.clone(), dim, f32::INFINITY), dim);
        let mut bx = right - (40.0 + 4.0 + 40.0 + 4.0 + 48.0);
        let by = y + line_height(ui, &gf) + 3.0;
        for (mode, label, w) in [(BuyMode::One, "x1", 40.0), (BuyMode::Ten, "x10", 40.0), (BuyMode::Max, "Max", 48.0)] {
            let r = Rect::from_min_size(pos2(bx, by), vec2(w, 30.0));
            let id = Id::new(("clicker-buy-mode", label));
            let clicked = if self.buy_mode == mode {
                // `Button.active`: the accent fill and its foreground.
                let resp = ui.interact(r, id, egui::Sense::click());
                border(ui.painter(), r, 4.0, 1.0, Some(Brush::accent(ui.ctx())), Some(p.border));
                let f = sharex_ui::controls::content_font(13.0, 400);
                let tw = text_width(ui, label, f.clone());
                ui.painter().galley(pos2(r.min.x + ((w - tw) / 2.0).round_ties_even(), center_y(r, line_height(ui, &f))), galley(ui, label, f, p.accent_foreground, f32::INFINITY), p.accent_foreground);
                resp.clicked()
            } else {
                Button { padding: sharex_ui::theme::Thickness::hv(4.0, 4.0), ..Button::default() }.show(ui, r, id, label, true).clicked()
            };
            sharex_ui::tooltip::show(ui, r, id.with("tip"), &format!("Buy {label}"));
            if clicked {
                self.buy_mode = mode;
            }
            bx += w + 4.0;
        }
        y += header_h.max(by + 30.0 - y) + 10.0;
        // Upgrades.
        y = self.heading(ui, x0, y, "Upgrades", icon::Sparkles) + 10.0;
        for u in ups {
            self.upgrade_row(ui, Rect::from_min_size(pos2(x0, y), vec2(content_w, row_h)), u);
            y += row_h + 10.0;
        }
        y = self.heading(ui, x0, y, "Generators", icon::Factory) + 10.0;
        for b in blds {
            self.building_row(ui, Rect::from_min_size(pos2(x0, y), vec2(content_w, row_h)), b);
            y += row_h + 10.0;
        }
        ui.set_clip_rect(clip);
        super::layout::scroll_bar(ui, view, total, self.store_scroll, 16.0);
    }

    fn heading(&self, ui: &egui::Ui, x: f32, y: f32, text: &str, key: &str) -> f32 {
        let p = palette(ui.ctx());
        let f = font(16.0, 600);
        let lh = line_height(ui, &f);
        let top = y + 6.0;
        icons::paint(ui.painter(), Rect::from_min_size(pos2(x, top + ((lh - 16.0) / 2.0).round()), Vec2::splat(16.0)), key, p.accent_icon);
        ui.painter().galley(pos2(x + 16.0 + 7.0, top), galley(ui, text, f, p.text_secondary, f32::INFINITY), p.text_secondary);
        top + lh
    }

    /// `CreateRow`: `Padding="12,7,8,7"`, corners 6, the panel colour;
    /// columns 26, 1.4*, 1.2*, Auto (the buy button, `MinWidth="94"`).
    #[allow(clippy::too_many_arguments)]
    fn row(&self, ui: &egui::Ui, rect: Rect, key: &str, title: &str, details: &str, button: &str, price: bool, enabled: bool, unlocked: bool) -> Option<Pos2> {
        let p = palette(ui.ctx());
        let alpha = if unlocked { 1.0 } else { 0.55 };
        let painter = ui.painter().clone();
        border(&painter, rect, 6.0, 0.0, Some(Brush::Solid(p.background_panel.gamma_multiply(alpha))), None);
        let inner = Rect::from_min_max(pos2(rect.min.x + 12.0, rect.min.y + 7.0), pos2(rect.max.x - 8.0, rect.max.y - 7.0));
        icons::paint(&painter, Rect::from_min_size(pos2(inner.min.x, center_y(inner, 18.0)), Vec2::splat(18.0)), key, p.accent_icon.gamma_multiply(alpha));
        let bf = sharex_ui::controls::content_font(13.0, 400);
        let price_w = if price { 12.0 + 6.0 + text_width(ui, button, bf.clone()) } else { text_width(ui, button, bf.clone()) };
        let bw = (price_w + 2.0 * 11.0 + 2.0).max(94.0);
        let button_rect = Rect::from_min_size(pos2(inner.max.x - bw, center_y(inner, 32.0)), vec2(bw, 32.0));
        let star = (inner.width() - 26.0 - 8.0 - bw).max(0.0);
        let (title_w, details_w) = (star * 1.4 / 2.6, star * 1.2 / 2.6);
        let tf = font(15.0, 600);
        let text = p.text_secondary.gamma_multiply(alpha);
        painter.galley(pos2(inner.min.x + 26.0, center_y(inner, line_height(ui, &tf))), galley(ui, title, tf, text, title_w), text);
        let df = font(13.0, 600);
        let dw = text_width(ui, details, df.clone()).min(details_w);
        let dim = text.gamma_multiply(0.75);
        painter.galley(pos2(inner.min.x + 26.0 + title_w + details_w - dw, center_y(inner, line_height(ui, &df))), galley(ui, details, df, dim, details_w), dim);
        let b = Button::default();
        let r = b.show(ui, button_rect, Id::new(("clicker-row", title)), "", enabled);
        let fg = if enabled { p.text } else { p.text_secondary.gamma_multiply(0.5) };
        let mut x = button_rect.min.x + ((bw - price_w) / 2.0).round_ties_even();
        if price {
            let logo = Rect::from_min_size(pos2(x, center_y(button_rect, 12.0)), Vec2::splat(12.0));
            crate::app_icon::paint(ui, logo);
            x += 18.0;
        }
        painter.galley(pos2(x, center_y(button_rect, line_height(ui, &bf))), galley(ui, button, bf, fg, f32::INFINITY), fg);
        (r.clicked() && enabled).then(|| button_rect.center())
    }

    fn upgrade_row(&mut self, ui: &egui::Ui, rect: Rect, u: &Upgrade) {
        let unlocked = self.game.upgrade_unlocked(u);
        let purchased = self.game.is_purchased(u);
        let key = if unlocked { u.icon } else { icon::Lock };
        let details = if unlocked { u.effect.to_owned() } else { format!("Unlock at {} logos", clicker::format_number(u.unlock_at)) };
        let (label, price) = if purchased {
            ("Owned".to_owned(), false)
        } else if unlocked {
            (clicker::format_number(u.cost), true)
        } else {
            ("Locked".to_owned(), false)
        };
        let enabled = self.game.can_buy_upgrade(u);
        if let Some(at) = self.row(ui, rect, key, u.name, &details, &label, price, enabled, unlocked)
            && self.game.buy_upgrade(u)
        {
            self.celebrate(at, format!("{} unlocked!", u.name), 20);
        }
    }

    fn building_row(&mut self, ui: &egui::Ui, rect: Rect, b: &Building) {
        let unlocked = self.game.building_unlocked(b);
        let key = if unlocked { b.icon } else { icon::Lock };
        let title = format!("{} ({})", b.name, self.game.owned(b));
        let details = if !unlocked {
            format!("Unlock at {} logos", clicker::format_number(b.unlock_at))
        } else {
            let each = clicker::format_number(self.game.building_production(b));
            match self.game.next_milestone(b) {
                Some(m) => format!("+{each}/sec each • 2x at {m}"),
                None => format!("+{each}/sec each • max boost"),
            }
        };
        let quantity = self.buy_quantity(b);
        let shown = quantity.max(1);
        let cost = self.game.building_cost(b, shown);
        let price = if shown > 1 { format!("x{shown}  {}", clicker::format_number(cost)) } else { clicker::format_number(cost) };
        let (label, is_price) = if unlocked { (price, true) } else { ("Locked".to_owned(), false) };
        let enabled = quantity > 0 && self.game.can_buy_building(b, quantity);
        if let Some(at) = self.row(ui, rect, key, &title, &details, &label, is_price, enabled, unlocked) {
            let before = self.game.owned(b);
            if self.game.buy_building(b, quantity) {
                let after = self.game.owned(b);
                let reached = BUILDING_MILESTONES.iter().rev().find(|m| before < **m && after >= **m).copied();
                let message = match reached {
                    Some(_) => format!("{} {}x boost!", b.name, self.game.milestone_multiplier(b)),
                    None => format!("Bought {quantity} {}", b.name),
                };
                self.celebrate(at, message, if reached.is_some() { 24 } else { 8 });
            }
        }
    }
}
