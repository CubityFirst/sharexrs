//! The tool options under the capture toolbar: `AnnotationToolbar.axaml`'s
//! `ToolOptionsBorder` (`ShowToolOptions` while an annotation tool is
//! active), each control as it looks closed: `StrengthSlider`,
//! `ColorPickerDropdown`, `WidthPickerDropdown`, `BorderStylePickerDropdown`,
//! `CornerRadiusPickerDropdown`, `FontFamilyPickerDropdown`,
//! `TextHorizontalAlignmentPickerDropdown`, `ArrowStylePickerDropdown`,
//! `FontSizePickerDropdown`, `StepTypePickerDropdown`, `NumberPickerDropdown`
//! and the toggle buttons.

use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, Ui, pos2, vec2};
use sharex_image_editor::editor::{ToolState, Visible, max_effect_strength};
use sharex_image_editor::model::{ArrowStyle, BorderStyle, CursorType, EditorTool, StepType, TextHorizontalAlignment};
use sharex_strings::image_editor as ie;
use sharex_ui::keys::{app_icons, image_editor_icons, image_effect_icons};
use sharex_ui::{Palette, icons, widgets};

/// One control of the row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionItem {
    Indicator,
    Strength,
    BorderColor,
    FillColor,
    TextColor,
    Thickness,
    BorderStyle,
    CornerRadius,
    FontFamily,
    TextAlignment,
    ArrowStyle,
    CursorType,
    FontSize,
    Bold,
    Italic,
    StepType,
    StepStart,
    SpotlightBlurIcon,
    SpotlightBlur,
    EffectEllipse,
    ArrowRoundedCorners,
    FreehandArrow,
    Tail,
    AutoSize,
    Shadow,
}

impl OptionItem {
    /// The item named as its variant (`Bold`, `Shadow`), for `--render`.
    pub fn from_name(name: &str) -> Option<Self> {
        use OptionItem as O;
        let all = [
            O::Indicator, O::Strength, O::BorderColor, O::FillColor, O::TextColor, O::Thickness, O::BorderStyle,
            O::CornerRadius, O::FontFamily, O::TextAlignment, O::ArrowStyle, O::CursorType, O::FontSize, O::Bold,
            O::Italic, O::StepType, O::StepStart, O::SpotlightBlurIcon, O::SpotlightBlur, O::EffectEllipse,
            O::ArrowRoundedCorners, O::FreehandArrow, O::Tail, O::AutoSize, O::Shadow,
        ];
        all.into_iter().find(|i| format!("{i:?}") == name)
    }

    pub fn is_toggle(self) -> bool {
        matches!(
            self,
            Self::Bold
                | Self::Italic
                | Self::EffectEllipse
                | Self::ArrowRoundedCorners
                | Self::FreehandArrow
                | Self::Tail
                | Self::AutoSize
                | Self::Shadow
        )
    }

    pub fn is_slider(self) -> bool {
        matches!(self, Self::Strength | Self::SpotlightBlur)
    }

    /// `ToolTip.Tip`.
    pub fn tooltip(self, active_tool_name: &str) -> String {
        match self {
            Self::Indicator => active_tool_name.to_string(),
            Self::Strength => ie::AnnotationToolbar_Effectstrength().into(),
            Self::BorderColor => ie::AnnotationToolbar_Bordercolor().into(),
            Self::FillColor => ie::AnnotationToolbar_Fillcolor().into(),
            Self::TextColor => ie::AnnotationToolbar_Textcolor().into(),
            Self::Thickness => ie::AnnotationToolbar_Thickness().into(),
            Self::BorderStyle => ie::AnnotationToolbar_Borderstyle().into(),
            Self::CornerRadius => ie::AnnotationToolbar_Cornerradius().into(),
            Self::FontFamily => ie::AnnotationToolbar_Fontfamily().into(),
            Self::TextAlignment => ie::AnnotationToolbar_Horizontalalignment().into(),
            Self::ArrowStyle => ie::AnnotationToolbar_Arrowstyle().into(),
            Self::CursorType => ie::AnnotationToolbar_Cursortype().into(),
            Self::FontSize => ie::AnnotationToolbar_Fontsize().into(),
            Self::Bold => ie::AnnotationToolbar_Togglebold().into(),
            Self::Italic => ie::AnnotationToolbar_Toggleitalic().into(),
            Self::StepType => ie::AnnotationToolbar_Steptype().into(),
            Self::StepStart => ie::AnnotationToolbar_Startingnumber().into(),
            Self::SpotlightBlurIcon | Self::SpotlightBlur => ie::AnnotationToolbar_Bluramount().into(),
            Self::EffectEllipse => ie::AnnotationToolbar_Toggleellipseshape().into(),
            Self::ArrowRoundedCorners => ie::AnnotationToolbar_Roundedcorners().into(),
            Self::FreehandArrow => ie::AnnotationToolbar_Freehandarrow().into(),
            Self::Tail => ie::AnnotationToolbar_Toggletail().into(),
            Self::AutoSize => ie::AnnotationToolbar_Autosizetotext().into(),
            Self::Shadow => ie::AnnotationToolbar_ToggleshadowShadowoptionsRightclick().into(),
        }
    }
}

