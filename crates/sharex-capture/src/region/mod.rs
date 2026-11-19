//! The region capture overlay: `Presentation/RegionCapture`
//! (`RegionCaptureWindow.axaml.cs`, `RegionSelectionOverlay.cs`,
//! `MagnifierPixelGrid.cs`, `CaptureRegion.cs`, `RegionCaptureToolbar.cs`).
//!
//! [`RegionCapture`] is the overlay's state and its handling of input, in
//! screenshot pixels, apart from any window: the app shows one window per
//! monitor (as snapr does) and feeds every window's input to the one
//! session, and [`RegionCapture::paint`] draws a monitor's part. That lets
//! `--render` draw the overlay offscreen and tests drive it.
//!
//! Ported: the default mode (rectangle region, quick capture): window and
//! control snapping, dimming, the selection's border and centre crosshair,
//! the magnifier and both info labels, the toolbar (moving it, hiding it
//! with Ctrl+H, following the pointer between monitors), keys (Enter,
//! Space, `~`, arrows, Delete, Escape, Shift for squares, Ctrl to move
//! while drawing), mouse actions (right click cancels, X buttons capture
//! fullscreen and the active monitor), the wheel on the magnifier, and
//! selection resizing when quick capture is off, and the annotation tools:
//! the image editor's workspace ([`Workspace`], ShareX's `EditorView`)
//! holds the frozen screen and its annotations, the toolbar's tools switch
//! to it (`ActivateAnnotationTool`), Tab or the region button back
//! (`ActivateRegionTool`), and the capture includes the annotations
//! (`GetSnapshot(Rect)`). The region tool selects rectangles, ellipses or
//! freehand regions ([`shape`], `CaptureRegion`; the ellipse and freehand
//! items are hidden toolbar items by default), several of them in multi
//! region mode (`MultiRegionMode` on a request that allows it), captured
//! with everything outside them transparent (`ApplyRegionMask`).

pub mod chrome;
pub mod cursors;
pub mod magnifier;
pub mod options_row;
pub mod paint;
pub mod shape;
pub mod toolbar;
pub mod tooltip;

use std::time::{Duration, Instant};

use image::RgbaImage;

use crate::geometry::{PointF, Rect, RectF, create_pixel_rectangle};
use sharex_settings::region_capture::limits;
use sharex_settings::types::Point;
use sharex_settings::{RegionCaptureAction, RegionCaptureMagnifierScrollAction, RegionCaptureOptions};
use crate::{Screen, SimpleWindowInfo, WindowInfo};
use sharex_image_editor::ImageEditorOptions;
use sharex_image_editor::editor::{self as ed, Workspace};
use sharex_image_editor::model::EditorTool;

pub use paint::OverlayView;
pub use shape::{CaptureRegion, RegionShape};

/// `AvaloniaRegionCaptureRequest`: what the overlay shows and how.
pub struct Request {
    /// The frozen screen (the pointer already drawn in, when shown).
    pub screenshot: RgbaImage,
    /// The desktop pixels the screenshot covers.
    pub screen_bounds: Rect,
    /// The monitors, for placing the toolbar and keeping labels on screen.
    pub screens: Vec<Screen>,
    pub options: RegionCaptureOptions,
    /// The windows and controls to snap to (`DetectWindows`), topmost
    /// first, listed before the overlay opened.
    pub windows: Vec<SimpleWindowInfo>,
    /// Where the pointer is, in desktop pixels, if known.
    pub cursor: Option<(i32, i32)>,
    /// `RegionCaptureIntegration.LastRegionRectangles`.
    pub last_regions: Vec<Rect>,
    /// Shows the annotation toolbar (`!RegionCaptureDisableAnnotation`).
    pub enable_annotations: bool,
    /// The annotation tools' options (`ToolsSettingsReference.ImageEditorOptions`).
    pub editor_options: ImageEditorOptions,
    /// The pointer (`ShowCursor`, `CursorData.ToBitmap`), inserted as an
    /// image annotation over the frozen screen.
    pub cursor_image: Option<crate::CursorImage>,
    /// `AllowMultipleRegions`: `MultiRegionMode` may select several regions
    /// (Capture › Region; not the screen recorder's or the tools' selection).
    pub allow_multiple_regions: bool,
    /// `CodeMenuEntryPixelInfo.Parse(text, colour, position)`, for the
    /// custom info text (`UseCustomInfoText`); the app's, from the tools.
    pub pixel_info: Option<PixelInfoParser>,
}

/// `CodeMenuEntryPixelInfo.Parse`: the text, the pixel's RGBA, its
/// desktop position.
pub type PixelInfoParser = fn(&str, [u8; 4], (i32, i32)) -> String;

/// `AvaloniaRegionCaptureResult`.
#[derive(Clone, Debug)]
pub struct CaptureResult {
    pub image: RgbaImage,
    /// Desktop pixels.
    pub screen_rectangle: Rect,
    /// The window it's of (the hovered window picked, else the window under
    /// the pointer), for naming.
    pub window_info: Option<WindowInfo>,
    /// Annotations were drawn (`MainViewModel.IsDirty`).
    pub image_modified: bool,
    /// The window captured whole, or 0.
    pub window_handle: isize,
    /// Every region, for Capture › Last region.
    pub regions: Vec<Rect>,
}

/// What became of the input.
#[derive(Clone, Debug)]
pub enum Outcome {
    Continue,
    Cancelled,
    Captured(Box<CaptureResult>),
}

/// The pointer's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

/// The keys the overlay acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Escape,
    Enter,
    Space,
    Tab,
    NumPad0,
    /// `OemTilde` (`` ` `` on US keyboards).
    Tilde,
    Left,
    Right,
    Up,
    Down,
    Delete,
    Shift,
    Control,
    Alt,
    Super,
    H,
    Backspace,
    Home,
    End,
    PageUp,
    PageDown,
    /// A letter key, upper case.
    Letter(char),
    /// A digit of the main keyboard.
    Digit(u8),
    /// NumPad1 to NumPad9 (NumPad0 is its own key).
    NumPad(u8),
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// The cursor the overlay wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorKind {
    Custom(cursors::CustomCursor),
    Arrow,
    Move,
    /// The text box's I-beam.
    Text,
}

/// `RegionInteraction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interaction {
    None,
    PendingHover,
    Creating,
    Moving,
    Resizing,
}

/// `SelectionResizeNodeKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeNode {
    TopLeft,
    TopCenter,
    TopRight,
    RightCenter,
    BottomRight,
    BottomCenter,
    BottomLeft,
    LeftCenter,
}

impl ResizeNode {
    pub const ALL: [ResizeNode; 8] = [
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::RightCenter,
        Self::BottomRight,
        Self::BottomCenter,
        Self::BottomLeft,
        Self::LeftCenter,
    ];

    /// `SelectionResizeNode.GetPosition`.
    pub fn position(self, r: RectF) -> PointF {
        let c = r.center();
        match self {
            Self::TopLeft => PointF::new(r.x, r.y),
            Self::TopCenter => PointF::new(c.x, r.y),
            Self::TopRight => PointF::new(r.right(), r.y),
            Self::RightCenter => PointF::new(r.right(), c.y),
            Self::BottomRight => PointF::new(r.right(), r.bottom()),
            Self::BottomCenter => PointF::new(c.x, r.bottom()),
            Self::BottomLeft => PointF::new(r.x, r.bottom()),
            Self::LeftCenter => PointF::new(r.x, c.y),
        }
    }

    fn direction(self) -> (i32, i32) {
        match self {
            Self::TopLeft => (-1, -1),
            Self::TopCenter => (0, -1),
            Self::TopRight => (1, -1),
            Self::RightCenter => (1, 0),
            Self::BottomRight => (1, 1),
            Self::BottomCenter => (0, 1),
            Self::BottomLeft => (-1, 1),
            Self::LeftCenter => (-1, 0),
        }
    }

    /// `SelectionResizeNode.Resize`, minimum 1x1.
    fn resize(self, r: RectF, dx: f64, dy: f64) -> RectF {
        let (h, v) = self.direction();
        let (mut left, mut top, mut width, mut height) = (r.x, r.y, r.w, r.h);
        if h > 0 {
            width = (width + dx).max(1.0);
        } else if h < 0 {
            let change = (width - 1.0).min(dx);
            left += change;
            width -= change;
        }
        if v > 0 {
            height = (height + dy).max(1.0);
        } else if v < 0 {
            let change = (height - 1.0).min(dy);
            top += change;
            height -= change;
        }
        RectF::new(left, top, width, height)
    }
}

/// `SelectionResizeNode.Create`: 15 DIPs across.
pub const RESIZE_NODE_SIZE: f64 = 15.0;

/// The toolbar's state: where its top centre is (desktop pixels), whether
/// Ctrl+H hid it, and whether it still follows the pointer.
#[derive(Clone, Debug)]
struct ToolbarState {
    /// Top centre, desktop pixels.
    anchor: Option<(f64, f64)>,
    /// The monitor it's on (`_captureToolbarScreenBounds`).
    screen: Rect,
    hidden: bool,
    hidden_changed: Option<Instant>,
    follows_pointer: bool,
    /// Dragging by the handle: the pointer's offset from the anchor.
    drag: Option<(f64, f64)>,
}

