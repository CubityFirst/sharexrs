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
//! selection resizing when quick capture is off. Not yet: annotation tools
//! (the image editor), ellipse and freehand regions (hidden toolbar items),
//! multi region mode, the screen crosshair option, custom info text,
//! tooltips.

pub mod cursors;
pub mod magnifier;
mod paint;
pub mod toolbar;

use std::time::{Duration, Instant};

use image::RgbaImage;

use crate::geometry::{PointF, Rect, RectF, create_pixel_rectangle};
use crate::options::{RegionCaptureAction, RegionCaptureMagnifierScrollAction, RegionCaptureOptions};
use crate::{Screen, SimpleWindowInfo, WindowInfo};

pub use paint::OverlayView;

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
}

/// `AvaloniaRegionCaptureResult`.
#[derive(Clone, Debug)]
pub struct CaptureResult {
    pub image: RgbaImage,
    /// Desktop pixels.
    pub screen_rectangle: Rect,
    /// The window it's of (the hovered window picked, else the window under
    /// the pointer), for naming.
    pub window_info: Option<WindowInfo>,
    /// Annotations were drawn (never yet: annotations aren't ported).
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
    interaction_start: RectF,
    /// `_regionOverlay.Selection` (rectangles only).
    pub(crate) selection: RectF,
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
    /// The toolbar part under the pointer.
    pub(crate) toolbar_hover: Option<toolbar::Part>,
    pub(crate) pointer_known: bool,
    closing: bool,
}

/// `Distance(...) >= 4` before a press on a hovered window starts a new
/// region instead.
const DRAG_THRESHOLD: f64 = 4.0;
/// System double-click time and distance (Windows defaults).
const DOUBLE_CLICK: Duration = Duration::from_millis(500);
const DOUBLE_CLICK_DISTANCE: f64 = 4.0;