/// The row as laid out, in points.
#[derive(Clone, Debug)]
pub struct OptionsLayout {
    pub rect: Rect,
    pub items: Vec<(OptionItem, Rect)>,
}

impl OptionsLayout {
    pub fn hit(&self, p: Pos2) -> Option<OptionItem> {
        if !self.rect.contains(p) {
            return None;
        }
        self.items.iter().find(|(_, r)| r.contains(p)).map(|(i, _)| *i)
    }

    pub fn item_rect(&self, item: OptionItem) -> Option<Rect> {
        self.items.iter().find(|(i, _)| *i == item).map(|(_, r)| *r)
    }
}

/// `StackPanel.toolbar-row`'s spacing and the buttons' size.
const SPACING: f32 = 4.0;
const BUTTON: f32 = 40.0;
/// `Button`'s padding (10,6) and border.
const PAD: f32 = 10.0;
/// `StrengthSlider`: the 24px value, 5px gap, 120px slider, 4px margin.
const SLIDER_WIDTH: f32 = 24.0 + 5.0 + 120.0 + 4.0;

/// The dropdowns' 12px text: Segoe UI Regular (the image editor's
/// controls don't take the app's SemiBold `TextBlock` style; measured).
fn text_font() -> egui::FontId {
    widgets::font(12.0, 400)
}

pub fn border_style_name(s: &BorderStyle) -> &'static str {
    match s {
        BorderStyle::Dash => ie::BorderStyleDisplayConverter_Dash(),
        BorderStyle::Dot => ie::BorderStyleDisplayConverter_Dot(),
        BorderStyle::DashDot => ie::BorderStyleDisplayConverter_DashDot(),
        BorderStyle::DashDotDot => ie::BorderStyleDisplayConverter_DashDotDot(),
        _ => ie::BorderStyleDisplayConverter_Solid(),
    }
}

pub fn arrow_style_name(s: &ArrowStyle) -> &'static str {
    match s {
        ArrowStyle::Modern => ie::ArrowStyleDisplayNameConverter_Modern(),
        ArrowStyle::Double => ie::ArrowStyleDisplayNameConverter_Double(),
        ArrowStyle::Basic => ie::ArrowStyleDisplayNameConverter_Basic(),
        ArrowStyle::Line => ie::ArrowStyleDisplayNameConverter_Line(),
        _ => ie::ArrowStyleDisplayNameConverter_Classic(),
    }
}

/// `StepTypeFormatter.GetPreviewText`.
pub fn step_type_preview(s: &StepType) -> &'static str {
    match s {
        StepType::UppercaseLetter => "A, B, C ...",
        StepType::LowercaseLetter => "a, b, c ...",
        StepType::UppercaseRoman => "I, II, III ...",
        StepType::LowercaseRoman => "i, ii, iii ...",
        _ => "1, 2, 3 ...",
    }
}

/// `{0} pt` of a float as .NET formats it.
fn font_size_text(v: f32) -> String {
    format!("{} pt", v)
}

