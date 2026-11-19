//! The main window: `ShareX/Presentation/MainWindow/MainWindow.axaml`.
//!
//! A borderless window with ShareX's own 38px title bar (icon, title, theme
//! toggle, always-on-top, minimize, maximize, close), the navigation
//! sidebar (`MainMenuBuilder.BuildNavigation`) and the task area
//! ([`crate::task_view`]), which shows the "ShareX hotkeys" panel while
//! there are no tasks. A sidebar
//! entry with a chevron opens its menu (`OnNavigationClick`), built by
//! [`crate::main_menu`].

use egui::{Align, CornerRadius, Layout, Rect, Sense, Stroke, Ui, UiBuilder, Vec2, pos2, vec2};
use sharex_settings::{HotkeySettings, HotkeyStatus};
use sharex_strings::sharex as s;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::{FAILED, FONT_SIZE, density, palette};
use sharex_ui::widgets::{self, Caption, font, mono};
use sharex_ui::icons;
use sharex_ui::menu::{Flyout, FlyoutOutput, Placement};

use crate::main_menu::{self, MenuCommand, MenuState};
use crate::task_view::{TaskAction, TaskView};

/// `MainWindow.axaml`'s `ExtendClientAreaTitleBarHeightHint`.
pub const TITLE_BAR_HEIGHT: f32 = 38.0;
/// `Width`/`Height`/`MinWidth` in `MainWindow.axaml`.
pub const WIDTH: f32 = 900.0;
pub const HEIGHT: f32 = 720.0;
pub const MIN_WIDTH: f32 = 760.0;
/// `WindowFrame`'s `BorderThickness`.
pub const FRAME: f32 = 1.0;

/// What the user did, for the app to carry out.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    DragWindow,
    ToggleMaximized,
    Minimize,
    Close,
    ToggleTopmost,
    ToggleTheme,
    /// A sidebar entry without a menu, by its index in [`nav_sections`].
    Nav(usize, Rect),
    /// A menu entry was chosen.
    Menu(MenuCommand),
    /// Something in the task area.
    Task(TaskAction),
    /// The tool grid's edit mode showed or hid tools
    /// (`ApplicationConfig.HiddenTools`).
    HiddenTools(Vec<String>),
}

/// One `HotkeyTipViewModel`.
#[derive(Debug, Clone)]
pub struct HotkeyTip {
    pub registered: bool,
    pub hotkey: String,
    pub description: String,
}

pub struct MainWindow {
    pub title: String,
    /// `ApplicationInfo.Title`: what the title is without a progress.
    pub base_title: String,
    pub maximized: bool,
    pub topmost: bool,
    pub hotkey_tips: Vec<HotkeyTip>,
    /// Hide uploads (`SystemOptions.DisableUpload`).
    pub uploads_disabled: bool,
    /// What the menus show (settings, workflows, monitors).
    pub menu_state: MenuState,
    /// The open sidebar menu, if any.
    pub flyout: Option<Flyout<MenuCommand>>,
    /// Where menus must stay, in window points: the monitor's work area in
    /// Avalonia. `None` keeps them inside the window.
    pub work_area: Option<Rect>,
    /// Each visible sidebar entry's rect last frame, by section index.
    pub nav_rects: Vec<(usize, Rect)>,
    /// Files are being dragged over the window (`DropOverlay`).
    pub drop_overlay: bool,
    /// The task area (`ThumbnailPanel`) and the tasks behind it.
    pub tasks: TaskView,
}

/// A sidebar entry: `MainNavigationSection`.
pub struct NavSection {
    pub header: &'static str,
    pub icon: &'static str,
    pub has_children: bool,
    pub visible: bool,
}

