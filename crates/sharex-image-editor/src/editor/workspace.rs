//! The workspace state machine: `EditorCore` (annotations, history),
//! `EditorInputController` (the tools' gestures), `EditorSelectionController`
//! (selection, handles, hover outlines) and `EditorView`'s keys.

use std::time::{Duration, Instant};

use sharex_skia::Pixmap;

use super::geometry::{self, Rect, Segment, rotate_point, shape_hit};
use super::history::{History, Memento};
use super::tool_state::{Target, ToolState};
use crate::model::{Annotation, EditorTool, Kind, Point};
use crate::options::ImageEditorOptions;
use crate::render;

/// A point in image pixels (Avalonia's doubles).
pub type P = (f64, f64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { shift: false, ctrl: false, alt: false };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// The keys the workspace acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Escape,
    Enter,
    Delete,
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Control,
    Shift,
    /// A letter, upper case.
    Letter(char),
    Digit(u8),
    NumPad(u8),
    Other,
}

/// `SelectionResizeNodeKind` and the other handle tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleKind {
    TopLeft,
    TopCenter,
    TopRight,
    RightCenter,
    BottomRight,
    BottomCenter,
    BottomLeft,
    LeftCenter,
    Rotate,
    SegmentStart,
    SegmentEnd,
    SegmentCenter,
    StepTail,
}

impl HandleKind {
    pub const NODES: [HandleKind; 8] = [
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::RightCenter,
        Self::BottomRight,
        Self::BottomCenter,
        Self::BottomLeft,
        Self::LeftCenter,
    ];

    /// `SelectionResizeNode.TryGetDirection`.
    pub fn direction(self) -> Option<(i32, i32)> {
        let d = match self {
            Self::TopLeft => (-1, -1),
            Self::TopCenter => (0, -1),
            Self::TopRight => (1, -1),
            Self::RightCenter => (1, 0),
            Self::BottomRight => (1, 1),
            Self::BottomCenter => (0, 1),
            Self::BottomLeft => (-1, 1),
            Self::LeftCenter => (-1, 0),
            _ => return None,
        };
        Some(d)
    }

    /// `SelectionResizeNode.GetPosition` on `(left, top, width, height)`.
    fn position(self, r: (f64, f64, f64, f64)) -> P {
        let (l, t, w, h) = r;
        let (cx, cy) = (l + w / 2.0, t + h / 2.0);
        match self {
            Self::TopLeft => (l, t),
            Self::TopCenter => (cx, t),
            Self::TopRight => (l + w, t),
            Self::RightCenter => (l + w, cy),
            Self::BottomRight => (l + w, t + h),
            Self::BottomCenter => (cx, t + h),
            Self::BottomLeft => (l, t + h),
            _ => (l, cy),
        }
    }
}

/// A selection handle (`SelectionResizeNode.Create`: 15 across, white,
/// a soft shadow), centred on its point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handle {
    pub kind: HandleKind,
    pub at: P,
}

/// `SelectionResizeNode`'s size.
pub const HANDLE_SIZE: f64 = 15.0;

/// The dashed outline around the hovered or selected shape
/// (`UpdateHoverOutline`: black and white 1px dashes, 3 on 3).
#[derive(Clone, Debug, PartialEq)]
pub enum Outline {
    Polyline(Vec<P>),
    Ellipse { rect: (f64, f64, f64, f64) },
    /// The rectangle, rotated by `angle` about `center`.
    Rectangle { rect: (f64, f64, f64, f64), angle: f64, center: P },
}

/// The cursor the workspace wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Arrow,
    Crosshair,
    OpenHand,
    ClosedHand,
    IBeam,
}

/// Text being typed in place (the creation `TextBox`, or the editor of an
/// existing text annotation).
#[derive(Clone, Debug)]
pub struct TextEdit {
    /// The annotation being made, or a copy of the one being edited.
    pub annotation: Annotation,
    /// Editing an existing annotation (its id).
    pub existing: Option<String>,
    pub text: String,
    /// The caret, in chars.
    pub caret: usize,
}

struct Drawing {
    /// The annotation being drawn (in the list), or none (Select).
    id: Option<String>,
    start: P,
    last: P,
    moving: bool,
    ctrl_was_held: bool,
    freehand_straight: bool,
}

enum Drag {
    Handle { kind: HandleKind, start: P, resize_start_rect: (f64, f64, f64, f64), resize_start_point: P },
    Shape { last: P },
}

/// What a key did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyResult {
    pub handled: bool,
    /// A tool was chosen by its hotkey.
    pub tool: Option<EditorTool>,
}

/// `MinShapeSize`: smaller shapes are dropped.
const MIN_SHAPE_SIZE: f64 = 5.0;
const DOUBLE_CLICK: Duration = Duration::from_millis(500);
const DOUBLE_CLICK_DISTANCE: f64 = 4.0;

/// The shared workspace (`EditorView` with its view model).
pub struct Workspace {
    source: Pixmap,
    annotations: Vec<Annotation>,
    history: History,
    pub tools: ToolState,
    tool: EditorTool,
    selected: Option<String>,
    hovered: Option<String>,
    drawing: Option<Drawing>,
    drag: Option<Drag>,
    number_counter: i32,
    dirty: bool,
    text: Option<TextEdit>,
    mods: Mods,
    pointer: Option<P>,
    last_press: Option<(Instant, P)>,
    next_id: u64,
    version: u64,
    cache: render::Cache,
    composite: Option<(u64, Pixmap)>,
}

fn sk(p: P) -> Point {
    Point::new(p.0 as f32, p.1 as f32)
}

fn tool_of(a: &Annotation) -> EditorTool {
    a.common.tool_type.clone()
}

/// `SnapTo45Degrees`.
pub fn snap_45(start: P, end: P) -> P {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let d = (dx * dx + dy * dy).sqrt();
    if d < 0.001 {
        return end;
    }
    let step = std::f64::consts::PI / 4.0;
    let a = ((dy.atan2(dx)) / step).round() * step;
    (start.0 + d * a.cos(), start.1 + d * a.sin())
}

/// `IsColorLight`.
fn is_color_light(hex: &str) -> bool {
    let c = render::parse_color(hex);
    (0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64) / 255.0 > 0.5
}

impl Workspace {
    /// `LoadWorkspaceImage`, with the view model made from `options`.
    pub fn new(source: Pixmap, options: ImageEditorOptions) -> Self {
        let tool = initial_tool(&options);
        Self {
            source,
            annotations: Vec::new(),
            history: History::default(),
            tools: ToolState::new(options, tool.clone()),
            tool,
            selected: None,
            hovered: None,
            drawing: None,
            drag: None,
            number_counter: 1,
            dirty: false,
            text: None,
            mods: Mods::NONE,
            pointer: None,
            last_press: None,
            next_id: 0,
            version: 0,
            cache: render::Cache::default(),
            composite: None,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.source.width(), self.source.height())
    }

    pub fn source(&self) -> &Pixmap {
        &self.source
    }

    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    pub fn active_tool(&self) -> EditorTool {
        self.tool.clone()
    }

