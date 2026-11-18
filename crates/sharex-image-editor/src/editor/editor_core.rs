//! `EditorCore` (`Core/Editor/EditorCore.cs`) without its input handling:
//! the source image and its annotations, their history (`EditorHistory`
//! with canvas mementos), and the operations that change the image itself
//! (crop, cut out, rotate, flip, resize, resize canvas, auto crop, flatten,
//! auto-expand) with what they do to the annotations, plus the clone and
//! smart eraser steps the editor's tools take (`EditorView.CloneRegion`,
//! `SmartEraserAnnotation.ConfigureFill` and `ClampVisualBounds`).
//!
//! [`super::Workspace`] is the interactive editor; this is the model those
//! operations need, usable on its own (and checked against ShareX's
//! `EditorCore` by the editor harness).

use std::sync::atomic::{AtomicU64, Ordering};

use sharex_skia::Pixmap;

use super::geometry::{self, Rect};
use super::history::{History, Snapshot};
use super::image_ops;
use crate::model::{Annotation, EditorTool, ImageProps, Kind, Point, SmartEraserFillMode};

/// `EditorMemento`: the annotations (and the selection), with the image
/// for the destructive operations.
#[derive(Clone, Debug)]
pub struct CoreMemento {
    pub annotations: Vec<Annotation>,
    pub canvas_size: (u32, u32),
    pub canvas: Option<Pixmap>,
    pub selected: Option<String>,
}

impl Snapshot for CoreMemento {
    fn has_canvas(&self) -> bool {
        self.canvas.is_some()
    }
}

/// `EditorCore.MaxAutoExpandPerSide`.
const MAX_AUTO_EXPAND_PER_SIDE: i32 = 20000;

