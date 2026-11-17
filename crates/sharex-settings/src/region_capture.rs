//! `ShareX.ScreenCaptureLib/RegionCaptureOptions.cs`.

use serde_json::Value;

use crate::enums::{RegionCaptureAction, RegionCaptureMagnifierScrollAction};
use crate::settings;
use crate::types::Point;

/// `RegionCaptureOptions.MinimumSize` and the other limits it declares.
pub mod limits {
    pub const MINIMUM_SIZE: i32 = 5;
    pub const INPUT_DELAY: i32 = 500;
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

settings! {
    /// `ShareX.ScreenCaptureLib.RegionCaptureOptions`.
    pub struct RegionCaptureOptions {
        QuickCapture: bool = true,
        MultiRegionMode: bool = false,
        DetectWindows: bool = true,
        DetectControls: bool = true,
        ActiveMonitorMode: bool = false,
        BackgroundDimStrength: i32 = 20,
        DimAllExceptSelection: bool = false,
        /// `List<ImageEditorToolbarItemOptions>` (not ported yet).
        ToolbarItems: Vec<Value> = Vec::new(),
        CompactToolbar: bool = false,
        /// When on, the toolbar is scaled so its buttons are this many pixels tall.
        UseCustomToolbarButtonSize: bool = false,
        ToolbarButtonSize: i32 = 32,
        RegionCaptureActionRightClick: RegionCaptureAction = RegionCaptureAction::RemoveShapeCancelCapture,
        RegionCaptureActionMiddleClick: RegionCaptureAction = RegionCaptureAction::SwapToolType,
        RegionCaptureActionX1Click: RegionCaptureAction = RegionCaptureAction::CaptureFullscreen,
        RegionCaptureActionX2Click: RegionCaptureAction = RegionCaptureAction::CaptureActiveMonitor,
        /// What the mouse wheel does to the magnifier; Ctrl+wheel does the other.
        MagnifierScrollAction: RegionCaptureMagnifierScrollAction = RegionCaptureMagnifierScrollAction::Resize,
        ShowInfo: bool = true,
        UseCustomInfoText: bool = false,
        /// Formats: $x, $y, $r, $g, $b, $hex, $HEX, $n.
        CustomInfoText: String = "X: $x, Y: $y$nR: $r, G: $g, B: $b$nHex: $hex".into(),
        ShowMagnifier: bool = true,
        UseSquareMagnifier: bool = false,
        MagnifierSize: i32 = 150,
        /// Must be odd (11, 13, 15, ...).
        MagnifierPixelCount: i32 = 15,
        ShowCenterCrosshair: bool = true,
        ShowScreenCrosshair: bool = false,
        /// The toolbar's top centre on the desktop, in physical pixels;
        /// empty centres it on the active monitor.
        ToolbarPosition: Point = Point::default(),
    }
}
