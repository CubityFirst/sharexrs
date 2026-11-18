//! The editor's commands beyond drawing: the annotation clipboard
//! (`EditorView.ClipboardHandler`), z-order (`EditorCore.BringToFront`...),
//! flatten, crop and cut out (`EditorCore.Crop`/`CutOut`, the input
//! controller's crop, cut out and clone regions, `CropAnnotation`,
//! `CropAdorner`), inserting images (`EditorView.ImageInsert`), the smart
//! eraser's fill (`SmartEraserAnnotation.ConfigureFill`) and the right
//! button (`EditorInputController.OnCanvasPointerPressed`).

use std::sync::Mutex;

use super::*;
use crate::model::EmojiProps;

/// `EditorView._clipboardAnnotation`: shared by every editor (a static).
static CLIPBOARD: Mutex<Option<Annotation>> = Mutex::new(None);

/// `EditorView.ClearAnnotationClipboard`.
pub fn clear_annotation_clipboard() {
    *CLIPBOARD.lock().unwrap() = None;
}

/// A region tool being dragged: crop, cut out or clone.
pub(super) struct RegionDraw {
    tool: EditorTool,
    start: P,
    /// Left, top, width, height.
    rect: (f64, f64, f64, f64),
    visible: bool,
    /// `_cutOutDirection`: vertical (a column cut) or horizontal.
    cut_vertical: Option<bool>,
}

/// A crop drawn and waiting for confirmation (`_cropActive`; only without
/// `QuickCrop`).
pub(super) struct PendingCrop {
    rect: (f64, f64, f64, f64),
    /// The handle dragged, the crop and the point where it started.
    drag: Option<CropDrag>,
}

type CropDrag = (CropHandle, (f64, f64, f64, f64), P);

/// The crop's handles (`Crop_TopLeft`...) and the move drag (`Crop_Move`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropHandle {
    TopLeft,
    TopCenter,
    TopRight,
    RightCenter,
    BottomRight,
    BottomCenter,
    BottomLeft,
    LeftCenter,
    Move,
}

impl CropHandle {
    pub const NODES: [CropHandle; 8] = [
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::RightCenter,
        Self::BottomRight,
        Self::BottomCenter,
        Self::BottomLeft,
        Self::LeftCenter,
    ];

    pub fn is_corner(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight | Self::BottomRight | Self::BottomLeft)
    }

    /// Along the top or bottom edge.
    pub fn is_horizontal_edge(self) -> bool {
        matches!(self, Self::TopCenter | Self::BottomCenter)
    }

    /// `CreateResizeHandle`'s hit area: 40x40 corners, 40x28 edges.
    pub fn hit_size(self) -> (f64, f64) {
        if self.is_corner() {
            (CROP_CORNER, CROP_CORNER)
        } else if self.is_horizontal_edge() {
            (CROP_EDGE_LENGTH, CROP_EDGE_THICKNESS)
        } else {
            (CROP_EDGE_THICKNESS, CROP_EDGE_LENGTH)
        }
    }

    fn position(self, (l, t, w, h): (f64, f64, f64, f64)) -> P {
        let (cx, cy) = (l + w / 2.0, t + h / 2.0);
        match self {
            Self::TopLeft => (l, t),
            Self::TopCenter => (cx, t),
            Self::TopRight => (l + w, t),
            Self::RightCenter => (l + w, cy),
            Self::BottomRight => (l + w, t + h),
            Self::BottomCenter => (cx, t + h),
            Self::BottomLeft => (l, t + h),
            Self::LeftCenter => (l, cy),
            Self::Move => (cx, cy),
        }
    }
}