/// `RegionCaptureWindow`'s state.
pub struct RegionCapture {
    pub(crate) screenshot: RgbaImage,
    pub(crate) screen_bounds: Rect,
    pub(crate) screens: Vec<Screen>,
    pub(crate) options: RegionCaptureOptions,
    windows: Vec<SimpleWindowInfo>,
    last_regions: Vec<Rect>,
    pub(crate) enable_annotations: bool,
    width: f64,
    height: f64,
    opened: Instant,
    keyboard_enabled: bool,

    interaction: Interaction,
    resize_node: Option<ResizeNode>,
    press_point: PointF,
    pub(crate) last_pointer: PointF,
    last_creation_point: PointF,
    interaction_start: CaptureRegion,
    /// `_regionOverlay.Selection`: the region being drawn or edited.
    pub(crate) selection: CaptureRegion,
    /// `_regions`: the regions already added in multi region mode.
    pub(crate) regions: Vec<CaptureRegion>,
    /// `_regionShape`: what the region tool selects.
    pub(crate) region_shape: RegionShape,
    /// The freehand region being drawn (`_freehandPoints`).
    freehand_points: Option<Vec<PointF>>,
    /// Shift draws a straight edge (`_isFreehandStraightLine`).
    freehand_straight: bool,
    allow_multiple_regions: bool,
    pixel_info: Option<PixelInfoParser>,
    pub(crate) hover: RectF,
    hover_candidate: Option<usize>,
    selected_candidate: Option<usize>,
    pub(crate) selection_pending: bool,
    pub(crate) magnifier_hidden_by_wheel: bool,
    moving_during_creation: bool,
    ctrl_held_during_creation: bool,
    suppress_right_release: bool,
    last_left_press: Option<(Instant, PointF)>,
    toolbar: ToolbarState,
    /// The toolbar's visible items (`SetHostToolbarItems` with
    /// `RegionCaptureToolbar.CreateItems(options.ToolbarItems)`).
    pub(crate) bar: Vec<toolbar::Item>,
    /// The toolbar part under the pointer.
    pub(crate) toolbar_hover: Option<toolbar::Part>,
    /// A colour popup's screen colour picker was asked for (the option
    /// whose colour it sets).
    pub(crate) pick_request: Option<options_row::OptionItem>,
    pub(crate) pointer_known: bool,
    closing: bool,
    /// The image editor's workspace, when annotations are enabled.
    pub(crate) editor: Option<Workspace>,
    /// `_regionToolActive`: the region tool, not an annotation tool.
    pub(crate) region_tool_active: bool,
    annotation_right_pressed: bool,
    /// A slider of the options row being dragged.
    slider_drag: Option<options_row::OptionItem>,
    /// The tool options' open popup.
    pub(crate) popups: crate::editor_popups::Popups,

    /// Escape went to the text box, so its release doesn't cancel.
    escape_to_text: bool,
    /// The toolbar's tooltip (`ToolTipService`).
    pub(crate) tip: tooltip::TipState,
    /// Where the pointer is, desktop pixels (the tooltip opens there).
    tip_pointer: (f64, f64),
}

/// `Distance(...) >= 4` before a press on a hovered window starts a new
/// region instead.
const DRAG_THRESHOLD: f64 = 4.0;
/// System double-click time and distance (Windows defaults).
const DOUBLE_CLICK: Duration = Duration::from_millis(500);
const DOUBLE_CLICK_DISTANCE: f64 = 4.0;

impl RegionCapture {
    pub fn new(request: Request) -> Self {
        let Request {
            screenshot,
            screen_bounds,
            screens,
            options,
            windows,
            cursor,
            last_regions,
            enable_annotations,
            editor_options,
            cursor_image,
            allow_multiple_regions,
            pixel_info,
        } = request;
        let editor = enable_annotations.then(|| {
            let pm = sharex_skia::Pixmap::from_rgba(screenshot.width(), screenshot.height(), screenshot.as_raw());
            // `ConfigureForFullscreenWorkspace`: the workspace host mode.
            let mut ws = Workspace::new(pm, editor_options);
            ws.host_mode = true;
            if let Some(c) = &cursor_image {
                let cpm = sharex_skia::Pixmap::from_rgba(c.image.width(), c.image.height(), c.image.as_raw());
                ws.insert_workspace_image(cpm, (c.position.0 as f64, c.position.1 as f64));
            }
            ws
        });
        let width = screen_bounds.w.max(1) as f64;
        let height = screen_bounds.h.max(1) as f64;
        let mut me = Self {
            screenshot,
            screen_bounds,
            screens,
            windows: if options.DetectWindows { windows } else { Vec::new() },
            bar: toolbar::items(&options.ToolbarItems),
            pick_request: None,
            options,
            last_regions,
            enable_annotations,
            width,
            height,
            opened: Instant::now(),
            keyboard_enabled: false,
            interaction: Interaction::None,
            resize_node: None,
            press_point: PointF::default(),
            last_pointer: PointF::default(),
            last_creation_point: PointF::default(),
            interaction_start: CaptureRegion::default(),
            selection: CaptureRegion::default(),
            regions: Vec::new(),
            region_shape: RegionShape::Rectangle,
            freehand_points: None,
            freehand_straight: false,
            allow_multiple_regions,
            pixel_info,
            hover: RectF::default(),
            hover_candidate: None,
            selected_candidate: None,
            selection_pending: false,
            magnifier_hidden_by_wheel: false,
            moving_during_creation: false,
            ctrl_held_during_creation: false,
            suppress_right_release: false,
            last_left_press: None,
            toolbar: ToolbarState {
                anchor: None,
                screen: Rect::default(),
                hidden: false,
                hidden_changed: None,
                follows_pointer: true,
                drag: None,
            },
            toolbar_hover: None,
            pointer_known: false,
            closing: false,
            editor,
            region_tool_active: true,
            annotation_right_pressed: false,
            slider_drag: None,
            popups: Default::default(),
            escape_to_text: false,
            tip: tooltip::TipState::default(),
            tip_pointer: (0.0, 0.0),
        };
        // `OnOpened`: the pointer, the toolbar on its monitor, the hover.
        let desktop = cursor.or_else(|| me.screens.iter().find(|s| s.primary).map(|s| (s.bounds.x, s.bounds.y)));
        if let Some((x, y)) = cursor {
            me.last_pointer = me.clamp(PointF::new((x - screen_bounds.x) as f64, (y - screen_bounds.y) as f64));
            me.pointer_known = true;
        }
        if let Some(d) = desktop {
            me.update_toolbar_position(d);
        }
        me.update_hover(me.last_pointer);
        me
    }

    /// The screenshot's size, `GetImageSize`.
    pub fn image_size(&self) -> (f64, f64) {
        (self.width, self.height)
    }

    pub fn screen_bounds(&self) -> Rect {
        self.screen_bounds
    }

    pub fn screens(&self) -> &[Screen] {
        &self.screens
    }

    pub fn options(&self) -> &RegionCaptureOptions {
        &self.options
    }