    /// `IsDirty`: annotations were added or removed.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn number_counter(&self) -> i32 {
        self.number_counter
    }

    pub fn text_edit(&self) -> Option<&TextEdit> {
        self.text.as_ref()
    }

    pub fn selected(&self) -> Option<&Annotation> {
        self.selected.as_ref().and_then(|id| self.find(id)).map(|i| &self.annotations[i])
    }

    /// Changes whenever what [`Self::composite`] shows changes.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn is_drawing(&self) -> bool {
        self.drawing.as_ref().is_some_and(|d| d.id.is_some())
    }

    /// Something is being drawn, moved or resized.
    pub fn is_interacting(&self) -> bool {
        self.is_drawing() || self.drag.is_some()
    }

    fn find(&self, id: &str) -> Option<usize> {
        self.annotations.iter().position(|a| a.common.id == id)
    }

    fn get(&self, id: &Option<String>) -> Option<&Annotation> {
        id.as_ref().and_then(|id| self.find(id)).map(|i| &self.annotations[i])
    }

    fn get_mut(&mut self, id: &Option<String>) -> Option<&mut Annotation> {
        let i = id.as_ref().and_then(|id| self.find(id))?;
        Some(&mut self.annotations[i])
    }

    fn changed(&mut self) {
        self.version += 1;
    }

    fn new_id(&mut self) -> String {
        self.next_id += 1;
        format!("00000000-0000-4000-8000-{:012x}", self.next_id)
    }

    /// The image with every annotation, as exported. The text being typed
    /// isn't in it: it's in its text box ([`text_box`]) until committed.
    pub fn composite(&mut self) -> &Pixmap {
        if self.composite.as_ref().is_none_or(|(v, _)| *v != self.version) {
            let list: Vec<Annotation> = self
                .annotations
                .iter()
                .filter(|a| self.text.as_ref().and_then(|t| t.existing.as_ref()) != Some(&a.common.id))
                .cloned()
                .collect();
            let pm = render::render_cached(&self.source, &list, &mut self.cache);
            self.composite = Some((self.version, pm));
        }
        &self.composite.as_ref().unwrap().1
    }

    /// `GetSnapshot(Rect)`: the image pixels in `(x, y, w, h)` with the
    /// annotations, no selection chrome.
    pub fn snapshot(&mut self, x: u32, y: u32, w: u32, h: u32) -> Pixmap {
        self.commit_text();
        let pm = if self.annotations.is_empty() { self.source.clone() } else { self.composite().clone() };
        let mut out = Pixmap::new(w, h);
        for row in 0..h {
            for col in 0..w {
                *out.pixel_mut(col as i32, row as i32) = pm.pixel((x + col) as i32, (y + row) as i32);
            }
        }
        out
    }

    // History.

    fn memento(&self, exclude: Option<&str>) -> Memento {
        Memento {
            annotations: self.annotations.iter().filter(|a| Some(a.common.id.as_str()) != exclude).cloned().collect(),
            selected: self.selected.clone(),
        }
    }

    /// `EditorCore` raised `HistoryChanged`: dirty, and the step counter
    /// recalculated.
    fn history_changed(&mut self) {
        self.dirty = true;
        self.recalculate_number_counter();
        self.changed();
    }

    /// `RecalculateNumberCounter`.
    fn recalculate_number_counter(&mut self) {
        let max = self
            .annotations
            .iter()
            .filter_map(|a| match &a.kind {
                Kind::Number(p) => Some(p.number),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            .max(0);
        self.number_counter = (max + 1).max(self.tools.step_start_number);
    }

    /// `Undo`.
    pub fn undo(&mut self) {
        let current = self.memento(None);
        if let Some(m) = self.history.undo(current) {
            self.restore(m);
        }
    }

    /// `Redo`.
    pub fn redo(&mut self) {
        let current = self.memento(None);
        if let Some(m) = self.history.redo(current) {
            self.restore(m);
        }
    }

    fn restore(&mut self, m: Memento) {
        self.annotations = m.annotations;
        self.drag = None;
        self.hovered = None;
        self.selected = m.selected.filter(|id| self.find(id).is_some());
        self.hovered = self.selected.clone();
        self.history_changed();
    }

    /// `AddAnnotation`: the history first, then the annotation (here it's
    /// already in the list, where its visual went).
    fn add_annotation(&mut self, id: &str) {
        let m = self.memento(Some(id));
        self.history.add(m);
        self.history_changed();
    }

    /// `RemoveAnnotation`, renumbering the steps after a deleted step.
    fn remove_annotation(&mut self, id: &str) {
        let Some(i) = self.find(id) else { return };
        let m = self.memento(None);
        self.history.add(m);
        let removed = self.annotations.remove(i);
        if let Kind::Number(n) = &removed.kind {
            for a in &mut self.annotations {
                if let Kind::Number(p) = &mut a.kind
                    && p.number > n.number
                {
                    p.number -= 1;
                }
            }
        }
        self.history_changed();
    }

    /// `PerformDelete`: the selected annotation.
    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected.clone() {
            self.remove_annotation(&id);
            self.clear_selection();
        }
    }

    /// `ClearAllAnnotations` (`ClearAll(resetHistory: false)`).
    pub fn clear_annotations(&mut self) {
        if self.annotations.is_empty() {
            return;
        }
        self.clear_selection();
        let m = self.memento(None);
        self.history.add(m);
        self.annotations.clear();
        self.history_changed();
        self.number_counter = self.tools.step_start_number;
    }

    /// `MoveSelectedAnnotation` (Home, End, Page Up, Page Down).
    fn move_selected_to(&mut self, target: impl Fn(usize, usize) -> Option<usize>) {
        let Some(i) = self.selected.clone().and_then(|id| self.find(&id)) else { return };
        let n = self.annotations.len();
        let Some(t) = target(i, n).filter(|t| *t < n && *t != i) else { return };
        let m = self.memento(None);
        self.history.add(m);
        let a = self.annotations.remove(i);
        self.annotations.insert(t, a);
        self.history_changed();
    }

    // Tools.

    /// `SelectToolCommand`: deselects, then makes `tool` active (loading
    /// its options when it changes, `OnActiveToolChanged`).
    pub fn select_tool(&mut self, tool: EditorTool) {
        self.commit_text();
        self.clear_selection();
        if matches!(tool, EditorTool::Image | EditorTool::Emoji) {
            // `RequestImageInsertion` / `ShowEmojiPickerDialog` (not ported):
            // the active tool stays.
            return;
        }
        if tool == self.tool {
            return;
        }
        if is_rememberable(&tool) {
            self.tools.options.LastUsedAnnotationTool = tool.clone();
        }
        self.tool = tool.clone();
        self.tools.load_for_tool(tool);
        self.changed();
    }

    /// Changes a tool option with one of [`ToolState`]'s setters, for the
    /// active tool (and the selected annotation in Select).
    pub fn set_option(&mut self, f: impl FnOnce(&mut ToolState, Target)) {
        let selected = self.selected().cloned();
        let t = Target { tool: &self.tool, selected: selected.as_ref() };
        f(&mut self.tools, t);
        self.changed();
    }

    /// `StepStartNumber`: resets the counter.
    pub fn set_step_start_number(&mut self, v: i32) {
        if self.tools.set_step_start_number(v) {
            self.number_counter = self.tools.step_start_number;
            self.recalculate_number_counter();
            self.changed();
        }
    }

    // Selection.

    /// `ClearSelection`.
    pub fn clear_selection(&mut self) {
        if self.selected.is_some() || self.hovered.is_some() || self.drag.is_some() {
            self.changed();
        }
        self.selected = None;
        self.drag = None;
        self.hovered = None;
    }

    /// `SetSelectedShape`.
    fn set_selected(&mut self, id: String) {
        self.selected = Some(id.clone());
        self.hovered = Some(id);
        self.changed();
    }

    /// `ShouldIgnoreSelection`: Ctrl with a drawing tool draws over shapes.
    fn ignore_selection(&self) -> bool {
        self.tool != EditorTool::Select && self.mods.ctrl
    }

    /// `HitTestShape`: the topmost annotation under `p`.
    fn hit(&self, p: P) -> Option<usize> {
        self.annotations.iter().rposition(|a| !matches!(a.kind, Kind::Crop) && shape_hit(a, p.0, p.1))
    }

    /// `IsActiveToolShape`.
    fn is_active_tool_shape(&self, a: &Annotation) -> bool {
        match self.tool {
            EditorTool::Select => true,
            EditorTool::Crop | EditorTool::CutOut => false,
            EditorTool::Spotlight => matches!(a.kind, Kind::Spotlight(_)),
            _ => tool_of(a) == self.tool,
        }
    }

    /// `HitTestActiveToolShape`.
    fn hit_active(&self, p: P) -> Option<usize> {
        self.hit(p).filter(|i| self.is_active_tool_shape(&self.annotations[*i]))
    }

    /// `DeleteWorkspaceAnnotationAt`: the topmost annotation of the active
    /// tool under `p`.
    pub fn delete_at(&mut self, p: P) -> bool {
        let Some(i) = self.hit_active(p) else { return false };
        let id = self.annotations[i].common.id.clone();
        self.set_selected(id);
        self.delete_selected();
        true
    }

    /// `CancelActiveInteractionOrSelection`: the selection goes, if any.
    pub fn cancel_interaction_or_selection(&mut self) -> bool {
        if self.selected.is_some() {
            self.clear_selection();
            return true;
        }
        false
    }

    /// `GetLogicalRect`: the annotation's bounds (left, top, width, height).
    fn logical_rect(a: &Annotation) -> (f64, f64, f64, f64) {
        let b = geometry::bounds(a);
        (b.l as f64, b.t as f64, b.width() as f64, b.height() as f64)
    }

    fn rotatable(a: &Annotation) -> bool {
        matches!(
            a.kind,
            Kind::SpeechBalloon(_)
                | Kind::Highlight(_)
                | Kind::Blur(_)
                | Kind::Pixelate(_)
                | Kind::Magnify(_)
                | Kind::Rectangle(_)
                | Kind::SmartEraser(_)
                | Kind::Ellipse(_)
                | Kind::Text(_)
                | Kind::Emoji(_)
                | Kind::Image(_)
        )
    }

    /// `UpdateSelectionHandles`: the selected annotation's handles, and the
    /// dotted line to the rotate handle.
    pub fn handles(&self) -> (Vec<Handle>, Option<(P, P)>) {
        let Some(a) = self.selected() else { return (Vec::new(), None) };
        let interacting = self.drag.is_some();
        if !interacting && self.ignore_selection() {
            return (Vec::new(), None);
        }
        let mut out = Vec::new();
        if let Some(seg) = Segment::of(a) {
            let h = |kind, p: Point| Handle { kind, at: (p.x as f64, p.y as f64) };
            out.push(h(HandleKind::SegmentStart, seg.start));
            out.push(h(HandleKind::SegmentEnd, seg.end));
            if seg.supports_curve {
                out.push(h(HandleKind::SegmentCenter, seg.effective_curve_point()));
            }
            return (out, None);
        }
        match &a.kind {
            Kind::Number(p) => {
                if p.tail_enabled {
                    let at = if p.tail_point_initialized {
                        (p.tail_point.x as f64, p.tail_point.y as f64)
                    } else {
                        let b = geometry::bounds(a);
                        (b.r as f64, b.b as f64)
                    };
                    out.push(Handle { kind: HandleKind::StepTail, at });
                }
                return (out, None);
            }
            Kind::Cursor(_) | Kind::Freehand(_) => return (out, None),
            _ => {}
        }
        let (l, t, mut w, mut h) = Self::logical_rect(a);
        if Self::rotatable(a) {
            if w <= 0.0 {
                w = 20.0;
            }
            if h <= 0.0 {
                h = 20.0;
            }
            let c = (l + w / 2.0, t + h / 2.0);
            let angle = a.common.rotation_angle as f64;
            for kind in HandleKind::NODES {
                out.push(Handle { kind, at: rotate_point(kind.position((l, t, w, h)), c, angle) });
            }
            let top = rotate_point((l + w / 2.0, t), c, angle);
            let rot = rotate_point((l + w / 2.0, t - 30.0), c, angle);
            out.push(Handle { kind: HandleKind::Rotate, at: rot });
            return (out, Some((top, rot)));
        }
        if w <= 0.0 || h <= 0.0 {
            return (out, None);
        }
        for kind in HandleKind::NODES {
            out.push(Handle { kind, at: kind.position((l, t, w, h)) });
        }
        (out, None)
    }

    fn handle_at(&self, p: P) -> Option<HandleKind> {
        let half = HANDLE_SIZE / 2.0;
        // Later handles are on top.
        self.handles()
            .0
            .iter()
            .rev()
            .find(|h| (p.0 - h.at.0).abs() <= half && (p.1 - h.at.1).abs() <= half)
            .map(|h| h.kind)
    }

    /// `UpdateHoverOutline`: the hovered (or selected) shape's outline.
    pub fn outline(&self) -> Option<Outline> {
        let a = self.get(&self.hovered)?;
        if let Some(seg) = Segment::of(a) {
            return Some(Outline::Polyline(seg.path_points().iter().map(|p| (p.x as f64, p.y as f64)).collect()));
        }
        if let Kind::Freehand(f) = &a.kind {
            return Some(Outline::Polyline(f.points.iter().map(|p| (p.x as f64, p.y as f64)).collect()));
        }
        let tail_visible = matches!(a.kind, Kind::Number(_)) && geometry::step_tail(a).is_some();
        let (l, t, w, h) = if tail_visible {
            let b = geometry::step_interaction_bounds(a, 0.0);
            (b.l as f64, b.t as f64, b.width() as f64, b.height() as f64)
        } else {
            Self::logical_rect(a)
        };
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let rect = (l - 2.0, t - 2.0, w + 4.0, h + 4.0);
        let ellipse = match &a.kind {
            Kind::Ellipse(_) => true,
            Kind::Magnify(m) => m.is_ellipse,
            Kind::Spotlight(s) => s.is_ellipse,
            Kind::Number(_) => !tail_visible,
            _ => false,
        };
        if ellipse {
            return Some(Outline::Ellipse { rect });
        }
        let angle = if Self::rotatable(a) { a.common.rotation_angle as f64 } else { 0.0 };
        Some(Outline::Rectangle { rect, angle, center: (l + w / 2.0, t + h / 2.0) })
    }

    /// The cursor over `p`.
    pub fn cursor(&self) -> Cursor {
        if self.drag.is_some() {
            return Cursor::ClosedHand;
        }
        if self.is_drawing() {
            return Cursor::Crosshair;
        }
        if let Some(p) = self.pointer {
            if self.handle_at(p).is_some() {
                return Cursor::OpenHand;
            }
            if let Some(t) = &self.text
                && t.existing.is_none()
                && text_rect(t).contains_inclusive(p.0, p.1)
            {
                return Cursor::IBeam;
            }
        }
        if self.hovered.is_some() && !matches!(self.tool, EditorTool::Crop | EditorTool::CutOut) {
            return Cursor::Arrow;
        }
        if self.tool == EditorTool::Select { Cursor::Arrow } else { Cursor::Crosshair }
    }

    /// `UpdateHoverState`.
    fn update_hover(&mut self, p: P) {
        if matches!(self.tool, EditorTool::Crop | EditorTool::CutOut | EditorTool::Freehand) {
            self.set_hovered(None);
            return;
        }
        let hit = self.hit_active(p).map(|i| self.annotations[i].common.id.clone());
        self.set_hovered(hit);
    }

    fn set_hovered(&mut self, id: Option<String>) {
        if self.hovered != id {
            self.hovered = id;
            self.changed();
        }
    }

    /// `RefreshHoverFeedback` (Ctrl pressed or released).
    fn refresh_hover(&mut self) {
        if self.drag.is_some() {
            return;
        }
        if self.ignore_selection() {
            self.set_hovered(None);
        } else if let Some(p) = self.pointer {
            self.update_hover(p);
        }
        self.changed();
    }

    // Input.

    /// The modifier keys changed.
    pub fn set_modifiers(&mut self, m: Mods) {
        let ctrl_changed = m.ctrl != self.mods.ctrl;
        self.mods = m;
        if ctrl_changed && !self.is_drawing() {
            self.refresh_hover();
        }
    }

    /// A button went down at `p` (`OnCanvasPointerPressed`).
    pub fn pointer_pressed(&mut self, p: P, button: Button, m: Mods) {
        self.mods = m;
        self.pointer = Some(p);
        if button == Button::Right && self.is_drawing() {
            self.cancel_drawing();
            return;
        }
        if button != Button::Left {
            return;
        }
        let now = Instant::now();
        let double = self.last_press.is_some_and(|(t, at)| {
            now.duration_since(t) <= DOUBLE_CLICK
                && ((at.0 - p.0).powi(2) + (at.1 - p.1).powi(2)).sqrt() <= DOUBLE_CLICK_DISTANCE
        });
        self.last_press = Some((now, p));
        if self.text.is_some() {
            // The text box loses the focus: its text is committed.
            let inside = self.text.as_ref().is_some_and(|t| text_rect(t).contains_inclusive(p.0, p.1));
            if inside {
                return;
            }
            self.commit_text();
        }
        if self.selection_pressed(p, double) {
            return;
        }
        if !matches!(self.tool, EditorTool::Select | EditorTool::Image | EditorTool::Emoji) {
            self.clear_selection();
        } else {
            self.set_hovered(None);
        }
        self.start_drawing(p);
    }

    /// `EditorSelectionController.OnPointerPressed`.
    fn selection_pressed(&mut self, p: P, double: bool) -> bool {
        if self.ignore_selection() {
            return false;
        }
        if let Some(kind) = self.handle_at(p) {
            if kind == HandleKind::SegmentCenter
                && let Some(a) = self.get_mut(&self.selected.clone())
            {
                geometry::ensure_curve_activated(a);
            }
            let rect = self.selected().map(Self::logical_rect).unwrap_or_default();
            self.drag = Some(Drag::Handle { kind, start: p, resize_start_rect: rect, resize_start_point: p });
            self.changed();
            return true;
        }
        let tool = self.tool.clone();
        let hit = match tool {
            EditorTool::Select => self.hit(p),
            EditorTool::Spotlight => self.hit(p).filter(|i| matches!(self.annotations[*i].kind, Kind::Spotlight(_))),
            EditorTool::Freehand => return false,
            _ => self.hit(p).filter(|i| tool_of(&self.annotations[*i]) == tool),
        };
        let Some(i) = hit else {
            if matches!(tool, EditorTool::Select | EditorTool::Spotlight) {
                self.clear_selection();
            }
            return false;
        };
        let id = self.annotations[i].common.id.clone();
        if double && self.edit_text_of(&id) {
            return true;
        }
        self.selected = Some(id);
        self.drag = Some(Drag::Shape { last: p });
        self.changed();
        true
    }

    /// `TryHandleDoubleClickEdit`: text opens its editor.
    fn edit_text_of(&mut self, id: &str) -> bool {
        let Some(i) = self.find(id) else { return false };
        let a = &self.annotations[i];
        let Kind::Text(t) = &a.kind else { return false };
        let len = t.text.chars().count();
        self.text = Some(TextEdit { annotation: a.clone(), existing: Some(id.to_string()), text: t.text.clone(), caret: len });
        self.selected = Some(id.to_string());
        self.changed();
        true
    }

    /// The pointer moved to `p` (`OnCanvasPointerMoved`).
    pub fn pointer_moved(&mut self, p: P, m: Mods) {
        let ctrl_changed = m.ctrl != self.mods.ctrl;
        self.mods = m;
        self.pointer = Some(p);
        if !self.is_drawing() {
            if self.drag.is_some() {
                self.drag_to(p);
                return;
            }
            if self.ignore_selection() {
                self.set_hovered(None);
            } else {
                self.update_hover(p);
            }
            if ctrl_changed {
                self.changed();
            }
            return;
        }
        self.draw_to(p);
    }

    /// A button came up (`OnCanvasPointerReleased`).
    pub fn pointer_released(&mut self, p: P, button: Button, m: Mods) {
        self.mods = m;
        self.pointer = Some(p);
        if button != Button::Left {
            return;
        }
        if self.drag.take().is_some() {
            self.changed();
            return;
        }
        let Some(d) = self.drawing.take() else { return };
        let Some(id) = d.id else { return };
        let Some(i) = self.find(&id) else { return };
        let tool = self.tool.clone();
        let size_based = !matches!(tool, EditorTool::Step | EditorTool::Freehand | EditorTool::Text | EditorTool::Cursor);
        if size_based {
            let (w, h) = ((p.0 - d.start.0).abs(), (p.1 - d.start.1).abs());
            if w < MIN_SHAPE_SIZE && h < MIN_SHAPE_SIZE {
                self.annotations.remove(i);
                self.changed();
                return;
            }
        }
        if !matches!(self.annotations[i].kind, Kind::Freehand(_)) {
            self.set_selected(id.clone());
        }
        self.add_annotation(&id);
    }

    /// Discards the shape being drawn (`CancelActiveDrawing`).
    pub fn cancel_drawing(&mut self) {
        let Some(d) = self.drawing.take() else { return };
        if let Some(i) = d.id.and_then(|id| self.find(&id)) {
            let a = self.annotations.remove(i);
            if matches!(a.kind, Kind::Number(_)) {
                self.number_counter -= 1;
            }
            self.changed();
        }
    }

    /// The annotation a tool starts on a press at `p`, from the options.
    fn new_annotation(&mut self, p: P) -> Option<Annotation> {
        let s = &self.tools;
        let pt = sk(p);
        let shadow = |a: &mut Annotation, s: &ToolState| {
            a.common.shadow_enabled = s.shadow_enabled;
            a.common.shadow_color = s.options.ShadowColorHex.clone();
            a.common.shadow_blur_radius = s.options.ShadowBlurRadius;
            a.common.shadow_opacity = s.options.ShadowOpacity;
            a.common.shadow_offset_x = s.options.ShadowOffsetX;
            a.common.shadow_offset_y = s.options.ShadowOffsetY;
        };
        let name = match self.tool {
            EditorTool::Rectangle => "Rectangle",
            EditorTool::Ellipse => "Ellipse",
            EditorTool::Line => "Line",
            EditorTool::Arrow => "Arrow",
            EditorTool::Freehand => "Freehand",
            EditorTool::Spotlight => "Spotlight",
            EditorTool::Blur => "Blur",
            EditorTool::Pixelate => "Pixelate",
            EditorTool::Magnify => "Magnify",
            EditorTool::Highlight => "Highlight",
            EditorTool::SpeechBalloon => "SpeechBalloon",
            EditorTool::Step => "Number",
            EditorTool::Text => "Text",
            // Image, Clone, Emoji, Cursor and Smart eraser draw nothing yet.
            _ => return None,
        };
        let mut a = Annotation::new(name)?;
        let c = &mut a.common;
        c.start_point = pt;
        c.end_point = pt;
        match &mut a.kind {
            Kind::Rectangle(r) => {
                c.stroke_color = s.selected_color.clone();
                c.stroke_width = s.stroke_width as f32;
                r.border_style = s.border_style.clone();
                c.fill_color = s.fill_color.clone();
                r.corner_radius = s.corner_radius;
            }
            Kind::Ellipse(e) => {
                c.stroke_color = s.selected_color.clone();
                c.stroke_width = s.stroke_width as f32;
                e.border_style = s.border_style.clone();
                c.fill_color = s.fill_color.clone();
            }
            Kind::Line(l) => {
                c.stroke_color = s.selected_color.clone();
                c.stroke_width = s.stroke_width as f32;
                l.border_style = s.border_style.clone();
            }
            Kind::Arrow(r) => {
                c.stroke_color = s.selected_color.clone();
                c.stroke_width = s.stroke_width as f32;
                r.style = s.arrow_style.clone();
                r.rounded_corners = s.arrow_rounded_corners;
            }
            Kind::Freehand(f) => {
                c.stroke_color = s.selected_color.clone();
                c.stroke_width = s.stroke_width as f32;
                f.border_style = s.border_style.clone();
                f.show_arrow_head = s.freehand_arrow;
                f.points = vec![pt];
            }
            Kind::Spotlight(sp) => {
                // EffectStrength (0-100) to DarkenOpacity (0-255).
                sp.darken_opacity = (s.effect_strength / 100.0 * 255.0).clamp(0.0, 255.0) as u8;
                sp.blur_amount = s.spotlight_blur;
                sp.is_ellipse = s.effect_ellipse;
            }
            Kind::Blur(e) | Kind::Pixelate(e) => e.amount = s.effect_strength,
            Kind::Magnify(m) => {
                m.amount = s.effect_strength;
                m.is_ellipse = s.effect_ellipse;
                c.stroke_color = s.selected_color.clone();
                c.stroke_width = s.stroke_width as f32;
                m.border_style = s.border_style.clone();
            }
            Kind::Highlight(_) => c.fill_color = s.fill_color.clone(),
            Kind::SpeechBalloon(b) => {
                let mut fill = s.fill_color.clone();
                if fill.is_empty() || fill == "#00000000" || fill == "Transparent" {
                    fill = if is_color_light(&s.selected_color) { "#FF000000".into() } else { "#FFFFFFFF".into() };
                }
                c.stroke_color = s.selected_color.clone();
                c.stroke_width = s.stroke_width as f32;
                c.fill_color = fill;
                b.text_color = s.text_color.clone();
                b.font_size = s.font_size;
                b.font_family = s.font_family.clone();
                b.is_bold = s.text_bold;
                b.is_italic = s.text_italic;
                b.horizontal_alignment = s.text_alignment.clone();
                b.corner_radius = s.corner_radius;
                b.tail_enabled = s.speech_balloon_tail;
                b.auto_size = s.speech_balloon_auto_size;
            }
            Kind::Number(n) => {
                c.stroke_color = s.selected_color.clone();
                c.stroke_width = s.stroke_width as f32;
                c.fill_color = s.fill_color.clone();
                n.text_color = s.text_color.clone();
                n.font_size = s.font_size;
                n.is_bold = s.text_bold;
                n.step_type = s.step_type.clone();
                n.tail_enabled = s.speech_balloon_tail;
                n.number = self.number_counter;
            }
            Kind::Text(t) => {
                // A transparent text colour falls back to the option's.
                let mut text_color = s.text_color.clone();
                if render::parse_color(&text_color).a == 0 {
                    text_color = super::tool_state::normalize_hex(&s.options.TextTextColorHex);
                }
                c.stroke_color = s.selected_color.clone();
                c.fill_color = s.fill_color.clone();
                t.text_color = text_color;
                c.stroke_width = s.stroke_width as f32;
                t.font_size = s.font_size;
                t.font_family = s.font_family.clone();
                t.horizontal_alignment = s.text_alignment.clone();
                t.is_bold = s.text_bold;
                t.is_italic = s.text_italic;
            }
            _ => {}
        }
        if matches!(
            a.kind,
            Kind::Rectangle(_) | Kind::Ellipse(_) | Kind::Line(_) | Kind::Arrow(_) | Kind::Freehand(_)
                | Kind::SpeechBalloon(_) | Kind::Number(_) | Kind::Text(_)
        ) {
            shadow(&mut a, s);
        }
        if let Kind::Text(t) = &a.kind
            && render::parse_color(&s.text_color).a == 0
        {
            let tc = t.text_color.clone();
            self.tools.text_color = tc;
        }
        Some(a)
    }

    fn start_drawing(&mut self, p: P) {
        let mut d = Drawing { id: None, start: p, last: p, moving: false, ctrl_was_held: self.mods.ctrl, freehand_straight: false };
        if let Some(mut a) = self.new_annotation(p) {
            a.common.id = self.new_id();
            if let Kind::Text(_) = a.kind {
                // `HandleTextTool`: a text box at the point, typed into.
                self.text = Some(TextEdit { annotation: a, existing: None, text: String::new(), caret: 0 });
                self.drawing = None;
                self.changed();
                return;
            }
            if let Kind::Number(_) = a.kind {
                self.number_counter += 1;
            }
            d.id = Some(a.common.id.clone());
            if matches!(a.kind, Kind::Highlight(_)) {
                // Below the first non-effect shape.
                let at = self
                    .annotations
                    .iter()
                    .position(|x| {
                        !matches!(
                            x.kind,
                            Kind::Highlight(_) | Kind::Blur(_) | Kind::Pixelate(_) | Kind::Magnify(_)
                        )
                    })
                    .unwrap_or(self.annotations.len());
                self.annotations.insert(at, a);
            } else {
                self.annotations.push(a);
            }
            self.changed();
        }
        self.drawing = Some(d);
    }

    fn can_move_during_creation(a: &Annotation) -> bool {
        !matches!(a.kind, Kind::Number(_) | Kind::Text(_) | Kind::Cursor(_) | Kind::Image(_) | Kind::Emoji(_))
    }

    fn draw_to(&mut self, p: P) {
        let Some(mut d) = self.drawing.take() else { return };
        let Some(id) = d.id.clone() else {
            self.drawing = Some(d);
            return;
        };
        let Some(i) = self.find(&id) else { return };
        let m = self.mods;
        if !m.ctrl {
            d.moving = false;
        } else if Self::can_move_during_creation(&self.annotations[i]) && (d.moving || !d.ctrl_was_held) {
            // `MoveShapeDuringCreation`.
            let (dx, dy) = (p.0 - d.last.0, p.1 - d.last.1);
            if dx != 0.0 || dy != 0.0 {
                d.start = (d.start.0 + dx, d.start.1 + dy);
                let a = &mut self.annotations[i];
                if let Kind::Freehand(f) = &mut a.kind {
                    f.points.iter_mut().for_each(|q| *q = Point::new(q.x + dx as f32, q.y + dy as f32));
                } else {
                    let (s, e) = (a.common.start_point, a.common.end_point);
                    let rect_like = !matches!(a.kind, Kind::Line(_) | Kind::Arrow(_) | Kind::SpeechBalloon(_) | Kind::Spotlight(_));
                    if rect_like {
                        // Canvas.Left/Top + delta, in doubles.
                        let (l, t) = (s.x.min(e.x) as f64 + dx, s.y.min(e.y) as f64 + dy);
                        let (w, h) = ((e.x - s.x).abs() as f64, (e.y - s.y).abs() as f64);
                        a.common.start_point = sk((l, t));
                        a.common.end_point = sk((l + w, t + h));
                    } else {
                        a.common.start_point = Point::new(s.x + dx as f32, s.y + dy as f32);
                        a.common.end_point = Point::new(e.x + dx as f32, e.y + dy as f32);
                    }
                }
                self.changed();
            }
            d.moving = true;
            d.last = p;
            d.ctrl_was_held = true;
            self.drawing = Some(d);
            return;
        }
        let a = &mut self.annotations[i];
        if let Kind::Freehand(f) = &mut a.kind {
            add_freehand_point(&mut f.points, sk(p), m.shift, &mut d.freehand_straight);
            d.last = p;
            d.ctrl_was_held = m.ctrl;
            self.drawing = Some(d);
            self.changed();
            return;
        }
        let start = d.start;
        let (mut left, mut top) = (start.0.min(p.0), start.1.min(p.1));
        let (mut w, mut h) = ((p.0 - start.0).abs(), (p.1 - start.1).abs());
        if m.shift {
            w = w.max(h);
            h = w;
            if p.0 < start.0 {
                left = start.0 - w;
            }
            if p.1 < start.1 {
                top = start.1 - h;
            }
        }
        match &mut a.kind {
            Kind::Line(_) | Kind::Arrow(_) => {
                let end = if m.shift { snap_45(start, p) } else { p };
                a.common.end_point = sk(end);
            }
            Kind::Number(_) => a.common.start_point = sk(p),
            _ => {
                a.common.start_point = sk((left, top));
                a.common.end_point = sk((left + w, top + h));
            }
        }
        d.last = p;
        d.ctrl_was_held = m.ctrl;
        self.drawing = Some(d);
        self.changed();
    }

    /// `HandleResize` / `HandleMove` while a handle or shape is dragged.
    fn drag_to(&mut self, p: P) {
        let shift = self.mods.shift;
        let sel = self.selected.clone();
        let Some(mut drag) = self.drag.take() else { return };
        match &mut drag {
            Drag::Shape { last } => {
                let (dx, dy) = (p.0 - last.0, p.1 - last.1);
                *last = p;
                if let Some(a) = self.get_mut(&sel) {
                    move_shape(a, dx, dy);
                }
            }
            Drag::Handle { kind, start, resize_start_rect, resize_start_point } => {
                let (dx, dy) = (p.0 - start.0, p.1 - start.1);
                *start = p;
                if let Some(a) = self.get_mut(&sel)
                    && let Some(r) = resize(a, *kind, p, (dx, dy), shift, *resize_start_rect, *resize_start_point)
                {
                    *resize_start_rect = r;
                    *resize_start_point = p;
                }
            }
        }
        self.drag = Some(drag);
        self.changed();
    }

    // Keys.

    /// A key went down (`EditorView.OnKeyDown`); `hotkey` maps a key to the
    /// toolbar's tool for it (`TrySelectToolForToolbarHotkey`).
    pub fn key_down(&mut self, key: Key, m: Mods, hotkey: &dyn Fn(Key, Mods) -> Option<EditorTool>) -> KeyResult {
        self.set_modifiers(m);
        if self.text.is_some() {
            return self.text_key(key, m);
        }
        let handled = KeyResult { handled: true, tool: None };
        let pick = |me: &mut Self, k| -> Option<KeyResult> {
            let tool = hotkey(k, m)?;
            me.select_tool(tool.clone());
            Some(KeyResult { handled: true, tool: Some(tool) })
        };
        if key == Key::Delete {
            if m.shift {
                if self.annotations.is_empty() {
                    return KeyResult::default();
                }
                self.clear_annotations();
            } else {
                self.delete_selected();
            }
            return handled;
        }
        if m.ctrl && m.shift {
            if let Some(r) = pick(self, key) {
                return r;
            }
            if key == Key::Letter('Z') {
                self.redo();
                return handled;
            }
            return KeyResult::default();
        }
        if m.ctrl {
            if let Some(r) = pick(self, key) {
                return r;
            }
            match key {
                Key::Letter('Z') => self.undo(),
                Key::Letter('Y') => self.redo(),
                _ => return KeyResult::default(),
            }
            return handled;
        }
        if m.alt {
            return KeyResult::default();
        }
        if matches!(key, Key::Left | Key::Right | Key::Up | Key::Down) && self.selected.is_some() {
            let step = if m.shift { 10.0 } else { 1.0 };
            let dx = match key {
                Key::Left => -step,
                Key::Right => step,
                _ => 0.0,
            };
            let dy = match key {
                Key::Up => -step,
                Key::Down => step,
                _ => 0.0,
            };
            let sel = self.selected.clone();
            if let Some(a) = self.get_mut(&sel) {
                move_shape(a, dx, dy);
            }
            self.changed();
            return handled;
        }
        if let Some(r) = pick(self, key) {
            return r;
        }
        if m.shift {
            return KeyResult::default();
        }
        match key {
            Key::Home => self.move_selected_to(|_, n| Some(n.saturating_sub(1))),
            Key::End => self.move_selected_to(|_, _| Some(0)),
            Key::PageUp => self.move_selected_to(|i, _| Some(i + 1)),
            Key::PageDown => self.move_selected_to(|i, _| i.checked_sub(1)),
            _ => return KeyResult::default(),
        }
        handled
    }

    /// A key came up (`OnKeyUp`): Ctrl refreshes the hover.
    pub fn key_up(&mut self, key: Key, m: Mods) {
        if key == Key::Control {
            self.mods = m;
            if !self.is_drawing() {
                self.refresh_hover();
            }
            if let Some(d) = self.drawing.as_mut() {
                d.moving = false;
                d.ctrl_was_held = false;
            }
        } else {
            self.set_modifiers(m);
        }
    }

    /// Typed text (while a text box is open).
    pub fn text_input(&mut self, s: &str) {
        let Some(t) = self.text.as_mut() else { return };
        let mut chars: Vec<char> = t.text.chars().collect();
        for c in s.chars().filter(|c| !c.is_control()) {
            chars.insert(t.caret, c);
            t.caret += 1;
        }
        t.text = chars.into_iter().collect();
        self.changed();
    }

    /// `AnnotationTextEditorHelper`: Enter and Escape finish, Shift+Enter
    /// and Ctrl+Enter add a line.
    fn text_key(&mut self, key: Key, m: Mods) -> KeyResult {
        let handled = KeyResult { handled: true, tool: None };
        let Some(t) = self.text.as_mut() else { return KeyResult::default() };
        let mut chars: Vec<char> = t.text.chars().collect();
        match key {
            Key::Enter if m.shift || m.ctrl => {
                chars.insert(t.caret, '\n');
                t.caret += 1;
            }
            Key::Enter | Key::Escape => {
                self.commit_text();
                return handled;
            }
            Key::Backspace if t.caret > 0 => {
                t.caret -= 1;
                chars.remove(t.caret);
            }
            Key::Delete if t.caret < chars.len() => {
                chars.remove(t.caret);
            }
            Key::Left => t.caret = t.caret.saturating_sub(1),
            Key::Right => t.caret = (t.caret + 1).min(chars.len()),
            Key::Home => t.caret = 0,
            Key::End => t.caret = chars.len(),
            _ => return handled,
        }
        t.text = chars.into_iter().collect();
        self.changed();
        handled
    }

    /// The text box lost the focus: non-blank text becomes (or updates)
    /// its annotation, sized to the text (`SetCreatedText`), and is
    /// selected.
    pub fn commit_text(&mut self) {
        let Some(t) = self.text.take() else { return };
        self.changed();
        if let Some(id) = t.existing {
            let Some(i) = self.find(&id) else { return };
            if let Kind::Text(p) = &mut self.annotations[i].kind {
                p.text = t.text;
            }
            return;
        }
        if t.text.trim().is_empty() {
            return;
        }
        let mut a = t.annotation;
        if let Kind::Text(p) = &mut a.kind {
            p.text = t.text;
        }
        fit_text(&mut a);
        let id = a.common.id.clone();
        self.annotations.push(a);
        self.add_annotation(&id);
        self.set_selected(id);
    }
}