impl RegionCapture {
    pub fn new(request: Request) -> Self {
        let Request { screenshot, screen_bounds, screens, options, windows, cursor, last_regions, enable_annotations } =
            request;
        let width = screen_bounds.w.max(1) as f64;
        let height = screen_bounds.h.max(1) as f64;
        let mut me = Self {
            screenshot,
            screen_bounds,
            screens,
            windows: if options.detect_windows { windows } else { Vec::new() },
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
            interaction_start: RectF::default(),
            selection: RectF::default(),
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
        let at = self.opened + Duration::from_millis(RegionCaptureOptions::INPUT_DELAY);
        if now >= at {
            self.keyboard_enabled = true;
        }
        let animating = self.toolbar.hidden_changed.is_some_and(|t| now < t + Duration::from_millis(200));
        if animating {
            return Some(now + Duration::from_millis(16));
        }
        (!self.keyboard_enabled).then_some(at)
    }

    fn clamp(&self, p: PointF) -> PointF {
        PointF::new(p.x.clamp(0.0, self.width), p.y.clamp(0.0, self.height))
    }

    fn bounds(&self) -> RectF {
        RectF::new(0.0, 0.0, self.width, self.height)
    }

    /// `HasValidSelection`.
    pub(crate) fn has_valid_selection(&self) -> bool {
        self.selection.w >= RegionCaptureOptions::MINIMUM_SIZE && self.selection.h >= RegionCaptureOptions::MINIMUM_SIZE
    }

    /// `IsQuickCapture` (multi region mode isn't ported, so it's the option).
    fn quick_capture(&self) -> bool {
        self.options.quick_capture
    }

    /// Whether the resize nodes show (`SetRegionResizeNodesVisible`).
    pub(crate) fn resize_nodes_visible(&self) -> bool {
        !self.quick_capture() && self.interaction == Interaction::None && self.has_valid_selection()
    }

    /// `ActiveRectangle`: the selection, else the hovered window.
    pub(crate) fn active_rectangle(&self) -> RectF {
        if self.selection.is_valid() { self.selection } else { self.hover }
    }

    pub(crate) fn magnifier_shown(&self) -> bool {
        self.options.show_magnifier && !self.magnifier_hidden_by_wheel
    }

    fn set_selection(&mut self, r: RectF, candidate: Option<usize>) {
        self.selection = r.intersect(&self.bounds());
        self.selected_candidate = candidate;
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
        self.selection = RectF::default();
        self.update_hover(self.last_pointer);
    }

    /// `UpdateHoverRectangle`: the window or control under the pointer.
    fn update_hover(&mut self, p: PointF) {
        if self.has_valid_selection() || self.interaction != Interaction::None {
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
        if !m.ctrl {
            self.moving_during_creation = false;
        } else if self.selection.is_valid() && (self.moving_during_creation || !self.ctrl_held_during_creation) {
            // `MoveSelectionDuringCreation`
            let before = self.selection;
            let (dx, dy) = (p.x - self.last_creation_point.x, p.y - self.last_creation_point.y);
            self.move_selection(dx, dy, before);
            let after = self.selection;
            self.press_point =
                self.clamp(PointF::new(self.press_point.x + after.x - before.x, self.press_point.y + after.y - before.y));
            self.moving_during_creation = true;
            self.last_creation_point = p;
            self.ctrl_held_during_creation = true;
            return;
        }
        let r = create_selection_rectangle(self.press_point, p, (self.width, self.height), m.shift);
        self.set_selection(r, None);
        self.last_creation_point = p;
        self.ctrl_held_during_creation = m.ctrl;
    }

    /// `MoveSelection`: kept inside the screenshot.
    fn move_selection(&mut self, dx: f64, dy: f64, source: RectF) {
        let x = (source.x + dx).clamp(0.0, (self.width - source.w).max(0.0));
        let y = (source.y + dy).clamp(0.0, (self.height - source.h).max(0.0));
        self.set_selection(source.translate(x - source.x, y - source.y), None);
    }

    fn resize_selection(&mut self, p: PointF) {
        let Some(node) = self.resize_node else { return };
        let start = self.interaction_start;
        let at = node.position(start);
        let r = node.resize(start, p.x - at.x, p.y - at.y);
        self.set_selection(r, None);
    }

    /// Which resize node is under `p` (image pixels), when they show.
    fn resize_node_at(&self, p: PointF, scale: f64) -> Option<ResizeNode> {
        if !self.resize_nodes_visible() {
            return None;
        }
        let half = RESIZE_NODE_SIZE * scale / 2.0;
        ResizeNode::ALL.into_iter().find(|n| {
            let c = n.position(self.selection);
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
        // `FollowPointerWithCaptureToolbar`
        let desktop = (self.screen_bounds.x + p.x.round() as i32, self.screen_bounds.y + p.y.round() as i32);
        if let Some((ox, oy)) = self.toolbar.drag {
            let (dx, dy) = (self.screen_bounds.x as f64 + p.x - ox, self.screen_bounds.y as f64 + p.y - oy);
            self.toolbar.anchor = Some((dx, dy));
            self.last_pointer = self.clamp(p);
            return Outcome::Continue;
        }
        if self.toolbar.follows_pointer
            && self.options.toolbar_position == (0, 0)
            && !self.toolbar.screen.contains(desktop.0, desktop.1)
        {
            self.update_toolbar_position(desktop);
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
                    self.selection = RectF::default();
                    self.update_selection_during_creation(p, m);
                }
            }
            Interaction::Creating => self.update_selection_during_creation(p, m),
            Interaction::Moving => {
                let start = self.interaction_start;
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
        if button == Button::Left
            && let Some(part) = self.toolbar_hover
        {
            return self.toolbar_pressed(part, p);
        }
        if self.toolbar_hover.is_some() {
            return Outcome::Continue;
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
            Button::Middle => return self.run_action(self.options.region_capture_action_middle_click),
            Button::X1 => return self.run_action(self.options.region_capture_action_x1_click),
            Button::X2 => return self.run_action(self.options.region_capture_action_x2_click),
            Button::Left => {}
        }
        let now = Instant::now();
        let double = self
            .last_left_press
            .is_some_and(|(t, at)| now.duration_since(t) <= DOUBLE_CLICK && at.distance(p) <= DOUBLE_CLICK_DISTANCE);
        self.last_left_press = Some((now, p));
        if double && self.has_valid_selection() && self.selection.contains(p) {
            return self.complete_selection();
        }
        self.press_point = p;
        self.interaction_start = self.selection;
        self.resize_node = self.resize_node_at(p, scale);
        if self.resize_node.is_some() {
            self.set_interaction(Interaction::Resizing);
            self.resize_selection(p);
        } else if self.has_valid_selection() && self.selection.contains(p) {
            self.set_interaction(Interaction::Moving);
        } else if self.hover.is_valid() {
            self.set_selection(self.hover, self.hover_candidate);
            self.interaction_start = self.selection;
            self.set_interaction(Interaction::PendingHover);
        } else {
            self.set_selection(RectF::default(), None);
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
    pub fn pointer_released(&mut self, p: PointF, button: Button, _m: Modifiers) -> Outcome {
        if self.closing {
            return Outcome::Continue;
        }
        if button == Button::Left && self.toolbar.drag.take().is_some() {
            // `EndToolbarDrag`: the position is kept for the next capture.
            if let Some((x, y)) = self.toolbar.anchor {
                self.options.toolbar_position = (x.round() as i32, y.round() as i32);
            }
            return Outcome::Continue;
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
            return self.run_action(self.options.region_capture_action_right_click);
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
        if !self.has_valid_selection() {
            if self.hover.is_valid() {
                self.set_selection(self.hover, self.hover_candidate);
            } else {
                self.clear_selection();
            }
        }
        if self.has_valid_selection() && self.quick_capture() {
            return self.complete(vec![self.selection], true);
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
                let current = if self.options.use_custom_toolbar_button_size { self.options.toolbar_button_size } else { 40 };
                let step = if delta_y > 0.0 { 4 } else { -4 };
                self.options.use_custom_toolbar_button_size = true;
                self.options.toolbar_button_size = (current + step).clamp(
                    RegionCaptureOptions::TOOLBAR_BUTTON_SIZE_MINIMUM,
                    RegionCaptureOptions::TOOLBAR_BUTTON_SIZE_MAXIMUM,
                );
            }
            return Outcome::Continue;
        }
        let direction = delta_y.signum();
        if delta_y == 0.0 || !self.options.show_magnifier {
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
        let mut action = self.options.magnifier_scroll_action;
        if m.ctrl {
            action = match action {
                RegionCaptureMagnifierScrollAction::Zoom => RegionCaptureMagnifierScrollAction::Resize,
                RegionCaptureMagnifierScrollAction::Resize => RegionCaptureMagnifierScrollAction::Zoom,
            };
        }
        let scale = self.monitor_scale_at(self.last_pointer);
        match action {
            RegionCaptureMagnifierScrollAction::Resize => self.resize_magnifier(up, scale),
            RegionCaptureMagnifierScrollAction::Zoom => {
                let count = magnifier::normalize_pixel_count(self.options.magnifier_pixel_count, self.options.magnifier_size, scale);
                self.options.magnifier_pixel_count = magnifier::normalize_pixel_count(
                    count + if up { -2 } else { 2 },
                    self.options.magnifier_size,
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
            .magnifier_size
            .clamp(RegionCaptureOptions::MAGNIFIER_SIZE_MINIMUM, RegionCaptureOptions::MAGNIFIER_SIZE_MAXIMUM);
        let count = magnifier::normalize_pixel_count(o.magnifier_pixel_count, o.magnifier_size, scale);
        let next = count + if grow { 2 } else { -2 };
        let size = (current as f64 / count as f64 * next as f64).round() as i32;
        if size < RegionCaptureOptions::MAGNIFIER_SIZE_MINIMUM && current == RegionCaptureOptions::MAGNIFIER_SIZE_MINIMUM {
            self.magnifier_hidden_by_wheel = true;
            return;
        }
        let size = size.clamp(RegionCaptureOptions::MAGNIFIER_SIZE_MINIMUM, RegionCaptureOptions::MAGNIFIER_SIZE_MAXIMUM);
        if size == current {
            return;
        }
        o.magnifier_size = size;
        o.magnifier_pixel_count = next.clamp(
            RegionCaptureOptions::MAGNIFIER_PIXEL_COUNT_MINIMUM,
            RegionCaptureOptions::MAGNIFIER_PIXEL_COUNT_MAXIMUM,
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
        let none = m == Modifiers::default();
        if self.keyboard_enabled && none && matches!(key, Key::Tab | Key::NumPad0) {
            // `ActivateRegionTool`: it's the only tool here.
            self.update_hover(self.last_pointer);
            return Outcome::Continue;
        }
        if self.keyboard_enabled && key == Key::H && m.ctrl && !m.shift && !m.alt {
            self.toolbar.hidden = !self.toolbar.hidden;
            self.toolbar.hidden_changed = Some(Instant::now());
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
        match key {
            Key::Enter => {
                if !self.has_valid_selection() && self.hover.is_valid() {
                    self.set_selection(self.hover, self.hover_candidate);
                }
                if self.has_valid_selection() {
                    return self.complete_selection();
                }
            }
            Key::Space => return self.complete(vec![self.bounds()], false),
            Key::Tilde => return self.complete_active_monitor(),
            Key::Left | Key::Right | Key::Up | Key::Down => {
                let d = if m.shift { RegionCaptureOptions::MOVE_SPEED_MAXIMUM } else { RegionCaptureOptions::MOVE_SPEED_MINIMUM };
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
            }
            Key::Delete if self.has_valid_selection() => self.clear_selection(),
            _ => {}
        }
        Outcome::Continue
    }

    /// A key came up (`OnKeyUp`): Escape cancels.
    pub fn key_up(&mut self, key: Key, m: Modifiers) -> Outcome {
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
        self.cancel()
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
            RegionCaptureAction::None => Outcome::Continue,
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
            // Switches to the annotation tools, which aren't ported yet.
            RegionCaptureAction::SwapToolType => Outcome::Continue,
            RegionCaptureAction::CaptureFullscreen => self.complete(vec![self.bounds()], false),
            RegionCaptureAction::CaptureActiveMonitor => self.complete_active_monitor(),
            RegionCaptureAction::CaptureLastRegion => self.complete_last_region(),
        }
    }

    fn try_remove_region_at(&mut self, p: PointF) -> bool {
        if self.has_valid_selection() && self.selection.contains(p) {
            self.clear_selection();
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
        self.complete(regions, false)
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
        self.complete(vec![r], false)
    }

    fn complete_selection(&mut self) -> Outcome {
        let mut regions = Vec::new();
        if self.has_valid_selection() {
            regions.push(self.selection);
        }
        self.complete(regions, true)
    }

    /// `Complete`: crops the screenshot to the regions' bounds.
    fn complete(&mut self, selections: Vec<RectF>, include_window_info: bool) -> Outcome {
        if self.closing {
            return Outcome::Continue;
        }
        let (iw, ih) = (self.width as i32, self.height as i32);
        let regions: Vec<Rect> = selections
            .iter()
            .filter(|s| s.is_valid())
            .map(|s| {
                let left = (s.x.floor() as i32).clamp(0, iw - 1);
                let top = (s.y.floor() as i32).clamp(0, ih - 1);
                let right = (s.right().ceil() as i32).clamp(left + 1, iw);
                let bottom = (s.bottom().ceil() as i32).clamp(top + 1, ih);
                Rect::new(left, top, right - left, bottom - top)
            })
            .collect();
        let Some(bounds) = regions.iter().copied().reduce(|a, b| a.union(&b)) else { return Outcome::Continue };
        let image = image::imageops::crop_imm(&self.screenshot, bounds.x as u32, bounds.y as u32, bounds.w as u32, bounds.h as u32)
            .to_image();
        let to_screen = |r: Rect| r.offset(self.screen_bounds.x, self.screen_bounds.y);
        let screen_rectangle = to_screen(bounds);
        let window_info = if include_window_info { self.find_top_level_window_info(screen_rectangle) } else { None };
        let window_handle = match self.selected_candidate.and_then(|i| self.windows.get(i)) {
            Some(w) if include_window_info && regions.len() == 1 && w.is_window && w.rect == screen_rectangle => w.handle,
            _ => 0,
        };
        self.closing = true;
        Outcome::Captured(Box::new(CaptureResult {
            image,
            screen_rectangle,
            window_info,
            image_modified: false,
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
        let saved = self.options.toolbar_position;
        if saved != (0, 0) && self.screens.iter().any(|s| s.bounds.contains(saved.0, saved.1)) {
            self.toolbar.anchor = Some((saved.0 as f64, saved.1 as f64));
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
                    self.options.toolbar_position = (0, 0);
                    let d = (self.screen_bounds.x + p.x.round() as i32, self.screen_bounds.y + p.y.round() as i32);
                    self.update_toolbar_position(d);
                    return Outcome::Continue;
                }
                if let Some((ax, ay)) = self.toolbar.anchor {
                    let (px, py) = (self.screen_bounds.x as f64 + p.x, self.screen_bounds.y as f64 + p.y);
                    self.toolbar.drag = Some((px - ax, py - ay));
                }
            }
            toolbar::Part::RegionButton => self.update_hover(self.last_pointer),
            // The annotation tools aren't ported yet.
            toolbar::Part::Tool(_) | toolbar::Part::Background => {}
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
        if self.options.use_custom_toolbar_button_size {
            self.options
                .toolbar_button_size
                .clamp(RegionCaptureOptions::TOOLBAR_BUTTON_SIZE_MINIMUM, RegionCaptureOptions::TOOLBAR_BUTTON_SIZE_MAXIMUM)
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