    /// Keys start working 500 ms after the overlay opens (`InputDelay`).
    /// Returns when to look again, if still waiting.
    pub fn tick(&mut self, now: Instant) -> Option<Instant> {
        let at = self.opened + Duration::from_millis(limits::INPUT_DELAY as u64);
        if now >= at {
            self.keyboard_enabled = true;
        }
        let tip = if self.toolbar.hidden || self.toolbar.drag.is_some() { None } else { self.tip.tick(now, self.tip_pointer) };
        let animating = self.toolbar.hidden_changed.is_some_and(|t| now < t + Duration::from_millis(200));
        let next = if animating { Some(now + Duration::from_millis(16)) } else { (!self.keyboard_enabled).then_some(at) };
        match (next, tip) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// The text of a toolbar part's tooltip (`ToolTip.Tip`).
    pub(crate) fn tooltip_text(&self, part: toolbar::Part) -> Option<String> {
        match part {
            toolbar::Part::Option(item) => {
                let tool = self.editor.as_ref().map(|e| e.active_tool());
                let name = tool
                    .and_then(|t| toolbar::TOOLS.iter().find(|i| i.tool == t).map(|i| (i.name)().to_string()))
                    .unwrap_or_default();
                Some(item.tooltip(&name))
            }
            other => toolbar::tooltip(other, &self.bar),
        }
    }

    fn clamp(&self, p: PointF) -> PointF {
        PointF::new(p.x.clamp(0.0, self.width), p.y.clamp(0.0, self.height))
    }

    fn bounds(&self) -> RectF {
        RectF::new(0.0, 0.0, self.width, self.height)
    }

    /// `GetPointerInfoText` for the image pixel (`ix`, `iy`): its desktop
    /// position, or with `UseCustomInfoText` the custom text (or colour,
    /// hex and position) of the screenshot's pixel there.
    pub(crate) fn pointer_info_text(&self, ix: i32, iy: i32) -> String {
        let (x, y) = (self.screen_bounds.x + ix, self.screen_bounds.y + iy);
        if !self.options.UseCustomInfoText {
            return format!("X: {x} Y: {y}");
        }
        // `GetWorkspacePixel`: the source image's pixel, unpremultiplied.
        let c = if ix >= 0 && iy >= 0 && (ix as u32) < self.screenshot.width() && (iy as u32) < self.screenshot.height() {
            self.screenshot.get_pixel(ix as u32, iy as u32).0
        } else {
            [0, 0, 0, 0]
        };
        if !self.options.CustomInfoText.is_empty()
            && let Some(parse) = self.pixel_info
        {
            return parse(&self.options.CustomInfoText, c, (x, y));
        }
        // `ColorHelpers.ColorToHex(color)`: RGB, upper case.
        format!("RGB: {}, {}, {}\r\nHex: {:02X}{:02X}{:02X}\r\nX: {x} Y: {y}", c[0], c[1], c[2], c[0], c[1], c[2])
    }

    /// `HasValidSelection`: big enough, and a freehand region of 3 points.
    pub(crate) fn has_valid_selection(&self) -> bool {
        let b = self.selection.bounds;
        b.w >= limits::MINIMUM_SIZE as f64
            && b.h >= limits::MINIMUM_SIZE as f64
            && (self.selection.shape != RegionShape::Freehand
                || self.selection.points.as_ref().is_some_and(|p| p.len() >= 3))
    }

    /// `CanResizeSelection`: a freehand region moves but has no nodes.
    fn can_resize_selection(&self) -> bool {
        self.has_valid_selection() && self.selection.shape != RegionShape::Freehand
    }

    /// `IsMultiRegionMode`.
    pub(crate) fn multi_region_mode(&self) -> bool {
        self.allow_multiple_regions && self.options.MultiRegionMode
    }

    /// `IsQuickCapture`.
    pub(crate) fn quick_capture(&self) -> bool {
        self.options.QuickCapture && !self.multi_region_mode()
    }

    /// `HasAnyRegion`.
    pub(crate) fn has_any_region(&self) -> bool {
        self.has_valid_selection() || !self.regions.is_empty()
    }

    /// `GetSelectedRegions`: the added regions, then the selection.
    fn selected_regions(&self) -> Vec<CaptureRegion> {
        let mut regions = self.regions.clone();
        if self.has_valid_selection() {
            regions.push(self.selection.clone());
        }
        regions
    }

    /// `FindRegionAt`: the topmost added region containing `p`.
    fn find_region_at(&self, p: PointF) -> Option<usize> {
        (0..self.regions.len()).rev().find(|&i| self.regions[i].contains(p))
    }

    /// `IsPointInAnyRegion`.
    fn point_in_any_region(&self, p: PointF) -> bool {
        (self.has_valid_selection() && self.selection.contains(p)) || self.find_region_at(p).is_some()
    }

    /// `AddSelectionToRegions`: in multi region mode the selection is kept
    /// before another one starts.
    fn add_selection_to_regions(&mut self) {
        if self.multi_region_mode() && self.has_valid_selection() {
            self.regions.push(self.selection.clone());
        }
    }

    /// `RemoveRegion`.
    fn remove_region(&mut self, index: usize) {
        if index < self.regions.len() {
            self.regions.remove(index);
            self.update_hover(self.last_pointer);
        }
    }

    /// `_regionOverlay.HoverShape`: an ellipse with the ellipse region,
    /// else a rectangle.
    pub(crate) fn hover_shape(&self) -> RegionShape {
        if self.region_shape == RegionShape::Ellipse { RegionShape::Ellipse } else { RegionShape::Rectangle }
    }

    /// `ShowHoverWithSelection`: in multi region mode a window can be
    /// hovered while a region is selected.
    pub(crate) fn show_hover_with_selection(&self) -> bool {
        self.interaction == Interaction::None && self.multi_region_mode()
    }

    /// Whether the resize nodes show (`SetRegionResizeNodesVisible`).
    pub(crate) fn resize_nodes_visible(&self) -> bool {
        !self.quick_capture() && self.interaction == Interaction::None && self.can_resize_selection()
    }

    /// `ActiveRectangle`: the selection, else the hovered window.
    pub(crate) fn active_rectangle(&self) -> RectF {
        if self.selection.bounds.is_valid() { self.selection.bounds } else { self.hover }
    }

    pub(crate) fn magnifier_shown(&self) -> bool {
        self.options.ShowMagnifier && !self.magnifier_hidden_by_wheel
    }

    /// `SetSelection(CaptureRegion, candidate)`: kept on the screenshot.
    fn set_selection(&mut self, region: CaptureRegion, candidate: Option<usize>) {
        let bounds = region.bounds.intersect(&self.bounds());
        self.selection = CaptureRegion { bounds, ..region };
        self.selected_candidate = candidate;
    }

    /// `SetSelection(Rect, candidate)`: the selection's bounds.
    fn set_selection_rect(&mut self, r: RectF, candidate: Option<usize>) {
        self.set_selection(self.selection.with_bounds(r), candidate);
    }

    fn set_interaction(&mut self, i: Interaction) {
        self.interaction = i;
        self.selection_pending = i == Interaction::PendingHover;
    }

    fn reset_creation_modifiers(&mut self) {
        self.moving_during_creation = false;
        self.ctrl_held_during_creation = false;
    }

    fn clear_selection(&mut self) {
        self.set_interaction(Interaction::None);
        self.reset_creation_modifiers();
        self.selected_candidate = None;
        self.freehand_points = None;
        self.selection = CaptureRegion::default();
        self.update_hover(self.last_pointer);
    }

    /// `UpdateHoverRectangle`: the window or control under the pointer.
    fn update_hover(&mut self, p: PointF) {
        let blocked = if self.multi_region_mode() { self.point_in_any_region(p) } else { self.has_valid_selection() };
        if blocked || self.interaction != Interaction::None || self.region_shape == RegionShape::Freehand {
            self.hover = RectF::default();
            self.hover_candidate = None;
            return;
        }
        let sx = self.screen_bounds.x as f64 + p.x.round();
        let sy = self.screen_bounds.y as f64 + p.y.round();
        let found = self.windows.iter().position(|w| {
            let r = w.rect;
            r.w > 0 && r.h > 0 && sx >= r.x as f64 && sy >= r.y as f64 && sx < r.right() as f64 && sy < r.bottom() as f64
        });
        let Some(i) = found else {
            self.hover_candidate = None;
            self.hover = RectF::default();
            return;
        };
        let r = self.windows[i].rect;
        let (cl, ct) = ((r.x - self.screen_bounds.x) as f64, (r.y - self.screen_bounds.y) as f64);
        let (left, top) = (cl.max(0.0), ct.max(0.0));
        let right = self.width.min(cl + r.w as f64);
        let bottom = self.height.min(ct + r.h as f64);
        let hover = if right > left && bottom > top {
            RectF::new(left, top, right - left, bottom - top)
        } else {
            RectF::default()
        };
        self.hover_candidate = hover.is_valid().then_some(i);
        self.hover = hover;
    }

    /// `UpdateSelectionDuringCreation`: Shift makes a square, Ctrl moves the
    /// region being drawn.
    fn update_selection_during_creation(&mut self, p: PointF, m: Modifiers) {
        if self.freehand_points.is_some() {
            return;
        }
        if !m.ctrl {
            self.moving_during_creation = false;
        } else if self.selection.bounds.is_valid() && (self.moving_during_creation || !self.ctrl_held_during_creation) {
            // `MoveSelectionDuringCreation`
            let before = self.selection.bounds;
            let (dx, dy) = (p.x - self.last_creation_point.x, p.y - self.last_creation_point.y);
            self.move_selection(dx, dy, self.selection.clone());
            let after = self.selection.bounds;
            self.press_point =
                self.clamp(PointF::new(self.press_point.x + after.x - before.x, self.press_point.y + after.y - before.y));
            self.moving_during_creation = true;
            self.last_creation_point = p;
            self.ctrl_held_during_creation = true;
            return;
        }
        let r = create_selection_rectangle(self.press_point, p, (self.width, self.height), m.shift);
        self.set_selection_rect(r, None);
        self.last_creation_point = p;
        self.ctrl_held_during_creation = m.ctrl;
    }

    /// `AddFreehandPoint`: while Shift is held the region draws one
    /// straight edge from its last point to the pointer.
    fn add_freehand_point(&mut self, p: PointF, straight: bool) {
        let Some(points) = self.freehand_points.as_mut() else { return };
        if points.last() == Some(&p) {
            return;
        }
        if straight && self.freehand_straight {
            *points.last_mut().expect("a freehand region starts with a point") = p;
        } else {
            points.push(p);
        }
        self.freehand_straight = straight;
        let bounds = CaptureRegion::points_bounds(points);
        let region = CaptureRegion { shape: RegionShape::Freehand, bounds, points: Some(points.clone()) };
        self.set_selection(region, None);
    }

    /// `MoveSelection`: kept inside the screenshot.
    fn move_selection(&mut self, dx: f64, dy: f64, source: CaptureRegion) {
        let r = source.bounds;
        let x = (r.x + dx).clamp(0.0, (self.width - r.w).max(0.0));
        let y = (r.y + dy).clamp(0.0, (self.height - r.h).max(0.0));
        self.set_selection(source.offset(x - r.x, y - r.y), None);
    }

    fn resize_selection(&mut self, p: PointF) {
        let Some(node) = self.resize_node else { return };
        let start = self.interaction_start.bounds;
        let at = node.position(start);
        let r = node.resize(start, p.x - at.x, p.y - at.y);
        self.set_selection_rect(r, None);
    }

    /// Which resize node is under `p` (image pixels), when they show.
    fn resize_node_at(&self, p: PointF, scale: f64) -> Option<ResizeNode> {
        if !self.resize_nodes_visible() {
            return None;
        }
        let half = RESIZE_NODE_SIZE * scale / 2.0;
        ResizeNode::ALL.into_iter().find(|n| {
            let c = n.position(self.selection.bounds);
            (p.x - c.x).abs() <= half && (p.y - c.y).abs() <= half
        })
    }

    /// The scaling of the monitor at an image point (`GetMonitorScaling`).
    pub(crate) fn monitor_scale_at(&self, p: PointF) -> f64 {
        let d = (self.screen_bounds.x + p.x.floor() as i32, self.screen_bounds.y + p.y.floor() as i32);
        crate::screenshot::screen_from_point(&self.screens, d).map(|s| s.scale).filter(|s| *s > 0.0).unwrap_or(1.0)
    }

    /// The pointer moved to `p` (image pixels).
    pub fn pointer_moved(&mut self, p: PointF, m: Modifiers) -> Outcome {
        if self.closing {
            return Outcome::Continue;
        }
        self.pointer_known = true;
        self.tip_pointer = (self.screen_bounds.x as f64 + p.x, self.screen_bounds.y as f64 + p.y);
        // `FollowPointerWithCaptureToolbar`
        let desktop = (self.screen_bounds.x + p.x.round() as i32, self.screen_bounds.y + p.y.round() as i32);
        if let Some((ox, oy)) = self.toolbar.drag {
            let (dx, dy) = (self.screen_bounds.x as f64 + p.x - ox, self.screen_bounds.y as f64 + p.y - oy);
            self.toolbar.anchor = Some((dx, dy));
            self.last_pointer = self.clamp(p);
            return Outcome::Continue;
        }
        if self.toolbar.follows_pointer
            && self.options.ToolbarPosition == Point::default()
            && !self.toolbar.screen.contains(desktop.0, desktop.1)
        {
            self.update_toolbar_position(desktop);
        }
        if self.slider_drag.is_some() {
            return Outcome::Continue;
        }
        if !self.region_tool_active {
            self.last_pointer = self.clamp(p);
            if let Some(e) = self.editor.as_mut()
                && (self.toolbar_hover.is_none() || e.is_interacting())
            {
                e.pointer_moved((p.x, p.y), editor_mods(m));
            }
            return Outcome::Continue;
        }
        // Over the toolbar the input surface sees nothing (unless dragging).
        if self.interaction == Interaction::None && self.toolbar_hover.is_some() {
            return Outcome::Continue;
        }
        let p = self.clamp(p);
        self.last_pointer = p;
        match self.interaction {
            Interaction::None => self.update_hover(p),
            Interaction::PendingHover => {
                if self.press_point.distance(p) >= DRAG_THRESHOLD {
                    self.set_interaction(Interaction::Creating);
                    self.selected_candidate = None;
                    self.set_selection(CaptureRegion::new(self.hover_shape(), RectF::default()), None);
                    self.update_selection_during_creation(p, m);
                }
            }
            Interaction::Creating if self.freehand_points.is_some() => self.add_freehand_point(p, m.shift),
            Interaction::Creating => self.update_selection_during_creation(p, m),
            Interaction::Moving => {
                let start = self.interaction_start.clone();
                self.move_selection(p.x - self.press_point.x, p.y - self.press_point.y, start);
            }
            Interaction::Resizing => self.resize_selection(p),
        }
        Outcome::Continue
    }

    /// A button went down at `p`; `scale` is the window's scaling (for
    /// the resize nodes' size).
    pub fn pointer_pressed(&mut self, p: PointF, button: Button, m: Modifiers, scale: f64) -> Outcome {
        if self.closing {
            return Outcome::Continue;
        }
        // `OnCaptureHostPointerPressed`
        self.toolbar.follows_pointer = false;
        // A press closes the tooltip.
        self.tip.press(Instant::now());
        // An open popup has the press; one elsewhere dismisses it (its own
        // button's press toggles it below).
        if let Some(open) = self.popups.open {
            match self.toolbar_hover {
                Some(toolbar::Part::Popup) => return Outcome::Continue,
                Some(toolbar::Part::Option(item)) if item == open && button == Button::Left => {}
                _ => {
                    self.popups.close();
                    return Outcome::Continue;
                }
            }
        }
        // A right click on the shadow button opens its options.
        if button == Button::Right
            && self.toolbar_hover == Some(toolbar::Part::Option(options_row::OptionItem::Shadow))
            && let Some(e) = self.editor.as_ref()
        {
            self.popups.toggle(options_row::OptionItem::Shadow, &e.tools);
            return Outcome::Continue;
        }
        if button == Button::Left
            && let Some(part) = self.toolbar_hover
        {
            return self.toolbar_pressed(part, p);
        }
        if self.toolbar_hover.is_some() {
            return Outcome::Continue;
        }
        if !self.region_tool_active {
            return self.annotation_pressed(p, button, m);
        }
        let p = self.clamp(p);
        self.last_pointer = p;
        // `TryCancelRegionCreation`
        if button == Button::Right && matches!(self.interaction, Interaction::Creating | Interaction::PendingHover) {
            self.suppress_right_release = true;
            self.clear_selection();
            return Outcome::Continue;
        }
        match button {
            Button::Right => return Outcome::Continue,
            Button::Middle => return self.run_action(self.options.RegionCaptureActionMiddleClick.clone()),
            Button::X1 => return self.run_action(self.options.RegionCaptureActionX1Click.clone()),
            Button::X2 => return self.run_action(self.options.RegionCaptureActionX2Click.clone()),
            Button::Left => {}
        }
        let now = Instant::now();
        let double = self
            .last_left_press
            .is_some_and(|(t, at)| now.duration_since(t) <= DOUBLE_CLICK && at.distance(p) <= DOUBLE_CLICK_DISTANCE);
        self.last_left_press = Some((now, p));
        if double && self.point_in_any_region(p) {
            return self.complete_selection();
        }
        self.press_point = p;
        self.interaction_start = self.selection.clone();
        self.resize_node = self.resize_node_at(p, scale);
        if self.resize_node.is_some() {
            self.set_interaction(Interaction::Resizing);
            self.resize_selection(p);
        } else if self.has_valid_selection() && self.selection.contains(p) {
            self.set_interaction(Interaction::Moving);
        } else if let Some(i) = self.find_region_at(p) {
            // Multi region mode: pressing an added region makes it the one
            // being edited.
            let region = self.regions.remove(i);
            self.add_selection_to_regions();
            self.set_selection(region.clone(), None);
            self.interaction_start = region;
            self.set_interaction(Interaction::Moving);
        } else if self.hover.is_valid() {
            self.add_selection_to_regions();
            self.set_selection(CaptureRegion::new(self.hover_shape(), self.hover), self.hover_candidate);
            self.interaction_start = self.selection.clone();
            self.set_interaction(Interaction::PendingHover);
        } else {
            self.add_selection_to_regions();
            if self.region_shape == RegionShape::Freehand {
                self.freehand_points = Some(vec![p]);
                self.freehand_straight = false;
                let region = CaptureRegion { shape: RegionShape::Freehand, bounds: RectF::default(), points: Some(vec![p]) };
                self.set_selection(region, None);
            } else {
                self.set_selection(CaptureRegion::new(self.region_shape, RectF::default()), None);
            }
            self.set_interaction(Interaction::Creating);
        }
        if matches!(self.interaction, Interaction::Creating | Interaction::PendingHover) {
            self.last_creation_point = p;
            self.moving_during_creation = false;
            self.ctrl_held_during_creation = m.ctrl;
        }
        Outcome::Continue
    }

    /// A button came up at `p`.
    pub fn pointer_released(&mut self, p: PointF, button: Button, m: Modifiers) -> Outcome {
        if self.closing {
            return Outcome::Continue;
        }
        if button == Button::Left && self.toolbar.drag.take().is_some() {
            // `EndToolbarDrag`: the position is kept for the next capture.
            if let Some((x, y)) = self.toolbar.anchor {
                self.options.ToolbarPosition = Point::new(x.round() as i32, y.round() as i32);
            }
            return Outcome::Continue;
        }
        if button == Button::Left && self.slider_drag.take().is_some() {
            return Outcome::Continue;
        }
        if !self.region_tool_active {
            return self.annotation_released(p, button, m);
        }
        let p = self.clamp(p);
        if self.interaction == Interaction::None && self.toolbar_hover.is_some() {
            if button == Button::Right {
                // The toolbar handles its own right clicks.
                return Outcome::Continue;
            }
            return Outcome::Continue;
        }
        self.last_pointer = p;
        if button == Button::Right && matches!(self.interaction, Interaction::Creating | Interaction::PendingHover) {
            self.clear_selection();
            return Outcome::Continue;
        }
        if button == Button::Right {
            // `TryHandleRightButtonRelease`
            if std::mem::take(&mut self.suppress_right_release) {
                return Outcome::Continue;
            }
            return self.run_action(self.options.RegionCaptureActionRightClick.clone());
        }
        if self.interaction == Interaction::None || button != Button::Left {
            if button == Button::Left {
                self.suppress_right_release = false;
            }
            return Outcome::Continue;
        }
        self.set_interaction(Interaction::None);
        self.resize_node = None;
        self.reset_creation_modifiers();
        if let Some(points) = self.freehand_points.take() {
            let region = CaptureRegion { points: Some(points), ..self.selection.clone() };
            self.set_selection(region, None);
        }
        if !self.has_valid_selection() {
            if self.hover.is_valid() {
                self.set_selection(CaptureRegion::new(self.hover_shape(), self.hover), self.hover_candidate);
            } else {
                self.clear_selection();
            }
        }
        if self.has_valid_selection() && self.quick_capture() {
            return self.complete(vec![self.selection.clone()], true);
        }
        self.update_hover(self.last_pointer);
        Outcome::Continue
    }

    /// The pointer left every overlay window.
    pub fn pointer_left(&mut self) {
        self.toolbar_hover = None;
    }

    /// The wheel turned by `delta_y` (positive: up) over the input surface.
    pub fn wheel(&mut self, delta_y: f64, m: Modifiers) -> Outcome {
        if let Some(_part) = self.toolbar_hover {
            // Ctrl + wheel over the toolbar resizes its buttons.
            if m.ctrl && delta_y != 0.0 {
                let current = if self.options.UseCustomToolbarButtonSize { self.options.ToolbarButtonSize } else { 40 };
                let step = if delta_y > 0.0 { 4 } else { -4 };
                self.options.UseCustomToolbarButtonSize = true;
                self.options.ToolbarButtonSize = (current + step).clamp(
                    limits::TOOLBAR_BUTTON_SIZE_MINIMUM,
                    limits::TOOLBAR_BUTTON_SIZE_MAXIMUM,
                );
            }
            return Outcome::Continue;
        }
        if !self.region_tool_active {
            return Outcome::Continue;
        }
        let direction = delta_y.signum();
        if delta_y == 0.0 || !self.options.ShowMagnifier {
            return Outcome::Continue;
        }
        let up = direction > 0.0;
        if self.magnifier_hidden_by_wheel {
            // Comes back on scroll up, as it was.
            if up {
                self.magnifier_hidden_by_wheel = false;
            }
            return Outcome::Continue;
        }
        let mut action = self.options.MagnifierScrollAction.clone();
        if m.ctrl {
            action = match action {
                RegionCaptureMagnifierScrollAction::Zoom => RegionCaptureMagnifierScrollAction::Resize,
                _ => RegionCaptureMagnifierScrollAction::Zoom,
            };
        }
        let scale = self.monitor_scale_at(self.last_pointer);
        match action {
            // A value from a newer ShareX acts as the default.
            RegionCaptureMagnifierScrollAction::Resize | RegionCaptureMagnifierScrollAction::Unknown(_) => {
                self.resize_magnifier(up, scale)
            }
            RegionCaptureMagnifierScrollAction::Zoom => {
                let count = magnifier::normalize_pixel_count(self.options.MagnifierPixelCount, self.options.MagnifierSize, scale);
                self.options.MagnifierPixelCount = magnifier::normalize_pixel_count(
                    count + if up { -2 } else { 2 },
                    self.options.MagnifierSize,
                    scale,
                );
            }
        }
        Outcome::Continue
    }

    /// `ResizeMagnifier`: same pixel size, more or fewer pixels; shrinking
    /// past the smallest size hides it.
    fn resize_magnifier(&mut self, grow: bool, scale: f64) {
        let o = &mut self.options;
        let current = o
            .MagnifierSize
            .clamp(limits::MAGNIFIER_SIZE_MINIMUM, limits::MAGNIFIER_SIZE_MAXIMUM);
        let count = magnifier::normalize_pixel_count(o.MagnifierPixelCount, o.MagnifierSize, scale);
        let next = count + if grow { 2 } else { -2 };
        let size = (current as f64 / count as f64 * next as f64).round() as i32;
        if size < limits::MAGNIFIER_SIZE_MINIMUM && current == limits::MAGNIFIER_SIZE_MINIMUM {
            self.magnifier_hidden_by_wheel = true;
            return;
        }
        let size = size.clamp(limits::MAGNIFIER_SIZE_MINIMUM, limits::MAGNIFIER_SIZE_MAXIMUM);
        if size == current {
            return;
        }
        o.MagnifierSize = size;
        o.MagnifierPixelCount = next.clamp(
            limits::MAGNIFIER_PIXEL_COUNT_MINIMUM,
            limits::MAGNIFIER_PIXEL_COUNT_MAXIMUM,
        );
    }

    /// A key went down (`OnKeyDown`). `set_cursor` moves the pointer
    /// (desktop pixels), for the arrow keys.
    pub fn key_down(&mut self, key: Key, m: Modifiers, set_cursor: &mut dyn FnMut(i32, i32)) -> Outcome {
        if self.closing {
            return Outcome::Continue;
        }
        // `OnCaptureHostKeyDown`: keys other than modifiers count as
        // interacting, and the toolbar stays where it is.
        if self.keyboard_enabled && !matches!(key, Key::Shift | Key::Control | Key::Alt | Key::Super) {
            self.toolbar.follows_pointer = false;
        }
        // An open popup has the keys; Escape closes it.
        if self.popups.open.is_some() {
            if key == Key::Escape {
                self.popups.close();
                self.escape_to_text = true;
            }
            return Outcome::Continue;
        }
        let none = m == Modifiers::default();
        if self.keyboard_enabled && none && matches!(key, Key::Tab | Key::NumPad0) {
            self.activate_region_tool(None);
            return Outcome::Continue;
        }
        if self.keyboard_enabled && key == Key::H && m.ctrl && !m.shift && !m.alt {
            self.toolbar.hidden = !self.toolbar.hidden;
            self.toolbar.hidden_changed = Some(Instant::now());
            return Outcome::Continue;
        }
        // The text box being typed in has the keys.
        if let Some(e) = self.editor.as_mut()
            && e.text_edit().is_some()
        {
            if let Some(k) = editor_key(key) {
                self.escape_to_text |= key == Key::Escape;
                e.key_down(k, editor_mods(m), &|_, _| None);
            }
            return Outcome::Continue;
        }
        if !self.keyboard_enabled {
            return Outcome::Continue;
        }
        if self.interaction == Interaction::Creating && key == Key::Shift {
            let p = self.last_creation_point;
            self.update_selection_during_creation(p, Modifiers { shift: true, ..m });
            return Outcome::Continue;
        }
        let region = self.region_tool_active;
        match key {
            Key::Enter => {
                if region && !self.has_any_region() && self.hover.is_valid() {
                    self.set_selection(CaptureRegion::new(self.hover_shape(), self.hover), self.hover_candidate);
                }
                if self.has_any_region() {
                    return self.complete_selection();
                }
            }
            Key::Space if region => return self.complete_rects(vec![self.bounds()], false),
            Key::Tilde if region => return self.complete_active_monitor(),
            Key::Left | Key::Right | Key::Up | Key::Down if region => {
                let d = if m.shift { limits::MOVE_SPEED_MAXIMUM } else { limits::MOVE_SPEED_MINIMUM };
                let dx = match key {
                    Key::Left => -d,
                    Key::Right => d,
                    _ => 0,
                };
                let dy = match key {
                    Key::Up => -d,
                    Key::Down => d,
                    _ => 0,
                };
                let (x, y) = crate::screenshot::cursor_position().unwrap_or((
                    self.screen_bounds.x + self.last_pointer.x.round() as i32,
                    self.screen_bounds.y + self.last_pointer.y.round() as i32,
                ));
                set_cursor(x + dx, y + dy);
                return Outcome::Continue;
            }
            Key::Delete if region && self.has_any_region() => {
                if self.has_valid_selection() {
                    self.clear_selection();
                } else {
                    self.remove_region(self.regions.len() - 1);
                }
                return Outcome::Continue;
            }
            _ => {}
        }
        // `EditorView.OnKeyDown`: the editor's keys and the tools' hotkeys.
        let Some(k) = editor_key(key) else { return Outcome::Continue };
        let Some(e) = self.editor.as_mut() else { return Outcome::Continue };
        let bar = self.bar.clone();
        let r = e.key_down(k, editor_mods(m), &|k, m| tool_hotkey(&bar, k, m));
        if r.tool.is_some() {
            self.activate_annotation_tool();
        }
        Outcome::Continue
    }

    /// A key came up (`OnKeyUp`): Escape cancels.
    pub fn key_up(&mut self, key: Key, m: Modifiers) -> Outcome {
        if let Some(e) = self.editor.as_mut()
            && let Some(k) = editor_key(key)
        {
            e.key_up(k, editor_mods(m));
        }
        if key == Key::Escape && std::mem::take(&mut self.escape_to_text) {
            return Outcome::Continue;
        }
        if matches!(self.interaction, Interaction::Creating | Interaction::PendingHover) && key == Key::Control {
            self.moving_during_creation = false;
            self.ctrl_held_during_creation = false;
        }
        if self.keyboard_enabled && !self.closing && self.interaction == Interaction::Creating && key == Key::Shift {
            let p = self.last_creation_point;
            self.update_selection_during_creation(p, Modifiers { shift: false, ..m });
            return Outcome::Continue;
        }
        if key != Key::Escape || m != Modifiers::default() || self.closing {
            return Outcome::Continue;
        }
        if !self.region_tool_active && self.editor.as_mut().is_some_and(|e| e.cancel_interaction_or_selection()) {
            return Outcome::Continue;
        }
        self.cancel()
    }

    /// Text typed while the text box is open.
    pub fn text_input(&mut self, text: &str) {
        if let Some(e) = self.editor.as_mut()
            && e.text_edit().is_some()
        {
            e.text_input(text);
        }
    }

    /// Whether typed text goes to a text box.
    pub fn wants_text(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.text_edit().is_some())
    }