/// The text box being typed in (`CreateCreationTextEditor`'s `TextBox`):
/// its top-left at the click, the text in the annotation's font with the
/// box's 4px padding and border around it.
#[derive(Clone, Debug, PartialEq)]
pub struct TextBox {
    /// Left, top, width, height in image pixels.
    pub rect: (f64, f64, f64, f64),
    /// Where the first line's text starts.
    pub text_origin: P,
    pub line_height: f64,
    pub font_family: String,
    pub font_size: f64,
    pub bold: bool,
    pub italic: bool,
    pub color: String,
    /// The caret's x (image pixels) and line.
    pub caret: (f64, usize),
}

/// The `TextBox`'s padding (4) and border (1) on each side.
const TEXT_BOX_INSET: f64 = 5.0;

pub fn text_box(t: &TextEdit) -> Option<TextBox> {
    let Kind::Text(p) = &t.annotation.kind else { return None };
    let font = render::text::load_face(&p.font_family, p.is_bold, p.is_italic)?;
    let measure = |s: &str| render::text::layout(&font, s, p.font_size as f64, render::text::Align::Left, f64::INFINITY);
    let lay = measure(if t.text.is_empty() { "" } else { &t.text })?;
    let line_height = measure(" ")?.height().ceil();
    let lines = t.text.split('\n').count().max(1) as f64;
    let width = lay.lines.iter().map(|l| l.width_including_trailing_whitespace).fold(0.0, f64::max).ceil();
    let s = t.annotation.common.start_point;
    let (x, y) = (s.x as f64, s.y as f64);
    let rect = (x, y, width + 2.0 * TEXT_BOX_INSET, line_height * lines + 2.0 * TEXT_BOX_INSET);
    // The caret: after the text before it on its line.
    let before: String = t.text.chars().take(t.caret).collect();
    let line = before.matches('\n').count();
    let tail = before.rsplit('\n').next().unwrap_or("");
    let cx = measure(tail).map(|l| l.lines.first().map(|l| l.width_including_trailing_whitespace).unwrap_or(0.0)).unwrap_or(0.0);
    Some(TextBox {
        rect,
        text_origin: (x + TEXT_BOX_INSET, y + TEXT_BOX_INSET),
        line_height,
        font_family: p.font_family.clone(),
        font_size: p.font_size as f64,
        bold: p.is_bold,
        italic: p.is_italic,
        color: p.text_color.clone(),
        caret: (x + TEXT_BOX_INSET + cx, line),
    })
}