/// `MainMenuBuilder.BuildNavigation`, in order.
pub fn nav_sections(uploads: bool) -> Vec<NavSection> {
    let e = |header, icon, has_children, visible| NavSection { header, icon, has_children, visible };
    vec![
        e(s::MainMenuBuilder_Capture(), icon::Camera, true, true),
        e(s::MainMenuBuilder_Upload(), icon::Upload, true, uploads),
        e(s::MainMenuBuilder_Workflows(), icon::Workflows, true, true),
        e(s::MainMenuBuilder_Tools(), icon::Wrench, true, true),
        e(s::MainMenuBuilder_AfterCaptureTasks(), icon::AfterCaptureTasks, true, true),
        e(s::MainMenuBuilder_AfterUploadTasks(), icon::AfterUploadTasks, true, uploads),
        e(s::MainMenuBuilder_Destinations(), icon::Destinations, true, uploads),
        e(s::MainMenuBuilder_ApplicationSettings(), icon::Settings, false, true),
        e(s::MainMenuBuilder_TaskSettings(), icon::Sliders, false, true),
        e(s::MainMenuBuilder_HotkeySettings(), icon::Keyboard, false, true),
        e(s::MainMenuBuilder_DestinationSettings(), icon::DestinationSettings, false, uploads),
        e(s::MainMenuBuilder_CustomUploaderSettings(), icon::CustomUploaderSettings, false, uploads),
        e(s::MainMenuBuilder_ScreenshotsFolder(), icon::ScreenshotsFolder, false, true),
        e(s::MainMenuBuilder_History(), icon::History, false, true),
        e(s::MainMenuBuilder_ImageHistory(), icon::ImageHistory, false, true),
        e(s::MainMenuBuilder_Stats(), icon::BarChart, false, true),
        e(s::MainMenuBuilder_Debug(), icon::Bug, true, true),
        e(s::MainMenuBuilder_Donate(), icon::Heart, false, true),
        e(s::MainMenuBuilder_FollowShareX(), icon::Follow, false, true),
        e(s::MainMenuBuilder_Discord(), icon::Discord, false, true),
        e(s::MainMenuBuilder_About(), icon::Info, false, true),
    ]
}

/// `RefreshHotkeyTips`: a tip for every valid hotkey, with its
/// registration status (`HotkeyTipViewModel`).
pub fn hotkey_tips(hotkeys: &[HotkeySettings]) -> Vec<HotkeyTip> {
    hotkeys
        .iter()
        .filter(|h| h.HotkeyInfo.is_valid_hotkey())
        .map(|h| HotkeyTip {
            registered: h.HotkeyInfo.status == HotkeyStatus::Registered,
            hotkey: h.HotkeyInfo.to_string(),
            description: h.TaskSettings.display_name(),
        })
        .collect()
}

impl MainWindow {
    pub fn new() -> Self {
        let version = env!("CARGO_PKG_VERSION");
        let short = version.rsplit_once('.').map_or(version, |(v, _)| v);
        Self {
            title: format!("ShareXrs {short}"),
            base_title: format!("ShareXrs {short}"),
            maximized: false,
            topmost: false,
            hotkey_tips: Vec::new(),
            uploads_disabled: false,
            menu_state: MenuState::default(),
            flyout: None,
            work_area: None,
            nav_rects: Vec::new(),
            drop_overlay: false,
            tasks: TaskView::new(),
        }
    }

    /// The window's minimum height: the sidebar's, so every entry fits
    /// (`ConfigureWindowHeightFromNavigation`). ShareX also opens at it.
    pub fn nav_height(&self, ctx: &egui::Context) -> f32 {
        let d = density(ctx);
        let sections = nav_sections(!self.uploads_disabled);
        let visible = sections.iter().filter(|s| s.visible).count() as f32;
        let separators = separator_positions(&sections).len() as f32;
        d.nav_panel_margin.top + d.nav_panel_margin.bottom + visible * d.nav_button_height + separators * (1.0 + 2.0 * 2.0)
    }

    pub fn min_height(&self, ctx: &egui::Context) -> f32 {
        (self.nav_height(ctx) + TITLE_BAR_HEIGHT + 2.0 * FRAME).ceil()
    }

    /// The height ShareX opens at: [`min_height`](Self::min_height), plus
    /// the 7px Avalonia leaves at the bottom on Windows (its extended client
    /// area keeps the height of the resize border; measured on Windows 11
    /// at 100%: 703 against the 696 `ConfigureWindowHeightFromNavigation`
    /// asks for).
    pub fn opening_height(&self, ctx: &egui::Context) -> f32 {
        self.min_height(ctx) + if cfg!(windows) { 7.0 } else { 0.0 }
    }