    // The annotation tools.

    /// `ActivateAnnotationTool`: the editor gets the input, the region
    /// overlay shows only a kept selection, the HUD hides.
    pub(crate) fn activate_annotation_tool(&mut self) {
        if self.editor.is_none() {
            return;
        }
        self.region_tool_active = false;
        self.set_interaction(Interaction::None);
        self.hover = RectF::default();
        self.hover_candidate = None;
    }

    /// `ActivateRegionTool`: the region tool, selecting `shape` or else
    /// the last used shape.
    pub(crate) fn activate_region_tool(&mut self, shape: Option<RegionShape>) {
        if shape.is_some_and(|s| s != self.region_shape)
            && matches!(self.interaction, Interaction::Creating | Interaction::PendingHover)
        {
            self.clear_selection();
        }
        self.region_shape = shape.unwrap_or(self.region_shape);
        self.region_tool_active = true;
        if let Some(e) = self.editor.as_mut() {
            e.commit_text();
            e.cancel_interaction_or_selection();
        }
        self.update_hover(self.last_pointer);
    }

    /// The annotation tool shown active, if one is.
    pub fn active_annotation_tool(&self) -> Option<EditorTool> {
        if self.region_tool_active { None } else { self.editor.as_ref().map(|e| e.active_tool()) }
    }