/// The text box's area while typing.
pub fn text_rect(t: &TextEdit) -> Rect {
    match text_box(t) {
        Some(b) => Rect::new(b.rect.0 as f32, b.rect.1 as f32, (b.rect.0 + b.rect.2) as f32, (b.rect.1 + b.rect.3) as f32),
        None => {
            let s = t.annotation.common.start_point;
            Rect::new(s.x, s.y, s.x + 10.0, s.y + 10.0)
        }
    }
}

/// `OutlinedTextControl.MeasureNaturalSize` (padding 8 and the stroke on
/// each side), laid out rounded up: the text annotation's end point.
fn fit_text(a: &mut Annotation) {
    let Kind::Text(p) = &a.kind else { return };
    if p.text.is_empty() {
        a.common.end_point = a.common.start_point;
        return;
    }
    let Some(font) = render::text::load_face(&p.font_family, p.is_bold, p.is_italic) else { return };
    let Some(lay) = render::text::layout(&font, &p.text, p.font_size as f64, render::text::Align::Left, f64::INFINITY)
    else {
        return;
    };
    let stroke = a.common.stroke_width as f64;
    let w = (lay.width() + 16.0 + stroke * 2.0).ceil();
    let h = (lay.height() + 16.0 + stroke * 2.0).ceil();
    let s = a.common.start_point;
    a.common.end_point = Point::new((s.x as f64 + w) as f32, (s.y as f64 + h) as f32);
}