/// A new annotation id (ShareX's `Guid.NewGuid()`).
pub fn new_annotation_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!(
        "00000000-0000-4000-9000-{:012x}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

pub struct EditorCore {
    source: Pixmap,
    annotations: Vec<Annotation>,
    selected: Option<String>,
    history: History<CoreMemento>,
    live_expand_original: Option<Pixmap>,
    /// `ActiveTool`: annotation history is skipped while cropping.
    pub active_tool: EditorTool,
    /// `NumberCounter`.
    pub number_counter: i32,
}

impl EditorCore {
    /// `LoadImage`.
    pub fn new(source: Pixmap) -> Self {
        Self {
            source,
            annotations: Vec::new(),
            selected: None,
            history: History::default(),
            live_expand_original: None,
            active_tool: EditorTool::Select,
            number_counter: 1,
        }
    }

    /// `SourceImage`.
    pub fn source(&self) -> &Pixmap {
        &self.source
    }

    /// `CanvasSize`.
    pub fn canvas_size(&self) -> (u32, u32) {
        (self.source.width(), self.source.height())
    }

    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// The annotations, to change in place (ShareX's tools edit the
    /// annotation objects the list holds).
    pub fn annotations_mut(&mut self) -> &mut Vec<Annotation> {
        &mut self.annotations
    }

    /// `SelectedAnnotation`'s id.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// `Select` (`None`: `Deselect`).
    pub fn select(&mut self, id: Option<String>) {
        self.selected = id;
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// The image with every annotation, as `EditorView.GetSnapshot`
    /// exports it.
    pub fn composite(&self) -> Pixmap {
        crate::render::render(&self.source, &self.annotations)
    }

    /// `SampleCanvasColor`: the image's colour at `(x, y)` (rounded; the
    /// top-left pixel when outside), as `#RRGGBB`, what the smart eraser
    /// starts filled with.
    pub fn sample_canvas_color(&self, x: f32, y: f32) -> Option<String> {
        let (w, h) = (self.source.width() as i32, self.source.height() as i32);
        if w == 0 || h == 0 {
            return None;
        }
        let (mut px, mut py) = ((x as f64).round_ties_even() as i32, (y as f64).round_ties_even() as i32);
        if px < 0 || py < 0 || px >= w || py >= h {
            (px, py) = (0, 0);
        }
        let c = image_ops::get_pixel(&self.source, px, py);
        Some(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
    }

    /// `EditorView.GetSnapshot(Rect)`: region capture's export of `rect`
    /// (`x, y, width, height`), see [`crate::render::render_region`].
    pub fn snapshot_region(&self, rect: (f64, f64, f64, f64)) -> Option<Pixmap> {
        crate::render::render_region(&self.source, &self.annotations, rect)
    }

    // History.

    /// `GetMementoFromAnnotations`.
    fn annotations_memento(&self, exclude: Option<&str>) -> CoreMemento {
        CoreMemento {
            annotations: self
                .annotations
                .iter()
                .filter(|a| Some(a.common.id.as_str()) != exclude)
                .cloned()
                .collect(),
            canvas_size: self.canvas_size(),
            canvas: None,
            selected: self.selected.clone(),
        }
    }

    /// `GetMementoFromCanvas`.
    fn canvas_memento(&self) -> CoreMemento {
        CoreMemento {
            canvas: Some(self.source.clone()),
            ..self.annotations_memento(None)
        }
    }

    /// `CreateCanvasMemento`: before a destructive operation.
    pub fn create_canvas_memento(&mut self) {
        let m = self.canvas_memento();
        self.history.add(m);
    }

    /// `CreateAnnotationsMemento`: skipped while the crop tool is active
    /// unless `force`d.
    pub fn create_annotations_memento(&mut self, exclude: Option<&str>, force: bool) {
        if !force && self.active_tool == EditorTool::Crop {
            return;
        }
        let m = self.annotations_memento(exclude);
        self.history.add(m);
    }

    /// `Undo`.
    pub fn undo(&mut self) {
        let Some(canvas) = self.history.peek_undo().map(|m| m.has_canvas()) else {
            return;
        };
        let current = if canvas {
            self.canvas_memento()
        } else {
            self.annotations_memento(None)
        };
        if let Some(m) = self.history.undo(current) {
            self.restore_state(m);
        }
    }

    /// `Redo`.
    pub fn redo(&mut self) {
        let Some(canvas) = self.history.peek_redo().map(|m| m.has_canvas()) else {
            return;
        };
        let current = if canvas {
            self.canvas_memento()
        } else {
            self.annotations_memento(None)
        };
        if let Some(m) = self.history.redo(current) {
            self.restore_state(m);
        }
    }

    /// `RestoreState`.
    fn restore_state(&mut self, m: CoreMemento) {
        self.annotations = m.annotations;
        if let Some(canvas) = m.canvas {
            self.source = canvas;
        }
        self.selected = m
            .selected
            .filter(|id| self.annotations.iter().any(|a| &a.common.id == id));
    }

    // Annotations.

    /// `AddAnnotation`: the history first, then the annotation.
    pub fn add_annotation(&mut self, a: Annotation) {
        self.create_annotations_memento(None, false);
        self.annotations.push(a);
    }

    /// `RemoveAnnotation`, renumbering the steps after a deleted step.
    pub fn remove_annotation(&mut self, id: &str) {
        let Some(i) = self.annotations.iter().position(|a| a.common.id == id) else {
            return;
        };
        self.create_annotations_memento(None, false);
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
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
    }

    /// `ClearAll`.
    pub fn clear_all(&mut self, reset_history: bool) {
        if !reset_history {
            self.create_annotations_memento(None, true);
        }
        self.annotations.clear();
        if reset_history {
            self.history = History::default();
        }
        self.selected = None;
        self.number_counter = 1;
    }

    // Image operations.

    /// `ApplyImageOperation`: the new image (`None` or empty: nothing
    /// happens), a canvas memento, then the annotations cleared or
    /// transformed.
    pub fn apply_image_operation(
        &mut self,
        operation: impl FnOnce(&Pixmap) -> Option<Pixmap>,
        clear_annotations: bool,
        transform: Option<&dyn Fn(&mut Vec<Annotation>)>,
    ) -> bool {
        let Some(result) = operation(&self.source) else {
            return false;
        };
        if result.width() == 0 || result.height() == 0 {
            return false;
        }
        self.create_canvas_memento();
        self.source = result;
        if clear_annotations {
            // ClearAnnotationsForImageMutation
            if !self.annotations.is_empty() {
                self.annotations.clear();
                self.selected = None;
                self.number_counter = 1;
            }
        } else if let Some(t) = transform {
            t(&mut self.annotations);
        }
        true
    }

    /// `ApplyImageEffect`: an effect on the image, annotations kept.
    pub fn apply_image_effect(&mut self, effect: impl FnOnce(&Pixmap) -> Option<Pixmap>) -> bool {
        self.apply_image_operation(effect, false, None)
    }

    /// `ResizeImage` (Catmull-Rom), the annotations scaled.
    pub fn resize_image(&mut self, width: i32, height: i32) -> bool {
        if width <= 0 || height <= 0 {
            return false;
        }
        let sx = width as f32 / self.source.width() as f32;
        let sy = height as f32 / self.source.height() as f32;
        self.apply_image_operation(
            |img| Some(image_ops::resize(img, width as u32, height as u32)),
            false,
            Some(&|list: &mut Vec<Annotation>| {
                list.iter_mut().for_each(|a| geometry::scale(a, sx, sy))
            }),
        )
    }

    /// `ResizeCanvas`: room added on each side (negative crops), filled
    /// with `background` (straight RGBA), the annotations moved with the
    /// image.
    pub fn resize_canvas(
        &mut self,
        top: i32,
        right: i32,
        bottom: i32,
        left: i32,
        background: [u8; 4],
    ) -> bool {
        self.apply_image_operation(
            |img| image_ops::resize_canvas(img, left, top, right, bottom, background),
            false,
            Some(&|list: &mut Vec<Annotation>| translate(list, left as f32, top as f32)),
        )
    }

    /// `CanAutoExpandFor`: effects and region tools stay inside the image.
    pub fn can_auto_expand_for(a: &Annotation) -> bool {
        !matches!(
            a.kind,
            Kind::Highlight(_)
                | Kind::Blur(_)
                | Kind::Pixelate(_)
                | Kind::Magnify(_)
                | Kind::Spotlight(_)
                | Kind::Crop
                | Kind::SmartEraser(_)
        )
    }

    /// `GetAutoExpandBounds`: the bounds as drawn (rotation, a balloon's
    /// tail, half the stroke).
    pub fn auto_expand_bounds(a: &Annotation) -> Rect {
        let b = geometry::bounds(a);
        let mut r = Rect::new(b.l.min(b.r), b.t.min(b.b), b.l.max(b.r), b.t.max(b.b));
        if a.common.rotation_angle % 360.0 != 0.0 {
            let rad = a.common.rotation_angle * std::f32::consts::PI / 180.0;
            let (cos, sin) = (rad.cos(), rad.sin());
            let (cx, cy) = (r.mid_x(), r.mid_y());
            let (mut min_x, mut min_y, mut max_x, mut max_y) =
                (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (px, py) in [(r.l, r.t), (r.r, r.t), (r.r, r.b), (r.l, r.b)] {
                let x = cx + (px - cx) * cos - (py - cy) * sin;
                let y = cy + (px - cx) * sin + (py - cy) * cos;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
            r = Rect::new(min_x, min_y, max_x, max_y);
        }
        if let Kind::SpeechBalloon(p) = &a.kind
            && p.tail_enabled
        {
            let body = geometry::bounds(a);
            let tail = geometry::balloon_effective_tail_point(&body, p);
            let tail = rotate_around(tail, &body, a.common.rotation_angle);
            r = r.union(&Rect::new(tail.x, tail.y, tail.x, tail.y));
        }
        let stroke = a.common.stroke_width.max(0.0) / 2.0;
        r.inflate(stroke, stroke)
    }

    /// `AutoExpandCanvasToFit` over the annotations with `ids` (every one
    /// when `None`): the canvas grown on the sides they cross, by `margin`
    /// more, filled with `background`. Returns the room added left and top.
    pub fn auto_expand_canvas_to_fit(
        &mut self,
        ids: Option<&[String]>,
        margin: f32,
        background: [u8; 4],
    ) -> Option<(i32, i32)> {
        let mut union: Option<Rect> = None;
        for a in &self.annotations {
            if ids.is_some_and(|ids| !ids.contains(&a.common.id)) || !Self::can_auto_expand_for(a) {
                continue;
            }
            let b = Self::auto_expand_bounds(a);
            if b.l.is_nan() || b.t.is_nan() || b.r.is_nan() || b.b.is_nan() {
                continue;
            }
            union = Some(union.map_or(b, |u| u.union(&b)));
        }
        let u = union?;
        let (width, height) = (self.source.width() as f32, self.source.height() as f32);
        const TOLERANCE: f32 = 0.5;
        let side = |cond: bool, v: f32| if cond { (v as f64).ceil() as i32 } else { 0 };
        let left = side(u.l < -TOLERANCE, margin - u.l).clamp(0, MAX_AUTO_EXPAND_PER_SIDE);
        let top = side(u.t < -TOLERANCE, margin - u.t).clamp(0, MAX_AUTO_EXPAND_PER_SIDE);
        let right =
            side(u.r > width + TOLERANCE, u.r + margin - width).clamp(0, MAX_AUTO_EXPAND_PER_SIDE);
        let bottom = side(u.b > height + TOLERANCE, u.b + margin - height)
            .clamp(0, MAX_AUTO_EXPAND_PER_SIDE);
        if left == 0 && top == 0 && right == 0 && bottom == 0 {
            return None;
        }
        let shift = left > 0 || top > 0;
        let shift_by = |list: &mut Vec<Annotation>| translate(list, left as f32, top as f32);
        let transform: Option<&dyn Fn(&mut Vec<Annotation>)> =
            if shift { Some(&shift_by) } else { None };
        let expanded = self.apply_image_operation(
            |img| image_ops::resize_canvas(img, left, top, right, bottom, background),
            false,
            transform,
        );
        expanded.then_some((left, top))
    }

    /// `ExpandCanvasLive`: the canvas grown right and down while a text is
    /// typed, outside the history (`end_live_canvas_expansion` undoes it).
    pub fn expand_canvas_live(&mut self, right: i32, bottom: i32, background: [u8; 4]) -> bool {
        let right = right.clamp(0, MAX_AUTO_EXPAND_PER_SIDE);
        let bottom = bottom.clamp(0, MAX_AUTO_EXPAND_PER_SIDE);
        if right == 0 && bottom == 0 {
            return false;
        }
        let Some(result) = image_ops::resize_canvas(&self.source, 0, 0, right, bottom, background)
        else {
            return false;
        };
        let previous = std::mem::replace(&mut self.source, result);
        if self.live_expand_original.is_none() {
            self.live_expand_original = Some(previous);
        }
        true
    }

    /// `IsLiveCanvasExpansionActive`.
    pub fn is_live_canvas_expansion_active(&self) -> bool {
        self.live_expand_original.is_some()
    }

    /// `EndLiveCanvasExpansion`: the image from before `expand_canvas_live`.
    pub fn end_live_canvas_expansion(&mut self) -> bool {
        match self.live_expand_original.take() {
            Some(original) => {
                self.source = original;
                true
            }
            None => false,
        }
    }

    /// `Rotate90Clockwise`.
    pub fn rotate_90_clockwise(&mut self) -> bool {
        self.rotate_orthogonal(90)
    }

    /// `Rotate90CounterClockwise`.
    pub fn rotate_90_counter_clockwise(&mut self) -> bool {
        self.rotate_orthogonal(270)
    }

    /// `Rotate180`.
    pub fn rotate_180(&mut self) -> bool {
        self.rotate_orthogonal(180)
    }

    fn rotate_orthogonal(&mut self, degrees: i32) -> bool {
        let (w, h) = (self.source.width() as f32, self.source.height() as f32);
        let transform = move |list: &mut Vec<Annotation>| {
            for a in list.iter_mut() {
                geometry::transform_points(a, &|p| match degrees {
                    90 => Point::new(h - p.y, p.x),
                    180 => Point::new(w - p.x, h - p.y),
                    270 => Point::new(p.y, w - p.x),
                    _ => p,
                });
                a.common.rotation_angle = normalize_angle(a.common.rotation_angle + degrees as f32);
            }
        };
        let effect_angle = if degrees == 270 { -90 } else { degrees };
        self.apply_image_operation(
            |img| Some(image_ops::rotate_orthogonal(img, effect_angle)),
            false,
            Some(&transform),
        )
    }

    /// `RotateCustomAngle`: the image rotated about its centre (into its
    /// rotated bounds when `auto_resize`), the annotations with it.
    pub fn rotate_custom_angle(&mut self, angle: f32, auto_resize: bool) -> bool {
        let (old_w, old_h) = (self.source.width() as f32, self.source.height() as f32);
        let result = image_ops::rotate(&self.source, angle, auto_resize);
        let (new_w, new_h) = (result.width() as f32, result.height() as f32);
        let transform = move |list: &mut Vec<Annotation>| {
            let rad = angle * std::f32::consts::PI / 180.0;
            let (cos, sin) = (rad.cos(), rad.sin());
            let (cx, cy) = (old_w / 2.0, old_h / 2.0);
            let (tcx, tcy) = (new_w / 2.0, new_h / 2.0);
            for a in list.iter_mut() {
                geometry::transform_points(a, &|p| {
                    let nx = cx + (p.x - cx) * cos - (p.y - cy) * sin;
                    let ny = cy + (p.x - cx) * sin + (p.y - cy) * cos;
                    if auto_resize {
                        Point::new(nx - cx + tcx, ny - cy + tcy)
                    } else {
                        Point::new(nx, ny)
                    }
                });
                a.common.rotation_angle = normalize_angle(a.common.rotation_angle + angle);
            }
        };
        self.apply_image_operation(|_| Some(result), false, Some(&transform))
    }

    /// `FlipHorizontal`.
    pub fn flip_horizontal(&mut self) -> bool {
        self.flip(true, false)
    }

    /// `FlipVertical`.
    pub fn flip_vertical(&mut self) -> bool {
        self.flip(false, true)
    }

    fn flip(&mut self, h: bool, v: bool) -> bool {
        let (w, ht) = (self.source.width() as f32, self.source.height() as f32);
        let transform = move |list: &mut Vec<Annotation>| {
            for a in list.iter_mut() {
                geometry::transform_points(a, &|p| {
                    Point::new(
                        if h { w - p.x } else { p.x },
                        if v { ht - p.y } else { p.y },
                    )
                });
                let mut angle = a.common.rotation_angle;
                if h {
                    angle = 360.0 - angle;
                }
                if v {
                    angle = 360.0 - angle;
                }
                a.common.rotation_angle = normalize_angle(angle);
            }
        };
        self.apply_image_operation(
            |img| Some(image_ops::flip(img, h, v)),
            false,
            Some(&transform),
        )
    }

    /// `FlattenImage`: `composite` (the image with its annotations, from
    /// [`Self::composite`]) becomes the image; the annotations go.
    pub fn flatten_image(&mut self, composite: Pixmap) -> bool {
        if self.annotations.is_empty() {
            return false;
        }
        self.apply_image_operation(|_| Some(composite), true, None)
    }

    /// `AutoCrop`: cropped to what differs from the top-left pixel by more
    /// than `tolerance` (a uniform image becomes 1x1, transparent).
    pub fn auto_crop(&mut self, tolerance: i32) -> bool {
        let top_left = image_ops::get_pixel(&self.source, 0, 0);
        let bounds = image_ops::find_content_bounds(&self.source, top_left, tolerance);
        let (min_x, min_y) = bounds.map_or((0, 0), |b| (b.0, b.1));
        self.apply_image_operation(
            |img| match bounds {
                Some((l, t, r, b)) => image_ops::crop(img, l, t, r - l, b - t),
                None => Some(Pixmap::new(1, 1)),
            },
            false,
            Some(&|list: &mut Vec<Annotation>| translate(list, -(min_x as f32), -(min_y as f32))),
        )
    }

    /// `Crop(bounds)`: the image cut to `bounds` (truncated to whole
    /// pixels, inside the image); annotations entirely outside go, the
    /// others move with the image.
    pub fn crop(&mut self, bounds: Rect) -> bool {
        let x = bounds.l.max(0.0) as i32;
        let y = bounds.t.max(0.0) as i32;
        let width = ((self.source.width() as i32 - x) as f32).min(bounds.width()) as i32;
        let height = ((self.source.height() as i32 - y) as f32).min(bounds.height()) as i32;
        if width <= 0 || height <= 0 {
            return false;
        }
        if x == 0
            && y == 0
            && width == self.source.width() as i32
            && height == self.source.height() as i32
        {
            return false;
        }
        self.create_canvas_memento();
        let Some(cropped) = image_ops::crop(&self.source, x, y, width, height) else {
            return false;
        };
        let (xf, yf, wf, hf) = (x as f32, y as f32, width as f32, height as f32);
        let mut i = self.annotations.len();
        while i > 0 {
            i -= 1;
            if matches!(self.annotations[i].kind, Kind::Crop) {
                continue;
            }
            let b = geometry::bounds(&self.annotations[i]);
            if b.r < xf || b.l > xf + wf || b.b < yf || b.t > yf + hf {
                self.annotations.remove(i);
                continue;
            }
            geometry::transform_points(&mut self.annotations[i], &|p| {
                Point::new(p.x - xf, p.y - yf)
            });
        }
        self.source = cropped;
        true
    }

    /// `PerformCrop`: crops to the crop annotation's bounds, then removes it.
    pub fn perform_crop(&mut self) {
        let Some(i) = self
            .annotations
            .iter()
            .position(|a| matches!(a.kind, Kind::Crop))
        else {
            return;
        };
        let bounds = geometry::bounds(&self.annotations[i]);
        self.crop(bounds);
        if let Some(i) = self
            .annotations
            .iter()
            .position(|a| matches!(a.kind, Kind::Crop))
        {
            self.annotations.remove(i);
        }
    }

    /// `CutOut(start, end, vertical)`: columns (vertical) or rows
    /// `start..end` removed, the rest joined, the annotations adjusted.
    pub fn cut_out(&mut self, start: i32, end: i32, vertical: bool) -> bool {
        let (start, end) = if start > end {
            (end, start)
        } else {
            (start, end)
        };
        let length = if vertical {
            self.source.width()
        } else {
            self.source.height()
        } as i32;
        if start < 0 || end > length || start >= end {
            return false;
        }
        self.create_canvas_memento();
        if end - start >= length {
            return false;
        }
        self.apply_cut_out(start, end, vertical);
        true
    }

    /// `PerformCutOut` with the cut-out tool's rectangle: the cut starts at
    /// its middle (rounded) and is as long as it is wide (or high).
    pub fn perform_cut_out(&mut self, bounds: Rect, vertical: bool) {
        self.create_canvas_memento();
        let length = if vertical {
            self.source.width()
        } else {
            self.source.height()
        } as i32;
        if length <= 0 {
            return;
        }
        let mid = if vertical {
            bounds.mid_x()
        } else {
            bounds.mid_y()
        };
        let size = if vertical {
            bounds.width()
        } else {
            bounds.height()
        };
        let start = ((mid as f64).round_ties_even() as i32).clamp(0, length - 1);
        let cut = ((size as f64).round_ties_even().max(1.0) as i32).min(length - start);
        if cut <= 0 || cut >= length {
            return;
        }
        self.apply_cut_out(start, start + cut, vertical);
    }

    /// `ApplyCutOut`.
    fn apply_cut_out(&mut self, start: i32, end: i32, vertical: bool) {
        if let Some(result) = image_ops::cut_out(&self.source, start, end, vertical) {
            self.source = result;
        }
        let mut i = self.annotations.len();
        while i > 0 {
            i -= 1;
            if !adjust_for_cut_out(&mut self.annotations[i], start, end, vertical) {
                self.annotations.remove(i);
            }
        }
    }

    /// `EditorView.CloneRegion`: the image pixels in `rect` (`x, y, w, h`,
    /// clipped to the image, rounded out) as an image annotation with the
    /// Clone tool's type, added and selected. Returns its id.
    pub fn clone_region(&mut self, rect: (f64, f64, f64, f64)) -> Option<String> {
        let (iw, ih) = (self.source.width() as f64, self.source.height() as f64);
        // Avalonia Rect.Intersect
        let l = rect.0.max(0.0);
        let t = rect.1.max(0.0);
        let r = (rect.0 + rect.2).min(iw);
        let b = (rect.1 + rect.3).min(ih);
        if r <= l || b <= t {
            return None;
        }
        let (left, top) = (l.floor() as i32, t.floor() as i32);
        let (right, bottom) = (r.ceil() as i32, b.ceil() as i32);
        let region = image_ops::crop(&self.source, left, top, right - left, bottom - top)?;
        let mut a = Annotation::new("Image")?;
        a.common.id = new_annotation_id();
        a.common.tool_type = EditorTool::Clone;
        a.common.start_point = Point::new(left as f32, top as f32);
        a.common.end_point = Point::new(
            (left as f64 + region.width() as f64) as f32,
            (top as f64 + region.height() as f64) as f32,
        );
        let mut props = ImageProps::default();
        props.set_image(region);
        a.kind = Kind::Image(props);
        let id = a.common.id.clone();
        self.add_annotation(a);
        self.selected = Some(id.clone());
        Some(id)
    }
}

/// `TranslateAnnotations`.
fn translate(list: &mut [Annotation], dx: f32, dy: f32) {
    for a in list {
        geometry::transform_points(a, &|p| Point::new(p.x + dx, p.y + dy));
    }
}

/// The rotation kept in `[0, 360)`.
fn normalize_angle(mut a: f32) -> f32 {
    while a >= 360.0 {
        a -= 360.0;
    }
    while a < 0.0 {
        a += 360.0;
    }
    a
}

/// `SpeechBalloonAnnotation.RotateAroundBoundsCenter`.
fn rotate_around(p: Point, bounds: &Rect, angle: f32) -> Point {
    if angle == 0.0 {
        return p;
    }
    let (cx, cy) = (bounds.mid_x(), bounds.mid_y());
    let rad = angle * std::f32::consts::PI / 180.0;
    let (cos, sin) = (rad.cos(), rad.sin());
    let (dx, dy) = (p.x - cx, p.y - cy);
    Point::new(cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
}

/// `CutOutAnnotation.AdjustAnnotation`: an annotation after the strip
/// `start..end` was removed; `false` when it lay entirely inside it.
pub fn adjust_for_cut_out(a: &mut Annotation, start: i32, end: i32, vertical: bool) -> bool {
    let b = geometry::bounds(a);
    let (lead, trail) = if vertical { (b.l, b.r) } else { (b.t, b.b) };
    let (s, e) = (start as f32, end as f32);
    if lead >= s && trail <= e {
        return false;
    }
    let after = lead >= e;
    if !(after || trail > e) {
        return true;
    }
    let cut = (end - start) as f32;
    let pos = |p: Point| if vertical { p.x } else { p.y };
    let with = |p: Point, v: f32| {
        if vertical {
            Point::new(v, p.y)
        } else {
            Point::new(p.x, v)
        }
    };
    let shift = |p: Point| {
        let mut v = pos(p);
        if !after && v > s && v < e {
            v = s;
        }
        with(p, v - cut)
    };
    a.common.start_point = shift(a.common.start_point);
    a.common.end_point = shift(a.common.end_point);
    geometry::transform_additional_points(a, &|p| {
        let v = pos(p);
        let v = if v >= e {
            v - cut
        } else if v > s {
            s
        } else {
            v
        };
        with(p, v)
    });
    true
}

/// `SmartEraserAnnotation.ClampVisualBounds`: the eraser's bounds kept
/// inside the `width` x `height` canvas (when it's drawn).
pub fn clamp_smart_eraser(a: &mut Annotation, width: f64, height: f64) {
    let b = geometry::bounds(a);
    let (left, top) = (b.l as f64, b.t as f64);
    let (right, bottom) = (left + b.width() as f64, top + b.height() as f64);
    let cl = left.clamp(0.0, width);
    let ct = top.clamp(0.0, height);
    let cr = right.clamp(0.0, width);
    let cb = bottom.clamp(0.0, height);
    a.common.start_point = Point::new(cl as f32, ct as f32);
    a.common.end_point = Point::new(cr as f32, cb as f32);
}

/// `SmartEraserAnnotation.ConfigureFill`: stretch the left column across
/// when the left and right columns under the eraser match, else the top
/// row down when the top and bottom rows match, else keep the fill colour.
pub fn configure_smart_eraser_fill(a: &mut Annotation, source: &Pixmap) {
    let (w, h) = (source.width() as i32, source.height() as i32);
    if w <= 0 || h <= 0 {
        return;
    }
    let b = geometry::bounds(a);
    let Kind::SmartEraser(p) = &mut a.kind else {
        return;
    };
    let left = (b.l.floor() as i32).clamp(0, w - 1);
    let top = (b.t.floor() as i32).clamp(0, h - 1);
    let right = left.max(b.r.ceil() as i32 - 1).clamp(0, w - 1);
    let bottom = top.max(b.b.ceil() as i32 - 1).clamp(0, h - 1);
    p.fill_mode = SmartEraserFillMode::SolidColor;
    p.edge_pixels = None;
    let px = |x: i32, y: i32| image_ops::get_pixel(source, x, y);
    let pack =
        |c: [u8; 4]| (c[3] as u32) << 24 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32;
    if (top..=bottom).all(|y| px(left, y) == px(right, y)) {
        p.fill_mode = SmartEraserFillMode::StretchHorizontally;
        p.edge_pixels = Some((top..=bottom).map(|y| pack(px(left, y))).collect());
        return;
    }
    if (left..=right).all(|x| px(x, top) == px(x, bottom)) {
        p.fill_mode = SmartEraserFillMode::StretchVertically;
        p.edge_pixels = Some((left..=right).map(|x| pack(px(x, top))).collect());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(l: f32, t: f32, r: f32, b: f32) -> Annotation {
        let mut a = Annotation::new("Rectangle").unwrap();
        a.common.start_point = Point::new(l, t);
        a.common.end_point = Point::new(r, b);
        a
    }

    #[test]
    fn samples_the_image_colour() {
        let mut pm = Pixmap::new(4, 3);
        *pm.pixel_mut(2, 1) = 0xFF12_3456;
        let core = EditorCore::new(pm);
        assert_eq!(core.sample_canvas_color(2.4, 0.6).as_deref(), Some("#123456"));
        // Outside: the top-left pixel.
        assert_eq!(core.sample_canvas_color(9.0, 1.0).as_deref(), Some("#000000"));
    }

    #[test]
    fn history_keeps_five_image_copies() {
        let mut core = EditorCore::new(Pixmap::new(40, 30));
        core.add_annotation(rect(1.0, 1.0, 10.0, 10.0));
        for _ in 0..7 {
            assert!(core.rotate_90_clockwise());
        }
        let mut undos = 0;
        while core.can_undo() {
            core.undo();
            undos += 1;
        }
        // Five rotations undone (two remain: 180 degrees), nothing older
        // than the sixth image copy.
        assert_eq!(undos, 5);
        assert_eq!(core.canvas_size(), (40, 30));
        assert_eq!(core.annotations().len(), 1);
    }

    #[test]
    fn crop_drops_annotations_outside_and_moves_the_rest() {
        let mut core = EditorCore::new(Pixmap::new(100, 80));
        core.add_annotation(rect(10.0, 10.0, 30.0, 30.0));
        core.add_annotation(rect(70.0, 60.0, 90.0, 75.0));
        assert!(core.crop(Rect::new(5.5, 5.0, 55.0, 50.0)));
        assert_eq!(core.canvas_size(), (49, 45));
        assert_eq!(core.annotations().len(), 1);
        assert_eq!(core.annotations()[0].common.start_point, Point::new(5.0, 5.0));
        core.undo();
        assert_eq!(core.canvas_size(), (100, 80));
        assert_eq!(core.annotations().len(), 2);
    }

    #[test]
    fn cut_out_collapses_annotations_inside_the_strip() {
        let mut core = EditorCore::new(Pixmap::new(100, 50));
        core.add_annotation(rect(40.0, 5.0, 45.0, 10.0));
        core.add_annotation(rect(20.0, 5.0, 80.0, 10.0));
        assert!(core.cut_out(30, 60, true));
        assert_eq!(core.canvas_size(), (70, 50));
        // The one inside the strip goes; both ends of the one across it move
        // back by the cut, as ShareX shifts them.
        assert_eq!(core.annotations().len(), 1);
        let a = &core.annotations()[0].common;
        assert_eq!((a.start_point.x, a.end_point.x), (-10.0, 50.0));
    }
}
