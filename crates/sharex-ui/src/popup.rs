//! Popups past their window's edge. Avalonia opens every `Popup` (a menu
//! level, a combo box's dropdown, a flyout) in its own OS window, kept on
//! the screen's work area rather than inside the window that owns it: the
//! Video converter's codec list hangs 196 px below its window at 125%
//! (VID-02), the Sub folder pattern menu opens past the settings window's
//! right edge (SET-04a), a submenu flips at the screen's edge, not the
//! window's.
//!
//! Here a popup stays an egui layer of its owner's context (its layout,
//! state and input are the owner's), and says so with [`register`]. A
//! host that can open OS popups (ShareXrs's windows on Windows) calls
//! [`set_host`] with the work area each frame, so the popups place
//! themselves against it ([`bounds`]), and after the frame moves each
//! popup layer that doesn't fit inside the window into a popup window of
//! its own, sending that window's pointer input back to the owner. Without
//! a host (an offscreen render, other platforms) [`bounds`] is the window
//! and nothing changes.
//!
//! Menus place every level with [`position`], a port of Avalonia's
//! `ManagedPopupPositioner` (Avalonia 12.1.6,
//! `Avalonia.Controls.Primitives.PopupPositioning`): in device pixels on
//! the screen, against the work area of the screen under the anchor, with
//! the popup's position truncated to whole pixels as
//! `ManagedPopupPositionerPopupImplHelper.MoveAndResize` does. A host that
//! knows the screens passes them with [`set_host_screens`]; otherwise the
//! positioner sees one screen whose work area is [`bounds`].

use egui::{Context, Id, LayerId, Pos2, Rect, Vec2, pos2, vec2};

/// A popup drawn this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupLayer {
    /// The egui layer it's drawn on (its `Area`).
    pub layer: LayerId,
    /// Where, in the owner's points.
    pub rect: Rect,
    /// Its corner radius (the popup window is cut to it).
    pub corner_radius: f32,
}

#[derive(Clone, Default)]
struct Frame {
    /// The work area the popups may use, in the owner's points.
    host: Option<Rect>,
    /// The screens and where the owner is on them, when the host knows.
    screens: Option<Geometry>,
    layers: Vec<PopupLayer>,
}

fn key() -> Id {
    Id::new("sharex-ui-popups")
}

/// Called by the host at the start of each frame: popups may go anywhere
/// on `work_area` (the owner's screen's work area, in the owner's points),
/// or, with `None`, must stay inside the window.
pub fn set_host(ctx: &Context, work_area: Option<Rect>) {
    set_host_screens(ctx, work_area, None);
}

/// [`set_host`], with the screens and the owner's place on them for
/// [`position`] (Avalonia picks the screen under each popup's anchor).
pub fn set_host_screens(ctx: &Context, work_area: Option<Rect>, screens: Option<Geometry>) {
    let screens = screens.filter(|_| work_area.is_some());
    ctx.data_mut(|d| d.insert_temp(key(), Frame { host: work_area, screens, layers: Vec::new() }));
}

/// Whether popups can leave the window this frame.
pub fn is_hosted(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<Frame>(key())).is_some_and(|f| f.host.is_some())
}

/// The rectangle a popup is kept in (Avalonia's
/// `PopupPositioner` constraint): the host's work area, or `window` (what
/// the caller would have used) when there's no host.
pub fn bounds(ctx: &Context, window: Rect) -> Rect {
    ctx.data(|d| d.get_temp::<Frame>(key())).and_then(|f| f.host).unwrap_or(window)
}

fn open_key() -> Id {
    Id::new("sharex-ui-popup-open-pass")
}

/// A popup drawn on `layer` at `rect` this frame.
pub fn register(ctx: &Context, layer: LayerId, rect: Rect, corner_radius: f32) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        d.insert_temp(open_key(), pass);
        let f = d.get_temp_mut_or_default::<Frame>(key());
        f.layers.retain(|l| l.layer != layer);
        f.layers.push(PopupLayer { layer, rect, corner_radius });
    });
}