/// `CropAnnotation`'s handle sizes.
pub const CROP_CORNER: f64 = 40.0;
pub const CROP_EDGE_LENGTH: f64 = 40.0;
pub const CROP_EDGE_THICKNESS: f64 = 28.0;
pub const CROP_CORNER_ARM: f64 = 24.0;
pub const CROP_CENTER_BAR: f64 = 30.0;
pub const CROP_HANDLE_THICKNESS: f64 = 5.0;
const MIN_CROP_SIZE: f64 = 16.0;
/// `CropAdorner.MinCropGuideSize`.
const MIN_CROP_GUIDE_SIZE: f64 = 24.0;
/// The cut out's direction is decided past 15px (`directionThreshold`).
const CUT_OUT_THRESHOLD: f64 = 15.0;
/// `InsertEmojiAnnotation`'s default size.
pub const EMOJI_SIZE: f64 = 160.0;

/// What the crop tool shows (`CropOverlay` and `CropAdorner`), in image
/// pixels, for the host to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct CropChrome {
    /// The crop rectangle (`CropOverlay`: a 2px white `4,2` dashed stroke).
    pub rect: (f64, f64, f64, f64),
    /// The four shades around it (`#8C000000`), left, top, width, height.
    pub shades: Vec<(f64, f64, f64, f64)>,
    /// The thirds (`#D2FFFFFF` 1px `3,3` dashes), when it's 24px or more.
    pub guides: Vec<(P, P)>,
    /// `X: {x} Y: {y} W: {w} H: {h}`, when it has a size.
    pub info: Option<String>,
    /// The handles, once drawn without quick crop.
    pub handles: Vec<(CropHandle, P)>,
    /// The confirmation button is shown (the handles are).
    pub pending: bool,
}

/// A cut out or clone region being dragged.
#[derive(Clone, Debug, PartialEq)]
pub enum RegionPreview {
    /// `CutOutOverlay`: `#96000000`, a 1px white `5,3` dashed stroke.
    CutOut { rect: (f64, f64, f64, f64) },
    /// `CloneOverlay`: `#280078D7`, a 1px DodgerBlue `5,3` dashed stroke.
    Clone { rect: (f64, f64, f64, f64) },
}

fn clamp_safe(v: f64, min: f64, max: f64) -> f64 {
    if max < min { min } else { v.clamp(min, max) }
}

/// `CropAnnotation.ResizeBounds`.
fn resize_crop(handle: CropHandle, start: P, cur: P, r: (f64, f64, f64, f64), cw: f64, ch: f64) -> (f64, f64, f64, f64) {
    let (mut left, mut top, mut right, mut bottom) = (r.0, r.1, r.0 + r.2, r.1 + r.3);
    let cx = clamp_safe(cur.0, 0.0, cw);
    let cy = clamp_safe(cur.1, 0.0, ch);
    match handle {
        CropHandle::TopLeft => {
            left = clamp_safe(cx, 0.0, right - MIN_CROP_SIZE);
            top = clamp_safe(cy, 0.0, bottom - MIN_CROP_SIZE);
        }
        CropHandle::TopCenter => top = clamp_safe(cy, 0.0, bottom - MIN_CROP_SIZE),
        CropHandle::TopRight => {
            right = clamp_safe(cx, left + MIN_CROP_SIZE, cw);
            top = clamp_safe(cy, 0.0, bottom - MIN_CROP_SIZE);
        }
        CropHandle::RightCenter => right = clamp_safe(cx, left + MIN_CROP_SIZE, cw),
        CropHandle::BottomRight => {
            right = clamp_safe(cx, left + MIN_CROP_SIZE, cw);
            bottom = clamp_safe(cy, top + MIN_CROP_SIZE, ch);
        }
        CropHandle::BottomCenter => bottom = clamp_safe(cy, top + MIN_CROP_SIZE, ch),
        CropHandle::BottomLeft => {
            left = clamp_safe(cx, 0.0, right - MIN_CROP_SIZE);
            bottom = clamp_safe(cy, top + MIN_CROP_SIZE, ch);
        }
        CropHandle::LeftCenter => left = clamp_safe(cx, 0.0, right - MIN_CROP_SIZE),
        CropHandle::Move => {
            let (dx, dy) = (cur.0 - start.0, cur.1 - start.1);
            let max_left = (cw - r.2).max(0.0);
            let max_top = (ch - r.3).max(0.0);
            return (clamp_safe(r.0 + dx, 0.0, max_left), clamp_safe(r.1 + dy, 0.0, max_top), r.2, r.3);
        }
    }
    left = clamp_safe(left, 0.0, cw - MIN_CROP_SIZE);
    top = clamp_safe(top, 0.0, ch - MIN_CROP_SIZE);
    right = clamp_safe(right, left + MIN_CROP_SIZE, cw);
    bottom = clamp_safe(bottom, top + MIN_CROP_SIZE, ch);
    (left, top, (right - left).max(MIN_CROP_SIZE), (bottom - top).max(MIN_CROP_SIZE))
}

