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

use egui::{Context, Id, LayerId, Rect};

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
    layers: Vec<PopupLayer>,
}

fn key() -> Id {
    Id::new("sharex-ui-popups")
}

/// Called by the host at the start of each frame: popups may go anywhere
/// on `work_area` (the owner's screen's work area, in the owner's points),
/// or, with `None`, must stay inside the window.
pub fn set_host(ctx: &Context, work_area: Option<Rect>) {
    ctx.data_mut(|d| d.insert_temp(key(), Frame { host: work_area, layers: Vec::new() }));
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

/// A popup drawn on `layer` at `rect` this frame.
pub fn register(ctx: &Context, layer: LayerId, rect: Rect, corner_radius: f32) {
    ctx.data_mut(|d| {
        let f = d.get_temp_mut_or_default::<Frame>(key());
        f.layers.retain(|l| l.layer != layer);
        f.layers.push(PopupLayer { layer, rect, corner_radius });
    });
}

/// The popups drawn this frame (the host, after the frame's UI).
pub fn take(ctx: &Context) -> Vec<PopupLayer> {
    ctx.data_mut(|d| std::mem::take(&mut d.get_temp_mut_or_default::<Frame>(key()).layers))
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
    }
}