fn closed_key() -> Id {
    Id::new("sharex-ui-popup-escape-closed-pass")
}

/// A popup closed on Escape this frame (it took the key).
pub fn closed_by_escape(ctx: &Context) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(closed_key(), pass));
}

/// Whether a popup (a menu, a dropdown, a flyout) is open this frame, or
/// was last frame and didn't close on Escape: Escape is the popup's then.
pub fn was_open(ctx: &Context) -> bool {
    let now = ctx.cumulative_pass_nr();
    let (open, closed) = ctx.data(|d| (d.get_temp::<u64>(open_key()), d.get_temp::<u64>(closed_key())));
    match open {
        Some(pass) if pass == now => true,
        Some(pass) => pass + 1 == now && closed != Some(pass),
        None => false,
    }
}

/// Escape for a window or an editor (closing it, cancelling): pressed, and
/// no popup over it to close first. Avalonia routes the key to the open
/// popup, the innermost first (SET-05a).
pub fn escape_pressed(ctx: &Context) -> bool {
    ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !was_open(ctx)
}

/// Avalonia's `LightDismissOverlayLayer` for a popup with
/// `IsLightDismissEnabled` and `OverlayDismissEventPassThrough` off (the
/// default: ShareX's dropdowns, Fluent's `ComboBox`, `Flyout`s): while the
/// popup is open, an invisible element over the whole owner window takes
/// the pointer, so the press that dismisses it (`PointerPressedDismissOverlay`)
/// never reaches the control under it, and nothing under it is
/// `:pointerover`. A `ContextMenu` passes the press through and doesn't
/// call this.
///
/// Call it first in the popup's area: a widget covering everything at the
/// bottom of the popup's layer. egui's hit test then leaves the layers
/// below out (the overlay covers the pointer's search area), while the
/// popup's own widgets, registered after it in its layer, stay on top. It
/// takes the pointer from the next frame on (egui hit-tests last frame's
/// widgets), which is the press after the one that opened the popup.
/// Code under the popup that reads the raw pointer (not a `Response`)
/// isn't covered.
pub fn light_dismiss_overlay(ui: &mut egui::Ui) {
    let everything = Rect::from_min_max(pos2(-1.0e6, -1.0e6), pos2(1.0e6, 1.0e6));
    let mut overlay = ui.new_child(egui::UiBuilder::new().max_rect(everything));
    overlay.set_clip_rect(everything);
    // Clicks only: not focusable (Tab never reaches it), and a drag inside
    // the popup still goes to the popup's own drag widgets.
    overlay.interact(everything, ui.id().with("light-dismiss-overlay"), egui::Sense::CLICK);
}

/// The popups drawn this frame (the host, after the frame's UI).
pub fn take(ctx: &Context) -> Vec<PopupLayer> {
    ctx.data_mut(|d| std::mem::take(&mut d.get_temp_mut_or_default::<Frame>(key()).layers))
}

// ---- Avalonia's ManagedPopupPositioner ---------------------------------

/// A rectangle on the screen in device pixels, in doubles as Avalonia's
/// `Rect` (the positioner's maths).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PxRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl PxRect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    fn right(&self) -> f64 {
        self.x + self.w
    }

    fn bottom(&self) -> f64 {
        self.y + self.h
    }

    /// `Rect.ContainsExclusive`.
    fn contains_exclusive(&self, (x, y): (f64, f64)) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// `Rect.Intersects`.
    fn intersects(&self, r: &PxRect) -> bool {
        r.x < self.right() && self.x < r.right() && r.y < self.bottom() && self.y < r.bottom()
    }
}

/// `ManagedPopupPositionerScreenInfo`: a screen's bounds and work area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenInfo {
    pub bounds: PxRect,
    pub working_area: PxRect,
}

/// What the positioner knows about the popup's parent
/// (`IManagedPopupPositionerPopup`): the screens, the parent's client
/// area on them (`ParentClientAreaScreenGeometry`, device pixels) and its
/// `Scaling` (`DesktopScaling`).
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub screens: Vec<ScreenInfo>,
    pub parent: PxRect,
    pub scaling: f64,
}

