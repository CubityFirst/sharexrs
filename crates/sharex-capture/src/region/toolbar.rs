//! The capture toolbar: `RegionCaptureWindow.axaml`'s `CaptureToolbar`
//! (drag handle and rectangle region button) leading the image editor's
//! `AnnotationToolbar` (`AnnotationToolbar.axaml`) with the default tool
//! items (`ToolbarCustomizationItemViewModel`, without Crop and Cut out,
//! `RegionCaptureToolbar.CreateItems`); the active tool shows as active
//! (`ToolbarCustomizationItemViewModel.IsActive`) and its options row
//! ([`super::options_row`]) hangs under it.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use sharex_image_editor::model::EditorTool;
use sharex_strings::{image_editor as ie, screen_capture_lib as sc};
use sharex_ui::keys::image_editor_icons as icon;
use sharex_ui::{Palette, icons};

pub use crate::toolbar_items::Item;

/// An annotation tool on the toolbar (`ToolbarItemDefinition`).
#[derive(Clone, Debug)]
pub struct ToolItem {
    pub id: &'static str,
    pub tool: EditorTool,
    pub name: fn() -> &'static str,
    pub icon: &'static str,
    pub hotkey: &'static str,
    pub secondary_hotkey: &'static str,
    /// A separator before it (`BeginGroupByDefault`).
    pub begin_group: bool,
}

const fn tool(
    id: &'static str,
    tool: EditorTool,
    name: fn() -> &'static str,
    icon: &'static str,
    hotkey: &'static str,
    secondary_hotkey: &'static str,
) -> ToolItem {
    ToolItem { id, tool, name, icon, hotkey, secondary_hotkey, begin_group: false }
}

/// The default items, in order: every tool but Crop and Cut out (which
/// region capture leaves out), Select starting a group.
pub const TOOLS: [ToolItem; 19] = [
    ToolItem {
        id: "Select",
        tool: EditorTool::Select,
        name: ie::ToolbarCustomizationItemViewModel_Select,
        icon: icon::Select,
        hotkey: "V",
        secondary_hotkey: "",
        begin_group: true,
    },
    tool("Rectangle", EditorTool::Rectangle, ie::ToolbarCustomizationItemViewModel_Rectangle, icon::Rectangle, "R", "NumPad1"),
    tool("Ellipse", EditorTool::Ellipse, ie::ToolbarCustomizationItemViewModel_Ellipse, icon::Ellipse, "E", "NumPad2"),
    tool("Line", EditorTool::Line, ie::ToolbarCustomizationItemViewModel_Line, icon::Line, "L", "NumPad4"),
    tool("Arrow", EditorTool::Arrow, ie::ToolbarCustomizationItemViewModel_Arrow, icon::Arrow, "A", "NumPad5"),
    tool("Freehand", EditorTool::Freehand, ie::ToolbarCustomizationItemViewModel_Freehand, icon::Freehand, "F", "NumPad3"),
    tool("Text", EditorTool::Text, ie::ToolbarCustomizationItemViewModel_Text, icon::Text, "T", "NumPad6"),
    tool("SpeechBalloon", EditorTool::SpeechBalloon, ie::ToolbarCustomizationItemViewModel_SpeechBalloon, icon::SpeechBalloon, "O", ""),
    tool("Step", EditorTool::Step, ie::ToolbarCustomizationItemViewModel_Step, icon::Step, "N", "NumPad7"),
    tool("Image", EditorTool::Image, ie::ToolbarCustomizationItemViewModel_Image, icon::Image, "I", ""),
    tool("Clone", EditorTool::Clone, ie::ToolbarCustomizationItemViewModel_Clone, icon::Clone, "D", ""),
    tool("Emoji", EditorTool::Emoji, ie::ToolbarCustomizationItemViewModel_Emoji, icon::Emoji, "J", ""),
    tool("Cursor", EditorTool::Cursor, ie::ToolbarCustomizationItemViewModel_Cursor, icon::Cursor, "K", ""),
    tool("Highlight", EditorTool::Highlight, ie::ToolbarCustomizationItemViewModel_Highlight, icon::Highlight, "H", ""),
    tool("SmartEraser", EditorTool::SmartEraser, ie::ToolbarCustomizationItemViewModel_SmartEraser, icon::SmartEraser, "W", ""),
    tool("Blur", EditorTool::Blur, ie::ToolbarCustomizationItemViewModel_Blur, icon::Blur, "B", "NumPad8"),
    tool("Pixelate", EditorTool::Pixelate, ie::ToolbarCustomizationItemViewModel_Pixelate, icon::Pixelate, "P", "NumPad9"),
    tool("Magnify", EditorTool::Magnify, ie::ToolbarCustomizationItemViewModel_Magnify, icon::Magnify, "M", ""),
    tool("Spotlight", EditorTool::Spotlight, ie::ToolbarCustomizationItemViewModel_Spotlight, icon::Spotlight, "S", ""),
];