/// `AddFreehandPoint`: Shift draws one straight segment from the last point.
fn add_freehand_point(points: &mut Vec<Point>, p: Point, straight: bool, was_straight: &mut bool) {
    if straight {
        if *was_straight {
            if let Some(last) = points.last_mut() {
                *last = p;
            }
        } else {
            let last = *points.last().unwrap_or(&p);
            points.push(last);
            points.push(p);
            *was_straight = true;
        }
    } else {
        if *was_straight {
            let last = *points.last().unwrap_or(&p);
            points.push(last);
            *was_straight = false;
        }
        points.push(p);
    }
}

/// `HandleMove`: moved by `(dx, dy)`.
fn move_shape(a: &mut Annotation, dx: f64, dy: f64) {
    match a.kind {
        Kind::Line(_) | Kind::Arrow(_) | Kind::SpeechBalloon(_) | Kind::Number(_) | Kind::Freehand(_) | Kind::Spotlight(_) => {
            geometry::move_by(a, dx as f32, dy as f32)
        }
        _ => {
            // Canvas.Left/Top move, and the annotation's points by the
            // same (float) amounts.
            geometry::move_by(a, dx as f32, dy as f32)
        }
    }
}

/// `HandleResize` for one move; returns the new proportional-resize start
/// when the regular path ran.
#[allow(clippy::too_many_arguments)]
fn resize(
    a: &mut Annotation,
    kind: HandleKind,
    p: P,
    (dx, dy): P,
    shift: bool,
    resize_start_rect: (f64, f64, f64, f64),
    resize_start_point: P,
) -> Option<(f64, f64, f64, f64)> {
    if let Some(seg) = Segment::of(a) {
        let (s, e) = ((seg.start.x as f64, seg.start.y as f64), (seg.end.x as f64, seg.end.y as f64));
        match kind {
            HandleKind::SegmentStart => {
                let ns = if shift { snap_45(e, p) } else { p };
                geometry::set_segment(a, sk(ns), sk(e), None);
            }
            HandleKind::SegmentEnd => {
                let ne = if shift { snap_45(s, p) } else { p };
                geometry::set_segment(a, sk(s), sk(ne), None);
            }
            HandleKind::SegmentCenter => geometry::set_segment(a, sk(s), sk(e), Some(sk(p))),
            _ => {}
        }
        return None;
    }
    if let Kind::Number(n) = &mut a.kind {
        if kind == HandleKind::StepTail && n.tail_enabled {
            n.tail_point = sk(p);
            n.tail_point_initialized = true;
        }
        return None;
    }
    let rotatable = Workspace::rotatable(a);
    if kind == HandleKind::Rotate && rotatable {
        let (l, t, w, h) = Workspace::logical_rect(a);
        let (cx, cy) = (l + w / 2.0, t + h / 2.0);
        let mut deg = (p.0 - cx).atan2(-(p.1 - cy)) * 180.0 / std::f64::consts::PI;
        if shift {
            deg = (deg / 45.0).round() * 45.0;
        }
        a.common.rotation_angle = deg as f32;
        return None;
    }
    let dir = kind.direction()?;
    if rotatable && a.common.rotation_angle != 0.0 && !matches!(a.kind, Kind::Emoji(_)) {
        resize_rotated(a, dir, p);
        return None;
    }
    let (l, t, w, h) = Workspace::logical_rect(a);
    if let Kind::SpeechBalloon(_) = a.kind {
        let r = resize_rect((l, t, w, h), dir, (dx, dy), 20.0);
        set_rect(a, r);
        return None;
    }
    if let Kind::Spotlight(_) = a.kind {
        let r = resize_rect((l, t, w, h), dir, (dx, dy), 10.0);
        set_rect(a, r);
        return None;
    }
    if matches!(a.kind, Kind::Freehand(_) | Kind::Cursor(_)) {
        return None;
    }
    if shift && let Some(r) = proportional(dir, p, resize_start_rect, resize_start_point) {
        set_rect(a, r);
        return None;
    }
    let r = resize_rect((l, t, w, h), dir, (dx, dy), 1.0);
    set_rect(a, r);
    Some(r)
}