impl Geometry {
    /// One screen whose work area (and bounds) is `work` in the owner's
    /// points, the owner's client at the screen's origin: what a host
    /// without screens (an offscreen render, other platforms) gives.
    pub fn single(work: Rect, client: Rect, scaling: f32) -> Self {
        let s = scaling as f64;
        let area = PxRect::new(work.min.x as f64 * s, work.min.y as f64 * s, work.width() as f64 * s, work.height() as f64 * s);
        Self {
            screens: vec![ScreenInfo { bounds: area, working_area: area }],
            parent: PxRect::new(0.0, 0.0, client.width() as f64 * s, client.height() as f64 * s),
            scaling: s,
        }
    }
}

/// `PopupAnchor` / `PopupGravity` flags.
pub mod edge {
    pub const NONE: u8 = 0;
    pub const TOP: u8 = 1;
    pub const BOTTOM: u8 = 2;
    pub const LEFT: u8 = 4;
    pub const RIGHT: u8 = 8;
    pub const TOP_LEFT: u8 = TOP | LEFT;
    pub const TOP_RIGHT: u8 = TOP | RIGHT;
    pub const BOTTOM_LEFT: u8 = BOTTOM | LEFT;
    pub const BOTTOM_RIGHT: u8 = BOTTOM | RIGHT;
    pub const VERTICAL: u8 = TOP | BOTTOM;
    pub const HORIZONTAL: u8 = LEFT | RIGHT;
    pub const ALL: u8 = VERTICAL | HORIZONTAL;

    /// `PopupPositioningEdgeHelper.FlipX`.
    pub fn flip_x(e: u8) -> u8 {
        if e & HORIZONTAL != 0 { e ^ HORIZONTAL } else { e }
    }

    /// `PopupPositioningEdgeHelper.FlipY`.
    pub fn flip_y(e: u8) -> u8 {
        if e & VERTICAL != 0 { e ^ VERTICAL } else { e }
    }
}

/// `PopupPositionerConstraintAdjustment` flags.
pub mod adjust {
    pub const NONE: u8 = 0;
    pub const SLIDE_X: u8 = 1;
    pub const SLIDE_Y: u8 = 2;
    pub const FLIP_X: u8 = 4;
    pub const FLIP_Y: u8 = 8;
    pub const RESIZE_X: u8 = 16;
    pub const RESIZE_Y: u8 = 32;
    /// `Popup.PlacementConstraintAdjustment`'s default.
    pub const ALL: u8 = 63;
}

/// `PopupPositionerParameters` in the parent's client coordinates (DIPs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    /// `AnchorRectangle`: the placement target's bounds (a 1x1 rectangle
    /// at the pointer for `PlacementMode.Pointer`).
    pub anchor_rect: Rect,
    pub anchor: u8,
    pub gravity: u8,
    pub adjustment: u8,
    /// `HorizontalOffset`, `VerticalOffset`.
    pub offset: Vec2,
    /// The popup's size.
    pub size: Vec2,
}

impl Request {
    /// `PopupPositionerExtensions.Update` for a `PlacementMode` ShareX's
    /// menus use, with the default constraint adjustment.
    pub fn placement(mode: PlacementMode, target: Rect, offset: Vec2, size: Vec2) -> Self {
        let (anchor_rect, anchor, gravity) = match mode {
            // `new Rect(position, new Size(1.0, 1.0))`, `TopLeft`, `BottomRight`.
            PlacementMode::Pointer(p) => (Rect::from_min_size(p, vec2(1.0, 1.0)), edge::TOP_LEFT, edge::BOTTOM_RIGHT),
            PlacementMode::BottomEdgeAlignedLeft => (target, edge::BOTTOM_LEFT, edge::BOTTOM_RIGHT),
            PlacementMode::BottomEdgeAlignedRight => (target, edge::BOTTOM_RIGHT, edge::BOTTOM_LEFT),
            PlacementMode::RightEdgeAlignedTop => (target, edge::TOP_RIGHT, edge::BOTTOM_RIGHT),
            PlacementMode::AnchorAndGravity(a, g) => (target, a, g),
        };
        Self { anchor_rect, anchor, gravity, adjustment: adjust::ALL, offset, size }
    }
}