/// `EditorIcons.ForTool`.
pub fn tool_icon(t: &EditorTool) -> &'static str {
    use EditorTool as T;
    use image_editor_icons as i;
    match t {
        T::Rectangle => i::Rectangle,
        T::Ellipse => i::Ellipse,
        T::Line => i::Line,
        T::Arrow => i::Arrow,
        T::Freehand => i::Freehand,
        T::Text => i::Text,
        T::SpeechBalloon => i::SpeechBalloon,
        T::Step => i::Step,
        T::Image => i::Image,
        T::Clone => i::Clone,
        T::Emoji => i::Emoji,
        T::Cursor => i::Cursor,
        T::Highlight => i::Highlight,
        T::SmartEraser => i::SmartEraser,
        T::Blur => i::Blur,
        T::Pixelate => i::Pixelate,
        T::Magnify => i::Magnify,
        T::Spotlight => i::Spotlight,
        T::Crop => i::Crop,
        T::CutOut => i::CutOut,
        _ => i::Select,
    }
}

/// The controls shown, in `AnnotationToolbar.axaml`'s order, with their
/// widths at scale 1.
fn items(ui: &Ui, v: &Visible, s: &ToolState) -> Vec<(OptionItem, f32)> {
    use OptionItem as O;
    let tw = |t: &str| widgets::text_width(ui, t, text_font());
    let chevron = 10.0;
    let dropdown = |content: f32| 2.0 + 2.0 * PAD + content + chevron;
    let mut out = vec![(O::Indicator, BUTTON)];
    let mut push = |show: bool, item, w: f32| {
        if show {
            out.push((item, w));
        }
    };
    push(v.strength, O::Strength, SLIDER_WIDTH);
    // The buttons' `MinWidth`s (EditorStyles.axaml): colour 60, corner
    // radius 60, font size 70, number 60, step type 90, alignment 58, width 60.
    push(v.border_color, O::BorderColor, dropdown(24.0 + 6.0).max(60.0));
    push(v.fill_color, O::FillColor, dropdown(24.0 + 6.0).max(60.0));
    push(v.text_color, O::TextColor, dropdown(24.0 + 6.0).max(60.0));
    push(v.thickness, O::Thickness, dropdown(24.0 + 6.0).max(60.0));
    push(v.border_style, O::BorderStyle, dropdown(tw(border_style_name(&s.border_style)) + 6.0 + 52.0).clamp(92.0, 168.0));
    push(v.corner_radius, O::CornerRadius, dropdown(24.0 + 6.0).max(60.0));
    push(v.font_family, O::FontFamily, dropdown(tw(&s.font_family) + 6.0).clamp(180.0, 240.0));
    push(v.text_alignment, O::TextAlignment, dropdown(18.0 + 8.0).max(58.0));
    push(v.arrow_style, O::ArrowStyle, dropdown(tw(arrow_style_name(&s.arrow_style)) + 6.0 + 52.0).clamp(80.0, 160.0));
    // The text, then the cursor's 24px preview with 8px margins (the
    // preview, `WindowsCursorBitmapRenderer`, isn't drawn yet).
    push(v.cursor_type, O::CursorType, dropdown(tw(&format!("{:?}", s.cursor_type)) + 6.0 + 40.0).clamp(90.0, 180.0));
    push(v.font_size, O::FontSize, dropdown(tw(&font_size_text(s.font_size)) + 6.0).max(70.0));
    push(v.text_style, O::Bold, BUTTON);
    push(v.text_italic, O::Italic, BUTTON);
    push(v.step_type, O::StepType, dropdown(tw(step_type_preview(&s.step_type)) + 8.0).max(90.0));
    push(v.step_start_number, O::StepStart, dropdown(tw(&s.step_start_number.to_string()) + 4.0 + tw("...")).max(60.0));
    push(v.spotlight_blur, O::SpotlightBlurIcon, BUTTON);
    push(v.spotlight_blur, O::SpotlightBlur, SLIDER_WIDTH);
    push(v.effect_ellipse, O::EffectEllipse, BUTTON);
    push(v.arrow_rounded_corners, O::ArrowRoundedCorners, BUTTON);
    push(v.freehand_arrow, O::FreehandArrow, BUTTON);
    push(v.speech_balloon_tail, O::Tail, BUTTON);
    push(v.speech_balloon_auto_size, O::AutoSize, BUTTON);
    push(v.shadow, O::Shadow, BUTTON);
    out
}