/// .NET's `Math.Round` (to even).
fn round(v: f64) -> i32 {
    v.round_ties_even() as i32
}

/// Unpremultiplied ARGB of a premultiplied pixel (`SKBitmap.GetPixel`).
fn unpremul(c: u32) -> u32 {
    let a = c >> 24;
    if a == 0 || a == 255 {
        return if a == 0 { 0 } else { c };
    }
    let f = |v: u32| ((v * 255 + a / 2) / a).min(255);
    (a << 24) | (f((c >> 16) & 0xFF) << 16) | (f((c >> 8) & 0xFF) << 8) | f(c & 0xFF)
}

impl Workspace {
    /// The pixels of an image annotation (inserted image, clone).
    pub fn image(&self, id: &str) -> Option<&Pixmap> {
        let i = self.find(id)?;
        match &self.core.annotations[i].kind {
            Kind::Image(p) => p.bitmap.as_deref(),
            _ => None,
        }
    }

    /// What the host has been asked to do since the last call.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    // Clipboard.

    /// `CanPaste` for the internal clipboard.
    pub fn has_clipboard_annotation() -> bool {
        CLIPBOARD.lock().unwrap().is_some()
    }

    fn selected_clone(&self) -> Option<Annotation> {
        self.selected().cloned()
    }

    /// `CopyAnnotationCommand` (Ctrl+Shift+C): the selected annotation to the
    /// annotation clipboard. The host clears the system clipboard, as ShareX
    /// does, so a paste takes this one.
    pub fn copy_annotation(&mut self) -> bool {
        let Some(c) = self.selected_clone() else { return false };
        *CLIPBOARD.lock().unwrap() = Some(c);
        true
    }

    /// `CutAnnotationCommand` (Ctrl+X): copied, then deleted (undoable).
    pub fn cut_annotation(&mut self) -> bool {
        if !self.copy_annotation() {
            return false;
        }
        self.delete_selected();
        true
    }

    /// `PasteInternalShape`: the copied annotation again, 20px further, and
    /// selected.
    pub fn paste_annotation(&mut self) -> bool {
        let Some(a) = CLIPBOARD.lock().unwrap().clone() else { return false };
        self.add_clone(a);
        true
    }

    /// `DuplicateSelectedAnnotation` (Ctrl+D).
    pub fn duplicate_selected(&mut self) -> bool {
        let Some(a) = self.selected_clone() else { return false };
        self.add_clone(a);
        true
    }

    /// A clone offset by 20px (`OffsetClonedAnnotation`), added and
    /// selected. It keeps the original's `Id` (`Annotation.Clone`) and gets
    /// its own key.
    fn add_clone(&mut self, mut a: Annotation) {
        self.commit_text();
        const OFFSET: f32 = 20.0;
        let shift = |p: Point| Point::new(p.x + OFFSET, p.y + OFFSET);
        a.common.start_point = shift(a.common.start_point);
        a.common.end_point = shift(a.common.end_point);
        match &mut a.kind {
            Kind::Freehand(f) => f.points.iter_mut().for_each(|q| *q = shift(*q)),
            Kind::SpeechBalloon(b) if b.tail_point_initialized || b.tail_point != Point::default() => {
                b.tail_point = shift(b.tail_point);
                b.tail_point_initialized = true;
            }
            _ => {}
        }
        a.common.key = crate::model::Key::fresh();
        let id = a.common.key.0.clone();
        self.annotations_memento(false);
        self.core.annotations.push(a);
        self.history_changed();
        self.set_selected(id);
    }