    /// `OnCaptureHostPointerPressed` and the editor's own press.
    fn annotation_pressed(&mut self, p: PointF, button: Button, m: Modifiers) -> Outcome {
        self.last_pointer = self.clamp(p);
        if button == Button::Middle {
            return self.run_annotation_action(self.options.RegionCaptureActionMiddleClick.clone(), p);
        }
        let Some(e) = self.editor.as_mut() else { return Outcome::Continue };
        match button {
            // A second button while drawing cancels the shape.
            Button::Right if e.is_drawing() => e.cancel_drawing(),
            Button::Right => self.annotation_right_pressed = true,
            Button::Left => e.pointer_pressed((p.x, p.y), ed::Button::Left, editor_mods(m)),
            _ => {}
        }
        Outcome::Continue
    }

    /// `OnCaptureHostPointerReleased` and the editor's own release.
    fn annotation_released(&mut self, p: PointF, button: Button, m: Modifiers) -> Outcome {
        self.last_pointer = self.clamp(p);
        match button {
            Button::Right if std::mem::take(&mut self.annotation_right_pressed) => {
                self.run_annotation_action(self.options.RegionCaptureActionRightClick.clone(), p)
            }
            Button::Left => {
                if let Some(e) = self.editor.as_mut() {
                    e.pointer_released((p.x, p.y), ed::Button::Left, editor_mods(m));
                }
                Outcome::Continue
            }
            _ => Outcome::Continue,
        }
    }