    pub fn ui(&mut self, ui: &mut Ui) -> Vec<Action> {
        let mut actions = Vec::new();
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        // `WindowFrame`: main background inside a 1px border, which goes
        // when maximized so the caption buttons reach the screen's edge
        // (`UpdateWindowFrame`).
        let frame = if self.maximized { 0.0 } else { FRAME };
        ui.painter().rect_filled(full, 0.0, p.background_main);
        let inner = full.shrink(frame);

        let title = Rect::from_min_size(inner.min, vec2(inner.width(), TITLE_BAR_HEIGHT));
        self.title_bar(ui, title, &mut actions);

        let body = Rect::from_min_max(pos2(inner.min.x, title.max.y), inner.max);
        let d = density(ui.ctx());
        let nav_width = d.nav_min_width.max(self.nav_content_width(ui));
        let nav = Rect::from_min_size(body.min, vec2(nav_width + 1.0, body.height()));
        self.sidebar(ui, nav, &mut actions);

        let content = Rect::from_min_max(pos2(nav.max.x, body.min.y), body.max);
        self.tasks.work_area = self.work_area;
        let task_actions = self.tasks.ui(ui, content, self.flyout.is_some());
        // `TryOpenContextMenu`: one menu at a time.
        if self.tasks.flyout.is_some() && self.flyout.is_some() {
            self.tasks.flyout = None;
        }
        actions.extend(task_actions.into_iter().map(|a| match a {
            TaskAction::CloseWindow => Action::Close,
            other => Action::Task(other),
        }));
        // `Grid IsVisible="{Binding IsEmpty}"`: the hotkeys over an empty area.
        if self.tasks.is_empty() {
            self.hotkey_panel(ui, content);
        }

        if self.drop_overlay {
            drop_overlay(ui, body);
        }
        if frame > 0.0 {
            ui.painter().rect_stroke(full, 0.0, Stroke::new(frame, p.border), egui::StrokeKind::Inside);
        }

        // `OnNavigationClick`: entries with a menu open it beside themselves
        // (`TryOpenContextMenu` / `TryOpenGridMenu`, only one at a time).
        let menu_open = self.flyout.is_some() || self.tasks.flyout.is_some();
        actions.retain(|a| match a {
            Action::Nav(i, rect) if !menu_open && self.open_nav(*i, *rect) => false,
            Action::Nav(..) if menu_open => false,
            _ => true,
        });
        if let Some(flyout) = &mut self.flyout {
            let bounds = self.work_area.unwrap_or(full);
            let FlyoutOutput { command, closed } = flyout.show(ui, bounds);
            if std::mem::take(&mut flyout.tools_changed) {
                self.menu_state.hidden_tools = flyout.hidden_tools.clone();
                actions.push(Action::HiddenTools(flyout.hidden_tools.clone()));
            }
            if closed {
                self.flyout = None;
            }
            if let Some(command) = command {
                actions.push(Action::Menu(command));
            }
        }
        actions
    }

