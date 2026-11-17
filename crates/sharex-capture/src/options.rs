//! `ShareX.ScreenCaptureLib/RegionCaptureOptions.cs` and its enums
//! (`Enums.cs`), with ShareX's JSON names so they read and write the same
//! settings (`TaskSettings.CaptureSettings.RegionCaptureOptions`).

use serde::{Deserialize, Serialize};

/// `RegionCaptureAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionCaptureAction {
    None,
    CancelCapture,
    RemoveShapeCancelCapture,
    RemoveShape,
    SwapToolType,
    CaptureFullscreen,
    CaptureActiveMonitor,
    CaptureLastRegion,
}

/// `RegionCaptureMagnifierScrollAction`: what the mouse wheel does to the
/// magnifier (Ctrl does the other).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionCaptureMagnifierScrollAction {
    Resize,
    Zoom,
}

/// `ImageEditorToolbarItemOptions`: a toolbar item's place and keys, as the
/// toolbar customisation saves them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ImageEditorToolbarItemOptions {
    pub id: String,
    pub begin_group: Option<bool>,
    pub is_visible: bool,
    pub hotkey: String,
    pub secondary_hotkey: Option<String>,
}

/// `RegionCaptureOptions`, with ShareX's defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct RegionCaptureOptions {
    pub quick_capture: bool,
    pub multi_region_mode: bool,
    pub detect_windows: bool,
    pub detect_controls: bool,
    pub active_monitor_mode: bool,
    pub background_dim_strength: i32,
    pub dim_all_except_selection: bool,
    pub toolbar_items: Vec<ImageEditorToolbarItemOptions>,
    pub compact_toolbar: bool,
    pub use_custom_toolbar_button_size: bool,
    pub toolbar_button_size: i32,
    pub region_capture_action_right_click: RegionCaptureAction,
    pub region_capture_action_middle_click: RegionCaptureAction,
    #[serde(rename = "RegionCaptureActionX1Click")]
    pub region_capture_action_x1_click: RegionCaptureAction,
    #[serde(rename = "RegionCaptureActionX2Click")]
    pub region_capture_action_x2_click: RegionCaptureAction,
    pub magnifier_scroll_action: RegionCaptureMagnifierScrollAction,
    pub show_info: bool,
    pub use_custom_info_text: bool,
    /// Formats: `$x`, `$y`, `$r`, `$g`, `$b`, `$hex`, `$HEX`, `$n`.
    pub custom_info_text: String,
    pub show_magnifier: bool,
    pub use_square_magnifier: bool,
    pub magnifier_size: i32,
    /// Must be odd: 11, 13, 15...
    pub magnifier_pixel_count: i32,
    pub show_center_crosshair: bool,
    pub show_screen_crosshair: bool,
    /// Desktop position of the toolbar's top centre, in physical pixels,
    /// `"x, y"` in JSON. `0, 0` means centred on the active monitor.
    #[serde(with = "point_string")]
    pub toolbar_position: (i32, i32),
}

impl RegionCaptureOptions {
    pub const MINIMUM_SIZE: f64 = 5.0;
    /// Milliseconds before keys work (`InputDelay`).
    pub const INPUT_DELAY: u64 = 500;
    pub const MAGNIFIER_SIZE_MINIMUM: i32 = 50;
    pub const MAGNIFIER_SIZE_MAXIMUM: i32 = 500;
    pub const MAGNIFIER_PIXEL_COUNT_MINIMUM: i32 = 3;
    pub const MAGNIFIER_PIXEL_COUNT_MAXIMUM: i32 = 35;
    pub const MAGNIFIER_PIXEL_SIZE_MINIMUM: i32 = 3;
    pub const MOVE_SPEED_MINIMUM: i32 = 1;
    pub const MOVE_SPEED_MAXIMUM: i32 = 10;
    pub const TOOLBAR_BUTTON_SIZE_MINIMUM: i32 = 16;
    pub const TOOLBAR_BUTTON_SIZE_MAXIMUM: i32 = 96;
}

impl Default for RegionCaptureOptions {
    fn default() -> Self {
        Self {
            quick_capture: true,
            multi_region_mode: false,
            detect_windows: true,
            detect_controls: true,
            active_monitor_mode: false,
            background_dim_strength: 20,
            dim_all_except_selection: false,
            toolbar_items: Vec::new(),
            compact_toolbar: false,
            use_custom_toolbar_button_size: false,
            toolbar_button_size: 32,
            region_capture_action_right_click: RegionCaptureAction::RemoveShapeCancelCapture,
            region_capture_action_middle_click: RegionCaptureAction::SwapToolType,
            region_capture_action_x1_click: RegionCaptureAction::CaptureFullscreen,
            region_capture_action_x2_click: RegionCaptureAction::CaptureActiveMonitor,
            magnifier_scroll_action: RegionCaptureMagnifierScrollAction::Resize,
            show_info: true,
            use_custom_info_text: false,
            custom_info_text: "X: $x, Y: $y$nR: $r, G: $g, B: $b$nHex: $hex".into(),
            show_magnifier: true,
            use_square_magnifier: false,
            magnifier_size: 150,
            magnifier_pixel_count: 15,
            show_center_crosshair: true,
            show_screen_crosshair: false,
            toolbar_position: (0, 0),
        }
    }
}

/// `System.Drawing.Point` as Json.NET writes it: `"x, y"`.
mod point_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(p: &(i32, i32), s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{}, {}", p.0, p.1))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<(i32, i32), D::Error> {
        let s = String::deserialize(d)?;
        let mut it = s.split(',').map(|v| v.trim().parse::<i32>().unwrap_or(0));
        Ok((it.next().unwrap_or(0), it.next().unwrap_or(0)))
    }
}
