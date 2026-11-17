//! The capture toolbar: `RegionCaptureWindow.axaml`'s `CaptureToolbar`
//! (drag handle and rectangle region button) leading the image editor's
//! `AnnotationToolbar` (`AnnotationToolbar.axaml`) with the default tool
//! items (`ToolbarCustomizationItemViewModel`, without Crop and Cut out,
//! `RegionCaptureToolbar.CreateItems`). The annotation tools are drawn but
//! inert until the image editor is ported.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use sharex_strings::{image_editor as ie, screen_capture_lib as sc};
use sharex_ui::keys::image_editor_icons as icon;
use sharex_ui::{Palette, icons};

/// An annotation tool on the toolbar (`ToolbarItemDefinition`).
#[derive(Clone, Copy, Debug)]
pub struct ToolItem {
    pub id: &'static str,
    pub name: fn() -> &'static str,
    pub icon: &'static str,
    pub hotkey: &'static str,
    pub secondary_hotkey: &'static str,
    /// A separator before it (`BeginGroupByDefault`).
    pub begin_group: bool,
}

const fn tool(
    id: &'static str,
    name: fn() -> &'static str,
    icon: &'static str,
    hotkey: &'static str,
    secondary_hotkey: &'static str,
) -> ToolItem {
    ToolItem { id, name, icon, hotkey, secondary_hotkey, begin_group: false }
}

/// The default items, in order: every tool but Crop and Cut out (which
/// region capture leaves out), Select starting a group.
pub const TOOLS: [ToolItem; 19] = [
    ToolItem { begin_group: true, ..tool("Select", ie::ToolbarCustomizationItemViewModel_Select, icon::Select, "V", "") },
    tool("Rectangle", ie::ToolbarCustomizationItemViewModel_Rectangle, icon::Rectangle, "R", "NumPad1"),
    tool("Ellipse", ie::ToolbarCustomizationItemViewModel_Ellipse, icon::Ellipse, "E", "NumPad2"),
    tool("Line", ie::ToolbarCustomizationItemViewModel_Line, icon::Line, "L", "NumPad4"),
    tool("Arrow", ie::ToolbarCustomizationItemViewModel_Arrow, icon::Arrow, "A", "NumPad5"),
    tool("Freehand", ie::ToolbarCustomizationItemViewModel_Freehand, icon::Freehand, "F", "NumPad3"),
    tool("Text", ie::ToolbarCustomizationItemViewModel_Text, icon::Text, "T", "NumPad6"),
    tool("SpeechBalloon", ie::ToolbarCustomizationItemViewModel_SpeechBalloon, icon::SpeechBalloon, "O", ""),
    tool("Step", ie::ToolbarCustomizationItemViewModel_Step, icon::Step, "N", "NumPad7"),
    tool("Image", ie::ToolbarCustomizationItemViewModel_Image, icon::Image, "I", ""),
    tool("Clone", ie::ToolbarCustomizationItemViewModel_Clone, icon::Clone, "D", ""),
    tool("Emoji", ie::ToolbarCustomizationItemViewModel_Emoji, icon::Emoji, "J", ""),
    tool("Cursor", ie::ToolbarCustomizationItemViewModel_Cursor, icon::Cursor, "K", ""),
    tool("Highlight", ie::ToolbarCustomizationItemViewModel_Highlight, icon::Highlight, "H", ""),
    tool("SmartEraser", ie::ToolbarCustomizationItemViewModel_SmartEraser, icon::SmartEraser, "W", ""),
    tool("Blur", ie::ToolbarCustomizationItemViewModel_Blur, icon::Blur, "B", "NumPad8"),
    tool("Pixelate", ie::ToolbarCustomizationItemViewModel_Pixelate, icon::Pixelate, "P", "NumPad9"),
    tool("Magnify", ie::ToolbarCustomizationItemViewModel_Magnify, icon::Magnify, "M", ""),
    tool("Spotlight", ie::ToolbarCustomizationItemViewModel_Spotlight, icon::Spotlight, "S", ""),
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

/// Something on the toolbar under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    DragHandle,
    RegionButton,
    Tool(usize),
    /// The toolbar's background (not a button).
    Background,
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

/// The toolbar's size at `scale` (the custom button size's scale).
pub fn size(scale: f32) -> egui::Vec2 {
    let row = HANDLE + SPACING + BUTTON + SPACING + tools_width();
    vec2(row + 2.0 * (PADDING + 1.0), BUTTON + 2.0 * (PADDING + 1.0)) * scale
}

fn tools_width() -> f32 {
    TOOLS.iter().enumerate().map(|(i, t)| {
        let sep = if t.begin_group { 1.0 + SPACING } else { 0.0 };
        sep + BUTTON + if i > 0 { SPACING } else { 0.0 }
    }).sum()
}

/// Lays the toolbar out with its top-left corner at `min`.
pub fn layout(min: Pos2, scale: f32) -> Layout {
    let s = scale;
    let rect = Rect::from_min_size(min, size(s));
    let top = min.y + (1.0 + PADDING) * s;
    let mut x = min.x + (1.0 + PADDING) * s;
    let handle = Rect::from_min_size(pos2(x, top), vec2(HANDLE, BUTTON) * s);
    x += (HANDLE + SPACING) * s;
    let region_button = Rect::from_min_size(pos2(x, top), vec2(BUTTON, BUTTON) * s);
    x += (BUTTON + SPACING) * s;
    let mut tools = Vec::new();
    let mut separators = Vec::new();
    for (i, t) in TOOLS.iter().enumerate() {
        if i > 0 {
            x += SPACING * s;
        }
        if t.begin_group {
            separators.push(Rect::from_min_size(pos2(x, top), vec2(1.0, BUTTON) * s));
            x += (1.0 + SPACING) * s;
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
fn button(painter: &Painter, p: &Palette, rect: Rect, key: &str, active: bool, hovered: bool, scale: f32) {
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
/// animation, with `hovered` under the pointer.
pub fn paint(painter: &Painter, p: &Palette, layout: &Layout, hovered: Option<Part>, region_active: bool, scale: f32, opacity: f32) {
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
    button(&painter, p, layout.region_button, icon::Region, region_active, hovered == Some(Part::RegionButton), scale);
    for sep in &layout.separators {
        painter.rect_filled(*sep, 0.0, p.separator);
    }
    for (i, (rect, item)) in layout.tools.iter().zip(TOOLS.iter()).enumerate() {
        button(&painter, p, *rect, item.icon, false, hovered == Some(Part::Tool(i)), scale);
    }
}

/// The tooltip of a part (`ToolTip.Tip`).
pub fn tooltip(part: Part) -> Option<String> {
    match part {
        Part::DragHandle => Some(sc::RegionCaptureWindow_MoveToolbar().to_string()),
        Part::RegionButton => Some(sc::RegionCaptureWindow_RectangleRegionTab().to_string()),
        Part::Tool(i) => TOOLS.get(i).map(ToolItem::tooltip),
        Part::Background => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_is_the_size_of_sharexs() {
        // Measured on ShareX at 100%: 909 x 50.
        assert_eq!(size(1.0), vec2(909.0, 50.0));
        let l = layout(pos2(58.0, 0.0), 1.0);
        assert_eq!(l.handle.min, pos2(63.0, 5.0));
        assert_eq!(l.region_button.min, pos2(81.0, 5.0));
        assert_eq!(l.separators[0].min.x, 125.0);
        assert_eq!(l.tools[0].min.x, 130.0);
        assert_eq!(l.tools[18].max.x, 962.0);
        assert_eq!(TOOLS[1].tooltip(), "Rectangle (R, NumPad1)");
    }
}