    // Z-order.

    /// `BringToFront` (Home).
    pub fn bring_to_front(&mut self) {
        self.move_selected_to(|_, n| Some(n.saturating_sub(1)));
    }

    /// `SendToBack` (End).
    pub fn send_to_back(&mut self) {
        self.move_selected_to(|_, _| Some(0));
    }

    /// `BringForward` (Page Up).
    pub fn bring_forward(&mut self) {
        self.move_selected_to(|i, _| Some(i + 1));
    }

    /// `SendBackward` (Page Down).
    pub fn send_backward(&mut self) {
        self.move_selected_to(|i, _| i.checked_sub(1));
    }

    // Image operations.

    /// `FlattenImage` (Ctrl+Shift+F): the annotations drawn into the image
    /// (`EditorCore.FlattenImage`).
    pub fn flatten(&mut self) -> bool {
        self.commit_text();
        if self.core.annotations.is_empty() {
            return false;
        }
        self.clear_selection();
        let composite = self.composite().clone();
        if !self.core.flatten_image(composite) {
            return false;
        }
        self.image_changed();
        true
    }

    /// `EditorCore.Crop`: the image cut to the rectangle, annotations
    /// outside it dropped and the rest moved with it.
    pub fn crop_image(&mut self, x: f64, y: f64, w: f64, h: f64) -> bool {
        let r = geometry::Rect::new(x as f32, y as f32, (x + w) as f32, (y + h) as f32);
        if !self.core.crop(r) {
            return false;
        }
        self.image_changed();
        true
    }

    /// `EditorCore.CutOut`: the strip between `start` and `end` removed
    /// (columns when `vertical`), annotations in it dropped and the rest
    /// moved up or left.
    pub fn cut_out_image(&mut self, start: i32, end: i32, vertical: bool) -> bool {
        if !self.core.cut_out(start, end, vertical) {
            return false;
        }
        self.image_changed();
        true
    }

    /// The image changed through `EditorCore` (with its canvas memento):
    /// the effect caches go, the selection if its annotation went.
    /// The editor's `EditorCore` as it is (the image effects' preview
    /// starts from it: `EffectPreview::start`).
    pub fn core(&self) -> &EditorCore {
        &self.core
    }

    /// Commits an image effect or editor operation of the effect browser
    /// through `EditorCore` (`effects::apply_to_core`,
    /// `EffectPreview::apply`); the view follows when the image changed.
    pub fn apply_effect<E>(&mut self, f: impl FnOnce(&mut EditorCore) -> Result<bool, E>) -> Result<bool, E> {
        self.commit_text();
        let r = f(&mut self.core);
        if matches!(r, Ok(true)) {
            self.image_changed();
        }
        r
    }

    pub(super) fn image_changed(&mut self) {
        self.cache = render::Cache::default();
        if self.core.selected.as_ref().is_some_and(|id| self.find(id).is_none()) {
            self.core.selected = None;
        }
        self.hovered = None;
        self.drag = None;
        self.history_changed();
    }

    /// `CloneRegion`: the image's pixels in `r` become an image annotation
    /// at the same place, selected, the Clone tool kept.
    pub(super) fn clone_region(&mut self, r: (f64, f64, f64, f64)) {
        self.commit_text();
        let tool = std::mem::replace(&mut self.core.active_tool, self.tool.clone());
        let key = self.core.clone_region(r);
        self.core.active_tool = tool;
        if let Some(key) = key {
            self.history_changed();
            self.set_selected(key);
        }
    }