/// Avalonia's `PlacementMode`s that ShareX's popups use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlacementMode {
    Pointer(Pos2),
    BottomEdgeAlignedLeft,
    BottomEdgeAlignedRight,
    RightEdgeAlignedTop,
    AnchorAndGravity(u8, u8),
}

/// `ManagedPopupPositioner.GetAnchorPoint`.
fn anchor_point(r: &PxRect, edge: u8) -> (f64, f64) {
    let x = if edge & edge::LEFT != 0 {
        r.x
    } else if edge & edge::RIGHT != 0 {
        r.right()
    } else {
        r.x + r.w / 2.0
    };
    let y = if edge & edge::TOP != 0 {
        r.y
    } else if edge & edge::BOTTOM != 0 {
        r.bottom()
    } else {
        r.y + r.h / 2.0
    };
    (x, y)
}

/// `ManagedPopupPositioner.Gravitate`.
fn gravitate((x, y): (f64, f64), (w, h): (f64, f64), gravity: u8) -> (f64, f64) {
    let dx = if gravity & edge::LEFT != 0 {
        -w
    } else if gravity & edge::RIGHT != 0 {
        0.0
    } else {
        -w / 2.0
    };
    let dy = if gravity & edge::TOP != 0 {
        -h
    } else if gravity & edge::BOTTOM != 0 {
        0.0
    } else {
        -h / 2.0
    };
    (x + dx, y + dy)
}

/// `ManagedPopupPositioner.Calculate`: where the popup goes on the
/// screen, in device pixels, before `MoveAndResize` truncates it.
pub fn calculate(g: &Geometry, req: &Request) -> PxRect {
    let s = g.scaling;
    let size = (req.size.x as f64 * s, req.size.y as f64 * s);
    let offset = (req.offset.x as f64 * s, req.offset.y as f64 * s);
    let parent = g.parent;
    // `anchorRect.Translate(parentGeometry.TopLeft)`.
    let a = req.anchor_rect;
    let anchor_rect = PxRect::new(a.min.x as f64 * s + parent.x, a.min.y as f64 * s + parent.y, a.width() as f64 * s, a.height() as f64 * s);
    let bounds = {
        // `GetBounds`: the screen under the anchor point, else one the
        // anchor touches, else the parent's.
        let p = anchor_point(&anchor_rect, req.anchor);
        let pp = anchor_point(&parent, req.anchor);
        let screen = g
            .screens
            .iter()
            .find(|s| s.bounds.contains_exclusive(p))
            .or_else(|| g.screens.iter().find(|s| s.bounds.intersects(&anchor_rect)))
            .or_else(|| g.screens.iter().find(|s| s.bounds.contains_exclusive(pp)))
            .or_else(|| g.screens.iter().find(|s| s.bounds.intersects(&parent)))
            .or_else(|| g.screens.first());
        match screen {
            Some(s) if s.working_area.w == 0.0 && s.working_area.h == 0.0 => s.bounds,
            Some(s) => s.working_area,
            None => PxRect::new(0.0, 0.0, f64::MAX, f64::MAX),
        }
    };
    let fits = |r: &PxRect, e: u8| {
        !((e & edge::LEFT != 0 && r.x < bounds.x)
            || (e & edge::TOP != 0 && r.y < bounds.y)
            || (e & edge::RIGHT != 0 && r.right() > bounds.right())
            || (e & edge::BOTTOM != 0 && r.bottom() > bounds.bottom()))
    };
    let unconstrained = |an: u8, gr: u8| {
        let (x, y) = gravitate(anchor_point(&anchor_rect, an), size, gr);
        PxRect::new(x + offset.0, y + offset.1, size.0, size.1)
    };
    let valid = |r: &PxRect| r.w > 0.0 && r.h > 0.0;
    let adj = req.adjustment;
    let mut rect = unconstrained(req.anchor, req.gravity);
    if !fits(&rect, edge::HORIZONTAL) && adj & adjust::FLIP_X != 0 {
        let flipped = unconstrained(edge::flip_x(req.anchor), edge::flip_x(req.gravity));
        if fits(&flipped, edge::HORIZONTAL) {
            rect.x = flipped.x;
        }
    }
    if adj & adjust::SLIDE_X != 0 {
        rect.x = rect.x.max(bounds.x);
        if rect.right() > bounds.right() {
            rect.x = bounds.right() - rect.w;
        }
    }
    if adj & adjust::RESIZE_X != 0 {
        let mut r = rect;
        if !fits(&r, edge::LEFT) {
            r.x = bounds.x;
        }
        if !fits(&r, edge::RIGHT) {
            // Avalonia's own: the width up to `bounds.Width`, not to
            // `bounds.Right` (the same on a screen at x 0).
            r.w = bounds.w - r.x;
        }
        if valid(&r) {
            rect = r;
        }
    }
    if !fits(&rect, edge::VERTICAL) && adj & adjust::FLIP_Y != 0 {
        let flipped = unconstrained(edge::flip_y(req.anchor), edge::flip_y(req.gravity));
        if fits(&flipped, edge::VERTICAL) {
            rect.y = flipped.y;
        }
    }
    if adj & adjust::SLIDE_Y != 0 {
        rect.y = rect.y.max(bounds.y);
        if rect.bottom() > bounds.bottom() {
            rect.y = bounds.bottom() - rect.h;
        }
    }
    if adj & adjust::RESIZE_Y != 0 {
        let mut r = rect;
        if !fits(&r, edge::TOP) {
            r.y = bounds.y;
        }
        if !fits(&r, edge::BOTTOM) {
            r.h = bounds.bottom() - r.y;
        }
        if valid(&r) {
            rect = r;
        }
    }
    rect
}

