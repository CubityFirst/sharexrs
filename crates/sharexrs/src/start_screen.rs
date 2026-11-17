//! The Start screen ("Welcome to ShareX"): `ShareX/Presentation/StartScreen`
//! (`StartScreenWindow.axaml`, `StartScreenViewModel.cs`). Shown before
//! the main window while `ShowStartScreen` is set; each choice changes the
//! settings, and the app, as it's made.

use egui::{Color32, CornerRadius, Id, Rect, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use sharex_settings::{ApplicationConfig, IconPack, SupportedLanguage};
use sharex_strings::sharex as s;
use sharex_ui::controls::{self, Button, CheckBox, ColorButton, ComboBox, ComboItem, ToggleSwitch};
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{Thickness, palette};
use sharex_ui::widgets::{font, line_height};
use sharex_ui::icons;

use crate::appearance;
use crate::startup;

/// `Width="700"`.
pub const WIDTH: f32 = 700.0;

/// egui draws 14px Segoe UI glyphs 1px lower in their line box than
/// DirectWrite does (measured against ShareX's Start screen; 12, 13, 15 and
/// 24px agree), so the 14px labels move up by that much.
const FONT14_NUDGE: f32 = -1.0;

/// The accent presets: ShareX's default first, so a fresh install starts
/// with it selected (`AccentOptions`).
const ACCENTS: [&str; 8] = ["#09AAFF", "#6366F1", "#8B5CF6", "#EC4899", "#EF4444", "#F97316", "#10B981", "#64748B"];

/// What a frame of the Start screen asks of the app.
#[derive(Default)]
pub struct Output {
    /// A theme option changed: theme, accent, icon pack or spacing.
    pub appearance: bool,
    /// "Get started" (or Enter): the window closes.
    pub close: bool,
}

#[derive(Default)]
pub struct StartScreen {
    /// The ScrollViewer's offset, when the screen is too short for the
    /// whole window (`FitToWorkingArea`).
    pub scroll: f32,
    color_open: bool,
    /// `RefreshStartWithWindows`, read when the screen opens and after a change.
    startup: Option<startup::StartupView>,
}

/// `RemoveTrailingColon`.
fn strip_colon(text: &str) -> &str {
    text.trim_end().trim_end_matches([':', '：']).trim_end()
}

/// The three theme tiles' values (`ThemeOptions`).
#[derive(Clone, Copy, PartialEq)]
enum ThemeChoice {
    System,
    Light,
    Dark,
}

fn theme_choice(settings: &ApplicationConfig) -> ThemeChoice {
    let o = settings.ThemeOptions.clone().unwrap_or_default();
    if o.UseSystemTheme {
        ThemeChoice::System
    } else if o.Theme.eq_ignore_ascii_case("Light") {
        ThemeChoice::Light
    } else {
        ThemeChoice::Dark
    }
}

impl StartScreen {
    fn startup(&mut self) -> &startup::StartupView {
        self.startup.get_or_insert_with(startup::view)
    }

    /// The window's height for its content (`SizeToContent="Height"`).
    pub fn content_height(&mut self, ui: &Ui, settings: &ApplicationConfig) -> f32 {
        let layout = self.layout(ui, settings);
        layout.content_bottom + layout.footer_height
    }

    fn layout(&mut self, ui: &Ui, _settings: &ApplicationConfig) -> Layout {
        let inner_w = WIDTH - 64.0;
        let title_font = font(24.0, 700);
        let title_h = controls::wrapped(ui, pos2(0.0, 0.0), inner_w, s::StartScreen_ThankYou(), title_font.clone(), Color32::WHITE, false);
        let section_h = line_height(ui, &font(14.0, 600)) + 32.0;
        let tile_text_h = line_height(ui, &font(13.0, 600));
        let theme_tile_h = 1.0 + 8.0 + 84.0 + 8.0 + tile_text_h + 10.0 + 1.0;
        // Icon pack tiles: 20px icons; spacing tiles: `MinHeight="22"`.
        let icon_tile_h = 1.0 + 12.0 + tile_text_h.max(20.0) + 12.0 + 1.0;
        let small_tile_h = 1.0 + 12.0 + tile_text_h.max(22.0) + 12.0 + 1.0;
        let mut y = 32.0;
        let logo = y;
        y += 72.0 + 16.0;
        let title = y;
        y += title_h + 6.0;
        let theme_section = y;
        y += section_h;
        let theme_tiles = y;
        y += theme_tile_h;
        let accent_section = y;
        y += section_h;
        let accent_row = y;
        y += 40.0;
        let icon_section = y;
        y += section_h;
        let icon_tiles = y;
        y += icon_tile_h;
        let spacing_section = y;
        y += section_h;
        let spacing_tiles = y;
        y += small_tile_h;
        y += 24.0;
        let list = y;
        let title14 = font(14.0, 600);
        let lang_h = (1.0 + 6.0 + line_height(ui, &ComboBox::item_font()) + 6.0 + 1.0).max(34.0).max(line_height(ui, &title14));
        let row1 = lang_h.max(56.0);
        let startup_text = self.startup().text;
        let text_w = inner_w - 32.0 - 22.0 - 24.0 - ToggleSwitch::SIZE.x;
        let row2 = controls::wrapped(ui, pos2(0.0, 0.0), text_w, startup_text, title14, Color32::WHITE, false).max(32.0).max(56.0);
        y += 1.0 + row1 + 1.0 + row2 + 1.0;
        let note_w = inner_w - 16.0 - Button::primary(Thickness::hv(24.0, 8.0), 8.0).width(ui, s::StartScreen_GetStarted());
        let note_h = controls::wrapped(ui, pos2(0.0, 0.0), note_w, s::StartScreen_SettingsNote(), font(12.0, 400), Color32::WHITE, false);
        let footer_height = 20.0 + note_h.max(40.0) + 24.0;
        Layout {
            logo,
            title,
            theme_section,
            theme_tiles,
            theme_tile_h,
            accent_section,
            accent_row,
            icon_section,
            icon_tiles,
            spacing_section,
            spacing_tiles,
            icon_tile_h,
            small_tile_h,
            list,
            row1,
            row2,
            content_bottom: y,
            footer_height,
            note_w,
        }
    }

    /// Draws the Start screen over all of `ui`, changing `settings`.
    pub fn ui(&mut self, ui: &mut Ui, settings: &mut ApplicationConfig) -> Output {
        let mut out = Output::default();
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let l = self.layout(ui, settings);
        // The footer is docked at the bottom; the rest scrolls above it.
        let footer = Rect::from_min_max(pos2(full.min.x, full.max.y - l.footer_height), full.max);
        let view = Rect::from_min_max(full.min, pos2(full.max.x, footer.min.y));
        let max_scroll = (l.content_bottom - view.height()).max(0.0);
        if ui.rect_contains_pointer(view) && !ComboBox::is_open(ui.ctx(), Id::new("start-language")) {
            self.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let origin = pos2(full.min.x + 32.0, view.min.y - self.scroll);
        let inner_w = WIDTH - 64.0;
        {
            let mut content = ui.new_child(egui::UiBuilder::new().max_rect(view));
            content.set_clip_rect(view.intersect(ui.clip_rect()));
            self.content(&mut content, settings, &l, origin, inner_w, &mut out);
            if max_scroll > 0.0 {
                // Fluent's ScrollViewer: a thin indicator until hovered.
                let f = sharex_ui::theme::fluent(ui.ctx());
                let track = view.shrink2(vec2(0.0, 2.0));
                let h = (track.height() * view.height() / l.content_bottom).max(30.0);
                let y = track.min.y + (track.height() - h) * self.scroll / max_scroll;
                let bar = Rect::from_min_size(pos2(view.max.x - 4.0, y), vec2(2.0, h));
                content.painter().rect_filled(bar, CornerRadius::same(1), f.base_medium_low);
            }
        }
        // Footer: the note and "Get started" (`IsDefault`).
        let fx = full.min.x + 32.0;
        let frow = Rect::from_min_max(pos2(fx, footer.min.y + 20.0), pos2(full.max.x - 32.0, footer.max.y - 24.0));
        let button = Button::primary(Thickness::hv(24.0, 8.0), 8.0);
        let bw = button.width(ui, s::StartScreen_GetStarted());
        let bh = 40.0;
        let brect = Rect::from_min_size(pos2(frow.max.x - bw, controls_center(frow, bh)), vec2(bw, bh));
        let note_font = font(12.0, 400);
        let note_h = controls::wrapped(ui, pos2(0.0, 0.0), l.note_w, s::StartScreen_SettingsNote(), note_font.clone(), p.text_secondary, false);
        controls::wrapped(ui, pos2(fx, controls_center(frow, note_h)), l.note_w, s::StartScreen_SettingsNote(), note_font, p.text_secondary, true);
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter)) && !ComboBox::is_open(ui.ctx(), Id::new("start-language"));
        if button.show(ui, brect, Id::new("start-get-started"), s::StartScreen_GetStarted(), true).clicked() || enter {
            out.close = true;
        }
        out
    }

    fn content(&mut self, ui: &mut Ui, settings: &mut ApplicationConfig, l: &Layout, origin: egui::Pos2, inner_w: f32, out: &mut Output) {
        let p = palette(ui.ctx());
        let x = origin.x;
        let y = |v: f32| origin.y + v;
        // Logo and title.
        let logo = Rect::from_min_size(pos2(x + (inner_w - 72.0) / 2.0, y(l.logo)), Vec2::splat(72.0));
        crate::app_icon::paint(ui, logo);
        let title_font = font(24.0, 700);
        let lh = line_height(ui, &title_font);
        for (i, line) in controls::wrap_lines(ui, s::StartScreen_ThankYou(), &title_font, inner_w).iter().enumerate() {
            controls::text_centered_at(ui, x + inner_w / 2.0, y(l.title) + lh * i as f32, line, title_font.clone(), p.text_secondary);
        }

        // Theme.
        self.section(ui, x, y(l.theme_section), strip_colon(s::ApplicationSettingsWindow_ThemeLabel()));
        let choice = theme_choice(settings);
        let col_w = ((inner_w - 24.0) / 3.0).floor();
        let names = [
            (ThemeChoice::System, s::StartScreen_UseSystemSetting()),
            (ThemeChoice::Light, s::ApplicationSettingsWindow_Light()),
            (ThemeChoice::Dark, s::ApplicationSettingsWindow_Dark()),
        ];
        for (i, (value, name)) in names.into_iter().enumerate() {
            let rect = Rect::from_min_size(pos2(x + (col_w + 12.0) * i as f32, y(l.theme_tiles)), vec2(col_w, l.theme_tile_h));
            let (selected, hovered, clicked) = tile(ui, rect, Id::new(("start-theme", i)), choice == value);
            let _ = hovered;
            let mini = Rect::from_min_size(pos2(rect.center().x - 93.0, rect.min.y + 9.0), vec2(186.0, 84.0));
            mini_window(ui, mini, value);
            let ty = mini.max.y + 8.0;
            controls::text_centered_at(ui, rect.center().x, ty, name, font(13.0, 600), p.text_secondary);
            if clicked && !selected {
                let o = settings.theme_options();
                match value {
                    ThemeChoice::System => o.UseSystemTheme = true,
                    ThemeChoice::Light | ThemeChoice::Dark => {
                        o.Theme = if value == ThemeChoice::Light { "Light" } else { "Dark" }.into();
                        o.UseSystemTheme = false;
                    }
                }
                out.appearance = true;
            }
        }

        // Accent colour.
        self.section(ui, x, y(l.accent_section), strip_colon(s::ApplicationSettingsWindow_AccentColorLabel()));
        let o = settings.ThemeOptions.clone().unwrap_or_default();
        let can_edit = !o.UseSystemAccentColor;
        let accent = appearance::accent_color(&o);
        let row = Rect::from_min_size(pos2(x, y(l.accent_row)), vec2(inner_w, 40.0));
        for (i, hex) in ACCENTS.iter().enumerate() {
            let c = sharex_ui::accent::parse_hex(hex).unwrap();
            let r = Rect::from_min_size(pos2(x + 36.0 * i as f32, controls_center(row, 34.0)), Vec2::splat(34.0));
            let resp = ui.interact(r, Id::new(("start-accent", i)), if can_edit { egui::Sense::click() } else { egui::Sense::hover() });
            let selected = c == accent;
            let opacity = if can_edit { 1.0 } else { 0.4 };
            let ring = if selected {
                Some(p.text)
            } else if can_edit && resp.hovered() {
                Some(p.control_border)
            } else {
                None
            };
            if let Some(ring) = ring {
                ui.painter().circle_stroke(r.center(), 16.0, Stroke::new(2.0, ring.gamma_multiply(opacity)));
            }
            ui.painter().circle_filled(r.center(), 12.0, c.gamma_multiply(opacity));
            if can_edit && resp.clicked() && !selected {
                settings.theme_options().AccentColorHex = sharex_ui::accent::to_hex(c);
                out.appearance = true;
            }
        }
        let swatches_w = 34.0 * 8.0 + 2.0 * 7.0;
        let cb = Rect::from_min_size(pos2(x + swatches_w + 8.0, row.min.y), vec2(62.0, 40.0));
        let cb_resp = ColorButton { enabled: can_edit }.show(ui, cb, Id::new("start-accent-picker"), accent);
        let just_opened = cb_resp.clicked() && !self.color_open;
        if cb_resp.clicked() {
            self.color_open = !self.color_open;
        }
        if self.color_open && can_edit {
            let mut c = accent;
            let (changed, keep) = controls::color_popup(ui, Id::new("start-accent-popup"), cb, &mut c, just_opened);
            if changed {
                settings.theme_options().AccentColorHex = sharex_ui::accent::to_hex(c);
                out.appearance = true;
            }
            self.color_open = keep;
        } else {
            self.color_open = false;
        }
        // "Follow system accent color", right-aligned (`TextBlock` content).
        let check_text = s::ApplicationSettingsWindow_FollowSystemAccentColor();
        let check = CheckBox { text_font: font(13.0, 600), text_color: Some(p.text_secondary), ..CheckBox::new(check_text) };
        let avail = row.max.x - cb.max.x - 8.0;
        let cw = check.width(ui).min(avail);
        let crect = Rect::from_min_size(pos2(row.max.x - cw, controls_center(row, 32.0)), vec2(cw, 32.0));
        let mut follow = o.UseSystemAccentColor;
        if check.show(ui, crect, Id::new("start-follow-accent"), &mut follow).changed() {
            settings.theme_options().UseSystemAccentColor = follow;
            out.appearance = true;
        }

        // Icon pack.
        self.section(ui, x, y(l.icon_section), strip_colon(s::ApplicationSettingsWindow_IconPackLabel()));
        let half = ((inner_w - 12.0) / 2.0).floor();
        let packs = [(IconPack::Modern, s::ApplicationSettingsWindow_IconPackModern()), (IconPack::Legacy, s::ApplicationSettingsWindow_IconPackLegacy())];
        for (i, (pack, name)) in packs.into_iter().enumerate() {
            let rect = Rect::from_min_size(pos2(x + (half + 12.0) * i as f32, y(l.icon_tiles)), vec2(half, l.icon_tile_h));
            let (selected, _, clicked) = tile(ui, rect, Id::new(("start-pack", i)), o.IconPack == pack);
            let inner = Rect::from_min_max(pos2(rect.min.x + 15.0, rect.min.y + 13.0), pos2(rect.max.x - 15.0, rect.max.y - 13.0));
            let ui_pack = if pack == IconPack::Legacy { icons::IconPack::Legacy } else { icons::IconPack::Modern };
            for (k, key) in [icon::Camera, icon::ScreenRecording, icon::ImageEditor, icon::CloudUpload, icon::History].into_iter().enumerate() {
                let r = Rect::from_min_size(pos2(inner.min.x + 32.0 * k as f32, controls_center(inner, 20.0)), Vec2::splat(20.0));
                icons::paint_in_pack(ui.painter(), r, key, p.text, ui_pack);
            }
            let tw = sharex_ui::widgets::text_width(ui, name, font(13.0, 600));
            controls::label(ui, Rect::from_min_max(pos2(inner.max.x - tw, inner.min.y), inner.max), name, font(13.0, 600), p.text_secondary);
            if clicked && !selected {
                settings.theme_options().IconPack = pack;
                out.appearance = true;
            }
        }

        // Spacing.
        self.section(ui, x, y(l.spacing_section), s::ApplicationSettingsWindow_Spacing());
        let spacings = [(false, s::StartScreen_Spacious()), (true, s::ApplicationSettingsWindow_SpacingCompact())];
        for (i, (compact, name)) in spacings.into_iter().enumerate() {
            let rect = Rect::from_min_size(pos2(x + (half + 12.0) * i as f32, y(l.spacing_tiles)), vec2(half, l.small_tile_h));
            let (selected, _, clicked) = tile(ui, rect, Id::new(("start-spacing", i)), o.CompactMode == compact);
            let inner = Rect::from_min_max(pos2(rect.min.x + 15.0, rect.min.y + 13.0), pos2(rect.max.x - 15.0, rect.max.y - 13.0));
            // Three list rows, spaced as the option spaces them.
            let gap = if compact { 2.0 } else { 6.0 };
            let total = 4.0 * 3.0 + gap * 2.0;
            let top = controls_center(inner, total);
            for (k, w) in [72.0, 56.0, 64.0].into_iter().enumerate() {
                let bar = Rect::from_min_size(pos2(inner.min.x, top + (4.0 + gap) * k as f32), vec2(w, 4.0));
                ui.painter().rect_filled(bar, CornerRadius::same(2), p.text_secondary.gamma_multiply(0.6));
            }
            let tw = sharex_ui::widgets::text_width(ui, name, font(13.0, 600));
            controls::label(ui, Rect::from_min_max(pos2(inner.max.x - tw, inner.min.y), inner.max), name, font(13.0, 600), p.text_secondary);
            if clicked && !selected {
                // Compact mode is a spacing preset: it drops per-area values.
                let o = settings.theme_options();
                appearance::reset_spacing(o);
                o.CompactMode = compact;
                out.appearance = true;
            }
        }

        // General: language and run at startup.
        let list = Rect::from_min_size(pos2(x, y(l.list)), vec2(inner_w, 1.0 + l.row1 + 1.0 + l.row2 + 1.0));
        controls::card(ui, list, 10.0);
        let row1 = Rect::from_min_size(pos2(list.min.x + 17.0, list.min.y + 1.0), vec2(inner_w - 34.0, l.row1));
        let row2 = Rect::from_min_size(pos2(list.min.x + 17.0, row1.max.y + 1.0), vec2(inner_w - 34.0, l.row2));
        controls::divider(ui, Rect::from_min_max(pos2(list.min.x + 1.0, row1.max.y), pos2(list.max.x - 1.0, row1.max.y + 1.0)));
        let title14 = font(14.0, 600);
        // Language.
        icons::paint(ui.painter(), Rect::from_min_size(pos2(row1.min.x + 2.0, controls_center(row1, 18.0)), Vec2::splat(18.0)), icon::Language, p.accent_icon);
        let combo_w = 200.0;
        let text_x = row1.min.x + 22.0 + 12.0;
        let combo = Rect::from_min_size(pos2(row1.max.x - combo_w, controls_center(row1, 34.0)), vec2(combo_w, 34.0));
        controls::label(ui, Rect::from_min_max(pos2(text_x, row1.min.y + FONT14_NUDGE), pos2(combo.min.x - 12.0, row1.max.y + FONT14_NUDGE)), strip_colon(s::ApplicationSettingsWindow_LanguageLabel()), title14.clone(), p.text_secondary);
        let items = language_items(ui.ctx());
        let current = SupportedLanguage::ALL.iter().position(|l| *l == settings.Language);
        let combo_box = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        if let Some(i) = combo_box.show(ui, combo, Id::new("start-language"), &items, current) {
            // `SelectedLanguage`: the language changes at once, and so does
            // every string on screen.
            settings.Language = SupportedLanguage::ALL[i].clone();
            appearance::change_language(&settings.Language);
            self.startup = None;
        }
        // Run ShareX on startup.
        icons::paint(ui.painter(), Rect::from_min_size(pos2(row2.min.x + 2.0, controls_center(row2, 18.0)), Vec2::splat(18.0)), icon::Rocket, p.accent_icon);
        let toggle = Rect::from_min_size(pos2(row2.max.x - ToggleSwitch::SIZE.x, controls_center(row2, ToggleSwitch::SIZE.y)), ToggleSwitch::SIZE);
        let view = self.startup();
        let (text, enabled, mut on) = (view.text, view.enabled, view.checked);
        let text_w = toggle.min.x - 12.0 - text_x;
        let th = controls::wrapped(ui, pos2(0.0, 0.0), text_w, text, title14.clone(), p.text_secondary, false);
        controls::wrapped(ui, pos2(text_x, controls_center(row2, th) + FONT14_NUDGE), text_w, text, title14, p.text_secondary, true);
        if (ToggleSwitch { enabled }).show(ui, toggle.min, Id::new("start-startup"), &mut on).changed() {
            let state = if on { startup::StartupState::Enabled } else { startup::StartupState::Disabled };
            if let Err(e) = startup::set_state(state) {
                eprintln!("startup: {e}");
            }
            self.startup = None;
        }
    }

    /// `TextBlock.section`: 14 SemiBold, `Margin="2,22,0,10"`.
    fn section(&self, ui: &Ui, x: f32, y: f32, text: &str) {
        let p = palette(ui.ctx());
        let f = font(14.0, 600);
        let g = sharex_ui::widgets::galley(ui, text, f, p.text_secondary, f32::INFINITY);
        ui.painter().galley(pos2(x + 2.0, y + 22.0 + FONT14_NUDGE), g, p.text_secondary);
    }
}