/// Lays the row out under the main toolbar `main` (centred, its top
/// border under the toolbar's bottom one: `Margin="0,-1,0,0"`).
pub fn layout(ui: &Ui, main: Rect, scale: f32, v: &Visible, s: &ToolState) -> Option<OptionsLayout> {
    if !v.any() {
        return None;
    }
    let list = items(ui, v, s);
    let content: f32 = list.iter().map(|(_, w)| w).sum::<f32>() + SPACING * (list.len() as f32 - 1.0);
    // `Padding="4,0,4,4"`, `BorderThickness="1,0,1,1"`.
    let width = (content + 2.0 * 4.0 + 2.0) * scale;
    let height = (BUTTON + 4.0 + 1.0) * scale;
    let left = (main.center().x - width / 2.0).round_ties_even();
    let rect = Rect::from_min_size(pos2(left, main.max.y - 1.0 * scale), vec2(width, height));
    let mut x = rect.min.x + 5.0 * scale;
    let mut items = Vec::new();
    for (item, w) in list {
        items.push((item, Rect::from_min_size(pos2(x, rect.min.y), vec2(w, BUTTON) * scale)));
        x += (w + SPACING) * scale;
    }
    Some(OptionsLayout { rect, items })
}

/// A `Button` (control background, hover background, 1px border, 4px
/// corners) or a `.toolbar-button.active` (the accent).
fn button_frame(painter: &egui::Painter, p: &Palette, rect: Rect, hovered: bool, active: bool, scale: f32) {
    let radius = CornerRadius::same((4.0 * scale).round() as u8);
    if active {
        super::toolbar::fill_accent(painter, rect, radius, p);
    } else {
        painter.rect_filled(rect, radius, if hovered { p.control_background_hover } else { p.control_background });
    }
    painter.rect_stroke(rect, radius, Stroke::new(1.0 * scale, p.border), StrokeKind::Inside);
}

/// `dropdown-chevron`: turned over while its popup is open (`.open`).
fn chevron(painter: &egui::Painter, p: &Palette, rect: Rect, scale: f32, open: bool) {
    let r = Rect::from_min_size(pos2(rect.max.x - (1.0 + PAD + 10.0) * scale, rect.center().y - 5.0 * scale), vec2(10.0, 10.0) * scale);
    let angle = if open { std::f32::consts::PI } else { 0.0 };
    icons::paint_transformed(painter, r, app_icons::ChevronDown, p.text_secondary, vec2(0.0, 0.0), angle);
}

/// The content's left edge and vertical centre.
fn content_origin(rect: Rect, scale: f32) -> (f32, f32) {
    (rect.min.x + (1.0 + PAD) * scale, rect.center().y)
}

fn text(ui: &Ui, painter: &egui::Painter, x: f32, rect: Rect, s: &str, color: Color32, scale: f32) {
    let font = egui::FontId::new(12.0 * scale, text_font().family);
    let g = widgets::galley(ui, s, font.clone(), color, f32::INFINITY);
    let y = widgets::center_y(rect, widgets::line_height(ui, &font));
    painter.galley(pos2(x, y), g, color);
}

#[allow(clippy::too_many_arguments)]
/// The 24x24 swatch of a colour dropdown: the colour over the small
/// checkerboard, with the border colour's hole or the text colour's "A".
fn swatch(ui: &Ui, painter: &egui::Painter, p: &Palette, at: Rect, color: Color32, hole: bool, label: bool, scale: f32) {
    checkered(painter, p, at, 4.0 * scale, scale);
    painter.rect_filled(at, CornerRadius::same((4.0 * scale) as u8), color);
    if hole {
        let inner = at.shrink(5.0 * scale);
        checkered(painter, p, inner, 2.0 * scale, scale);
    }
    if label {
        let font = egui::FontId::new(14.0 * scale, widgets::font(14.0, 700).family);
        let lum = 0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32;
        let fg = if color.a() < 128 { p.text } else if lum > 140.0 { Color32::BLACK } else { Color32::WHITE };
        let g = widgets::galley(ui, "A", font.clone(), fg, f32::INFINITY);
        let pos = pos2(at.center().x - g.size().x / 2.0, widgets::center_y(at, widgets::line_height(ui, &font)));
        painter.galley(pos, g, fg);
    }
}

