//! The tool options' popups (`AnnotationToolbar.axaml`'s dropdowns), shared
//! by region capture and the editor window: `ColorPickerDropdown` with its
//! `ColorPickerPanel` (Avalonia's `ColorView` on its palette tab, the screen
//! colour picker button and the recent colours), `WidthPickerDropdown`,
//! `BorderStylePickerDropdown`, `CornerRadiusPickerDropdown`,
//! `FontFamilyPickerDropdown`, `TextHorizontalAlignmentPickerDropdown`,
//! `ArrowStylePickerDropdown`, `CursorTypePickerDropdown`,
//! `FontSizePickerDropdown`, `StepTypePickerDropdown`, `NumberPickerDropdown`
//! and the shadow options popup (`ShadowOptionsPopup`, a right click on the
//! shadow button).
//!
//! Each is a `Popup` with `Placement="Bottom"` and `VerticalOffset="4"`
//! under its button, in a `Border.popup-border.popup-surface` (control
//! background, 1px border, 4px corners, 8px padding, `0 14 28 -14
//! #66000000` shadow) holding `Button.picker-button`s (transparent, 6px
//! corners, `Control.Background.Disabled` on hover, the accent when
//! active). Text in popups is Segoe UI Regular (ShareX's SemiBold
//! `TextBlock` style doesn't reach them).

use std::sync::Mutex;

use egui::{Color32, CornerRadius, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, pos2, vec2};
use sharex_image_editor::editor::{ToolState, Workspace, system_font_families};
use sharex_image_editor::model::{ArrowStyle, BorderStyle, CursorType, StepType, TextHorizontalAlignment};
use sharex_strings::image_editor as ie;
use sharex_ui::keys::app_icons;
use sharex_ui::{Palette, icons, widgets};

use crate::region::options_row::{self, OptionItem};

/// `EditorRecentColors`: newest first, shared by every editor (a static).
static RECENT_COLORS: Mutex<Vec<Color32>> = Mutex::new(Vec::new());
/// `EditorRecentColors.MaxCount`.
const MAX_RECENT_COLORS: usize = 32;
/// `ColorPickerPanel.RecentColorCount`.
const RECENT_COLOR_COUNT: usize = 5;

/// `EditorRecentColors.Add`.
pub fn add_recent_color(c: Color32) {
    let mut list = RECENT_COLORS.lock().unwrap();
    list.retain(|x| *x != c);
    list.insert(0, c);
    list.truncate(MAX_RECENT_COLORS);
}

/// `MainViewModel.StrokeWidths`... `WidthPickerDropdown.GetDefaultWidthOptions`.
const WIDTHS: [i32; 10] = [2, 4, 6, 8, 10, 12, 14, 16, 18, 20];
/// `CornerRadiusPickerDropdown.GetDefaultCornerRadiusOptions`.
const CORNER_RADII: [i32; 10] = [0, 2, 4, 6, 8, 10, 15, 20, 25, 30];
/// `FontSizePickerDropdown.GetDefaultFontSizes`.
const FONT_SIZES: [f32; 12] = [10.0, 13.0, 16.0, 20.0, 24.0, 30.0, 48.0, 72.0, 96.0, 144.0, 216.0, 288.0];
const BORDER_STYLES: [BorderStyle; 5] =
    [BorderStyle::Solid, BorderStyle::Dash, BorderStyle::Dot, BorderStyle::DashDot, BorderStyle::DashDotDot];
const ARROW_STYLES: [ArrowStyle; 5] = [ArrowStyle::Classic, ArrowStyle::Double, ArrowStyle::Modern, ArrowStyle::Basic, ArrowStyle::Line];
const ALIGNMENTS: [TextHorizontalAlignment; 3] =
    [TextHorizontalAlignment::Left, TextHorizontalAlignment::Center, TextHorizontalAlignment::Right];
const STEP_TYPES: [StepType; 5] =
    [StepType::Numeric, StepType::UppercaseLetter, StepType::LowercaseLetter, StepType::UppercaseRoman, StepType::LowercaseRoman];
/// `BuildAvailableCursorTypes`.
const CURSOR_TYPES: [CursorType; 28] = [
    CursorType::AppStarting,
    CursorType::Arrow,
    CursorType::Cross,
    CursorType::Default,
    CursorType::Hand,
    CursorType::Help,
    CursorType::HSplit,
    CursorType::IBeam,
    CursorType::No,
    CursorType::NoMove2D,
    CursorType::NoMoveHoriz,
    CursorType::NoMoveVert,
    CursorType::PanEast,
    CursorType::PanNE,
    CursorType::PanNorth,
    CursorType::PanNW,
    CursorType::PanSE,
    CursorType::PanSouth,
    CursorType::PanSW,
    CursorType::PanWest,
    CursorType::SizeAll,
    CursorType::SizeNESW,
    CursorType::SizeNS,
    CursorType::SizeNWSE,
    CursorType::SizeWE,
    CursorType::UpArrow,
    CursorType::VSplit,
    CursorType::WaitCursor,
];

/// Avalonia's `FluentColorPalette` as `ColorView` lays it out (8 rows of
/// 6), with `ColorPickerPanel.OnLoaded`'s three replacements (#EBEBEB,
/// #141414 and transparent, the last).
const PALETTE: [u32; 48] = [
    0xFF4343, 0xE74856, 0x0078D7, 0x0099BC, 0x7A7574, 0x767676, //
    0xD13438, 0xE81123, 0x0063B1, 0x2D7D9A, 0x5D5A50, 0x4C4A48, //
    0xEF6950, 0xEA005E, 0x8E8CD8, 0x00B7C3, 0x68768A, 0x69797E, //
    0xDA3B01, 0xC30052, 0x6B69D6, 0x038387, 0x515C6B, 0x4A5459, //
    0xCA5010, 0xE3008C, 0x8764B8, 0x00B294, 0x567C73, 0x647C64, //
    0xF7630C, 0xBF0077, 0x744DA9, 0x018574, 0x486860, 0xEBEBEB, //
    0xFF8C00, 0xC239B3, 0xB146C2, 0x00CC6A, 0x498205, 0x141414, //
    0xFFB900, 0x9A0089, 0x881798, 0x10893E, 0x107C10, 0x000000,
];

/// The open popup and what it keeps while open.
#[derive(Default)]
pub struct Popups {
    pub open: Option<OptionItem>,
    /// Opened this frame: the press that opened it isn't a light dismiss.
    just_opened: bool,
    /// `ColorView.SelectedIndex` (1, the palette).
    color_tab: usize,
    color_when_opened: Option<Color32>,
    font_size_text: String,
    font_search: String,
    font_scroll: f32,
    font_textures: Vec<(String, Option<TextureHandle>)>,
    /// The shadow popup's own colour dropdown.
    shadow_color_open: bool,
    /// The popup's rectangle as last drawn, in points.
    pub rect: Option<Rect>,
}