    /// `InsertImageAnnotationCore`: an image annotation with its top-left
    /// at `at`; returns its id. With `select`, it's selected and (but for
    /// clones) the Select tool chosen.
    pub fn insert_image_annotation(&mut self, pm: Pixmap, at: P, tool: EditorTool, select: bool) -> String {
        self.commit_text();
        let mut a = Annotation::new("Image").expect("Image");
        a.common.tool_type = tool.clone();
        a.common.start_point = sk(at);
        a.common.end_point = sk((at.0 + pm.width() as f64, at.1 + pm.height() as f64));
        a.common.id = crate::editor::editor_core::new_annotation_id();
        if let Kind::Image(p) = &mut a.kind {
            p.set_image(pm);
        }
        let id = a.common.key.0.clone();
        self.annotations_memento(false);
        self.core.annotations.push(a);
        self.history_changed();
        if select {
            if tool != EditorTool::Clone {
                self.select_tool(EditorTool::Select);
            }
            self.set_selected(id.clone());
        }
        id
    }

    /// `InsertImageAnnotation` with no position: centred on `center` (the
    /// visible canvas's centre, which the host knows).
    pub fn insert_image_centered(&mut self, pm: Pixmap, center: P) -> String {
        let at = (center.0 - pm.width() as f64 / 2.0, center.1 - pm.height() as f64 / 2.0);
        self.insert_image_annotation(pm, at, EditorTool::Image, true)
    }

    /// `HandleImageTool`'s result: the picked image centred on the click,
    /// added and selected (the tool stays).
    pub fn insert_picked_image(&mut self, pm: Pixmap, at: P) -> String {
        let pos = (at.0 - pm.width() as f64 / 2.0, at.1 - pm.height() as f64 / 2.0);
        self.commit_text();
        let mut a = Annotation::new("Image").expect("Image");
        a.common.start_point = sk(pos);
        a.common.end_point = sk((pos.0 + pm.width() as f64, pos.1 + pm.height() as f64));
        a.common.id = crate::editor::editor_core::new_annotation_id();
        if let Kind::Image(p) = &mut a.kind {
            p.set_image(pm);
        }
        let id = a.common.key.0.clone();
        self.annotations_memento(false);
        self.core.annotations.push(a);
        self.history_changed();
        self.set_selected(id.clone());
        id
    }

    /// Replaces an image annotation's pixels (a double click's picker,
    /// `ReplaceImageAnnotationFromFilePickerAsync`), keeping its centre;
    /// not an undo step, as in ShareX.
    pub fn replace_image(&mut self, id: &str, pm: Pixmap) {
        let Some(i) = self.find(id) else { return };
        let a = &mut self.core.annotations[i];
        let b = geometry::bounds(a);
        let (cx, cy) = (b.mid_x(), b.mid_y());
        let (w, h) = (pm.width() as f32, pm.height() as f32);
        a.common.start_point = Point::new(cx - w / 2.0, cy - h / 2.0);
        a.common.end_point = Point::new(cx + w / 2.0, cy + h / 2.0);
        if let Kind::Image(p) = &mut a.kind {
            p.set_image(pm);
        }
        self.dirty = true;
        self.changed();
    }

    /// `EditorCore.ResizeCanvas`: room added on each side (negative crops),
    /// filled with `color` (straight RGBA), the annotations moved with the
    /// image.
    pub fn resize_canvas(&mut self, top: i32, right: i32, bottom: i32, left: i32, color: [u8; 4]) -> bool {
        self.commit_text();
        if !self.core.resize_canvas(top, right, bottom, left, color) {
            return false;
        }
        self.image_changed();
        true
    }

    /// `InsertEmojiAnnotation`: a 160px emoji with its top-left at `at`,
    /// selected with the Select tool.
    pub fn insert_emoji(&mut self, unicode_sequence: &str, display_name: &str, at: P) -> String {
        self.commit_text();
        let mut a = Annotation::new("Emoji").expect("Emoji");
        if let Kind::Emoji(e) = &mut a.kind {
            *e = EmojiProps { unicode_sequence: unicode_sequence.into(), display_name: display_name.into() };
        }
        a.common.start_point = sk(at);
        a.common.end_point = sk((at.0 + EMOJI_SIZE, at.1 + EMOJI_SIZE));
        a.common.key = crate::model::Key::fresh();
        let id = a.common.key.0.clone();
        self.annotations_memento(false);
        self.core.annotations.push(a);
        self.history_changed();
        self.select_tool(EditorTool::Select);
        self.set_selected(id.clone());
        id
    }

