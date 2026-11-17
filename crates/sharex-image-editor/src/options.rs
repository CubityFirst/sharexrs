//! `ShareX.ImageEditor.Integration.ImageEditorOptions`: the editor's tool
//! settings, which ShareX keeps in `TaskSettings.ToolsSettings.
//! ImageEditorOptions` (Newtonsoft; `sharex-settings` holds it as JSON,
//! [`ImageEditorOptions::from_value`] reads it).

use serde_json::Value;
use sharex_settings::settings;

use crate::model::{
    ArrowStyle, BorderStyle, CursorType, EditorTool, StepType, TextHorizontalAlignment,
};

/// `ImageEditorOptions.PrimaryColor` (`#FFF23C3C`).
pub const PRIMARY_COLOR: &str = "#FFF23C3C";
/// `ImageEditorOptions.SecondaryColor` (`#FFFAFAFA`).
pub const SECONDARY_COLOR: &str = "#FFFAFAFA";
/// `Colors.Transparent` as `ColorToHex` writes it.
pub const TRANSPARENT: &str = "#00FFFFFF";

settings! {
    /// `ImageEditorOptions`, field for field.
    pub struct ImageEditorOptions {
        LastUsedAnnotationTool: EditorTool = EditorTool::Rectangle,
        RememberWindowState: bool = true,
        IsWindowMaximized: bool = true,
        WindowWidth: f64 = 1280.0,
        WindowHeight: f64 = 720.0,
        WindowX: Option<i32> = None,
        WindowY: Option<i32> = None,
        AutoSizeWindow: bool = false,
        ShowExitConfirmation: bool = true,
        ZoomToFitOnOpen: bool = false,
        QuickCrop: bool = true,
        RightClickDeletesShape: bool = false,
        AutoCloseEditorOnTask: bool = false,
        AutoCopyImageToClipboard: bool = false,
        ShowInsertImageDialog: bool = true,
        ShowNotifications: bool = true,
        AutoExpandCanvas: bool = true,
        CanvasFillColorHex: String = "#00000000".into(),
        /// `List<ImageEditorToolbarItemOptions>` (not ported yet).
        ToolbarItems: Vec<Value> = Vec::new(),

        // Shared
        BorderColorHex: String = PRIMARY_COLOR.into(),
        FillColorHex: String = TRANSPARENT.into(),
        Thickness: i32 = 4,
        CornerRadius: i32 = 4,
        Shadow: bool = false,
        ShadowColorHex: String = crate::model::DEFAULT_SHADOW_COLOR_HEX.into(),
        ShadowBlurRadius: f64 = crate::model::DEFAULT_SHADOW_BLUR_RADIUS,
        ShadowOpacity: f64 = crate::model::DEFAULT_SHADOW_OPACITY,
        ShadowOffsetX: f64 = crate::model::DEFAULT_SHADOW_OFFSET_X,
        ShadowOffsetY: f64 = crate::model::DEFAULT_SHADOW_OFFSET_Y,
        BorderStyle: BorderStyle = BorderStyle::Solid,
        ArrowStyle: ArrowStyle = ArrowStyle::Classic,
        ArrowRoundedCorners: bool = true,
        FreehandArrow: bool = false,
        CursorType: CursorType = CursorType::Default,

        // Text
        TextBorderColorHex: String = PRIMARY_COLOR.into(),
        TextTextColorHex: String = SECONDARY_COLOR.into(),
        TextThickness: i32 = 8,
        TextFontSize: f32 = 48.0,
        TextFontFamily: String = "Segoe UI".into(),
        TextHorizontalAlignment: TextHorizontalAlignment = TextHorizontalAlignment::Center,
        TextBold: bool = true,
        TextItalic: bool = false,

        // Speech balloon
        SpeechBalloonBorderColorHex: String = TRANSPARENT.into(),
        SpeechBalloonFillColorHex: String = PRIMARY_COLOR.into(),
        SpeechBalloonTextColorHex: String = SECONDARY_COLOR.into(),
        SpeechBalloonThickness: i32 = 4,
        SpeechBalloonFontSize: f32 = 48.0,
        SpeechBalloonFontFamily: String = "Segoe UI".into(),
        SpeechBalloonTextHorizontalAlignment: TextHorizontalAlignment = TextHorizontalAlignment::Center,
        SpeechBalloonTextBold: bool = true,
        SpeechBalloonTextItalic: bool = false,
        SpeechBalloonTail: bool = true,
        SpeechBalloonAutoSize: bool = false,

        // Step
        StepBorderColorHex: String = TRANSPARENT.into(),
        StepFillColorHex: String = PRIMARY_COLOR.into(),
        StepTextColorHex: String = SECONDARY_COLOR.into(),
        StepThickness: i32 = 4,
        StepFontSize: f32 = 30.0,
        StepTextBold: bool = true,
        StepType: StepType = StepType::Numeric,
        StepTail: bool = true,

        // Highlight
        HighlightFillColorHex: String = "#FFFFFF00".into(),

        // Effects
        BlurStrength: f32 = 30.0,
        PixelateStrength: f32 = 20.0,
        MagnifierStrength: f32 = 2.0,
        MagnifierEllipse: bool = false,
        MagnifierBorderColorHex: String = TRANSPARENT.into(),
        MagnifierThickness: i32 = 4,
        MagnifierBorderStyle: BorderStyle = BorderStyle::Solid,
        SpotlightStrength: f32 = 30.0,
        SpotlightBlur: f32 = 0.0,
        SpotlightEllipse: bool = false,

        // Background
        BackgroundMargin: f64 = 80.0,
        BackgroundPadding: f64 = 40.0,
        BackgroundSmartPadding: bool = true,
        BackgroundRoundedCorner: f64 = 20.0,
        BackgroundShadowRadius: f64 = 30.0,
        BackgroundType: String = "Transparent".into(),
        BackgroundGradientColor1Hex: String = "#FFDD2476".into(),
        BackgroundGradientColor2Hex: String = "#FFFF512F".into(),
        BackgroundColorHex: String = "#FF222222".into(),
        BackgroundImagePath: String = String::new(),

        // Recent image files
        RecentImageFiles: Vec<String> = Vec::new(),
        MaxRecentImageFiles: i32 = 10,

        // Image effects
        RecentEffects: Vec<String> = Vec::new(),
        FavoriteEffects: Vec<String> = DEFAULT_FAVORITE_EFFECTS.iter().map(|s| s.to_string()).collect(),
    }
}

/// `ImageEditorOptions.DefaultFavoriteEffects`.
pub const DEFAULT_FAVORITE_EFFECTS: &[&str] = &[
    "resize_image",
    "resize_canvas",
    "crop_image",
    "auto_crop_image",
    "rotate_90_clockwise",
    "rotate_90_counter_clockwise",
    "rotate_180",
    "flip_horizontal",
    "flip_vertical",
];

impl ImageEditorOptions {
    /// From the JSON `sharex-settings` keeps (`TaskSettingsTools::
    /// ImageEditorOptions`), leniently as ShareX reads it.
    pub fn from_value(v: &Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}