/// What showing the popup did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PopupResult {
    /// An option changed.
    pub changed: bool,
    /// The screen colour picker was asked for (`OnScreenColorPickerClick`).
    pub pick_screen_color: bool,
}

/// `Color.Parse` of an option's hex.
fn color_of(hex: &str) -> Color32 {
    let c = sharex_image_editor::render::parse_color(hex);
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// `#AARRGGBB` of a colour (the options' format).
fn hex_of(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}

fn text_font(size: f32) -> egui::FontId {
    widgets::font(size, 400)
}

impl Popups {
    /// Opens `item`'s popup, or closes it when it's the one open (a
    /// dropdown button's click).
    pub fn toggle(&mut self, item: OptionItem, tools: &ToolState) {
        if self.open == Some(item) {
            self.close();
            return;
        }
        self.close();
        self.open = Some(item);
        self.just_opened = true;
        self.color_tab = 1;
        self.color_when_opened = self.current_color(item, tools);
        self.font_size_text = format!("{}", tools.font_size);
        self.font_search.clear();
        self.font_scroll = 0.0;
    }

    pub fn close(&mut self) {
        self.open = None;
        self.rect = None;
        self.shadow_color_open = false;
    }

    /// Opens popups for these items (the shadow's is a right click).
    pub fn has_popup(item: OptionItem) -> bool {
        use OptionItem as O;
        matches!(
            item,
            O::BorderColor
                | O::FillColor
                | O::TextColor
                | O::Thickness
                | O::BorderStyle
                | O::CornerRadius
                | O::FontFamily
                | O::TextAlignment
                | O::ArrowStyle
                | O::CursorType
                | O::FontSize
                | O::StepType
                | O::StepStart
        )
    }

    fn current_color(&self, item: OptionItem, s: &ToolState) -> Option<Color32> {
        match item {
            OptionItem::BorderColor => Some(color_of(&s.selected_color)),
            OptionItem::FillColor => Some(color_of(&s.fill_color)),
            OptionItem::TextColor => Some(color_of(&s.text_color)),
            OptionItem::Shadow => Some(color_of(&s.shadow_color)),
            _ => None,
        }
    }

    /// Whether `p` (points) is on the open popup.
    pub fn contains(&self, p: Pos2) -> bool {
        self.rect.is_some_and(|r| r.contains(p))
    }

    /// Draws the open popup under `anchor` (its button, points) within
    /// `bounds` (the screen), applying choices to `ws`. Closes it on a
    /// choice, Escape or a press outside it and its button.
    pub fn show(&mut self, ui: &egui::Ui, p: &Palette, anchor: Rect, bounds: Rect, ws: &mut Workspace) -> PopupResult {
        let Some(item) = self.open else { return PopupResult::default() };
        let mut result = PopupResult::default();
        let mut close = false;
        let id = Id::new(("editor-popup", item as u8));
        let size = self.size(ui, item, &ws.tools);
        // `Placement="Bottom"`: centred under the button, 4px down; kept on
        // the screen.
        let mut size = size;
        // Avalonia's positioner keeps a popup on the work area, its height
        // cut to it (the cursor type list is taller than a 768px screen).
        size.y = size.y.min(bounds.height());
        let mut pos = pos2((anchor.center().x - size.x / 2.0).round(), anchor.max.y + 4.0);
        if pos.y + size.y > bounds.max.y {
            pos.y = (bounds.max.y - size.y).max(bounds.min.y);
        }
        pos.x = pos.x.clamp(bounds.min.x, (bounds.max.x - size.x).max(bounds.min.x));
        let rect = Rect::from_min_size(pos, size);
        self.rect = Some(rect);
        let area = egui::Area::new(id).order(Order::Foreground).fixed_pos(pos).fade_in(false).show(ui.ctx(), |ui| {
            let (r, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.set_clip_rect(r.expand(40.0));
            let colour = matches!(item, OptionItem::BorderColor | OptionItem::FillColor | OptionItem::TextColor);
            let shadow_popup = item == OptionItem::Shadow;
            // `popup-border`, or the colour and shadow popups' own panel
            // background and 8px corners.
            let (bg, radius) = if colour || shadow_popup { (p.background_panel, 8.0) } else { (p.control_background, 4.0) };
            paint_frame(ui, r, bg, p.border, radius);
            let inner = r.shrink(1.0);
            match item {
                OptionItem::BorderColor | OptionItem::FillColor | OptionItem::TextColor => {
                    let mut c = self.current_color(item, &ws.tools).unwrap_or(Color32::TRANSPARENT);
                    let (changed, pick) = self.color_panel(ui, p, inner.shrink(4.0), id, &mut c);
                    if changed {
                        let hex = hex_of(c);
                        ws.set_option(|s, t| match item {
                            OptionItem::BorderColor => s.set_selected_color(&hex, t),
                            OptionItem::FillColor => s.set_fill_color(&hex, t),
                            _ => s.set_text_color(&hex, t),
                        });
                        result.changed = true;
                    }
                    if pick {
                        result.pick_screen_color = true;
                        close = true;
                    }
                }
                OptionItem::Thickness => {
                    let cur = ws.tools.stroke_width;
                    if let Some(v) = grid(ui, p, inner.shrink(8.0), id, &WIDTHS, 5, 32.0, |v| *v == cur, |painter, r, v, _| {
                        painter.circle_filled(r.center(), *v as f32 / 2.0, p.text_secondary);
                    }) {
                        ws.set_option(|s, t| s.set_stroke_width(v, t));
                        close = true;
                    }
                }
                OptionItem::CornerRadius => {
                    let cur = ws.tools.corner_radius;
                    if let Some(v) = grid(ui, p, inner.shrink(8.0), id, &CORNER_RADII, 5, 36.0, |v| *v == cur, |painter, r, v, _| {
                        let at = Rect::from_center_size(r.center(), vec2(20.0, 20.0));
                        options_row::corner_preview(painter, at, *v as f32, p.text_secondary, 1.0);
                    }) {
                        ws.set_option(|s, t| s.set_corner_radius(v, t));
                        close = true;
                    }
                }
                OptionItem::BorderStyle => {
                    let cur = ws.tools.border_style.clone();
                    if let Some(v) = list(ui, p, inner.shrink(8.0), id, &BORDER_STYLES, |v| *v == cur, |painter, r, v, fg| {
                        text_left(ui, painter, r, options_row::border_style_name(v), 14.0, fg);
                        let right = r.max.x - 8.0;
                        options_row::dashed_preview(painter, right - 36.0 + 2.0, right - 2.0, r.center().y, v, fg, 1.0);
                    }) {
                        ws.set_option(|s, t| s.set_border_style(v, t));
                        close = true;
                    }
                }
                OptionItem::ArrowStyle => {
                    let cur = ws.tools.arrow_style.clone();
                    if let Some(v) = list(ui, p, inner.shrink(8.0), id, &ARROW_STYLES, |v| *v == cur, |painter, r, v, fg| {
                        text_left(ui, painter, r, options_row::arrow_style_name(v), 14.0, fg);
                        options_row::arrow_style_preview(ui, painter, r.max.x - 8.0 - 36.0, r.center().y, v, fg, 1.0);
                    }) {
                        ws.set_option(|s, t| s.set_arrow_style(v, t));
                        close = true;
                    }
                }
                OptionItem::TextAlignment => {
                    let cur = ws.tools.text_alignment.clone();
                    if let Some(v) = list(ui, p, inner.shrink(8.0), id, &ALIGNMENTS, |v| *v == cur, |painter, r, v, fg| {
                        let key = alignment_icon(v);
                        icons::paint(painter, Rect::from_min_size(pos2(r.min.x + 8.0, r.center().y - 8.0), vec2(16.0, 16.0)), key, fg);
                        let label = alignment_name(v);
                        let mut tr = r;
                        tr.min.x += 24.0;
                        text_left(ui, painter, tr, label, 14.0, fg);
                    }) {
                        ws.set_option(|s, t| s.set_text_alignment(v, t));
                        close = true;
                    }
                }
                OptionItem::StepType => {
                    let cur = ws.tools.step_type.clone();
                    if let Some(v) = list(ui, p, inner.shrink(8.0), id, &STEP_TYPES, |v| *v == cur, |painter, r, v, fg| {
                        text_left(ui, painter, r, options_row::step_type_preview(v), 12.0, fg);
                    }) {
                        ws.set_option(|s, _| s.set_step_type(v));
                        close = true;
                    }
                }
                OptionItem::StepStart => {
                    let cur = ws.tools.step_start_number;
                    let numbers: Vec<i32> = (1..=10).collect();
                    if let Some(v) = list(ui, p, inner.shrink(8.0), id, &numbers, |v| *v == cur, |painter, r, v, fg| {
                        text_center(ui, painter, r, &v.to_string(), 12.0, fg);
                    }) {
                        ws.set_step_start_number(v);
                        close = true;
                    }
                }
                OptionItem::CursorType => {
                    let cur = ws.tools.cursor_type.clone();
                    if let Some(v) = list(ui, p, inner.shrink(8.0), id, &CURSOR_TYPES, |v| *v == cur, |painter, r, v, fg| {
                        text_left(ui, painter, r, options_row::cursor_type_name(v), 14.0, fg);
                        let pr = Rect::from_min_size(pos2(r.max.x - 8.0 - 24.0, r.center().y - 12.0), vec2(24.0, 24.0));
                        options_row::cursor_preview(ui, painter, pr, v);
                    }) {
                        ws.set_option(|s, t| s.set_cursor_type(v, t));
                        close = true;
                    }
                }
                OptionItem::FontSize => {
                    if let Some(v) = self.font_size_panel(ui, p, inner.shrink(8.0), id, &ws.tools) {
                        ws.set_option(|s, t| s.set_font_size(v, t));
                        close = true;
                    }
                }
                OptionItem::FontFamily => {
                    if let Some(v) = self.font_family_panel(ui, p, inner.shrink(8.0), id, &ws.tools) {
                        ws.set_option(|s, t| s.set_font_family(&v, t));
                        close = true;
                    }
                }
                OptionItem::Shadow => {
                    result.changed |= self.shadow_panel(ui, p, inner.shrink(10.0), id, ws);
                }
                _ => {}
            }
        });
        let ctx = ui.ctx();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if !self.just_opened
            && ctx.input(|i| i.pointer.any_pressed())
            && let Some(at) = ctx.input(|i| i.pointer.interact_pos())
            && !area.response.rect.contains(at)
            && !anchor.contains(at)
            && !self.sub_popup_contains(ctx, at)
        {
            close = true;
        }
        self.just_opened = false;
        if close {
            // `OnHostPopupClosed`: the colour it closes with is used.
            if let (Some(before), Some(now)) = (self.color_when_opened, self.current_color(item, &ws.tools))
                && before != now
                && item != OptionItem::Shadow
            {
                add_recent_color(now);
            }
            self.close();
        }
        result
    }

    fn sub_popup_contains(&self, ctx: &egui::Context, at: Pos2) -> bool {
        self.shadow_color_open
            && ctx.memory(|m| m.area_rect(Id::new("editor-shadow-color"))).is_some_and(|r| r.contains(at))
    }

    /// The popup's size for `item`, in points.
    fn size(&self, ui: &egui::Ui, item: OptionItem, s: &ToolState) -> egui::Vec2 {
        let tw = |t: &str, size: f32| widgets::text_width(ui, t, text_font(size)).ceil();
        let frame = |w: f32, h: f32| vec2(w + 18.0, h + 18.0);
        let list_w = |min: f32, widest: f32| min.max(widest);
        match item {
            OptionItem::BorderColor | OptionItem::FillColor | OptionItem::TextColor => vec2(360.0, 450.0),
            OptionItem::Thickness => frame(5.0 * 32.0, 2.0 * 32.0),
            OptionItem::CornerRadius => frame(5.0 * 36.0, 2.0 * 36.0),
            OptionItem::BorderStyle => {
                let w = BORDER_STYLES.iter().map(|b| 8.0 + tw(options_row::border_style_name(b), 14.0) + 16.0 + 36.0 + 8.0).fold(0.0, f32::max);
                frame(list_w(132.0, w), 5.0 * 32.0)
            }
            OptionItem::ArrowStyle => {
                let w = ARROW_STYLES.iter().map(|a| 8.0 + tw(options_row::arrow_style_name(a), 14.0) + 8.0 + 36.0 + 8.0).fold(0.0, f32::max);
                frame(list_w(122.0, w), 5.0 * 32.0)
            }
            OptionItem::TextAlignment => {
                let w = ALIGNMENTS.iter().map(|a| 8.0 + 16.0 + 8.0 + tw(alignment_name(a), 14.0) + 8.0).fold(0.0, f32::max);
                frame(list_w(132.0, w), 3.0 * 32.0)
            }
            OptionItem::StepType => {
                let w = STEP_TYPES.iter().map(|t| 8.0 + tw(options_row::step_type_preview(t), 12.0) + 8.0).fold(0.0, f32::max);
                frame(list_w(118.0, w), 5.0 * 32.0)
            }
            OptionItem::StepStart => frame(list_w(50.0, 8.0 + tw("10", 12.0) + 8.0), 10.0 * 32.0),
            OptionItem::CursorType => {
                let w = CURSOR_TYPES.iter().map(|c| 8.0 + tw(options_row::cursor_type_name(c), 14.0) + 8.0 + 24.0 + 8.0).fold(0.0, f32::max);
                frame(list_w(136.0, w), CURSOR_TYPES.len() as f32 * 32.0)
            }
            OptionItem::FontSize => {
                let w = FONT_SIZES.iter().map(|v| 8.0 + tw(&format!("{v} pt"), 12.0) + 8.0).fold(70.0, f32::max);
                frame(w, 32.0 + 4.0 + FONT_SIZES.len() as f32 * 32.0)
            }
            OptionItem::FontFamily => {
                let _ = s;
                vec2(260.0, 18.0 + 40.0 + 4.0 + 360.0)
            }
            OptionItem::Shadow => {
                // Padding 10 and the border, the title, the colour row, four
                // blocks.
                let title = widgets::line_height(ui, &widgets::font(16.0, 600));
                vec2(240.0, 22.0 + title + 8.0 + 40.0 + 16.0 + 4.0 * (24.0 + 50.0) + 3.0 * 8.0)
            }
            _ => vec2(0.0, 0.0),
        }
    }

    /// `ColorPickerPanel`: the `ColorView` (tabs, the palette, the preview
    /// with its accents), then the screen colour picker button and the
    /// recent colours. Returns whether the colour changed and whether the
    /// screen picker was asked for.
    fn color_panel(&mut self, ui: &mut egui::Ui, p: &Palette, r: Rect, id: Id, c: &mut Color32) -> (bool, bool) {
        let painter = ui.painter().clone();
        let painter = &painter;
        let mut changed = false;
        // The tab strip: 48 high, `SystemControlBackgroundBaseLowBrush`, 3px
        // corners, three equal tabs with 20px icons; the selected one has
        // the accent pipe (2px over the bottom, 2px up).
        let strip = Rect::from_min_size(r.min, vec2(350.0, 48.0));
        painter.rect_filled(strip, CornerRadius::same(3), sharex_ui::theme::fluent(ui.ctx()).base_low);
        let tab_w = strip.width() / 3.0;
        // `PathIcon`: the geometry's bounds stretched uniformly into 20x20.
        let tab_icons = [
            (SPECTRUM_ICON, "2.5 2 15 16"),
            (PALETTE_ICON, "1.89915 1.57663 17.29995 16.78697"),
            (COMPONENTS_ICON, "2 3 16 14"),
        ];
        for (i, (icon, view_box)) in tab_icons.iter().enumerate() {
            let tr = Rect::from_min_size(pos2(strip.min.x + tab_w * i as f32, strip.min.y), vec2(tab_w, 48.0));
            let resp = ui.interact(tr, id.with(("tab", i)), Sense::click());
            if resp.clicked() {
                self.color_tab = i;
            }
            let fg = if self.color_tab == i { p.text } else { p.text_secondary };
            paint_path_icon(painter, Rect::from_center_size(tr.center(), vec2(20.0, 20.0)), icon, view_box, fg);
            if self.color_tab == i {
                let pipe = Rect::from_min_max(pos2(tr.min.x + 6.0, tr.max.y - 4.0), pos2(tr.max.x - 6.0, tr.max.y - 2.0));
                painter.rect_filled(pipe, CornerRadius::same(1), p.accent_start);
            }
        }
        let content = Rect::from_min_size(pos2(r.min.x, strip.max.y + 2.0), vec2(350.0, 338.0 - 48.0));
        match self.color_tab {
            1 => {
                // The palette: 12px in, 6 columns of 8 rows (54.33 x 33).
                let grid = content.shrink(12.0);
                let (cw, ch) = (grid.width() / 6.0, 33.0);
                for (i, rgb) in PALETTE.iter().enumerate() {
                    let (row, col) = (i / 6, i % 6);
                    let cell = Rect::from_min_size(pos2(grid.min.x + cw * col as f32, grid.min.y + ch * row as f32), vec2(cw, ch));
                    let color = if i == PALETTE.len() - 1 {
                        Color32::TRANSPARENT
                    } else {
                        Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8)
                    };
                    if color.a() < 255 {
                        checkered(painter, cell, 4.0, p);
                    }
                    painter.rect_filled(cell, 0.0, color);
                    let resp = ui.interact(cell, id.with(("palette", i)), Sense::click());
                    let selected = *c == color;
                    if selected || resp.hovered() {
                        // `BorderRectangle`: 3px, the contrasting colour,
                        // half opacity on hover.
                        let lum = 0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32;
                        let contrast = if color.a() < 128 || lum > 127.0 { Color32::BLACK } else { Color32::WHITE };
                        let k = if selected { contrast } else { contrast.gamma_multiply(0.5) };
                        painter.rect_stroke(cell, 0.0, Stroke::new(3.0, k), StrokeKind::Inside);
                    }
                    if resp.clicked() && *c != color {
                        *c = color;
                        changed = true;
                    }
                }
            }
            _ => {
                // The spectrum and components tabs (Avalonia's
                // `ColorSpectrum`, `ColorSlider`s) aren't ported: egui's
                // HSV picker stands in, as in the app's other colour popups.
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(content.shrink(12.0)));
                child.spacing_mut().slider_width = 300.0;
                changed |= egui::widgets::color_picker::color_picker_color32(&mut child, c, egui::widgets::color_picker::Alpha::OnlyBlend);
            }
        }
        // `ColorPreviewer`: 17px below the tab content (338 + 12 margin),
        // 40 high accents (V -0.2, -0.1, +0.1, +0.2) round a 50 high preview
        // with its `0 0 10 2 #BF000000` shadow.
        let pv = Rect::from_min_size(pos2(r.min.x + 12.0, r.min.y + 343.0), vec2(326.0, 40.0));
        let accents = [-2, -1, 1, 2].map(|k| accent(*c, k));
        let seg = |i: f32, w: f32| Rect::from_min_size(pos2(pv.min.x + i, pv.min.y), vec2(w, 40.0));
        let left = [seg(0.0, 40.0), seg(40.0, 40.0)];
        let right = [seg(246.0, 40.0), seg(286.0, 40.0)];
        painter.rect_filled(left[0], CornerRadius { nw: 4, sw: 4, ne: 0, se: 0 }, accents[0]);
        painter.rect_filled(left[1], 0.0, accents[1]);
        painter.rect_filled(right[0], 0.0, accents[2]);
        painter.rect_filled(right[1], CornerRadius { nw: 0, sw: 0, ne: 4, se: 4 }, accents[3]);
        let main = Rect::from_min_size(pos2(pv.min.x + 80.0, pv.min.y - 5.0), vec2(166.0, 50.0));
        let shadow = egui::epaint::Shadow { offset: [0, 0], blur: 10, spread: 2, color: Color32::from_black_alpha(0xBF) };
        painter.add(shadow.as_shape(main, CornerRadius::same(4)));
        if c.a() < 255 {
            checkered(painter, main, 4.0, p);
        }
        painter.rect_filled(main, CornerRadius::same(4), *c);
        for (i, r) in left.iter().chain(right.iter()).enumerate() {
            if ui.interact(*r, id.with(("accent", i)), Sense::click()).clicked() {
                *c = accents[i];
                changed = true;
            }
        }
        // The bottom row (`Margin="4"`, centred, 12 apart): the screen
        // colour picker button (40x32), then five recent colours (28x28, 6
        // apart).
        let row_y = r.min.y + 400.0 + 4.0;
        let row_w = 40.0 + 12.0 + RECENT_COLOR_COUNT as f32 * 28.0 + (RECENT_COLOR_COUNT as f32 - 1.0) * 6.0;
        let x0 = (r.center().x - row_w / 2.0).round();
        let br = Rect::from_min_size(pos2(x0, row_y), vec2(40.0, 32.0));
        let bresp = ui.interact(br, id.with("screen-picker"), Sense::click());
        let bg = if bresp.hovered() { p.control_background_hover } else { p.control_background };
        bordered(ui, br, 4.0, bg, p.border);
        icons::paint(painter, Rect::from_center_size(br.center(), vec2(16.0, 16.0)), app_icons::ScreenColorPicker, p.text);
        let recent = RECENT_COLORS.lock().unwrap().clone();
        for i in 0..RECENT_COLOR_COUNT {
            let cr = Rect::from_min_size(pos2(x0 + 52.0 + i as f32 * 34.0, row_y + 2.0), vec2(28.0, 28.0));
            match recent.get(i) {
                Some(rc) => {
                    let resp = ui.interact(cr, id.with(("recent", i)), Sense::click());
                    checkered(painter, cr, 4.0, p);
                    painter.rect_filled(cr, CornerRadius::same(4), *rc);
                    let (w, k) = if resp.hovered() { (2.0, p.text_secondary) } else { (1.0, p.border) };
                    painter.rect_stroke(cr, CornerRadius::same(4), Stroke::new(w, k), StrokeKind::Inside);
                    if resp.clicked() {
                        *c = *rc;
                        changed = true;
                    }
                }
                None => {
                    painter.rect_stroke(cr, CornerRadius::same(4), Stroke::new(1.0, p.border.gamma_multiply(0.6)), StrokeKind::Inside);
                }
            }
        }
        (changed, bresp.clicked())
    }

    /// `FontSizePickerDropdown`: a 70x32 box for a custom size (Enter
    /// applies it), then the sizes.
    fn font_size_panel(&mut self, ui: &mut egui::Ui, p: &Palette, r: Rect, id: Id, s: &ToolState) -> Option<f32> {
        let box_rect = Rect::from_min_size(pos2(r.center().x - 35.0, r.min.y), vec2(70.0, 32.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(box_rect));
        let tb = sharex_ui::controls::TextBox {
            padding: sharex_ui::theme::Thickness::new(8.0, 0.0, 8.0, 0.0),
            font: Some(text_font(12.0)),
            ..Default::default()
        };
        let resp = tb.show(&mut child, box_rect, id.with("custom-size"), &mut self.font_size_text);
        if self.just_opened {
            resp.request_focus();
        }
        if resp.has_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))
            && let Ok(v) = self.font_size_text.trim().parse::<f32>()
            && v > 0.0
        {
            return Some(v);
        }
        let items = Rect::from_min_max(pos2(r.min.x, box_rect.max.y + 4.0), r.max);
        let cur = s.font_size;
        list(ui, p, items, id, &FONT_SIZES, |v| *v == cur, |painter, r, v, fg| {
            text_center(ui, painter, r, &format!("{v} pt"), 12.0, fg);
        })
    }

    /// `FontFamilyPickerDropdown`: the search box, then the families in
    /// their own faces (360 high, scrolled).
    fn font_family_panel(&mut self, ui: &mut egui::Ui, p: &Palette, r: Rect, id: Id, s: &ToolState) -> Option<String> {
        // `TextBox.effect-search`: 40 high, 8,0 padding.
        let box_rect = Rect::from_min_size(r.min, vec2(r.width(), 40.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(box_rect));
        let tb = sharex_ui::controls::TextBox {
            placeholder: ie::FontFamilyPickerDropdown_SearchfontsEllipsis(),
            padding: sharex_ui::theme::Thickness::new(8.0, 0.0, 8.0, 0.0),
            ..Default::default()
        };
        let resp = tb.show(&mut child, box_rect, id.with("search"), &mut self.font_search);
        if self.just_opened {
            resp.request_focus();
        }
        let q = self.font_search.trim().to_lowercase();
        let names: Vec<&String> = system_font_families().iter().filter(|n| q.is_empty() || n.to_lowercase().contains(&q)).collect();
        let area = Rect::from_min_max(pos2(r.min.x, box_rect.max.y + 4.0), r.max);
        if names.is_empty() {
            text_left(ui, ui.painter(), Rect::from_min_size(area.min + vec2(0.0, 6.0), vec2(area.width(), 20.0)), ie::FontFamilyPickerDropdown_Nomatchingfonts(), 12.0, p.text_secondary);
            return None;
        }
        let total = names.len() as f32 * 32.0;
        if ui.rect_contains_pointer(area) {
            let dy = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            self.font_scroll = (self.font_scroll - dy).clamp(0.0, (total - area.height()).max(0.0));
        }
        let painter = ui.painter().with_clip_rect(area);
        let first = (self.font_scroll / 32.0).floor() as usize;
        let mut chosen = None;
        for (i, name) in names.iter().enumerate().skip(first).take((area.height() / 32.0).ceil() as usize + 1) {
            let ir = Rect::from_min_size(pos2(area.min.x, area.min.y + i as f32 * 32.0 - self.font_scroll), vec2(area.width(), 32.0));
            let resp = ui.interact(ir.intersect(area), id.with(("font", i)), Sense::click());
            let active = **name == s.font_family;
            picker_background(&painter, p, ir, resp.hovered(), active);
            let fg = if active { p.accent_foreground } else { p.text };
            match self.font_texture(ui.ctx(), name, fg) {
                Some(tex) => {
                    let size = tex.size_vec2() / ui.ctx().pixels_per_point();
                    let at = pos2(ir.min.x + 8.0, (ir.center().y - size.y / 2.0).round());
                    painter.image(tex.id(), Rect::from_min_size(at, size), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                None => text_left(ui, &painter, ir, name, 14.0, fg),
            }
            if resp.clicked() {
                chosen = Some((*name).clone());
            }
        }
        chosen
    }

    /// A family's name in its own face (cached).
    fn font_texture(&mut self, ctx: &egui::Context, name: &str, fg: Color32) -> Option<TextureHandle> {
        let key = format!("{name}#{:08X}", u32::from_be_bytes(fg.to_array()));
        if let Some((_, t)) = self.font_textures.iter().find(|(k, _)| *k == key) {
            return t.clone();
        }
        let ppp = ctx.pixels_per_point();
        let [r, g, b, a] = fg.to_srgba_unmultiplied();
        let argb = (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
        let tex = sharex_image_editor::editor::render_font_name(name, 14.0 * ppp, argb).map(|pm| {
            let mut rgba = Vec::with_capacity(pm.pixels().len() * 4);
            for c in pm.pixels() {
                rgba.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, *c as u8, (c >> 24) as u8]);
            }
            let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], &rgba);
            ctx.load_texture(format!("font-{key}"), img, egui::TextureOptions::NEAREST)
        });
        self.font_textures.push((key, tex.clone()));
        tex
    }

    /// The shadow options popup: the title (16px SemiBold, the accent
    /// text), the colour row (40 high, 8 below it), then four blocks 8
    /// apart, each a 24px row (the label, 13px SemiBold at its top, and
    /// the value capsule) over a 50px `Slider` whose track is
    /// `Control.Background.Disabled`.
    fn shadow_panel(&mut self, ui: &egui::Ui, p: &Palette, r: Rect, id: Id, ws: &mut Workspace) -> bool {
        let painter = ui.painter();
        let mut changed = false;
        let mut y = r.min.y;
        let title = widgets::font(16.0, 600);
        let g = widgets::galley(ui, ie::AnnotationToolbar_Shadow(), title.clone(), p.accent_text, f32::INFINITY);
        painter.galley(pos2(r.min.x, y), g, p.accent_text);
        y += widgets::line_height(ui, &title) + 8.0;
        let label_font = widgets::font(13.0, 600);
        // `Grid ColumnDefinitions="90,*"`: the label, the colour dropdown.
        let row = Rect::from_min_size(pos2(r.min.x, y), vec2(r.width(), 40.0));
        let g = widgets::galley(ui, ie::AnnotationToolbar_Color(), label_font.clone(), p.text, f32::INFINITY);
        painter.galley(pos2(row.min.x, widgets::center_y(row, widgets::line_height(ui, &label_font))), g, p.text);
        let btn = Rect::from_min_size(pos2(row.max.x - 60.0, row.min.y), vec2(60.0, 40.0));
        let resp = ui.interact(btn, id.with("shadow-color"), Sense::click());
        bordered(ui, btn, 4.0, if resp.hovered() { p.control_background_hover } else { p.control_background }, p.border);
        let sw = Rect::from_min_size(pos2(btn.min.x + 11.0, btn.center().y - 12.0), vec2(24.0, 24.0));
        checkered(painter, sw, 4.0, p);
        painter.rect_filled(sw, CornerRadius::same(4), color_of(&ws.tools.shadow_color));
        icons::paint(painter, Rect::from_min_size(pos2(btn.max.x - 21.0, btn.center().y - 5.0), vec2(10.0, 10.0)), app_icons::ChevronDown, p.text_secondary);
        if resp.clicked() {
            self.shadow_color_open = !self.shadow_color_open;
        }
        y += 40.0 + 8.0 + 8.0;
        let o = &ws.tools.options;
        let rows: [(&str, String, f32, f32, f32, f32); 4] = [
            (ie::AnnotationToolbar_Blur(), format!("{:.0} px", ws.tools.shadow_blur_radius), 0.0, 200.0, 1.0, ws.tools.shadow_blur_radius as f32),
            (ie::AnnotationToolbar_Opacity(), format!("{:.0}%", ws.tools.shadow_opacity * 100.0), 0.0, 1.0, 0.01, ws.tools.shadow_opacity as f32),
            (ie::AnnotationToolbar_OffsetX(), format!("{:.0} px", o.ShadowOffsetX), -200.0, 200.0, 1.0, o.ShadowOffsetX as f32),
            (ie::AnnotationToolbar_OffsetY(), format!("{:.0} px", o.ShadowOffsetY), -200.0, 200.0, 1.0, o.ShadowOffsetY as f32),
        ];
        for (i, (label, value, min, max, step, cur)) in rows.into_iter().enumerate() {
            let lr = Rect::from_min_size(pos2(r.min.x, y), vec2(r.width(), 24.0));
            let g = widgets::galley(ui, label, label_font.clone(), p.text, f32::INFINITY);
            painter.galley(lr.min, g, p.text);
            capsule(ui, painter, p, lr, &value);
            // The 50px slider: its track 25px down.
            let track = y + 24.0 + 25.0;
            let sr = Rect::from_min_size(pos2(r.min.x, track - 14.0), vec2(r.width(), 32.0));
            let mut v = cur;
            let slider = sharex_ui::controls::Slider { minimum: min, maximum: max, step, enabled: true };
            if slider.show_with_track(ui, sr, id.with(("shadow-slider", i)), &mut v, Some(p.control_background_disabled)) && v != cur {
                let t = &mut ws.tools;
                match i {
                    0 => t.set_shadow_blur_radius(v as f64),
                    1 => t.set_shadow_opacity(v as f64),
                    2 => t.options.ShadowOffsetX = v as f64,
                    _ => t.options.ShadowOffsetY = v as f64,
                }
                changed = true;
            }
            y += 24.0 + 50.0 + 8.0;
        }
        if self.shadow_color_open {
            let mut c = color_of(&ws.tools.shadow_color);
            let (ch, keep) = sharex_ui::controls::color_popup(ui, Id::new("editor-shadow-color"), btn, &mut c, resp.clicked());
            if ch {
                ws.tools.set_shadow_color(&hex_of(c));
                changed = true;
            }
            self.shadow_color_open = keep;
        }
        changed
    }
}