    /// Whether a menu (a sidebar menu or a task context menu) is open, and
    /// whether `pos` is on it (as drawn last frame).
    pub fn menu_at(&self, pos: Option<egui::Pos2>) -> Option<bool> {
        let f = self.flyout.as_ref().map(|f| pos.is_some_and(|p| f.contains(p)));
        let t = self.tasks.flyout.as_ref().map(|f| pos.is_some_and(|p| f.contains(p)));
        match (f, t) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(false) || b.unwrap_or(false)),
        }
    }

    /// Closes every open menu (light dismiss, losing activation).
    pub fn close_menus(&mut self) -> bool {
        let open = self.flyout.is_some() || self.tasks.flyout.is_some();
        self.flyout = None;
        self.tasks.flyout = None;
        open
    }

    /// Opens sidebar entry `index`'s menu beside `anchor` (the entry's
    /// rect). Returns false for entries without one.
    pub fn open_nav(&mut self, index: usize, anchor: Rect) -> bool {
        self.menu_state.uploads_enabled = !self.uploads_disabled;
        let state = &self.menu_state;
        let id = egui::Id::new(("nav-menu", index));
        let place = Placement::RightEdgeAlignedTop;
        let flyout = match index {
            0 => Flyout::menu(id, main_menu::capture_menu(state), anchor, place),
            1 => Flyout::menu(id, main_menu::upload_menu(), anchor, place),
            2 => Flyout::menu(id, main_menu::workflows_menu(state, false), anchor, place),
            3 => {
                let mut f = Flyout::grid(id, main_menu::tool_categories(), anchor, place);
                f.edit_label = s::MainMenuBuilder_EditTools().to_string();
                f.finish_label = s::MainMenuBuilder_FinishEditingTools().to_string();
                f.hidden_tools = state.hidden_tools.clone();
                f
            }
            4 => Flyout::menu(id, main_menu::after_capture_menu(state), anchor, place),
            5 => Flyout::menu(id, main_menu::after_upload_menu(state), anchor, place),
            6 => Flyout::menu(id, main_menu::destinations_menu(state), anchor, place),
            16 => Flyout::menu(id, main_menu::debug_menu(state), anchor, place),
            _ => return false,
        };
        self.flyout = Some(flyout);
        true
    }

    fn title_bar(&mut self, ui: &mut Ui, rect: Rect, actions: &mut Vec<Action>) {
        let p = palette(ui.ctx());
        ui.painter().rect_filled(rect, 0.0, p.background_toolbar);
        // BorderThickness="0,0,0,1": the separator is the bar's last pixel row.
        let line = Rect::from_min_max(pos2(rect.min.x, rect.max.y - 1.0), rect.max);
        ui.painter().rect_filled(line, 0.0, p.separator);

        // The drag area under everything else.
        let drag = ui.interact(rect, ui.id().with("title-drag"), Sense::click_and_drag());
        if drag.drag_started() {
            actions.push(Action::DragWindow);
        }
        if drag.double_clicked() {
            actions.push(Action::ToggleMaximized);
        }

        // Icon (20x20) and title, from 14px in, 9px apart.
        let content = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.max.y - 1.0));
        // The icon and title are a 20px-tall StackPanel centred in the bar;
        // the title is centred again within it.
        let stack = Rect::from_min_size(
            pos2(content.min.x, widgets::center_y(content, 20.0)),
            vec2(content.width(), 20.0),
        );
        let icon_rect = Rect::from_min_size(pos2(content.min.x + 14.0, stack.min.y), Vec2::splat(20.0));
        paint_app_icon(ui, icon_rect);
        let x = icon_rect.max.x + 9.0;
        widgets::text_left(ui, stack, x, &self.title, font(FONT_SIZE, 600), p.text);

        // Caption buttons, right-aligned.
        let buttons = Rect::from_min_max(pos2(rect.max.x - 46.0 * 5.0, rect.min.y), pos2(rect.max.x, rect.min.y + 38.0));
        ui.scope_builder(UiBuilder::new().max_rect(buttons).layout(Layout::left_to_right(Align::Min)), |ui| {
            let dark = ui.ctx().theme() == egui::Theme::Dark;
            let theme_tip = if dark { s::ApplicationSettingsWindow_Dark() } else { s::ApplicationSettingsWindow_Light() };
            // The icon is drawn by `theme_toggle_icons` below, animated.
            let r = widgets::caption_button(ui, "", 16.0, Caption::Normal);
            sharex_ui::tooltip::show(ui, r.rect, ui.id().with("theme-toggle-tip"), theme_tip);
            theme_toggle_icons(ui, r.rect, dark, p.text);
            if r.clicked() {
                actions.push(Action::ToggleTheme);
            }
            let pin = if self.topmost { icon::CaptionUnpin } else { icon::CaptionPin };
            if widgets::caption_button(ui, pin, 16.0, Caption::Normal).clicked() {
                actions.push(Action::ToggleTopmost);
            }
            if widgets::caption_button(ui, icon::CaptionMinimize, 16.0, Caption::Normal).clicked() {
                actions.push(Action::Minimize);
            }
            let max = if self.maximized { icon::CaptionRestore } else { icon::CaptionMaximize };
            if widgets::caption_button(ui, max, 16.0, Caption::Normal).clicked() {
                actions.push(Action::ToggleMaximized);
            }
            if widgets::caption_button(ui, icon::CaptionClose, 16.0, Caption::Close { maximized: self.maximized })
                .clicked()
            {
                actions.push(Action::Close);
            }
        });
    }

    /// The widest entry, so the sidebar grows to fit long translations
    /// (`MinWidth` only sets the floor).
    fn nav_content_width(&self, ui: &Ui) -> f32 {
        let d = density(ui.ctx());
        nav_sections(!self.uploads_disabled)
            .iter()
            .filter(|s| s.visible)
            .map(|s| {
                let text = widgets::text_width(ui, s.header, font(FONT_SIZE, 400));
                let chevron = if s.has_children { 5.0 + 14.0 } else { 0.0 };
                d.nav_panel_margin.left
                    + d.nav_button_padding.left
                    + 20.0
                    + d.nav_label_margin.left
                    + text
                    + chevron
                    + d.nav_button_padding.right
                    + d.nav_panel_margin.right
            })
            .fold(0.0, f32::max)
            .ceil()
    }

    fn sidebar(&mut self, ui: &mut Ui, rect: Rect, actions: &mut Vec<Action>) {
        let p = palette(ui.ctx());
        let d = density(ui.ctx());
        ui.painter().rect_filled(rect, 0.0, p.background_toolbar);
        let border = Rect::from_min_max(pos2(rect.max.x - 1.0, rect.min.y), rect.max);
        ui.painter().rect_filled(border, 0.0, p.border);

        let m = d.nav_panel_margin;
        let panel = Rect::from_min_max(
            pos2(rect.min.x + m.left, rect.min.y + m.top),
            pos2(rect.max.x - 1.0 - m.right, rect.max.y - m.bottom),
        );
        let sections = nav_sections(!self.uploads_disabled);
        let separators = separator_positions(&sections);
        self.nav_rects.clear();
        let nav_rects = &mut self.nav_rects;
        ui.scope_builder(UiBuilder::new().max_rect(panel).layout(Layout::top_down(Align::Min)), |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (i, section) in sections.iter().enumerate().filter(|(_, s)| s.visible) {
                    if separators.contains(&i) {
                        widgets::separator(ui, 5.0, 2.0);
                    }
                    let r = widgets::nav_button(ui, section.icon, section.header, section.has_children);
                    nav_rects.push((i, r.rect));
                    if r.clicked() {
                        actions.push(Action::Nav(i, r.rect));
                    }
                }
            });
        });
    }

    /// `Grid IsVisible="{Binding IsEmpty}" Margin="28"` and the
    /// `Border.hotkey-panel` in it.
    fn hotkey_panel(&self, ui: &mut Ui, area: Rect) {
        let p = palette(ui.ctx());
        let area = area.shrink(28.0);
        let width = area.width().min(530.0);
        let padding = 18.0;
        let inner_width = width - 2.0 * padding;

        // Header: 22px keyboard icon and the title, 9px apart, centred.
        let title_font = font(19.0, 600);
        let title = widgets::galley(ui, s::MainWindow_ShareXHotkeys(), title_font, p.text, f32::INFINITY);
        let header_h = widgets::line_height(ui, &font(19.0, 600)).max(22.0);
        // The hint wraps to the panel's width less its 8px side margins.
        let hint_job = {
            let mut job = egui::text::LayoutJob::single_section(
                s::MainWindow_HotkeyHint().to_owned(),
                egui::TextFormat::simple(font(FONT_SIZE, 400), p.text_secondary),
            );
            job.wrap.max_width = inner_width - 16.0;
            job.halign = Align::Center;
            job
        };
        let hint = ui.fonts_mut(|f| f.layout_job(hint_job));
        let text_h = widgets::line_height(ui, &font(FONT_SIZE, 400)).max(widgets::line_height(ui, &mono(FONT_SIZE)));
        let row_h = 8.0 + 1.0 + 8.0 + text_h.max(17.0);
        let rows_h = row_h * self.hotkey_tips.len() as f32;
        let hint_h = hint.rows.len() as f32 * widgets::line_height(ui, &font(FONT_SIZE, 400));
        let content_h = header_h + 8.0 + hint_h + 8.0 + 8.0 + rows_h;
        // Avalonia's `Border` puts `Padding` inside `BorderThickness`.
        let height = (content_h + 2.0 * (padding + 1.0)).min(430.0).min(area.height());

        // Centred with layout rounding, so its edges land on whole pixels.
        let panel = Rect::from_min_size(
            pos2(area.min.x + ((area.width() - width) / 2.0).round_ties_even(), widgets::center_y(area, height)),
            vec2(width, height),
        );
        ui.painter().rect(
            panel,
            CornerRadius::same(8),
            p.background_panel,
            Stroke::new(1.0, p.border),
            egui::StrokeKind::Inside,
        );
        let inner = panel.shrink(padding + 1.0);

        // Row 0: header.
        let total = 22.0 + 9.0 + title.size().x;
        let x0 = inner.center().x - total / 2.0;
        let header = Rect::from_min_size(inner.min, vec2(inner.width(), header_h));
        icons::paint(
            ui.painter(),
            Rect::from_min_size(pos2(x0, header.center().y - 11.0), Vec2::splat(22.0)),
            icon::Keyboard,
            p.accent_icon,
        );
        let ty = widgets::center_y(header, widgets::line_height(ui, &font(19.0, 600)));
        ui.painter().galley(pos2(x0 + 31.0, ty), title, p.text);

        // Row 1: the hint.
        let hint_top = header.max.y + 8.0;
        // A centre-aligned galley is laid out around x = 0.
        ui.painter().galley(pos2(inner.center().x, hint_top), hint.clone(), p.text_secondary);

        // Row 2: the hotkeys.
        let mut y = hint_top + hint_h + 8.0 + 8.0;
        // Column 1 is `Auto` with `MinWidth="155"` shared across rows.
        let hotkey_w = self
            .hotkey_tips
            .iter()
            .map(|t| widgets::galley(ui, &t.hotkey, mono(FONT_SIZE), p.text, f32::INFINITY).size().x)
            .fold(155.0f32, f32::max);
        let clip = ui.painter().with_clip_rect(inner);
        let mono_h = widgets::line_height(ui, &mono(FONT_SIZE));
        let text_h_normal = widgets::line_height(ui, &font(FONT_SIZE, 400));
        for tip in &self.hotkey_tips {
            let row = Rect::from_min_size(pos2(inner.min.x, y), vec2(inner.width(), row_h));
            clip.rect_filled(Rect::from_min_size(row.min, vec2(row.width(), 1.0)), 0.0, p.border);
            let cells = Rect::from_min_max(pos2(row.min.x + 6.0, row.min.y + 1.0 + 8.0), pos2(row.max.x - 6.0, row.max.y - 8.0));
            let (key, color) = if tip.registered { (icon::CircleCheck, p.accent_icon) } else { (icon::CircleX, FAILED) };
            // A 17px icon centred in the 24px column and the 18px row, with
            // layout rounding (half-to-even, so 0.5 rounds down to 0).
            let icon_min = pos2(
                cells.min.x + ((24.0f32 - 17.0) / 2.0).round_ties_even(),
                widgets::center_y(cells, 17.0),
            );
            icons::paint(&clip, Rect::from_min_size(icon_min, Vec2::splat(17.0)), key, color);
            let hx = cells.min.x + 24.0 + 10.0;
            let g = widgets::galley(ui, &tip.hotkey, mono(FONT_SIZE), p.text_secondary, hotkey_w);
            clip.galley(pos2(hx, widgets::center_y(cells, mono_h)), g, p.text_secondary);
            let dx = hx + hotkey_w + 10.0;
            let g = widgets::galley(ui, &tip.description, font(FONT_SIZE, 400), p.text_secondary, (cells.max.x - dx).max(0.0));
            clip.galley(pos2(dx, widgets::center_y(cells, text_h_normal)), g, p.text_secondary);
            // `ToolTip.Tip="{Binding Description}"`.
            let tip_rect = Rect::from_min_max(pos2(dx, cells.min.y), cells.max).intersect(inner);
            sharex_ui::tooltip::show(ui, tip_rect, ui.id().with(("hotkey-tip", &tip.hotkey)), &tip.description);
            y += row_h;
        }
    }
}