struct Layout {
    logo: f32,
    title: f32,
    theme_section: f32,
    theme_tiles: f32,
    theme_tile_h: f32,
    accent_section: f32,
    accent_row: f32,
    icon_section: f32,
    icon_tiles: f32,
    spacing_section: f32,
    spacing_tiles: f32,
    icon_tile_h: f32,
    small_tile_h: f32,
    list: f32,
    row1: f32,
    row2: f32,
    content_bottom: f32,
    footer_height: f32,
    note_w: f32,
}

/// Where Avalonia centres something `h` tall in `rect` (whole pixels).
fn controls_center(rect: Rect, h: f32) -> f32 {
    sharex_ui::widgets::center_y(rect, h)
}

/// A `ListBoxItem` drawn as `Border.tile`: panel background (control hover
/// background under the pointer), 1px border, 10px corners; selected, a
/// 2px accent border over the same box (`Margin="-1"`).
fn tile(ui: &Ui, rect: Rect, id: Id, selected: bool) -> (bool, bool, bool) {
    let p = palette(ui.ctx());
    let r = ui.interact(rect, id, egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let hovered = r.hovered();
    let fill = if hovered { p.control_background_hover } else { p.background_panel };
    let radius = CornerRadius::same(10);
    if selected {
        // `Margin="-1"` grows the border out of its `ListBoxItem`, which
        // clips it: 1px of the 2px border shows outside the tile's box.
        let outer = rect.expand(1.0);
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_filled(outer, radius, fill);
        painter.rect_stroke(outer, radius, Stroke::new(2.0, p.accent_border), StrokeKind::Inside);
    } else {
        ui.painter().rect_filled(rect, radius, fill);
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, p.border), StrokeKind::Inside);
    }
    (selected, hovered, r.clicked())
}