/// `AccentColorConverter`: the HSV value moved by 0.1 a step.
fn accent(c: Color32, step: i32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    let v = (max + 0.1 * step as f32).clamp(0.0, 1.0);
    let cc = v * s;
    let x = cc * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - cc;
    let (r1, g1, b1) = match (h / 60.0) as i32 {
        0 => (cc, x, 0.0),
        1 => (x, cc, 0.0),
        2 => (0.0, cc, x),
        3 => (0.0, x, cc),
        4 => (x, 0.0, cc),
        _ => (cc, 0.0, x),
    };
    let to = |v: f32| ((v + m) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(to(r1), to(g1), to(b1), a)
}

/// `Border.popup-border`: its fill, 1px border and the open shadow.
fn paint_frame(ui: &egui::Ui, r: Rect, bg: Color32, border: Color32, radius: f32) {
    // `0 14 28 -14 #66000000`: the shadow of the rectangle shrunk by 14.
    let shadow = egui::epaint::Shadow { offset: [0, 14], blur: 28, spread: 0, color: Color32::from_black_alpha(0x66) };
    ui.painter().add(shadow.as_shape(r.shrink(14.0), CornerRadius::same(radius as u8)));
    bordered(ui, r, radius, bg, border);
}

/// `Button.picker-button`'s background: hovered, or the accent when active.
fn picker_background(painter: &egui::Painter, p: &Palette, r: Rect, hovered: bool, active: bool) {
    if active {
        painter.rect_filled(r, CornerRadius::same(6), p.accent_start);
    } else if hovered {
        painter.rect_filled(r, CornerRadius::same(6), p.control_background_disabled);
    }
}

/// A `UniformGrid` of picker buttons; returns the one clicked.
#[allow(clippy::too_many_arguments)]
fn grid<T: Clone>(
    ui: &egui::Ui,
    p: &Palette,
    r: Rect,
    id: Id,
    items: &[T],
    cols: usize,
    cell: f32,
    active: impl Fn(&T) -> bool,
    paint: impl Fn(&egui::Painter, Rect, &T, Color32),
) -> Option<T> {
    let painter = ui.painter();
    let mut chosen = None;
    for (i, v) in items.iter().enumerate() {
        let ir = Rect::from_min_size(pos2(r.min.x + (i % cols) as f32 * cell, r.min.y + (i / cols) as f32 * cell), vec2(cell, cell));
        let resp = ui.interact(ir, id.with(("item", i)), Sense::click());
        let on = active(v);
        picker_background(painter, p, ir, resp.hovered(), on);
        paint(painter, ir, v, if on { p.accent_foreground } else { p.text });
        if resp.clicked() {
            chosen = Some(v.clone());
        }
    }
    chosen
}

/// A vertical `StackPanel` of 32px picker buttons; returns the one clicked.
fn list<T: Clone>(
    ui: &egui::Ui,
    p: &Palette,
    r: Rect,
    id: Id,
    items: &[T],
    active: impl Fn(&T) -> bool,
    paint: impl Fn(&egui::Painter, Rect, &T, Color32),
) -> Option<T> {
    let painter = ui.painter();
    let mut chosen = None;
    for (i, v) in items.iter().enumerate() {
        let ir = Rect::from_min_size(pos2(r.min.x, r.min.y + i as f32 * 32.0), vec2(r.width(), 32.0));
        let resp = ui.interact(ir, id.with(("item", i)), Sense::click());
        let on = active(v);
        picker_background(painter, p, ir, resp.hovered(), on);
        // `TextBlock`s keep `ShareX.Brush.Text` (a local value).
        paint(painter, ir, v, p.text);
        if resp.clicked() {
            chosen = Some(v.clone());
        }
    }
    chosen
}

fn text_left(ui: &egui::Ui, painter: &egui::Painter, r: Rect, s: &str, size: f32, color: Color32) {
    let font = text_font(size);
    let g = widgets::galley(ui, s, font.clone(), color, f32::INFINITY);
    painter.galley(pos2(r.min.x + 8.0, widgets::center_y(r, widgets::line_height(ui, &font))), g, color);
}

fn text_center(ui: &egui::Ui, painter: &egui::Painter, r: Rect, s: &str, size: f32, color: Color32) {
    let font = text_font(size);
    let w = widgets::text_width(ui, s, font.clone());
    let g = widgets::galley(ui, s, font.clone(), color, f32::INFINITY);
    painter.galley(pos2((r.center().x - w / 2.0).round(), widgets::center_y(r, widgets::line_height(ui, &font))), g, color);
}

/// `Border.capsule`: the value (13px) on the accent with 6,3 padding,
/// right-aligned on its row.
fn capsule(ui: &egui::Ui, painter: &egui::Painter, p: &Palette, row: Rect, s: &str) {
    let font = widgets::font(13.0, 400);
    let w = widgets::text_width(ui, s, font.clone()).ceil() + 12.0;
    let r = Rect::from_min_size(pos2(row.max.x - w, row.min.y), vec2(w, row.height()));
    sharex_ui::border::border(painter, r, 4.0, 0.0, Some(sharex_ui::border::Brush::Diagonal(p.accent_start, p.accent_end)), None);
    let g = widgets::galley(ui, s, font.clone(), p.accent_foreground, f32::INFINITY);
    painter.galley(pos2(r.min.x + 6.0, widgets::center_y(r, widgets::line_height(ui, &font))), g, p.accent_foreground);
}

/// `ShareX.Brush.Checkered.Small` (4px cells).
fn checkered(painter: &egui::Painter, r: Rect, cell: f32, p: &Palette) {
    painter.rect_filled(r, 0.0, p.checkered[0]);
    let clip = painter.with_clip_rect(r.intersect(painter.clip_rect()));
    let (nx, ny) = ((r.width() / cell).ceil() as i32, (r.height() / cell).ceil() as i32);
    for y in 0..ny {
        for x in 0..nx {
            if (x + y) % 2 == 1 {
                clip.rect_filled(Rect::from_min_size(pos2(r.min.x + x as f32 * cell, r.min.y + y as f32 * cell), vec2(cell, cell)), 0.0, p.checkered[1]);
            }
        }
    }
}

pub fn alignment_icon(a: &TextHorizontalAlignment) -> &'static str {
    match a {
        TextHorizontalAlignment::Left => app_icons::AlignLeft,
        TextHorizontalAlignment::Right => app_icons::AlignRight,
        _ => app_icons::AlignCenter,
    }
}