    /// Replaces an emoji annotation's emoji (`ShowEmojiPickerForReplacement`).
    pub fn replace_emoji(&mut self, id: &str, unicode_sequence: &str, display_name: &str) {
        let Some(i) = self.find(id) else { return };
        self.annotations_memento(false);
        if let Kind::Emoji(e) = &mut self.core.annotations[i].kind {
            *e = EmojiProps { unicode_sequence: unicode_sequence.into(), display_name: display_name.into() };
        }
        self.history_changed();
    }

    /// `InsertWorkspaceImageAnnotation`: an image annotation of `pm` with
    /// its top-left at `at`, not selected and not marking the editor dirty
    /// (region capture's shown pointer, `CursorData.ToBitmap`).
    pub fn insert_workspace_image(&mut self, pm: Pixmap, at: P) -> String {
        let key = self.insert_image_annotation(pm, at, EditorTool::Image, false);
        self.dirty = false;
        key
    }

    /// `SampleCanvasColor`: `#RRGGBB` of the image with its annotations at
    /// `p` (the top-left pixel outside it).
    pub(super) fn sample_color(&mut self, p: P) -> String {
        let pm = self.composite();
        let (mut x, mut y) = (round(p.0), round(p.1));
        if x < 0 || y < 0 || x >= pm.width() as i32 || y >= pm.height() as i32 {
            (x, y) = (0, 0);
        }
        let c = unpremul(pm.pixel(x, y));
        format!("#{:02X}{:02X}{:02X}", (c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF)
    }

    // Region tools.

    fn canvas_size(&self) -> (f64, f64) {
        (self.core.source.width() as f64, self.core.source.height() as f64)
    }

    /// `ClampPointToCanvasBounds`.
    pub(super) fn clamp_to_canvas(&self, p: P) -> P {
        let (w, h) = self.canvas_size();
        (p.0.clamp(0.0, w), p.1.clamp(0.0, h))
    }

    /// A press with a pending crop (`_cropActive`): its handles, a double
    /// click inside to confirm, a press inside to move it; outside cancels
    /// it and the press goes on.
    pub(super) fn pending_crop_pressed(&mut self, p: P, double: bool) -> bool {
        let Some(crop) = self.crop.as_ref() else { return false };
        let rect = crop.rect;
        let handle = CropHandle::NODES.into_iter().rev().find(|h| {
            let (c, (w, hh)) = (h.position(rect), h.hit_size());
            (p.0 - c.0).abs() <= w / 2.0 && (p.1 - c.1).abs() <= hh / 2.0
        });
        if let Some(h) = handle {
            self.crop.as_mut().unwrap().drag = Some((h, rect, p));
            self.changed();
            return true;
        }
        let inside = p.0 >= rect.0 && p.0 <= rect.0 + rect.2 && p.1 >= rect.1 && p.1 <= rect.1 + rect.3;
        if double && inside {
            self.confirm_crop();
            return true;
        }
        if inside {
            self.crop.as_mut().unwrap().drag = Some((CropHandle::Move, rect, p));
            self.changed();
            return true;
        }
        self.cancel_crop();
        false
    }

    /// A crop handle (or the crop) being dragged.
    pub(super) fn crop_dragging(&self) -> bool {
        self.crop.as_ref().is_some_and(|c| c.drag.is_some())
    }

    pub(super) fn crop_drag_to(&mut self, p: P) {
        let (cw, ch) = self.canvas_size();
        if let Some(c) = self.crop.as_mut()
            && let Some((h, start_rect, start)) = c.drag
        {
            c.rect = resize_crop(h, start, p, start_rect, cw, ch);
            self.changed();
        }
    }

    pub(super) fn crop_drag_end(&mut self) -> bool {
        match self.crop.as_mut() {
            Some(c) if c.drag.is_some() => {
                c.drag = None;
                self.changed();
                true
            }
            _ => false,
        }
    }

    /// `TryConfirmCrop` (Enter, a double click, the Crop button).
    pub fn confirm_crop(&mut self) -> bool {
        let Some(c) = self.crop.take() else { return false };
        self.perform_crop(c.rect);
        true
    }

    /// `CancelCrop` (Escape, a right click).
    pub fn cancel_crop(&mut self) -> bool {
        let had = self.crop.take().is_some() || self.region.take_if(|r| r.tool == EditorTool::Crop).is_some();
        if had {
            self.changed();
        }
        had
    }

    /// `CancelClone`.
    pub fn cancel_clone(&mut self) -> bool {
        if self.region.take_if(|r| r.tool == EditorTool::Clone).is_some() {
            self.changed();
            return true;
        }
        false
    }

    /// `PerformCrop`: the overlay's rectangle rounded.
    fn perform_crop(&mut self, (x, y, w, h): (f64, f64, f64, f64)) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (x, y, w, h) = (round(x), round(y), round(w), round(h));
        if self.crop_image(x as f64, y as f64, w as f64, h as f64) {
            let (width, height) = self.size();
            self.requests.push(Request::Cropped { width, height });
        }
        self.changed();
    }