/// `(int)devicePoint.X`, forgiving the float error of a position that
/// came back from points (a whole pixel computed as 99.99999).
fn truncate(v: f64) -> i32 {
    let r = v.round();
    if (v - r).abs() < 1e-3 { r as i32 } else { v.trunc() as i32 }
}

/// `ManagedPopupPositioner.Update` and `MoveAndResize`: the popup's
/// top-left on the screen in whole device pixels, and its size in DIPs.
pub fn update(g: &Geometry, req: &Request) -> ((i32, i32), Vec2) {
    let r = calculate(g, req);
    ((truncate(r.x), truncate(r.y)), vec2((r.w / g.scaling) as f32, (r.h / g.scaling) as f32))
}

/// Where the positioner puts a popup, in the owner's points (relative to
/// its client area), against `g`.
pub fn position_in(g: &Geometry, req: &Request) -> Rect {
    let ((x, y), size) = update(g, req);
    let s = g.scaling;
    Rect::from_min_size(pos2(((x as f64 - g.parent.x) / s) as f32, ((y as f64 - g.parent.y) / s) as f32), size)
}

/// The geometry popups are placed against this frame: the host's screens,
/// or one screen that is the host's work area, or `window` (what the
/// caller keeps popups in without a host).
pub fn geometry(ctx: &Context, window: Rect) -> Geometry {
    let frame = ctx.data(|d| d.get_temp::<Frame>(key()));
    if let Some(g) = frame.as_ref().and_then(|f| f.screens.clone()) {
        return g;
    }
    Geometry::single(bounds(ctx, window), ctx.content_rect(), ctx.pixels_per_point())
}