/// `TextHorizontalAlignmentConverter`'s labels.
pub fn alignment_name(a: &TextHorizontalAlignment) -> &'static str {
    match a {
        TextHorizontalAlignment::Left => ie::TextHorizontalAlignmentHelper_Left(),
        TextHorizontalAlignment::Right => ie::TextHorizontalAlignmentHelper_Right(),
        _ => ie::TextHorizontalAlignmentHelper_Center(),
    }
}

/// Fluent UI System Icons used by `ColorView`'s tabs (20px boxes).
const SPECTRUM_ICON: &str = "M3 2C3.27614 2 3.5 2.22386 3.5 2.5V5.5C3.5 5.77614 3.72386 6 4 6H16C16.2761 6 16.5 5.77614 16.5 5.5V2.5C16.5 2.22386 16.7239 2 17 2C17.2761 2 17.5 2.22386 17.5 2.5V5.5C17.5 6.32843 16.8284 7 16 7H15.809L12.2236 14.1708C12.0615 14.4951 11.7914 14.7431 11.4695 14.8802C11.4905 15.0808 11.5 15.2891 11.5 15.5C11.5 16.0818 11.4278 16.6623 11.2268 17.1165C11.019 17.5862 10.6266 18 10 18C9.37343 18 8.98105 17.5862 8.77323 17.1165C8.57222 16.6623 8.5 16.0818 8.5 15.5C8.5 15.2891 8.50952 15.0808 8.53051 14.8802C8.20863 14.7431 7.93851 14.4951 7.77639 14.1708L4.19098 7H4C3.17157 7 2.5 6.32843 2.5 5.5V2.5C2.5 2.22386 2.72386 2 3 2ZM9.11803 14H10.882C11.0714 14 11.2445 13.893 11.3292 13.7236L14.691 7H5.30902L8.67082 13.7236C8.75552 13.893 8.92865 14 9.11803 14ZM9.52346 15C9.50787 15.1549 9.5 15.3225 9.5 15.5C9.5 16.0228 9.56841 16.4423 9.6877 16.7119C9.8002 16.9661 9.90782 17 10 17C10.0922 17 10.1998 16.9661 10.3123 16.7119C10.4316 16.4423 10.5 16.0228 10.5 15.5C10.5 15.3225 10.4921 15.1549 10.4765 15H9.52346Z";
const PALETTE_ICON: &str = "M9.75003 6.5C10.1642 6.5 10.5 6.16421 10.5 5.75C10.5 5.33579 10.1642 5 9.75003 5C9.33582 5 9.00003 5.33579 9.00003 5.75C9.00003 6.16421 9.33582 6.5 9.75003 6.5ZM12.75 7.5C13.1642 7.5 13.5 7.16421 13.5 6.75C13.5 6.33579 13.1642 6 12.75 6C12.3358 6 12 6.33579 12 6.75C12 7.16421 12.3358 7.5 12.75 7.5ZM15.25 9C15.25 9.41421 14.9142 9.75 14.5 9.75C14.0858 9.75 13.75 9.41421 13.75 9C13.75 8.58579 14.0858 8.25 14.5 8.25C14.9142 8.25 15.25 8.58579 15.25 9ZM14.5 12.75C14.9142 12.75 15.25 12.4142 15.25 12C15.25 11.5858 14.9142 11.25 14.5 11.25C14.0858 11.25 13.75 11.5858 13.75 12C13.75 12.4142 14.0858 12.75 14.5 12.75ZM13.25 14C13.25 14.4142 12.9142 14.75 12.5 14.75C12.0858 14.75 11.75 14.4142 11.75 14C11.75 13.5858 12.0858 13.25 12.5 13.25C12.9142 13.25 13.25 13.5858 13.25 14ZM13.6972 2.99169C10.9426 1.57663 8.1432 1.7124 5.77007 3.16636C4.55909 3.9083 3.25331 5.46925 2.51605 7.05899C2.14542 7.85816 1.89915 8.70492 1.90238 9.49318C1.90566 10.2941 2.16983 11.0587 2.84039 11.6053C3.45058 12.1026 3.98165 12.353 4.49574 12.3784C5.01375 12.404 5.41804 12.1942 5.73429 12.0076C5.80382 11.9666 5.86891 11.927 5.93113 11.8892C6.17332 11.7421 6.37205 11.6214 6.62049 11.5426C6.90191 11.4534 7.2582 11.4205 7.77579 11.5787C7.96661 11.637 8.08161 11.7235 8.16212 11.8229C8.24792 11.9289 8.31662 12.0774 8.36788 12.2886C8.41955 12.5016 8.44767 12.7527 8.46868 13.0491C8.47651 13.1594 8.48379 13.2855 8.49142 13.4176C8.50252 13.6098 8.51437 13.8149 8.52974 14.0037C8.58435 14.6744 8.69971 15.4401 9.10362 16.1357C9.51764 16.8488 10.2047 17.439 11.307 17.8158C12.9093 18.3636 14.3731 17.9191 15.5126 17.0169C16.6391 16.125 17.4691 14.7761 17.8842 13.4272C19.1991 9.15377 17.6728 5.03394 13.6972 2.99169ZM6.29249 4.01905C8.35686 2.75426 10.7844 2.61959 13.2403 3.88119C16.7473 5.68275 18.1135 9.28161 16.9284 13.1332C16.5624 14.3227 15.8338 15.4871 14.8919 16.2329C13.963 16.9684 12.8486 17.286 11.6305 16.8696C10.7269 16.5607 10.2467 16.1129 9.96842 15.6336C9.68001 15.1369 9.57799 14.5556 9.52644 13.9225C9.51101 13.733 9.50132 13.5621 9.49147 13.3884C9.48399 13.2564 9.47642 13.1229 9.46618 12.9783C9.44424 12.669 9.41175 12.3499 9.33968 12.0529C9.26719 11.7541 9.14902 11.4527 8.93935 11.1937C8.72439 10.9282 8.43532 10.7346 8.06801 10.6223C7.36648 10.408 6.80266 10.4359 6.31839 10.5893C5.94331 10.7082 5.62016 10.9061 5.37179 11.0582C5.31992 11.0899 5.2713 11.1197 5.22616 11.1463C4.94094 11.3146 4.75357 11.39 4.54514 11.3796C4.33279 11.3691 4.00262 11.2625 3.47218 10.8301C3.0866 10.5158 2.90473 10.0668 2.90237 9.48908C2.89995 8.89865 3.08843 8.20165 3.42324 7.47971C4.09686 6.0272 5.28471 4.63649 6.29249 4.01905Z";
const COMPONENTS_ICON: &str = "M14.95 5C14.7184 3.85888 13.7095 3 12.5 3C11.2905 3 10.2816 3.85888 10.05 5H2.5C2.22386 5 2 5.22386 2 5.5C2 5.77614 2.22386 6 2.5 6H10.05C10.2816 7.14112 11.2905 8 12.5 8C13.7297 8 14.752 7.11217 14.961 5.94254C14.9575 5.96177 14.9539 5.98093 14.95 6H17.5C17.7761 6 18 5.77614 18 5.5C18 5.22386 17.7761 5 17.5 5H14.95ZM12.5 7C11.6716 7 11 6.32843 11 5.5C11 4.67157 11.6716 4 12.5 4C13.3284 4 14 4.67157 14 5.5C14 6.32843 13.3284 7 12.5 7ZM9.94999 14C9.71836 12.8589 8.70948 12 7.5 12C6.29052 12 5.28164 12.8589 5.05001 14H2.5C2.22386 14 2 14.2239 2 14.5C2 14.7761 2.22386 15 2.5 15H5.05001C5.28164 16.1411 6.29052 17 7.5 17C8.70948 17 9.71836 16.1411 9.94999 15H17.5C17.7761 15 18 14.7761 18 14.5C18 14.2239 17.7761 14 17.5 14H9.94999ZM7.5 16C6.67157 16 6 15.3284 6 14.5C6 13.6716 6.67157 13 7.5 13C8.32843 13 9 13.6716 9 14.5C9 15.3284 8.32843 16 7.5 16Z";

/// A `PathIcon` (`Stretch="Uniform"`) of path data in a 20px box.
fn paint_path_icon(painter: &egui::Painter, r: Rect, data: &str, view_box: &str, color: Color32) {
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}"><path d="{data}" fill="currentColor"/></svg>"#);
    let key = format!("colorview-tab-{}", data.len());
    icons::paint_svg(painter, r, &key, &svg, color);
}

/// A `Border` with a background and a 1px border.
fn bordered(ui: &egui::Ui, r: Rect, radius: f32, bg: Color32, border: Color32) {
    sharex_ui::border::border(ui.painter(), r, radius, 1.0, Some(sharex_ui::border::Brush::Solid(bg)), Some(border));
}