    /// The press of a region tool (Crop, Cut out, Clone): its overlay starts.
    pub(super) fn region_pressed(&mut self, p: P) {
        let p = self.clamp_to_canvas(p);
        self.region = Some(RegionDraw { tool: self.tool.clone(), start: p, rect: (p.0, p.1, 0.0, 0.0), visible: false, cut_vertical: None });
        if self.tool == EditorTool::Crop {
            // `BeginCrop`: shown at the start point, empty.
            self.region.as_mut().unwrap().visible = true;
        }
        self.changed();
    }

    pub(super) fn region_moved(&mut self, p: P, shift: bool) {
        let p = self.clamp_to_canvas(p);
        let (cw, ch) = self.canvas_size();
        let Some(r) = self.region.as_mut() else { return };
        let s = r.start;
        let (dx, dy) = ((p.0 - s.0).abs(), (p.1 - s.1).abs());
        match r.tool {
            EditorTool::CutOut => {
                r.visible = true;
                if dx < CUT_OUT_THRESHOLD && dy < CUT_OUT_THRESHOLD {
                    // A 30px square at the start point.
                    r.cut_vertical = None;
                    r.rect = (s.0 - 15.0, s.1 - 15.0, 30.0, 30.0);
                } else {
                    r.cut_vertical = Some(dx > dy);
                    r.rect = if dx > dy { (s.0.min(p.0), 0.0, dx, ch) } else { (0.0, s.1.min(p.1), cw, dy) };
                }
            }
            _ => {
                let (mut left, mut top, mut w, mut h) = (s.0.min(p.0), s.1.min(p.1), dx, dy);
                if shift {
                    w = w.max(h);
                    h = w;
                    if p.0 < s.0 {
                        left = s.0 - w;
                    }
                    if p.1 < s.1 {
                        top = s.1 - h;
                    }
                }
                r.rect = (left, top, w, h);
                if r.tool == EditorTool::Crop {
                    r.visible = w > 0.0 && h > 0.0;
                }
            }
        }
        self.changed();
    }

    pub(super) fn region_released(&mut self) {
        let Some(r) = self.region.take() else { return };
        self.changed();
        let (_, _, w, h) = r.rect;
        match r.tool {
            EditorTool::Crop => {
                if r.visible && w >= MIN_SHAPE_SIZE && h >= MIN_SHAPE_SIZE {
                    if self.tools.options.QuickCrop {
                        self.perform_crop(r.rect);
                    } else {
                        self.crop = Some(PendingCrop { rect: r.rect, drag: None });
                    }
                }
            }
            EditorTool::CutOut => {
                if w > 0.0 && h > 0.0
                    && let Some(vertical) = r.cut_vertical
                {
                    let (a, b) = if vertical { (r.rect.0, r.rect.0 + w) } else { (r.rect.1, r.rect.1 + h) };
                    if self.cut_out_image(round(a), round(b), vertical) {
                        let (width, height) = self.size();
                        self.requests.push(Request::CutOut { width, height });
                    }
                }
            }
            EditorTool::Clone if w >= MIN_SHAPE_SIZE && h >= MIN_SHAPE_SIZE => self.clone_region(r.rect),
            _ => {}
        }
    }