    /// `RunAnnotationCaptureAction`: removing a shape removes an annotation
    /// of the active tool under the pointer.
    fn run_annotation_action(&mut self, action: RegionCaptureAction, p: PointF) -> Outcome {
        let at = (p.x, p.y);
        match action {
            RegionCaptureAction::RemoveShapeCancelCapture => {
                if self.editor.as_mut().is_some_and(|e| e.delete_at(at)) {
                    Outcome::Continue
                } else {
                    self.cancel()
                }
            }
            RegionCaptureAction::RemoveShape => {
                if let Some(e) = self.editor.as_mut() {
                    e.delete_at(at);
                }
                Outcome::Continue
            }
            other => self.run_action(other),
        }
    }

    /// A control of the options row was pressed.
    fn option_pressed(&mut self, item: options_row::OptionItem) {
        use options_row::OptionItem as O;
        if crate::editor_popups::Popups::has_popup(item)
            && let Some(e) = self.editor.as_ref()
        {
            self.popups.toggle(item, &e.tools);
            return;
        }
        let Some(e) = self.editor.as_mut() else { return };
        if item.is_slider() {
            self.slider_drag = Some(item);
            return;
        }
        e.set_option(|s, t| match item {
            O::Bold => s.set_text_bold(!s.text_bold, t),
            O::Italic => s.set_text_italic(!s.text_italic, t),
            O::EffectEllipse => s.set_effect_ellipse(!s.effect_ellipse, t),
            O::ArrowRoundedCorners => s.set_arrow_rounded_corners(!s.arrow_rounded_corners, t),
            O::FreehandArrow => s.set_freehand_arrow(!s.freehand_arrow, t),
            O::Tail => s.set_speech_balloon_tail(!s.speech_balloon_tail, t),
            O::AutoSize => s.set_speech_balloon_auto_size(!s.speech_balloon_auto_size, t),
            O::Shadow => s.set_shadow_enabled(!s.shadow_enabled),
            _ => {}
        });
    }

    /// The options row slider being dragged, if any.
    pub fn slider_dragging(&self) -> Option<options_row::OptionItem> {
        self.slider_drag
    }

    /// Sets a slider of the options row to `t` (0 to 1) of its range.
    pub fn set_slider_fraction(&mut self, item: options_row::OptionItem, t: f32) {
        let Some(e) = self.editor.as_mut() else { return };
        let tool = e.active_tool();
        e.set_option(|s, target| {
            if item == options_row::OptionItem::Strength {
                let max = ed::max_effect_strength(tool);
                s.set_effect_strength((1.0 + t * (max - 1.0)).round(), target);
            } else {
                s.set_spotlight_blur((t * 200.0).round(), target);
            }
        });
    }

    /// The options the annotation tools changed, to keep as ShareX does.
    pub fn editor_options(&self) -> Option<ImageEditorOptions> {
        self.editor.as_ref().map(|e| e.tools.options.clone())
    }

    /// Closes without capturing.
    pub fn cancel(&mut self) -> Outcome {
        if self.closing {
            return Outcome::Continue;
        }
        self.closing = true;
        Outcome::Cancelled
    }

    /// `RunCaptureAction`.
    fn run_action(&mut self, action: RegionCaptureAction) -> Outcome {
        match action {
            RegionCaptureAction::None | RegionCaptureAction::Unknown(_) => Outcome::Continue,
            RegionCaptureAction::CancelCapture => self.cancel(),
            RegionCaptureAction::RemoveShapeCancelCapture => {
                if self.try_remove_region_at(self.last_pointer) {
                    Outcome::Continue
                } else {
                    self.cancel()
                }
            }
            RegionCaptureAction::RemoveShape => {
                self.try_remove_region_at(self.last_pointer);
                Outcome::Continue
            }
            RegionCaptureAction::SwapToolType => {
                if self.region_tool_active {
                    self.activate_annotation_tool();
                } else {
                    self.activate_region_tool(None);
                }
                Outcome::Continue
            }
            RegionCaptureAction::CaptureFullscreen => self.complete_rects(vec![self.bounds()], false),
            RegionCaptureAction::CaptureActiveMonitor => self.complete_active_monitor(),
            RegionCaptureAction::CaptureLastRegion => self.complete_last_region(),
        }
    }

    /// `TryRemoveRegionAt`: the selection, else an added region.
    fn try_remove_region_at(&mut self, p: PointF) -> bool {
        if self.has_valid_selection() && self.selection.contains(p) {
            self.clear_selection();
            return true;
        }
        if let Some(i) = self.find_region_at(p) {
            self.remove_region(i);
            return true;
        }
        false
    }

    fn complete_last_region(&mut self) -> Outcome {
        let regions: Vec<RectF> = self
            .last_regions
            .iter()
            .map(|r| r.intersect(&self.screen_bounds))
            .filter(Rect::is_valid)
            .map(|r| RectF::new((r.x - self.screen_bounds.x) as f64, (r.y - self.screen_bounds.y) as f64, r.w as f64, r.h as f64))
            .collect();
        if regions.is_empty() {
            return Outcome::Continue;
        }
        self.complete_rects(regions, false)
    }

