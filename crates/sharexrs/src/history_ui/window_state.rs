//! The history windows' `RememberWindowState` (`ApplySavedWindowState`,
//! `SaveWindowState`, `SaveNormalWindowSize`): `HelpersLib.WindowState` in
//! their settings, as Json.NET writes it (`"Location": "x, y"`,
//! `"Size": "w, h"`, `"IsMaximized"`). The size is the window's client size
//! in DIPs (`Bounds`, rounded), the location its position in pixels; an
//! empty one (`IsEmpty`: both 0) isn't applied, nor a location no screen
//! contains (`Screens.ScreenFromPoint`).

use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowState {
    pub location: (i32, i32),
    pub size: (i32, i32),
    pub maximized: bool,
}

/// `"x, y"` as `PointConverter`/`SizeConverter` write it.
fn pair(v: Option<&Value>) -> (i32, i32) {
    let s = v.and_then(Value::as_str).unwrap_or("");
    let mut it = s.split(',').map(|p| p.trim().parse::<i32>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0))
}

impl WindowState {
    pub fn from_json(v: &Value) -> Self {
        Self {
            location: pair(v.get("Location")),
            size: pair(v.get("Size")),
            maximized: v.get("IsMaximized").and_then(Value::as_bool).unwrap_or(false),
        }
    }

    pub fn to_json(self) -> Value {
        serde_json::json!({
            "Location": format!("{}, {}", self.location.0, self.location.1),
            "Size": format!("{}, {}", self.size.0, self.size.1),
            "IsMaximized": self.maximized,
        })
    }

    /// `Width`/`Height` from the saved size, else the window's own.
    pub fn size_or(&self, width: f32, height: f32) -> (f32, f32) {
        if self.size == (0, 0) { (width, height) } else { (self.size.0 as f32, self.size.1 as f32) }
    }

    /// `Position` from the saved location, when a screen contains it.
    pub fn position(&self, screens: &[sharex_capture::Rect]) -> Option<(i32, i32)> {
        let (x, y) = self.location;
        ((x, y) != (0, 0) && screens.iter().any(|s| s.contains(x, y))).then_some((x, y))
    }
}

/// What the window tracks while it's open.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tracker {
    pub saved: WindowState,
    /// The window is maximized now.
    pub maximized: bool,
    /// Its last client size (DIPs) and position while normal.
    pub size: Option<(f32, f32)>,
    pub location: Option<(i32, i32)>,
}

impl Tracker {
    pub fn new(v: &Value) -> Self {
        let saved = WindowState::from_json(v);
        Self { saved, maximized: saved.maximized, size: None, location: None }
    }

    /// `SaveNormalWindowSize`: only while the window is normal.
    pub fn normal_bounds(&mut self, size: (f32, f32), location: Option<(i32, i32)>) {
        if self.maximized || size.0 <= 0.0 || size.1 <= 0.0 {
            return;
        }
        self.size = Some(size);
        if location.is_some() {
            self.location = location;
        }
    }

    /// `SaveWindowState` into the settings' `WindowState`.
    pub fn save(&self) -> Value {
        let mut s = self.saved;
        if let Some((w, h)) = self.size {
            s.size = (w.round_ties_even() as i32, h.round_ties_even() as i32);
        }
        if let Some(l) = self.location {
            s.location = l;
        }
        s.maximized = self.maximized;
        s.to_json()
    }
}

/// The screens' bounds, for [`WindowState::position`].
pub fn screens() -> Vec<sharex_capture::Rect> {
    sharex_capture::screenshot::screens().iter().map(|s| s.bounds).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_saved_state_round_trips_as_sharex_writes_it() {
        let v = serde_json::json!({ "Location": "120, -8", "Size": "1300, 700", "IsMaximized": true });
        let s = WindowState::from_json(&v);
        assert_eq!(s, WindowState { location: (120, -8), size: (1300, 700), maximized: true });
        assert_eq!(s.to_json(), v);
        // ShareX's fresh state: nothing to apply.
        let fresh = WindowState::from_json(&serde_json::json!({ "Location": "0, 0", "Size": "0, 0", "IsMaximized": false }));
        assert_eq!(fresh.size_or(1300.0, 700.0), (1300.0, 700.0));
        assert_eq!(fresh.position(&[sharex_capture::Rect::new(0, 0, 1920, 1080)]), None);
    }

    #[test]
    fn a_location_off_every_screen_is_not_applied() {
        let s = WindowState { location: (3000, 200), size: (900, 600), maximized: false };
        assert_eq!(s.position(&[sharex_capture::Rect::new(0, 0, 1920, 1080)]), None);
        assert_eq!(s.position(&[sharex_capture::Rect::new(0, 0, 1920, 1080), sharex_capture::Rect::new(1920, 0, 1920, 1080)]), Some((3000, 200)));
        assert_eq!(s.size_or(1.0, 1.0), (900.0, 600.0));
    }

    #[test]
    fn only_a_normal_window_saves_its_bounds() {
        let mut t = Tracker::new(&WindowState { location: (10, 20), size: (800, 600), maximized: false }.to_json());
        t.normal_bounds((1000.4, 650.5), Some((30, 40)));
        t.maximized = true;
        t.normal_bounds((1920.0, 1040.0), Some((0, 0)));
        let s = WindowState::from_json(&t.save());
        assert_eq!(s, WindowState { location: (30, 40), size: (1000, 650), maximized: true });
    }
}