/// [`position_in`] against this frame's [`geometry`].
pub fn position(ctx: &Context, req: &Request, window: Rect) -> Rect {
    position_in(&geometry(ctx, window), req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Order, pos2, vec2};

    #[test]
    fn bounds_are_the_window_without_a_host() {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(850.0, 550.0));
        assert_eq!(bounds(&ctx, window), window);
        assert!(!is_hosted(&ctx));
        let work = Rect::from_min_size(pos2(-686.0, -334.0), vec2(2048.0, 1232.0));
        set_host(&ctx, Some(work));
        assert!(is_hosted(&ctx));
        assert_eq!(bounds(&ctx, window), work);
        let layer = LayerId::new(Order::Foreground, Id::new("a"));
        register(&ctx, layer, window, 5.0);
        register(&ctx, layer, window.translate(vec2(1.0, 0.0)), 5.0);
        let taken = take(&ctx);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].rect.min.x, 1.0);
        assert!(take(&ctx).is_empty());
        set_host(&ctx, None);
        assert_eq!(bounds(&ctx, window), window);
        assert!(was_open(&ctx));
    }

    /// REG5-04b: with a light-dismiss popup open, a click outside it never
    /// reaches the widget under it (nor hovers it); the popup's own widgets
    /// still get theirs; with no popup the same click goes through.
    #[test]
    fn the_light_dismiss_overlay_takes_presses_outside_the_popup() {
        let ctx = Context::default();
        let below = Rect::from_min_size(pos2(10.0, 10.0), vec2(100.0, 40.0));
        let inside = Rect::from_min_size(pos2(300.0, 300.0), vec2(80.0, 40.0));
        let mut t = 0.0;
        // Clicks on (below, inside), and whether `below` was hovered.
        let mut run = |at: Pos2, open: bool| {
            let mut got = (false, false, false);
            // Two frames first: an area's first frame is egui's sizing
            // pass, and the hit test uses the frame before's widgets.
            for step in 0..5 {
                let mut events = if step < 2 { Vec::new() } else { vec![egui::Event::PointerMoved(at)] };
                if step == 3 || step == 4 {
                    events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: step == 3, modifiers: Default::default() });
                }
                t += 1.0 / 60.0;
                let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), time: Some(t), events, ..Default::default() };
                let mut out = ctx.run_ui(input, |ui| {
                    let r = ui.interact(below, Id::new("below"), egui::Sense::click());
                    got.0 |= r.clicked() || r.is_pointer_button_down_on();
                    got.2 |= r.hovered();
                    if open {
                        egui::Area::new(Id::new("popup")).order(Order::Foreground).fixed_pos(inside.min).show(ui.ctx(), |ui| {
                            light_dismiss_overlay(ui);
                            let (_, r) = ui.allocate_exact_size(inside.size(), egui::Sense::click());
                            got.1 |= r.clicked();
                        });
                    }
                });
                out.textures_delta.clear();
            }
            got
        };
        assert_eq!(run(below.center(), true), (false, false, false), "taken by the overlay");
        assert_eq!(run(inside.center(), true), (false, true, false), "the popup's own");
        assert_eq!(run(below.center(), false), (true, false, true), "no popup: through");
    }

    const PRIMARY: ScreenInfo =ScreenInfo { bounds: PxRect::new(0.0, 0.0, 1920.0, 1080.0), working_area: PxRect::new(0.0, 0.0, 1920.0, 1040.0) };
    const RIGHT: ScreenInfo = ScreenInfo { bounds: PxRect::new(1920.0, 0.0, 1280.0, 1024.0), working_area: PxRect::new(1920.0, 0.0, 1280.0, 984.0) };

    fn submenu(item: Rect, size: Vec2) -> Request {
        Request::placement(PlacementMode::RightEdgeAlignedTop, item, vec2(-4.0, 0.0), size)
    }

    #[test]
    fn the_screen_under_the_anchor_point_bounds_the_popup() {
        // The owner on the primary screen, its client ending at 1900: an item
        // whose right edge is on the second screen opens its submenu there
        // (the anchor point, TopRight, is at 1930), not flipped back.
        let g = Geometry { screens: vec![PRIMARY, RIGHT], parent: PxRect::new(1000.0, 100.0, 900.0, 700.0), scaling: 1.0 };
        let item = Rect::from_min_max(pos2(700.0, 50.0), pos2(930.0, 78.0));
        let ((x, y), size) = update(&g, &submenu(item, vec2(300.0, 900.0)));
        // On the second screen (1930 - 4), slid up to end at its work
        // area's bottom (it can't flip up: 178 - 900 < 0).
        assert_eq!((x, y), (1926, 984 - 900));
        assert_eq!(size, vec2(300.0, 900.0));
        // Taller than the work area: cut to it (ResizeY) after sliding.
        let ((_, y), size) = update(&g, &submenu(item, vec2(300.0, 1200.0)));
        assert_eq!((y, size.y), (0, 984.0));
        // A screen with no work area: its bounds (1000 + 70 fits 1080; a
        // 1040 work area would have flipped it up).
        let bare = ScreenInfo { bounds: PRIMARY.bounds, working_area: PxRect::new(0.0, 0.0, 0.0, 0.0) };
        let g = Geometry { screens: vec![bare], parent: PxRect::new(0.0, 0.0, 900.0, 700.0), scaling: 1.0 };
        let ((_, y), _) = update(&g, &submenu(Rect::from_min_max(pos2(0.0, 1000.0), pos2(200.0, 1028.0)), vec2(200.0, 70.0)));
        assert_eq!(y, 1000);
    }

    #[test]
    fn resize_x_keeps_avalonias_width_arithmetic() {
        // `rc2.WithWidth(bounds.Width - rc2.X)`: on a screen at x 1920 the
        // width would be negative, so the resize is dropped and the popup
        // keeps its width and where SlideX put it (ending at the screen's
        // right edge, so past its left one).
        let g = Geometry { screens: vec![RIGHT], parent: PxRect::new(1920.0, 0.0, 900.0, 700.0), scaling: 1.0 };
        let r = calculate(&g, &Request::placement(PlacementMode::BottomEdgeAlignedLeft, Rect::from_min_size(pos2(10.0, 10.0), vec2(50.0, 20.0)), Vec2::ZERO, vec2(1400.0, 100.0)));
        assert_eq!((r.x, r.w), (1800.0, 1400.0));
        // On the primary screen it's cut to the screen's width.
        let g = Geometry { screens: vec![PRIMARY], parent: PxRect::new(0.0, 0.0, 900.0, 700.0), scaling: 1.0 };
        let r = calculate(&g, &Request::placement(PlacementMode::BottomEdgeAlignedLeft, Rect::from_min_size(pos2(10.0, 10.0), vec2(50.0, 20.0)), Vec2::ZERO, vec2(2000.0, 100.0)));
        assert_eq!((r.x, r.w), (0.0, 1920.0));
    }

    #[test]
    fn without_screens_the_host_work_area_or_the_window_is_the_screen() {
        let ctx = Context::default();
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let item = Rect::from_min_max(pos2(100.0, 20.0), pos2(380.0, 48.0));
        // In the window: it can't flip left of the item (100 - 4 - 200 < 0),
        // so it slides to end at the window's right edge.
        let r = position(&ctx, &submenu(item, vec2(200.0, 100.0)), window);
        assert_eq!(r.min, pos2(200.0, 20.0));
        // With a host's work area: right of the item, past the window.
        set_host(&ctx, Some(Rect::from_min_max(pos2(-50.0, -50.0), pos2(1500.0, 900.0))));
        let r = position(&ctx, &submenu(item, vec2(200.0, 100.0)), window);
        assert_eq!(r.min, pos2(376.0, 20.0));
        // With the host's screens, those: the owner at 1700 on a 1920 wide
        // screen, so it flips left of the item (1800 - 4 - 200 = 1596).
        let g = Geometry { screens: vec![PRIMARY], parent: PxRect::new(1700.0, 0.0, 400.0, 300.0), scaling: 1.0 };
        set_host_screens(&ctx, Some(Rect::from_min_max(pos2(-1700.0, 0.0), pos2(220.0, 1040.0))), Some(g));
        let r = position(&ctx, &submenu(item, vec2(200.0, 100.0)), window);
        assert_eq!(r.min, pos2(100.0 - 4.0 - 200.0, 20.0));
    }
}