    fn complete_active_monitor(&mut self) -> Outcome {
        let d = (
            self.screen_bounds.x + self.last_pointer.x.round() as i32,
            self.screen_bounds.y + self.last_pointer.y.round() as i32,
        );
        let Some(screen) = crate::screenshot::screen_from_point(&self.screens, d) else { return Outcome::Continue };
        let b = screen.bounds;
        let relative = RectF::new((b.x - self.screen_bounds.x) as f64, (b.y - self.screen_bounds.y) as f64, b.w as f64, b.h as f64);
        let r = relative.intersect(&self.bounds());
        self.complete_rects(vec![r], false)
    }

    fn complete_selection(&mut self) -> Outcome {
        let regions = self.selected_regions();
        self.complete(regions, true)
    }

    /// `Complete(IReadOnlyList<Rect>, includeWindowInfo)`.
    fn complete_rects(&mut self, selections: Vec<RectF>, include_window_info: bool) -> Outcome {
        self.complete(selections.into_iter().map(CaptureRegion::rectangle).collect(), include_window_info)
    }

    /// `Complete`: crops the screenshot to the regions' bounds. With more
    /// than one region, or an ellipse or freehand region, the pixels
    /// outside every region are transparent.
    fn complete(&mut self, selections: Vec<CaptureRegion>, include_window_info: bool) -> Outcome {
        if self.closing {
            return Outcome::Continue;
        }
        let (iw, ih) = (self.width as i32, self.height as i32);
        let mut regions: Vec<Rect> = Vec::new();
        let mut shapes: Vec<CaptureRegion> = Vec::new();
        for region in selections {
            let s = region.bounds;
            if !s.is_valid() {
                continue;
            }
            let left = (s.x.floor() as i32).clamp(0, iw - 1);
            let top = (s.y.floor() as i32).clamp(0, ih - 1);
            let right = (s.right().ceil() as i32).clamp(left + 1, iw);
            let bottom = (s.bottom().ceil() as i32).clamp(top + 1, ih);
            regions.push(Rect::new(left, top, right - left, bottom - top));
            shapes.push(region);
        }
        let Some(bounds) = regions.iter().copied().reduce(|a, b| a.union(&b)) else { return Outcome::Continue };
        // `EditorView.GetSnapshot(Rect)`: with annotations, rendered with
        // them; premultiplied, as the mask works on it.
        let annotated = self.editor.as_mut().filter(|e| !e.annotations().is_empty() || e.text_edit().is_some());
        let non_rect = shapes.iter().any(|s| !s.is_rectangle());
        let (bw, bh) = (bounds.w as u32, bounds.h as u32);
        let image = if non_rect {
            let premul = match annotated {
                Some(e) => e.snapshot(bounds.x as u32, bounds.y as u32, bw, bh).to_premul_rgba(),
                None => {
                    let crop = image::imageops::crop_imm(&self.screenshot, bounds.x as u32, bounds.y as u32, bw, bh).to_image();
                    sharex_skia::raster::Bitmap::from_straight(bw, bh, crop.as_raw()).into_data()
                }
            };
            let paths: Vec<_> = shapes.iter().map(|s| s.create_path(-bounds.x as f64, -bounds.y as f64)).collect();
            premul_to_image(bw, bh, &shape::apply_path_mask(bw, bh, &premul, &paths))
        } else {
            let image = match annotated {
                Some(e) => {
                    let pm = e.snapshot(bounds.x as u32, bounds.y as u32, bw, bh);
                    RgbaImage::from_raw(pm.width(), pm.height(), pm.to_rgba()).unwrap_or_default()
                }
                None => image::imageops::crop_imm(&self.screenshot, bounds.x as u32, bounds.y as u32, bw, bh).to_image(),
            };
            if regions.len() > 1 {
                let local: Vec<Rect> = regions.iter().map(|r| r.offset(-bounds.x, -bounds.y)).collect();
                shape::apply_rect_mask(&image, &local)
            } else {
                image
            }
        };
        let image_modified = self.editor.as_ref().is_some_and(|e| e.is_dirty());
        let to_screen = |r: Rect| r.offset(self.screen_bounds.x, self.screen_bounds.y);
        let screen_rectangle = to_screen(bounds);
        let window_info = if include_window_info { self.find_top_level_window_info(screen_rectangle) } else { None };
        let window_handle = match self.selected_candidate.and_then(|i| self.windows.get(i)) {
            Some(w)
                if include_window_info
                    && shapes.len() == 1
                    && shapes[0].is_rectangle()
                    && w.is_window
                    && w.rect == screen_rectangle =>
            {
                w.handle
            }
            _ => 0,
        };
        self.closing = true;
        Outcome::Captured(Box::new(CaptureResult {
            image,
            screen_rectangle,
            window_info,
            image_modified,
            window_handle,
            regions: regions.into_iter().map(to_screen).collect(),
        }))
    }

    /// `FindTopLevelWindowInfo`: the picked window, else the window under
    /// the pointer (the clicked one), else under the selection's centre.
    fn find_top_level_window_info(&self, selected: Rect) -> Option<WindowInfo> {
        if let Some(w) = self.selected_candidate.and_then(|i| self.windows.get(i))
            && w.is_window
        {
            return Some(crate::platform::window_info(w.handle));
        }
        let mut point = (
            self.screen_bounds.x + self.last_pointer.x as i32,
            self.screen_bounds.y + self.last_pointer.y as i32,
        );
        if !selected.contains(point.0, point.1) {
            point = (selected.x + selected.w / 2, selected.y + selected.h / 2);
        }
        self.windows
            .iter()
            .find(|w| w.is_window && w.rect.contains(point.0, point.1))
            .map(|w| crate::platform::window_info(w.handle))
    }

    // The toolbar.

    /// `UpdateToolbarPosition`: the top centre of the monitor at `desktop`,
    /// unless a position was saved by dragging it.
    fn update_toolbar_position(&mut self, desktop: (i32, i32)) {
        let saved = self.options.ToolbarPosition;
        if saved != Point::default() && self.screens.iter().any(|s| s.bounds.contains(saved.x, saved.y)) {
            self.toolbar.anchor = Some((saved.x as f64, saved.y as f64));
            return;
        }
        let Some(screen) = crate::screenshot::screen_from_point(&self.screens, desktop).copied() else { return };
        self.toolbar.screen = screen.bounds;
        self.toolbar.anchor = Some((screen.bounds.x as f64 + screen.bounds.w as f64 / 2.0, screen.bounds.y as f64));
    }

    fn toolbar_pressed(&mut self, part: toolbar::Part, p: PointF) -> Outcome {
        match part {
            toolbar::Part::DragHandle => {
                let now = Instant::now();
                let double = self
                    .last_left_press
                    .is_some_and(|(t, at)| now.duration_since(t) <= DOUBLE_CLICK && at.distance(p) <= DOUBLE_CLICK_DISTANCE);
                self.last_left_press = Some((now, p));
                if double {
                    // Back to the monitor's top centre, and forgotten.
                    self.toolbar.drag = None;
                    self.options.ToolbarPosition = Point::default();
                    let d = (self.screen_bounds.x + p.x.round() as i32, self.screen_bounds.y + p.y.round() as i32);
                    self.update_toolbar_position(d);
                    return Outcome::Continue;
                }
                if let Some((ax, ay)) = self.toolbar.anchor {
                    let (px, py) = (self.screen_bounds.x as f64 + p.x, self.screen_bounds.y as f64 + p.y);
                    self.toolbar.drag = Some((px - ax, py - ay));
                }
            }
            toolbar::Part::RegionButton => self.activate_region_tool(Some(RegionShape::Rectangle)),
            toolbar::Part::Tool(i) => {
                // `SelectToolCommand`, then `ActivateAnnotationTool`.
                match self.bar.get(i).map(|t| (t.def.tool.clone(), t.def.id)) {
                    Some((Some(tool), _)) => {
                        if let Some(e) = self.editor.as_mut() {
                            e.select_tool(tool);
                            self.activate_annotation_tool();
                        }
                    }
                    // `HostToolbarItemRequested`: the ellipse and freehand
                    // regions (`RegionCaptureToolbar.GetRegionShape`).
                    Some((None, id)) => {
                        if let Some(shape) = toolbar::region_shape(id) {
                            self.activate_region_tool(Some(shape));
                        }
                    }
                    None => {}
                }
            }
            toolbar::Part::Option(item) => self.option_pressed(item),
            toolbar::Part::Background | toolbar::Part::Popup => {}
        }
        Outcome::Continue
    }

    /// The toolbar's top centre in desktop pixels.
    pub(crate) fn toolbar_anchor(&self) -> Option<(f64, f64)> {
        self.toolbar.anchor
    }

    /// The toolbar's scale for a custom button size (`ToolbarButtonSize`
    /// over the 40 DIP buttons).
    pub(crate) fn toolbar_scale(&self) -> f32 {
        if self.options.UseCustomToolbarButtonSize {
            self.options
                .ToolbarButtonSize
                .clamp(limits::TOOLBAR_BUTTON_SIZE_MINIMUM, limits::TOOLBAR_BUTTON_SIZE_MAXIMUM)
                as f32
                / 40.0
        } else {
            1.0
        }
    }