    pub(super) fn region_active(&self) -> bool {
        self.region.is_some()
    }

    /// The crop overlay and its adorner, while cropping.
    pub fn crop_chrome(&self) -> Option<CropChrome> {
        let (rect, pending) = match (&self.crop, &self.region) {
            (Some(c), _) => (c.rect, true),
            (None, Some(r)) if r.tool == EditorTool::Crop && r.visible => (r.rect, false),
            _ => return None,
        };
        let (cw, ch) = self.canvas_size();
        let left = rect.0.clamp(0.0, cw);
        let top = rect.1.clamp(0.0, ch);
        let right = (rect.0 + rect.2).clamp(0.0, cw);
        let bottom = (rect.1 + rect.3).clamp(0.0, ch);
        let (w, h) = ((right - left).max(0.0), (bottom - top).max(0.0));
        let shades: Vec<_> = [
            (0.0, 0.0, cw, top),
            (0.0, top, left, h),
            (right, top, cw - right, h),
            (0.0, bottom, cw, ch - bottom),
        ]
        .into_iter()
        .filter(|r| r.2 > 0.0 && r.3 > 0.0)
        .collect();
        let guides = if w >= MIN_CROP_GUIDE_SIZE && h >= MIN_CROP_GUIDE_SIZE {
            let (v1, v2) = (left + w / 3.0, left + 2.0 * w / 3.0);
            let (h1, h2) = (top + h / 3.0, top + 2.0 * h / 3.0);
            vec![((v1, top), (v1, bottom)), ((v2, top), (v2, bottom)), ((left, h1), (right, h1)), ((left, h2), (right, h2))]
        } else {
            Vec::new()
        };
        let (iw, ih) = (round(w), round(h));
        let info = (iw > 0 && ih > 0).then(|| format!("X: {} Y: {} W: {} H: {}", round(left), round(top), iw, ih));
        let handles = if pending { CropHandle::NODES.iter().map(|h| (*h, h.position(rect))).collect() } else { Vec::new() };
        Some(CropChrome { rect, shades, guides, info, handles, pending })
    }

    /// The cut out or clone region being dragged.
    pub fn region_preview(&self) -> Option<RegionPreview> {
        let r = self.region.as_ref()?;
        match r.tool {
            EditorTool::CutOut if r.visible => Some(RegionPreview::CutOut { rect: r.rect }),
            EditorTool::Clone => Some(RegionPreview::Clone { rect: r.rect }),
            _ => None,
        }
    }

    // The right button.

    /// A right button press at `p` (`OnCanvasPointerPressed`): cancels the
    /// shape being drawn or the pending crop; else, with
    /// `RightClickDeletesShape`, deletes the active tool's shape under it
    /// (undoable) or opens the context menu; else selects the shape under
    /// it and opens the context menu.
    pub fn right_pressed(&mut self, p: P) -> RightClick {
        self.pointer = Some(p);
        if self.is_drawing() || self.region.as_ref().is_some_and(|r| r.tool != EditorTool::Crop) {
            self.cancel_drawing();
            self.region = None;
            self.changed();
            return RightClick::Cancelled;
        }
        if self.tool == EditorTool::Crop && (self.crop.is_some() || self.region.is_some()) {
            self.cancel_crop();
            return RightClick::Cancelled;
        }
        if self.tools.options.RightClickDeletesShape {
            if self.delete_at(p) {
                return RightClick::Deleted;
            }
            return RightClick::ContextMenu;
        }
        if let Some(i) = self.hit(p) {
            let id = self.core.annotations[i].common.key.0.clone();
            if self.core.selected.as_deref() != Some(id.as_str()) {
                self.set_selected(id);
            }
        }
        RightClick::ContextMenu
    }
}