fn set_rect(a: &mut Annotation, (l, t, w, h): (f64, f64, f64, f64)) {
    a.common.start_point = sk((l, t));
    a.common.end_point = sk((l + w, t + h));
}

/// `SelectionResizeNode.Resize`.
fn resize_rect((mut l, mut t, mut w, mut h): (f64, f64, f64, f64), (hd, vd): (i32, i32), (dx, dy): P, min: f64) -> (f64, f64, f64, f64) {
    if hd > 0 {
        w = (w + dx).max(min);
    } else if hd < 0 {
        let c = (w - min).min(dx);
        l += c;
        w -= c;
    }
    if vd > 0 {
        h = (h + dy).max(min);
    } else if vd < 0 {
        let c = (h - min).min(dy);
        t += c;
        h -= c;
    }
    (l, t, w, h)
}

/// `TryGetProportionalResizeRect`: corners keep the proportions the shape
/// had when Shift resizing started, the opposite corner fixed.
fn proportional((hd, vd): (i32, i32), p: P, start: (f64, f64, f64, f64), start_point: P) -> Option<(f64, f64, f64, f64)> {
    if hd == 0 || vd == 0 {
        return None;
    }
    let (sl, st, sw, sh) = start;
    if sw <= 0.0 || sh <= 0.0 {
        return None;
    }
    let (sr, sb) = (sl + sw, st + sh);
    let ax = if hd > 0 { sl } else { sr };
    let ay = if vd > 0 { st } else { sb };
    let cx = (if hd > 0 { sr } else { sl }) + p.0 - start_point.0;
    let cy = (if vd > 0 { sb } else { st }) + p.1 - start_point.1;
    let w = ((cx - ax) * hd as f64).max(1.0);
    let h = ((cy - ay) * vd as f64).max(1.0);
    let ratio = (w / sw).min(h / sh).max((1.0 / sw).max(1.0 / sh));
    let (w, h) = (sw * ratio, sh * ratio);
    Some((if hd > 0 { ax } else { ax - w }, if vd > 0 { ay } else { ay - h }, w, h))
}