    /// The hide animation: 0 shown, 1 hidden (0.2 s).
    pub(crate) fn toolbar_hidden_amount(&self, now: Instant) -> f32 {
        let t = self
            .toolbar
            .hidden_changed
            .map(|at| (now.duration_since(at).as_secs_f32() / 0.2).min(1.0))
            .unwrap_or(1.0);
        if self.toolbar.hidden { t } else { 1.0 - t }
    }

    /// Tells the session which toolbar part is under the pointer, as the
    /// window that has the pointer laid it out.
    pub fn set_toolbar_hover(&mut self, part: Option<toolbar::Part>) {
        self.toolbar_hover = if self.toolbar.hidden { None } else { part };
        self.tip.hover(self.toolbar_hover, Instant::now());
    }

    /// The toolbar part under the pointer, as last told.
    pub fn toolbar_hover(&self) -> Option<toolbar::Part> {
        self.toolbar_hover
    }

    /// Whether the toolbar is being dragged.
    pub fn dragging_toolbar(&self) -> bool {
        self.toolbar.drag.is_some()
    }

    /// The cursor to show.
    pub fn cursor(&self) -> CursorKind {
        use cursors::CustomCursor as C;
        if self.toolbar.drag.is_some() {
            return CursorKind::Move;
        }
        match self.interaction {
            Interaction::Moving | Interaction::Resizing => return CursorKind::Custom(C::ClosedHand),
            Interaction::Creating | Interaction::PendingHover => return CursorKind::Custom(C::Crosshair),
            Interaction::None => {}
        }
        if self.slider_drag.is_some() {
            return CursorKind::Arrow;
        }
        if !self.region_tool_active
            && self.toolbar_hover.is_none()
            && let Some(e) = &self.editor
        {
            return match e.cursor() {
                ed::Cursor::Arrow => CursorKind::Arrow,
                ed::Cursor::Crosshair => CursorKind::Custom(C::Crosshair),
                ed::Cursor::OpenHand => CursorKind::Custom(C::OpenHand),
                ed::Cursor::ClosedHand => CursorKind::Custom(C::ClosedHand),
                ed::Cursor::IBeam => CursorKind::Text,
            };
        }
        match self.toolbar_hover {
            Some(toolbar::Part::DragHandle) => CursorKind::Move,
            Some(_) => CursorKind::Arrow,
            None if self.resize_node_at(self.last_pointer, self.monitor_scale_at(self.last_pointer)).is_some() => {
                CursorKind::Custom(C::OpenHand)
            }
            None => CursorKind::Custom(C::Crosshair),
        }
    }

    /// The options as the overlay left them (magnifier size, toolbar place
    /// and size), to keep for the next capture as ShareX does.
    pub fn into_options(self) -> RegionCaptureOptions {
        self.options
    }
}

/// A premultiplied RGBA buffer as straight RGBA, unpremultiplied as
/// Skia's PNG encoder does when ShareX saves it (the float pipeline's
/// `unpremul`, stored `v * 255 + 0.5` truncated: `sharex_tools::imaging::
/// unpremul_encode`, which matched ShareX's ellipse captures where
/// `Pixmap::to_rgba` was a level off on ties).
fn premul_to_image(width: u32, height: u32, premul: &[u8]) -> RgbaImage {
    use sharex_skia::raster::{load, unpremul_f};
    let mut out = premul.to_vec();
    for p in out.chunks_exact_mut(4) {
        match p[3] {
            255 => {}
            0 => p.copy_from_slice(&[0, 0, 0, 0]),
            _ => {
                let c = unpremul_f(load([p[0], p[1], p[2], p[3]]));
                for i in 0..3 {
                    p[i] = (c[i].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            }
        }
    }
    RgbaImage::from_raw(width, height, out).unwrap_or_default()
}


fn editor_mods(m: Modifiers) -> ed::Mods {
    ed::Mods { shift: m.shift, ctrl: m.ctrl, alt: m.alt }
}

/// The editor's key for an overlay key.
fn editor_key(k: Key) -> Option<ed::Key> {
    Some(match k {
        Key::Escape => ed::Key::Escape,
        Key::Enter => ed::Key::Enter,
        Key::Delete => ed::Key::Delete,
        Key::Backspace => ed::Key::Backspace,
        Key::Left => ed::Key::Left,
        Key::Right => ed::Key::Right,
        Key::Up => ed::Key::Up,
        Key::Down => ed::Key::Down,
        Key::Home => ed::Key::Home,
        Key::End => ed::Key::End,
        Key::PageUp => ed::Key::PageUp,
        Key::PageDown => ed::Key::PageDown,
        Key::Control => ed::Key::Control,
        Key::Shift => ed::Key::Shift,
        Key::H => ed::Key::Letter('H'),
        Key::Letter(c) => ed::Key::Letter(c),
        Key::Digit(d) => ed::Key::Digit(d),
        Key::NumPad0 => ed::Key::NumPad(0),
        Key::NumPad(n) => ed::Key::NumPad(n),
        _ => return None,
    })
}

/// `TrySelectToolForToolbarHotkey` over the capture toolbar's items
/// (Avalonia's key names: `R`, `D1`, `NumPad1`).
fn tool_hotkey(bar: &[toolbar::Item], k: ed::Key, m: ed::Mods) -> Option<EditorTool> {
    let name = match k {
        ed::Key::Letter(c) => c.to_string(),
        ed::Key::Digit(d) => format!("D{d}"),
        ed::Key::NumPad(n) => format!("NumPad{n}"),
        _ => return None,
    };
    toolbar::tool_for_hotkey(bar, &name, m.ctrl, m.shift, m.alt)
}

impl RegionCapture {
    /// An option's popup is open (its widgets take the pointer, through
    /// the overlay window's egui input).
    pub fn popup_open(&self) -> bool {
        self.popups.open.is_some()
    }

    /// The colour popup's screen colour picker request
    /// (`OnScreenColorPickerClick`: `ScreenColorPickerWindow.PickAsync`),
    /// taken by the host, which opens the picker over the overlay.
    pub fn take_pick_request(&mut self) -> Option<options_row::OptionItem> {
        self.pick_request.take()
    }

    /// The picked colour (`SelectedColorValue`, a recent colour).
    pub fn set_picked_color(&mut self, item: options_row::OptionItem, c: sharex_ui::color_view::Rgba8) {
        crate::editor_popups::add_recent_color(c);
        let hex = sharex_ui::accent::to_hex(c);
        if let Some(e) = self.editor.as_mut() {
            e.set_option(|s, t| match item {
                options_row::OptionItem::BorderColor => s.set_selected_color(&hex, t),
                options_row::OptionItem::FillColor => s.set_fill_color(&hex, t),
                options_row::OptionItem::TextColor => s.set_text_color(&hex, t),
                options_row::OptionItem::Shadow => s.set_shadow_color(&hex),
                _ => {}
            });
        }
    }

    /// The workspace's requests for the host (the emoji picker).
    pub fn take_editor_requests(&mut self) -> Vec<ed::Request> {
        self.editor.as_mut().map(|e| e.take_requests()).unwrap_or_default()
    }

    /// `InsertEmojiAnnotation` (`at`: the sticker's top-left, image pixels).
    pub fn insert_emoji(&mut self, unicode: &str, name: &str, at: (f64, f64)) {
        if let Some(e) = self.editor.as_mut() {
            e.insert_emoji(unicode, name, at);
        }
    }

    /// `ReplaceEmojiAnnotation`.
    pub fn replace_emoji(&mut self, id: &str, unicode: &str, name: &str) {
        if let Some(e) = self.editor.as_mut() {
            e.replace_emoji(id, unicode, name);
        }
    }

    /// The virtual screen's origin (image (0, 0)), desktop pixels.
    pub fn origin(&self) -> (i32, i32) {
        (self.screen_bounds.x, self.screen_bounds.y)
    }

    /// The toolbar's item with this id (`--render`, tests).
    pub fn toolbar_index(&self, id: &str) -> Option<usize> {
        self.bar.iter().position(|t| t.def.id == id)
    }
}

/// `CreateSelectionRectangle`: Shift makes it a square, kept on the image.
fn create_selection_rectangle(first: PointF, second: PointF, bounds: (f64, f64), square: bool) -> RectF {
    if !square {
        return create_pixel_rectangle(first, second, bounds);
    }
    let (dx, dy) = (second.x - first.x, second.y - first.y);
    let side = dx.abs().max(dy.abs());
    let available_w = if dx < 0.0 { first.x } else { bounds.0 - 1.0 - first.x };
    let available_h = if dy < 0.0 { first.y } else { bounds.1 - 1.0 - first.y };
    let side = side.min(available_w.min(available_h)).max(0.0);
    let constrained = PointF::new(
        first.x + if dx < 0.0 { -side } else { side },
        first.y + if dy < 0.0 { -side } else { side },
    );
    create_pixel_rectangle(first, constrained, bounds)
}

#[cfg(test)]
mod tests;