impl ToolItem {
    /// `ToolTip`: the name and its keys, `Rectangle (R, NumPad1)`.
    pub fn tooltip(&self) -> String {
        let keys: Vec<&str> = [self.hotkey, self.secondary_hotkey].into_iter().filter(|k| !k.is_empty()).collect();
        if keys.is_empty() { (self.name)().to_string() } else { format!("{} ({})", (self.name)(), keys.join(", ")) }
    }
}

/// `Button.toolbar-button`: 40 wide, 40 high (`Button`'s height).
const BUTTON: f32 = 40.0;
/// `StackPanel.toolbar-row`'s spacing.
const SPACING: f32 = 4.0;
/// `Border#MainToolbarBorder`'s padding.
const PADDING: f32 = 4.0;
/// `Border.toolbar-drag-handle`'s width.
const HANDLE: f32 = 14.0;
/// The icon in a toolbar button (`FontSize="20"`).
const ICON: f32 = 20.0;

/// `RegionCaptureToolbar.GetRegionShape`: the region an item selects.
pub fn region_shape(id: &str) -> Option<super::RegionShape> {
    if id.eq_ignore_ascii_case(crate::toolbar_items::REGION_ELLIPSE) {
        Some(super::RegionShape::Ellipse)
    } else if id.eq_ignore_ascii_case(crate::toolbar_items::REGION_FREEHAND) {
        Some(super::RegionShape::Freehand)
    } else {
        None
    }
}

/// Something on the toolbar under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    DragHandle,
    RegionButton,
    Tool(usize),
    /// A control of the tool options row.
    Option(super::options_row::OptionItem),
    /// The toolbar's background (not a button).
    Background,
    /// An open option popup.
    Popup,
}

/// Where the toolbar's parts are, in points.
#[derive(Clone, Debug)]
pub struct Layout {
    pub rect: Rect,
    pub handle: Rect,
    pub region_button: Rect,
    pub tools: Vec<Rect>,
    pub separators: Vec<Rect>,
}

/// The toolbar's size at `scale` (the custom button size's scale) with
/// `items` (the visible ones, `VisibleToolbarItems`).
pub fn size(scale: f32, items: &[Item]) -> egui::Vec2 {
    let row = HANDLE + SPACING + BUTTON + SPACING + tools_width(items);
    vec2(row + 2.0 * (PADDING + 1.0), BUTTON + 2.0 * (PADDING + 1.0)) * scale
}

fn tools_width(items: &[Item]) -> f32 {
    items.iter().enumerate().map(|(i, t)| {
        let sep = if t.separator() { t.separator_width() + SPACING } else { 0.0 };
        sep + BUTTON + if i > 0 { SPACING } else { 0.0 }
    }).sum()
}

/// The capture toolbar's items from `RegionCaptureOptions.ToolbarItems`
/// (`RegionCaptureToolbar.CreateItems`): the visible ones, in order.
pub fn items(saved: &[serde_json::Value]) -> Vec<Item> {
    crate::toolbar_items::region_items(saved).into_iter().filter(|i| i.visible).collect()
}

/// The default items.
pub fn default_items() -> Vec<Item> {
    items(&[])
}

/// Lays the toolbar out with its top-left corner at `min`.
pub fn layout(min: Pos2, scale: f32, items: &[Item]) -> Layout {
    let s = scale;
    let rect = Rect::from_min_size(min, size(s, items));
    let top = min.y + (1.0 + PADDING) * s;
    let mut x = min.x + (1.0 + PADDING) * s;
    let handle = Rect::from_min_size(pos2(x, top), vec2(HANDLE, BUTTON) * s);
    x += (HANDLE + SPACING) * s;
    let region_button = Rect::from_min_size(pos2(x, top), vec2(BUTTON, BUTTON) * s);
    x += (BUTTON + SPACING) * s;
    let mut tools = Vec::new();
    let mut separators = Vec::new();
    for (i, t) in items.iter().enumerate() {
        if i > 0 {
            x += SPACING * s;
        }
        if t.separator() {
            // The group's spacer width around the line (`SpacerMargin`); a
            // blank spacer keeps the space but not the line.
            let (left, _) = t.spacer_margin();
            if t.show_spacer_line {
                separators.push(Rect::from_min_size(pos2(x + left * s, top), vec2(1.0, BUTTON) * s));
            }
            x += (t.separator_width() + SPACING) * s;
        }
        tools.push(Rect::from_min_size(pos2(x, top), vec2(BUTTON, BUTTON) * s));
        x += BUTTON * s;
    }
    Layout { rect, handle, region_button, tools, separators }
}