/// `TryResizeRotatedAnnotation`: resized along its own (rotated) axes.
fn resize_rotated(a: &mut Annotation, (hd, vd): (i32, i32), p: P) {
    let (l, t, w, h) = Workspace::logical_rect(a);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let min = if matches!(a.kind, Kind::Text(_)) { 10.0 } else { 1.0 };
    let angle = a.common.rotation_angle as f64;
    let center = (l + w / 2.0, t + h / 2.0);
    let local_center = rotate_point(center, (0.0, 0.0), -angle);
    let local_p = rotate_point(p, (0.0, 0.0), -angle);
    let ax = local_center.0 - hd as f64 * w / 2.0;
    let ay = local_center.1 - vd as f64 * h / 2.0;
    let (mut ncx, mut ncy, mut nw, mut nh) = (local_center.0, local_center.1, w, h);
    if hd != 0 {
        let cx = if hd < 0 { local_p.0.min(ax - min) } else { local_p.0.max(ax + min) };
        nw = (ax - cx).abs();
        ncx = (ax + cx) / 2.0;
    }
    if vd != 0 {
        let cy = if vd < 0 { local_p.1.min(ay - min) } else { local_p.1.max(ay + min) };
        nh = (ay - cy).abs();
        ncy = (ay + cy) / 2.0;
    }
    let c = rotate_point((ncx, ncy), (0.0, 0.0), angle);
    set_rect(a, (c.0 - nw / 2.0, c.1 - nh / 2.0, nw, nh));
}

/// `IsRememberableAnnotationTool`.
fn is_rememberable(t: &EditorTool) -> bool {
    use EditorTool as T;
    matches!(
        t,
        T::Rectangle
            | T::Ellipse
            | T::Line
            | T::Arrow
            | T::Freehand
            | T::Text
            | T::SpeechBalloon
            | T::Step
            | T::Cursor
            | T::Highlight
            | T::SmartEraser
            | T::Blur
            | T::Pixelate
            | T::Magnify
            | T::Spotlight
    )
}

/// `GetInitialAnnotationTool`: the last used annotation tool.
fn initial_tool(o: &ImageEditorOptions) -> EditorTool {
    if is_rememberable(&o.LastUsedAnnotationTool) { o.LastUsedAnnotationTool.clone() } else { EditorTool::Rectangle }
}

#[cfg(test)]
mod tests;