/// `BuildNavigation` puts a separator before the 5th, 8th, 13th and 17th
/// visible entries (`index is 4 or 7 or 12 or 16`, counting visible ones).
fn separator_positions(sections: &[NavSection]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut visible = 0;
    for (i, s) in sections.iter().enumerate() {
        if !s.visible {
            continue;
        }
        if matches!(visible, 4 | 7 | 12 | 16) {
            out.push(i);
        }
        visible += 1;
    }
    out
}

/// The theme toggle's two icons (`IconPresenter.theme-toggle-icon`): the
/// active one (moon when dark) at rest, the other faded out 20px away and
/// turned 45°, sliding over 320ms with a cubic ease-out, clipped to a 20x20
/// box.
fn theme_toggle_icons(ui: &Ui, button: Rect, dark: bool, color: egui::Color32) {
    let t = ui.ctx().animate_bool_with_time_and_easing(
        ui.id().with("theme-toggle"),
        dark,
        0.32,
        egui::emath::easing::cubic_out,
    );
    let box_rect = Rect::from_center_size(button.center(), Vec2::splat(20.0));
    let painter = ui.painter().with_clip_rect(box_rect);
    let icon = Rect::from_center_size(button.center(), Vec2::splat(16.0));
    // t = 1: moon active. Moon at rest is translateY(20) rotate(-45deg).
    let moon = 1.0 - t;
    icons::paint_transformed(
        &painter,
        icon,
        icon::CaptionMoon,
        color.gamma_multiply(t),
        vec2(0.0, 20.0 * moon),
        (-45.0f32).to_radians() * moon,
    );
    icons::paint_transformed(
        &painter,
        icon,
        icon::CaptionSun,
        color.gamma_multiply(1.0 - t),
        vec2(0.0, -20.0 * t),
        45.0f32.to_radians() * t,
    );
}