impl Layout {
    pub fn hit(&self, p: Pos2) -> Option<Part> {
        if !self.rect.contains(p) {
            return None;
        }
        if self.handle.contains(p) {
            return Some(Part::DragHandle);
        }
        if self.region_button.contains(p) {
            return Some(Part::RegionButton);
        }
        if let Some(i) = self.tools.iter().position(|r| r.contains(p)) {
            return Some(Part::Tool(i));
        }
        Some(Part::Background)
    }
}

/// `TrySelectToolForToolbarHotkey`: the first item whose hotkey is `key`
/// with the modifiers (`ToolbarHotkeyHelper.Matches`). Items hidden from the
/// toolbar keep their shortcuts (ShareX 60a85ea), so pass every item
/// ([`hotkey_items`]), not just the toolbar's.
pub fn item_for_hotkey<'a>(items: &'a [Item], key: &str, ctrl: bool, shift: bool, alt: bool) -> Option<&'a Item> {
    let m = crate::toolbar_items::Mods { ctrl, shift, alt, meta: false };
    items.iter().find(|i| i.matches(key, m))
}

/// Every item of the capture toolbar, shown or hidden, for its shortcuts.
pub fn hotkey_items(saved: &[serde_json::Value]) -> Vec<Item> {
    crate::toolbar_items::region_items(saved)
}

/// The tool of [`item_for_hotkey`].
pub fn tool_for_hotkey(items: &[Item], key: &str, ctrl: bool, shift: bool, alt: bool) -> Option<EditorTool> {
    item_for_hotkey(items, key, ctrl, shift, alt).and_then(|i| i.def.tool.clone())
}

/// `.toolbar-button.active`'s background: `ShareX.Brush.Accent`.
pub fn fill_accent(painter: &Painter, rect: Rect, radius: CornerRadius, p: &Palette) {
    fill_gradient(painter, rect, radius, p.accent_start, p.accent_end);
}