/// `ShareX.Brush.Checkered.Small` with rounded corners.
fn checkered(painter: &egui::Painter, p: &Palette, rect: Rect, radius: f32, scale: f32) {
    painter.rect_filled(rect, CornerRadius::same(radius as u8), p.checkered[0]);
    let clip = painter.with_clip_rect(rect.shrink(radius / 2.0).intersect(painter.clip_rect()));
    let cell = 4.0 * scale;
    let (nx, ny) = ((rect.width() / cell).ceil() as i32, (rect.height() / cell).ceil() as i32);
    for y in 0..ny {
        for x in 0..nx {
            if (x + y) % 2 == 1 {
                let r = Rect::from_min_size(pos2(rect.min.x + x as f32 * cell, rect.min.y + y as f32 * cell), vec2(cell, cell));
                clip.rect_filled(r, 0.0, p.checkered[1]);
            }
        }
    }
}

fn parse(hex: &str) -> Color32 {
    let c = sharex_image_editor::render::parse_color(hex);
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

#[allow(clippy::too_many_arguments)]
/// Draws the row; `open` is the item whose popup is open.
pub fn paint(
    ui: &Ui,
    painter: &egui::Painter,
    p: &Palette,
    layout: &OptionsLayout,
    tool: &EditorTool,
    s: &ToolState,
    hovered: Option<OptionItem>,
    open: Option<OptionItem>,
    scale: f32,
) {
    let r = (4.0 * scale).round() as u8;
    let radius = CornerRadius { nw: 0, ne: 0, sw: r, se: r };
    painter.rect_filled(layout.rect, radius, p.background_toolbar);
    // `BorderThickness="1,0,1,1"`: no top border.
    let b = layout.rect;
    let mut border = painter.clone();
    border.set_clip_rect(Rect::from_min_max(pos2(b.min.x, b.min.y + 1.0 * scale), b.max).intersect(painter.clip_rect()));
    border.rect_stroke(b, radius, Stroke::new(1.0 * scale, p.border), StrokeKind::Inside);
    for (item, rect) in &layout.items {
        let rect = *rect;
        let over = hovered == Some(*item);
        let icon_at = |size: f32| Rect::from_center_size(rect.center(), vec2(size, size) * scale);
        let (cx, _) = content_origin(rect, scale);
        let sw = || Rect::from_min_size(pos2(cx, rect.center().y - 12.0 * scale), vec2(24.0, 24.0) * scale);
        match item {
            OptionItem::Indicator => icons::paint(painter, icon_at(24.0), tool_icon(tool), p.accent_icon),
            OptionItem::SpotlightBlurIcon => icons::paint(painter, icon_at(24.0), image_editor_icons::Blur, p.text_secondary),
            OptionItem::Strength | OptionItem::SpotlightBlur => {
                let (value, max) = if *item == OptionItem::Strength {
                    (s.effect_strength, max_effect_strength(tool.clone()))
                } else {
                    (s.spotlight_blur, 200.0)
                };
                let label = Rect::from_min_size(rect.min, vec2(24.0, BUTTON) * scale);
                let t = format!("{}", value.round() as i64);
                let tw = widgets::text_width(ui, &t, egui::FontId::new(12.0 * scale, text_font().family));
                text(ui, painter, label.center().x - tw / 2.0, rect, &t, p.text_secondary, scale);
                // The 50px-high slider centred in the 40px row.
                let slider = Rect::from_min_size(pos2(rect.min.x + 29.0 * scale, rect.center().y - 25.0 * scale), vec2(120.0, 50.0) * scale);
                let mut v = value;
                let min = if *item == OptionItem::Strength { 1.0 } else { 0.0 };
                sharex_ui::controls::Slider { minimum: min, maximum: max, step: 1.0, enabled: true }.show(
                    ui,
                    // The track on the row's centre (measured).
                    Rect::from_min_size(pos2(slider.min.x, slider.center().y - 14.0 * scale), vec2(120.0, 32.0) * scale),
                    egui::Id::new(("option-slider", *item as u8)),
                    &mut v,
                );
            }
            OptionItem::BorderColor | OptionItem::FillColor | OptionItem::TextColor => {
                button_frame(painter, p, rect, over, false, scale);
                let c = match item {
                    OptionItem::BorderColor => &s.selected_color,
                    OptionItem::FillColor => &s.fill_color,
                    _ => &s.text_color,
                };
                swatch(ui, painter, p, sw(), parse(c), *item == OptionItem::BorderColor, *item == OptionItem::TextColor, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::Thickness => {
                button_frame(painter, p, rect, over, false, scale);
                let d = (s.stroke_width as f32).min(24.0) * scale;
                painter.circle_filled(sw().center(), d / 2.0, p.text_secondary);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::BorderStyle => {
                button_frame(painter, p, rect, over, false, scale);
                text(ui, painter, cx, rect, border_style_name(&s.border_style), p.text_secondary, scale);
                // The 36x14 preview path `M 2,7 L 34,7`, 8px before the chevron.
                let right = rect.max.x - (1.0 + PAD + 10.0 + 8.0) * scale;
                let (x0, y) = (right - 36.0 * scale + 2.0 * scale, rect.center().y);
                dashed_preview(painter, x0, right - 2.0 * scale, y, &s.border_style, p.text_secondary, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::CornerRadius => {
                button_frame(painter, p, rect, over, false, scale);
                let at = Rect::from_center_size(sw().center(), vec2(20.0, 20.0) * scale);
                corner_preview(painter, at, s.corner_radius as f32 * scale, p.text_secondary, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::FontFamily => {
                button_frame(painter, p, rect, over, false, scale);
                text(ui, painter, cx, rect, &s.font_family, p.text_secondary, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::TextAlignment => {
                button_frame(painter, p, rect, over, false, scale);
                let key = match s.text_alignment {
                    TextHorizontalAlignment::Left => app_icons::AlignLeft,
                    TextHorizontalAlignment::Right => app_icons::AlignRight,
                    _ => app_icons::AlignCenter,
                };
                icons::paint(painter, Rect::from_min_size(pos2(cx, rect.center().y - 9.0 * scale), vec2(18.0, 18.0) * scale), key, p.text);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::ArrowStyle => {
                button_frame(painter, p, rect, over, false, scale);
                text(ui, painter, cx, rect, arrow_style_name(&s.arrow_style), p.text_secondary, scale);
                let right = rect.max.x - (1.0 + PAD + 10.0 + 8.0) * scale;
                arrow_preview(painter, right - 36.0 * scale, rect.center().y, p.text_secondary, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::CursorType => {
                button_frame(painter, p, rect, over, false, scale);
                text(ui, painter, cx, rect, &format!("{:?}", s.cursor_type), p.text_secondary, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::FontSize => {
                button_frame(painter, p, rect, over, false, scale);
                text(ui, painter, cx, rect, &font_size_text(s.font_size), p.text_secondary, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::StepType => {
                button_frame(painter, p, rect, over, false, scale);
                text(ui, painter, cx, rect, step_type_preview(&s.step_type), p.text, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            OptionItem::StepStart => {
                button_frame(painter, p, rect, over, false, scale);
                let n = s.step_start_number.to_string();
                text(ui, painter, cx, rect, &n, p.text, scale);
                let nw = widgets::text_width(ui, &n, egui::FontId::new(12.0 * scale, text_font().family));
                text(ui, painter, cx + nw + 4.0 * scale, rect, "...", p.text_secondary, scale);
                chevron(painter, p, rect, scale, open == Some(*item));
            }
            toggle => {
                let (active, key) = match toggle {
                    OptionItem::Bold => (s.text_bold, app_icons::Bold),
                    OptionItem::Italic => (s.text_italic, app_icons::Italic),
                    OptionItem::EffectEllipse => (s.effect_ellipse, image_editor_icons::Ellipse),
                    OptionItem::ArrowRoundedCorners => (s.arrow_rounded_corners, image_effect_icons::RoundedCorner),
                    OptionItem::FreehandArrow => (s.freehand_arrow, image_editor_icons::Arrow),
                    OptionItem::Tail => (s.speech_balloon_tail, app_icons::MessageCheck),
                    OptionItem::AutoSize => (s.speech_balloon_auto_size, app_icons::Shrink),
                    _ => (s.shadow_enabled, app_icons::Theme),
                };
                button_frame(painter, p, rect, over, active, scale);
                let fg = if active { p.accent_foreground } else { p.text };
                icons::paint(painter, icon_at(20.0), key, fg);
            }
        }
    }
}

/// The border style preview: a 1.75px line with the style's dashes.
pub fn dashed_preview(painter: &egui::Painter, x0: f32, x1: f32, y: f32, style: &BorderStyle, color: Color32, scale: f32) {
    let w = 1.75 * scale;
    let dashes: &[f32] = match style {
        BorderStyle::Dash => &[3.0, 1.0],
        BorderStyle::Dot => &[1.0, 1.0],
        BorderStyle::DashDot => &[3.0, 1.0, 1.0, 1.0],
        BorderStyle::DashDotDot => &[3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
        _ => &[],
    };
    if dashes.is_empty() {
        painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(w, color));
        return;
    }
    let mut x = x0;
    let mut i = 0;
    while x < x1 {
        let len = dashes[i % dashes.len()] * w;
        if i % 2 == 0 {
            painter.line_segment([pos2(x, y), pos2((x + len).min(x1), y)], Stroke::new(w, color));
        }
        x += len;
        i += 1;
    }
}

/// `CornerRadiusPreviewGeometryConverter` at 20: a corner from bottom-left
/// to top-right with the radius, 2.5px with round joins.
pub fn corner_preview(painter: &egui::Painter, at: Rect, radius: f32, color: Color32, scale: f32) {
    let size = at.width();
    let r = radius.clamp(0.0, size);
    let stroke = Stroke::new(2.5 * scale, color);
    let o = at.min;
    if r <= 0.01 {
        painter.add(egui::Shape::line(vec![o + vec2(0.0, size), o, o + vec2(size, 0.0)], stroke));
        return;
    }
    let mut pts = vec![o + vec2(0.0, size), o + vec2(0.0, r)];
    for i in 1..=12 {
        let a = std::f32::consts::PI + std::f32::consts::FRAC_PI_2 * i as f32 / 12.0;
        pts.push(o + vec2(r + r * a.cos(), r + r * a.sin()));
    }
    pts.push(o + vec2(size, 0.0));
    painter.add(egui::Shape::line(pts, stroke));
}

/// `CursorTypeDisplayNameConverter`.
pub fn cursor_type_name(c: &CursorType) -> &'static str {
    use CursorType as C;
    match c {
        C::AppStarting => ie::CursorTypeDisplayNameConverter_AppStarting(),
        C::Arrow => ie::CursorTypeDisplayNameConverter_Arrow(),
        C::Cross => ie::CursorTypeDisplayNameConverter_Cross(),
        C::Hand => ie::CursorTypeDisplayNameConverter_Hand(),
        C::Help => ie::CursorTypeDisplayNameConverter_Help(),
        C::HSplit => ie::CursorTypeDisplayNameConverter_HSplit(),
        C::IBeam => ie::CursorTypeDisplayNameConverter_IBeam(),
        C::No => ie::CursorTypeDisplayNameConverter_No(),
        C::NoMove2D => ie::CursorTypeDisplayNameConverter_NoMove2D(),
        C::NoMoveHoriz => ie::CursorTypeDisplayNameConverter_NoMoveHoriz(),
        C::NoMoveVert => ie::CursorTypeDisplayNameConverter_NoMoveVert(),
        C::PanEast => ie::CursorTypeDisplayNameConverter_PanEast(),
        C::PanNE => ie::CursorTypeDisplayNameConverter_PanNE(),
        C::PanNorth => ie::CursorTypeDisplayNameConverter_PanNorth(),
        C::PanNW => ie::CursorTypeDisplayNameConverter_PanNW(),
        C::PanSE => ie::CursorTypeDisplayNameConverter_PanSE(),
        C::PanSouth => ie::CursorTypeDisplayNameConverter_PanSouth(),
        C::PanSW => ie::CursorTypeDisplayNameConverter_PanSW(),
        C::PanWest => ie::CursorTypeDisplayNameConverter_PanWest(),
        C::SizeAll => ie::CursorTypeDisplayNameConverter_SizeAll(),
        C::SizeNESW => ie::CursorTypeDisplayNameConverter_SizeNESW(),
        C::SizeNS => ie::CursorTypeDisplayNameConverter_SizeNS(),
        C::SizeNWSE => ie::CursorTypeDisplayNameConverter_SizeNWSE(),
        C::SizeWE => ie::CursorTypeDisplayNameConverter_SizeWE(),
        C::UpArrow => ie::CursorTypeDisplayNameConverter_UpArrow(),
        C::VSplit => ie::CursorTypeDisplayNameConverter_VSplit(),
        C::WaitCursor => ie::CursorTypeDisplayNameConverter_WaitCursor(),
        _ => ie::CursorTypeDisplayNameConverter_Default(),
    }
}

/// `ArrowStylePreviewGeometryConverter`: a 36x14 arrow of `style` (1.75px,
/// round caps), drawn by the editor's own renderer, its left edge at `x`.
pub fn arrow_style_preview(ui: &Ui, painter: &egui::Painter, x: f32, y: f32, style: &ArrowStyle, color: Color32, scale: f32) {
    let ppp = ui.ctx().pixels_per_point() * scale;
    let (w, h) = ((36.0 * ppp).round() as u32, (14.0 * ppp).round() as u32);
    let key = format!("arrow-preview-{style:?}-{w}x{h}-{:08X}", u32::from_be_bytes(color.to_array()));
    let tex = ui.ctx().data_mut(|d| d.get_temp::<egui::TextureHandle>(egui::Id::new(&key))).or_else(|| {
        let mut a = sharex_image_editor::model::Annotation::new("Arrow")?;
        a.common.stroke_width = 1.75 * ppp;
        let [r, g, b, al] = color.to_srgba_unmultiplied();
        a.common.stroke_color = format!("#{al:02X}{r:02X}{g:02X}{b:02X}");
        a.common.start_point = sharex_image_editor::model::Point::new(2.0 * ppp, 7.0 * ppp);
        a.common.end_point = sharex_image_editor::model::Point::new((36.0 - 2.0) * ppp, 7.0 * ppp);
        if let sharex_image_editor::model::Kind::Arrow(p) = &mut a.kind {
            p.style = style.clone();
        }
        let pm = sharex_image_editor::render::render(&sharex_skia::Pixmap::new(w, h), &[a]);
        let mut rgba = Vec::with_capacity(pm.pixels().len() * 4);
        for c in pm.pixels() {
            rgba.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, *c as u8, (c >> 24) as u8]);
        }
        let img = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba);
        let tex = ui.ctx().load_texture(key.clone(), img, egui::TextureOptions::NEAREST);
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(&key), tex.clone()));
        Some(tex)
    });
    if let Some(tex) = tex {
        let r = Rect::from_min_size(pos2(x, (y - 7.0 * scale).round()), vec2(36.0, 14.0) * scale);
        painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
}

/// The classic arrow preview: a 1.75px line from 2 to 34 with its head.
fn arrow_preview(painter: &egui::Painter, x: f32, y: f32, color: Color32, scale: f32) {
    let w = 1.75 * scale;
    let (x0, x1) = (x + 2.0 * scale, x + 34.0 * scale);
    let head = w * 2.0 * 3.0;
    painter.line_segment([pos2(x0, y), pos2(x1 - head * 0.5, y)], Stroke::new(w, color));
    painter.add(egui::Shape::convex_polygon(
        vec![pos2(x1, y), pos2(x1 - head, y - head / 2.0), pos2(x1 - head, y + head / 2.0)],
        color,
        Stroke::NONE,
    ));
}