/// `DropOverlay`: a modal tint over everything below the title bar, and a
/// panel saying files will be uploaded. (ShareX also shows it for dragged
/// text; winit only reports dragged files.)
fn drop_overlay(ui: &Ui, area: Rect) {
    let p = palette(ui.ctx());
    ui.painter().rect_filled(area, 0.0, p.overlay_modal);
    let title_font = font(22.0, 600);
    let desc_font = font(FONT_SIZE, 400);
    let inner_w = 440.0 - 2.0 * (32.0 + 2.0);
    let desc = {
        let mut job = egui::text::LayoutJob::single_section(
            s::MainWindow_FilesDroppedUploadDescription().to_owned(),
            egui::TextFormat::simple(desc_font.clone(), p.text_secondary),
        );
        job.wrap.max_width = inner_w;
        job.halign = Align::Center;
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let title_h = widgets::line_height(ui, &title_font);
    let desc_h = desc.rows.len() as f32 * widgets::line_height(ui, &desc_font);
    let content_h = 48.0 + 12.0 + title_h + 12.0 + desc_h;
    let h = content_h + 2.0 * (32.0 + 2.0);
    let panel = Rect::from_min_size(
        pos2(area.min.x + ((area.width() - 440.0) / 2.0).round_ties_even(), widgets::center_y(area, h)),
        vec2(440.0, h),
    );
    ui.painter().rect(
        panel,
        CornerRadius::same(12),
        p.background_panel,
        Stroke::new(2.0, p.accent_border),
        egui::StrokeKind::Inside,
    );
    let inner = panel.shrink(34.0);
    let cx = inner.center().x;
    icons::paint(ui.painter(), Rect::from_min_size(pos2(cx - 24.0, inner.min.y), Vec2::splat(48.0)), icon::Upload, p.accent_icon);
    let ty = inner.min.y + 48.0 + 12.0;
    let title = widgets::galley(ui, s::MainWindow_ReleaseToUploadFiles(), title_font, p.text, f32::INFINITY);
    ui.painter().galley(pos2(cx - (title.size().x / 2.0).round(), ty), title, p.text);
    ui.painter().galley(pos2(cx, ty + title_h + 12.0), desc, p.text_secondary);
}

/// The ShareX logo in the title bar (`TitleBarIcon`, from
/// `ShareXResources.IconBytes`: white with `UseWhiteShareXIcon`, SET-08a).
fn paint_app_icon(ui: &Ui, rect: Rect) {
    crate::app_icon::paint_current(ui, rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sharex_ui::offscreen::Offscreen;

    /// Runs frames of the main window with `events` in the first one.
    fn frame(screen: &mut Offscreen, state: &mut MainWindow, events: Vec<egui::Event>) -> Vec<Action> {
        let mut out = Vec::new();
        screen.run(events, &mut |ui: &mut egui::Ui| out.extend(state.ui(ui)));
        out
    }

    fn click(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn sidebar_menu_runs_a_command() {
        let mut screen = Offscreen::new([900, 703], 1.0, true).unwrap();
        let mut state = MainWindow::new();
        frame(&mut screen, &mut state, vec![]);
        let capture = egui::pos2(100.0, 60.0);
        frame(&mut screen, &mut state, vec![egui::Event::PointerMoved(capture), click(capture, true)]);
        frame(&mut screen, &mut state, vec![click(capture, false)]);
        assert!(state.flyout.is_some(), "Capture opens its menu");
        // Capture › Region…, the fourth item.
        let region = egui::pos2(300.0, 150.0);
        let mut actions = frame(&mut screen, &mut state, vec![egui::Event::PointerMoved(region)]);
        actions.extend(frame(&mut screen, &mut state, vec![click(region, true)]));
        actions.extend(frame(&mut screen, &mut state, vec![click(region, false)]));
        assert_eq!(actions, vec![Action::Menu(MenuCommand::CaptureRegion)]);
        assert!(state.flyout.is_none(), "a command closes the menu");
    }
}
