//! `ShareX.ScreenCaptureLib/Helpers/WindowsRectangleList.cs`: the windows
//! and controls the region capture snaps to.

use crate::SimpleWindowInfo;

/// The windows and controls on screen, topmost first, without the windows
/// in `ignore` (the overlay's own). Controls are listed with
/// `include_children` (`RegionCaptureOptions.DetectControls`).
pub fn window_info_list(include_children: bool, ignore: &[isize]) -> Vec<SimpleWindowInfo> {
    crate::platform::window_list(include_children, ignore)
}

/// `GetWindowInfoList`'s last pass: a control or client area wholly inside
/// something already kept (above it) can't be pointed at, so it's dropped.
pub fn remove_hidden_controls(windows: Vec<SimpleWindowInfo>) -> Vec<SimpleWindowInfo> {
    let mut result: Vec<SimpleWindowInfo> = Vec::with_capacity(windows.len());
    for w in windows {
        if w.is_window || !result.iter().any(|kept| kept.rect.contains_rect(&w.rect)) {
            result.push(w);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    #[test]
    fn controls_covered_by_earlier_entries_are_dropped() {
        let w = |rect, is_window| SimpleWindowInfo { handle: 0, rect, is_window };
        let list = remove_hidden_controls(vec![
            w(Rect::new(0, 0, 100, 100), true),
            w(Rect::new(10, 10, 10, 10), false),
            w(Rect::new(200, 0, 10, 10), false),
            w(Rect::new(20, 20, 5, 5), true),
        ]);
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].rect, Rect::new(200, 0, 10, 10));
    }
}