/// The `MiniWindowTemplate` picture of a ShareX window, in the colours of
/// ShareXLight or ShareXDark whatever the theme; "System" is the two split
/// diagonally (`Clip="M 140,0 L 186,0 L 186,84 L 98,84 Z"`).
fn mini_window(ui: &Ui, rect: Rect, which: ThemeChoice) {
    let p = palette(ui.ctx());
    let svg = mini_svg(which, p.accent_start, p.accent_end);
    let key = format!("mini-{}-{:?}-{:?}", which as u8, p.accent_start, p.accent_end);
    icons::paint_svg_color(ui.painter(), rect, &key, &svg);
}

fn hex(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// One mini window as SVG (its fills, the accent gradients in user space as
/// Skia draws `RelativePoint` gradients).
fn mini_svg(which: ThemeChoice, start: Color32, end: Color32) -> String {
    let (a, b) = (hex(start), hex(end));
    let body = |dark: bool, id: &str| -> String {
        let (frame, frame_border, side, side_border, line) = if dark {
            ("#272727", "#3A3A3A", "#222222", "#333333", "#454545")
        } else {
            ("#F5F5F7", "#DBDBDF", "#FDFDFD", "#E5E5E8", "#D4D4D8")
        };
        let grad = |gid: &str, x: f32, y: f32, w: f32, h: f32| {
            format!(r#"<linearGradient id="{id}{gid}" gradientUnits="userSpaceOnUse" x1="{x}" y1="{y}" x2="{}" y2="{}"><stop offset="0" stop-color="{a}"/><stop offset="1" stop-color="{b}"/></linearGradient>"#, x + w, y + h)
        };
        // The frame's children sit inside its 1px border (`Border` lays its
        // child out inside `BorderThickness`), clipped to its inner corners.
        let mut s = String::new();
        s += &format!(r#"<defs><clipPath id="{id}c"><rect x="1" y="1" width="184" height="82" rx="5"/></clipPath>{}{}</defs>"#, grad("g1", 8.0, 11.0, 14.0, 4.0), grad("g2", 39.0, 49.0, 46.0, 13.0));
        s += &format!(r#"<rect x="0.5" y="0.5" width="185" height="83" rx="5.5" fill="{frame}" stroke="{frame_border}" stroke-width="1"/>"#);
        s += &format!(r#"<g clip-path="url(#{id}c)">"#);
        s += &format!(r#"<rect x="1" y="1" width="28" height="82" fill="{side}"/><rect x="28" y="1" width="1" height="82" fill="{side_border}"/>"#);
        s += &format!(r#"<rect x="8" y="11" width="14" height="4" rx="2" fill="url(#{id}g1)"/>"#);
        for k in 1..4 {
            s += &format!(r#"<rect x="8" y="{}" width="14" height="4" rx="2" fill="{line}"/>"#, 11 + 10 * k);
        }
        for (k, w) in [64, 96, 48].into_iter().enumerate() {
            s += &format!(r#"<rect x="39" y="{}" width="{w}" height="5" rx="2.5" fill="{line}"/>"#, 11 + 11 * k);
        }
        s += &format!(r#"<rect x="39" y="49" width="46" height="13" rx="4" fill="url(#{id}g2)"/>"#);
        s += "</g>";
        s
    };
    let inner = match which {
        ThemeChoice::Light => body(false, "l"),
        ThemeChoice::Dark => body(true, "d"),
        ThemeChoice::System => format!(
            r#"{}<defs><clipPath id="split"><path d="M 140,0 L 186,0 L 186,84 L 98,84 Z"/></clipPath></defs><g clip-path="url(#split)">{}</g>"#,
            body(false, "l"),
            body(true, "d")
        ),
    };
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="186" height="84" viewBox="0 0 186 84">{inner}</svg>"#)
}

/// `LanguageOptions`: every language, with its flag (Automatic: the
/// language icon).
pub fn language_items(ctx: &egui::Context) -> Vec<ComboItem> {
    SupportedLanguage::ALL
        .iter()
        .map(|l| ComboItem {
            text: l.localized_description(),
            icon: (*l == SupportedLanguage::Automatic).then_some(icon::Language),
            flag: l.flag_code().and_then(|c| icons::flag(ctx, &c, 16.0, 12.0)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_trailing_colons() {
        assert_eq!(strip_colon("Language:"), "Language");
        assert_eq!(strip_colon("言語："), "言語");
        assert_eq!(strip_colon("Theme"), "Theme");
    }

    #[test]
    fn mini_svgs_parse() {
        for which in [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark] {
            let svg = mini_svg(which, Color32::from_rgb(9, 170, 255), Color32::from_rgb(8, 153, 230));
            assert!(svg.starts_with("<svg"));
        }
    }
}
