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

use egui::{Color32, CornerRadius, Id, Order, Pos2, Rect, Sense, TextureHandle, pos2, vec2};
use sharex_image_editor::editor::{ToolState, Workspace, system_font_families};
use sharex_image_editor::model::{ArrowStyle, BorderStyle, CursorType, StepType, TextHorizontalAlignment};
use sharex_strings::image_editor as ie;
use sharex_ui::keys::app_icons;
use sharex_ui::{Palette, color_view, icons, widgets};

use crate::region::options_row::{self, OptionItem};

/// `EditorRecentColors.Add` (shared with every `ColorPickerPanel`).
pub use sharex_ui::color_view::{Rgba8, add_recent_color};

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

/// The open popup and what it keeps while open.
#[derive(Default)]
pub struct Popups {
    pub open: Option<OptionItem>,
    /// Opened this frame: the press that opened it isn't a light dismiss.
    just_opened: bool,
    color_when_opened: Option<Rgba8>,
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

/// `Color.Parse` of an option's hex, unmultiplied.
fn color_of(hex: &str) -> Rgba8 {
    let c = sharex_image_editor::render::parse_color(hex);
    [c.r, c.g, c.b, c.a]
}

/// `#AARRGGBB` of a colour (the options' format).
fn hex_of(c: Rgba8) -> String {
    let [r, g, b, a] = c;
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

    fn current_color(&self, item: OptionItem, s: &ToolState) -> Option<Rgba8> {
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
                    let mut c = self.current_color(item, &ws.tools).unwrap_or([0; 4]);
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
                        options_row::dashed_preview(painter, right - 36.0 + 2.0, right - 2.0, r.center().y, v, p.text, 1.0);
                    }) {
                        ws.set_option(|s, t| s.set_border_style(v, t));
                        close = true;
                    }
                }
                OptionItem::ArrowStyle => {
                    let cur = ws.tools.arrow_style.clone();
                    if let Some(v) = list(ui, p, inner.shrink(8.0), id, &ARROW_STYLES, |v| *v == cur, |painter, r, v, fg| {
                        text_left(ui, painter, r, options_row::arrow_style_name(v), 14.0, fg);
                        options_row::arrow_style_preview(ui, painter, r.max.x - 8.0 - 36.0, r.center().y, v, p.text, 1.0);
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
                    // Its colour dropdown's screen colour picker (the
                    // dropdown closed for it; the shadow popup stays).
                    if color_view::take_screen_pick(ui.ctx(), Id::new("editor-shadow-color")) {
                        result.pick_screen_color = true;
                    }
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
            && ctx.memory(|m| m.area_rect(Id::new("editor-shadow-color").with("color-popup"))).is_some_and(|r| r.contains(at))
    }

    /// The popup's size for `item`, in points.
    fn size(&self, ui: &egui::Ui, item: OptionItem, s: &ToolState) -> egui::Vec2 {
        let tw = |t: &str, size: f32| widgets::text_width(ui, t, text_font(size)).ceil();
        let frame = |w: f32, h: f32| vec2(w + 18.0, h + 18.0);
        let list_w = |min: f32, widest: f32| min.max(widest);
        match item {
            OptionItem::BorderColor | OptionItem::FillColor | OptionItem::TextColor => color_view::panel_size(color_view::Px::of(ui.ctx())) + vec2(10.0, 10.0),
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

    /// `ColorPickerPanel` (`sharex_ui::color_view`): Avalonia's `ColorView`
    /// on its palette tab, then the screen colour picker button and the
    /// recent colours. Returns whether the colour changed and whether the
    /// screen picker was asked for.
    fn color_panel(&mut self, ui: &mut egui::Ui, _p: &Palette, r: Rect, id: Id, c: &mut Rgba8) -> (bool, bool) {
        color_view::color_picker_panel(ui, r.min, id, c)
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
        painter.rect_filled(sw, CornerRadius::same(4), color_view::to_color32(color_of(&ws.tools.shadow_color)));
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

/// A `ColorPickerDropdown` of its own (not an option of the tools): the
/// New image dialog's background colour, the Background panel's colours.
/// The same `ColorPickerPanel` popup as the options' colour dropdowns.
#[derive(Default)]
pub struct ColorDropdown {
    popups: Popups,
    pub open: bool,
}

impl ColorDropdown {
    /// The dropdown button's click.
    pub fn toggle(&mut self, color: Rgba8) {
        if self.open {
            self.open = false;
            return;
        }
        self.open = true;
        self.popups.just_opened = true;
        self.popups.color_when_opened = Some(color);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.popups.rect = None;
    }

    /// Whether `p` is on the open popup.
    pub fn contains(&self, p: Pos2) -> bool {
        self.open && self.popups.contains(p)
    }

    /// Draws the open popup under `anchor` within `bounds`, editing
    /// `color`; closes it on Escape or a press outside it and `anchor`.
    pub fn show(&mut self, ui: &egui::Ui, p: &Palette, anchor: Rect, bounds: Rect, key: &str, color: &mut Rgba8) -> PopupResult {
        let mut result = PopupResult::default();
        if !self.open {
            return result;
        }
        let id = Id::new(("color-dropdown", key));
        let mut size = color_view::panel_size(color_view::Px::of(ui.ctx())) + vec2(10.0, 10.0);
        size.y = size.y.min(bounds.height());
        let mut pos = pos2((anchor.center().x - size.x / 2.0).round(), anchor.max.y + 4.0);
        if pos.y + size.y > bounds.max.y {
            pos.y = (bounds.max.y - size.y).max(bounds.min.y);
        }
        pos.x = pos.x.clamp(bounds.min.x, (bounds.max.x - size.x).max(bounds.min.x));
        self.popups.rect = Some(Rect::from_min_size(pos, size));
        let mut close = false;
        let popups = &mut self.popups;
        let area = egui::Area::new(id).order(Order::Foreground).fixed_pos(pos).fade_in(false).show(ui.ctx(), |ui| {
            let (r, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.set_clip_rect(r.expand(40.0));
            paint_frame(ui, r, p.background_panel, p.border, 8.0);
            let (changed, pick) = popups.color_panel(ui, p, r.shrink(1.0).shrink(4.0), id, color);
            result.changed = changed;
            if pick {
                result.pick_screen_color = true;
                close = true;
            }
        });
        let ctx = ui.ctx();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if !self.popups.just_opened
            && ctx.input(|i| i.pointer.any_pressed())
            && let Some(at) = ctx.input(|i| i.pointer.interact_pos())
            && !area.response.rect.contains(at)
            && !anchor.contains(at)
        {
            close = true;
        }
        self.popups.just_opened = false;
        if close {
            if self.popups.color_when_opened.is_some_and(|b| b != *color) {
                add_recent_color(*color);
            }
            self.close();
        }
        result
    }
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
/// `paint` gets the colour of the item's text and icons.
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
        // The item template's `Foreground="{DynamicResource
        // ShareX.Brush.Text}"` is set at Template priority, so on the active
        // row `Button.active /template/ ContentPresenter TextBlock` (and
        // `IconPresenter`) win with the accent foreground; the previews'
        // `Path` strokes keep `ShareX.Brush.Text` (`--control-dump
        // borderstyle`).
        paint(painter, ir, v, if on { p.accent_foreground } else { p.text });
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

/// A `Border` with a background and a 1px border.
fn bordered(ui: &egui::Ui, r: Rect, radius: f32, bg: Color32, border: Color32) {
    sharex_ui::border::border(ui.painter(), r, radius, 1.0, Some(sharex_ui::border::Brush::Solid(bg)), Some(border));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The options' `#AARRGGBB` round-trips colours with partial alpha
    /// exactly (`Color32` premultiplied them).
    #[test]
    fn option_colours_keep_partial_alpha() {
        for a in [0u8, 1, 64, 128, 166, 254, 255] {
            for v in 0..=255u8 {
                for c in [[v, 169, 225, a], [54, v, 225, a], [54, 169, v, a]] {
                    assert_eq!(color_of(&hex_of(c)), c, "{c:?}");
                }
            }
        }
        assert_eq!(hex_of([54, 169, 225, 166]), "#A636A9E1");
    }
}