/// `ShareX.Brush.Accent`: the accent gradient, top-left to bottom-right,
/// over a tessellated shape's mesh (its alpha keeps the edges' smoothing).
fn fill_gradient(painter: &Painter, rect: Rect, radius: CornerRadius, start: Color32, end: Color32) {
    let ppp = painter.ctx().pixels_per_point();
    let shape = egui::epaint::RectShape::filled(rect, radius, Color32::WHITE);
    let mut mesh = egui::Mesh::default();
    let mut tess = egui::epaint::Tessellator::new(
        ppp,
        egui::epaint::TessellationOptions::default(),
        [1, 1],
        Vec::new(),
    );
    tess.tessellate_rect(&shape, &mut mesh);
    let lerp = |a: u8, b: u8, t: f32| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    for v in &mut mesh.vertices {
        let u = ((v.pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
        let w = ((v.pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
        let t = (u + w) / 2.0;
        let c = Color32::from_rgb(lerp(start.r(), end.r(), t), lerp(start.g(), end.g(), t), lerp(start.b(), end.b(), t));
        v.color = c.gamma_multiply(v.color.a() as f32 / 255.0);
    }
    painter.add(mesh);
}

/// One `Button.toolbar-button`: control background (hover background
/// under the pointer), 1px border, 4px corners; `.active` the accent.
pub fn button(painter: &Painter, p: &Palette, rect: Rect, key: &str, active: bool, hovered: bool, scale: f32) {
    let radius = CornerRadius::same((4.0 * scale).round() as u8);
    if active {
        fill_gradient(painter, rect, radius, p.accent_start, p.accent_end);
    } else {
        let fill = if hovered { p.control_background_hover } else { p.control_background };
        painter.rect_filled(rect, radius, fill);
    }
    painter.rect_stroke(rect, radius, Stroke::new(1.0 * scale, p.border), StrokeKind::Inside);
    let fg = if active { p.accent_foreground } else { p.text };
    icons::paint(painter, Rect::from_center_size(rect.center(), egui::Vec2::splat(ICON * scale)), key, fg);
}

/// Draws the toolbar laid out as `layout`, `opacity` for its hide
/// animation, with `hovered` under the pointer. `active_region`: the
/// region tool's shape while it's active (the region button shows the
/// rectangle, `SetActiveHostToolbarItem` the ellipse and freehand items);
/// `active_tool`: the tool shown active (none while the region tool is).
#[allow(clippy::too_many_arguments)]
pub fn paint(
    painter: &Painter,
    p: &Palette,
    layout: &Layout,
    hovered: Option<Part>,
    active_region: Option<super::RegionShape>,
    active_tool: Option<&EditorTool>,
    scale: f32,
    opacity: f32,
    items: &[Item],
) {
    let painter = painter.clone().with_layer_id(painter.layer_id());
    let mut painter = painter;
    painter.set_opacity(opacity);
    // `MainToolbarCornerRadius="0,0,4,4"`, `MainToolbarBorderThickness="1"`.
    let r = (4.0 * scale).round() as u8;
    let radius = CornerRadius { nw: 0, ne: 0, sw: r, se: r };
    painter.rect_filled(layout.rect, radius, p.background_toolbar);
    painter.rect_stroke(layout.rect, radius, Stroke::new(1.0 * scale, p.border), StrokeKind::Inside);
    // The drag handle's dots: `M0,0h2v2h-2z ...`, 6x14, centred.
    let fill = if hovered == Some(Part::DragHandle) { p.text } else { p.text_secondary };
    let origin = layout.handle.center() - vec2(3.0, 7.0) * scale;
    for row in 0..4 {
        for col in 0..2 {
            let min = origin + vec2(col as f32 * 4.0, row as f32 * 4.0) * scale;
            painter.rect_filled(Rect::from_min_size(min, egui::Vec2::splat(2.0 * scale)), 0.0, fill);
        }
    }
    let region_active = active_region == Some(super::RegionShape::Rectangle);
    button(&painter, p, layout.region_button, icon::Region, region_active, hovered == Some(Part::RegionButton), scale);
    for sep in &layout.separators {
        painter.rect_filled(*sep, 0.0, p.separator);
    }
    for (i, (rect, item)) in layout.tools.iter().zip(items.iter()).enumerate() {
        let active = match &item.def.tool {
            Some(tool) => active_tool == Some(tool),
            None => active_region.is_some() && region_shape(item.def.id) == active_region,
        };
        button(&painter, p, *rect, item.def.icon, active, hovered == Some(Part::Tool(i)), scale);
    }
}

/// The tooltip of a part (`ToolTip.Tip`).
pub fn tooltip(part: Part, items: &[Item]) -> Option<String> {
    match part {
        Part::DragHandle => Some(sc::RegionCaptureWindow_MoveToolbar().to_string()),
        Part::RegionButton => Some(sc::RegionCaptureWindow_RectangleRegionTab().to_string()),
        Part::Tool(i) => items.get(i).map(Item::tooltip),
        Part::Option(_) | Part::Background | Part::Popup => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_is_the_size_of_sharexs() {
        // Measured on ShareX at 100%: 909 x 50.
        let items = default_items();
        assert_eq!(items.len(), 19);
        assert_eq!(size(1.0, &items), vec2(909.0, 50.0));
        let l = layout(pos2(58.0, 0.0), 1.0, &items);
        assert_eq!(l.handle.min, pos2(63.0, 5.0));
        assert_eq!(l.region_button.min, pos2(81.0, 5.0));
        assert_eq!(l.separators[0].min.x, 125.0);
        assert_eq!(l.tools[0].min.x, 130.0);
        assert_eq!(l.tools[18].max.x, 962.0);
        assert_eq!(TOOLS[1].tooltip(), "Rectangle (R, NumPad1)");
    }

    /// ShareX #9278: a spacer's width goes around its line, half each side;
    /// a blank one draws no line.
    #[test]
    fn spacers_widen_the_toolbar() {
        let saved = vec![serde_json::json!({"Id": "Rectangle", "BeginGroup": true, "SpacerWidth": 21, "IsVisible": true})];
        let items = items(&saved);
        let base = layout(pos2(58.0, 0.0), 1.0, &default_items());
        let l = layout(pos2(58.0, 0.0), 1.0, &items);
        assert_eq!(size(1.0, &items).x, 909.0 + 1.0 + 21.0 + SPACING);
        // Rectangle's separator: after Select (130..170) and the spacing.
        assert_eq!(l.separators[1].min.x, 174.0 + 10.0);
        assert_eq!(l.tools[1].min.x, base.tools[1].min.x + 1.0 + 21.0 + SPACING);
        let blank = vec![serde_json::json!({"Id": "Rectangle", "BeginGroup": true, "SpacerWidth": 8, "HideSpacerLine": true, "IsVisible": true})];
        let l = layout(pos2(58.0, 0.0), 1.0, &super::items(&blank));
        assert_eq!(l.separators.len(), 1);
        assert_eq!(l.tools[1].min.x, base.tools[1].min.x + 1.0 + 8.0 + SPACING);
    }
}
