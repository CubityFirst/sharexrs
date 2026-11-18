//! The annotation model of `ShareX.ImageEditor/Core/Annotations`: every
//! annotation type with ShareX's properties and their defaults.
//!
//! ShareX doesn't save annotations, but marks `Annotation` up for
//! System.Text.Json (`[JsonDerivedType(..., "Rectangle")]`, ...); this
//! model reads and writes that shape: `{"$type": "Rectangle", "StrokeColor":
//! ..., "StartPoint": {"X": .., "Y": ..}}`, property names as ShareX's,
//! missing properties taking the type's defaults (its constructor's).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sharex_settings::dotnet_enum;

dotnet_enum! {
    /// `EditorTool`.
    pub enum EditorTool {
        Select, Rectangle, Ellipse, Line, Arrow, Freehand, Text, SpeechBalloon, Step, Image, Clone, Emoji,
        Cursor, Highlight, SmartEraser, Blur, Pixelate, Magnify, Spotlight, Crop, CutOut,
    }
}

dotnet_enum! {
    /// `BorderStyle`.
    pub enum BorderStyle { Solid, Dash, Dot, DashDot, DashDotDot }
}

dotnet_enum! {
    /// `ArrowStyle`.
    pub enum ArrowStyle { Classic, Modern, Double, Basic, Line }
}

dotnet_enum! {
    /// `TextHorizontalAlignment`.
    pub enum TextHorizontalAlignment { Left, Center, Right }
}

dotnet_enum! {
    /// `StepType`.
    pub enum StepType { Numeric, UppercaseLetter, LowercaseLetter, UppercaseRoman, LowercaseRoman }
}

dotnet_enum! {
    /// `SmartEraserFillMode`.
    pub enum SmartEraserFillMode { SolidColor, StretchHorizontally, StretchVertically }
}

dotnet_enum! {
    /// `CursorType`.
    pub enum CursorType {
        AppStarting, Arrow, Cross, Default, Hand, Help, HSplit, IBeam, No, NoMove2D, NoMoveHoriz, NoMoveVert,
        PanEast, PanNE, PanNorth, PanNW, PanSE, PanSouth, PanSW, PanWest, SizeAll, SizeNESW, SizeNS,
        SizeNWSE, SizeWE, UpArrow, VSplit, WaitCursor,
    }
}

/// `SKPoint`, as `{"X": .., "Y": ..}`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// `Annotation`: what every annotation has.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Common {
    pub id: String,
    /// The editor's handle on this annotation object (ShareX's editor holds
    /// the object itself): unique per instance, not serialised. Clones keep
    /// ShareX's `Id` (`Annotation.Clone`), so the editor can't go by it.
    #[serde(skip)]
    pub key: Key,
    pub tool_type: EditorTool,
    pub stroke_color: String,
    pub stroke_width: f32,
    pub fill_color: String,
    pub shadow_enabled: bool,
    pub shadow_color: String,
    pub shadow_blur_radius: f64,
    pub shadow_opacity: f64,
    pub shadow_offset_x: f64,
    pub shadow_offset_y: f64,
    pub start_point: Point,
    pub end_point: Point,
    pub is_selected: bool,
    pub z_index: i32,
    pub rotation_angle: f32,
}

/// An annotation object's identity in the editor ([`Common::key`]): a new
/// one for every annotation made or read, copied by `clone` (an undo
/// snapshot is the same object again). It never makes two annotations
/// unequal: `==` compares what ShareX serialises.
#[derive(Clone, Debug)]
pub struct Key(pub String);

impl Key {
    pub fn fresh() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Key(format!("k{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)))
    }
}

impl Default for Key {
    fn default() -> Self {
        Self::fresh()
    }
}

impl PartialEq for Key {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

pub const DEFAULT_SHADOW_COLOR_HEX: &str = "#80000000";
pub const DEFAULT_SHADOW_BLUR_RADIUS: f64 = 4.0;
pub const DEFAULT_SHADOW_OPACITY: f64 = 1.0;
pub const DEFAULT_SHADOW_OFFSET_X: f64 = 3.0;
pub const DEFAULT_SHADOW_OFFSET_Y: f64 = 3.0;

impl Default for Common {
    fn default() -> Self {
        Self {
            id: String::new(),
            key: Key::fresh(),
            tool_type: EditorTool::Select,
            stroke_color: "#ef4444".into(),
            stroke_width: 4.0,
            fill_color: "#00000000".into(),
            shadow_enabled: false,
            shadow_color: DEFAULT_SHADOW_COLOR_HEX.into(),
            shadow_blur_radius: DEFAULT_SHADOW_BLUR_RADIUS,
            shadow_opacity: DEFAULT_SHADOW_OPACITY,
            shadow_offset_x: DEFAULT_SHADOW_OFFSET_X,
            shadow_offset_y: DEFAULT_SHADOW_OFFSET_Y,
            start_point: Point::default(),
            end_point: Point::default(),
            is_selected: false,
            z_index: 0,
            rotation_angle: 0.0,
        }
    }
}

/// `RectangleAnnotation` (and `SmartEraserAnnotation`, which derives from it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct RectangleProps {
    pub border_style: BorderStyle,
    pub corner_radius: i32,
}

impl Default for RectangleProps {
    fn default() -> Self {
        Self {
            border_style: BorderStyle::Solid,
            corner_radius: 0,
        }
    }
}

/// `EllipseAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct EllipseProps {
    pub border_style: BorderStyle,
}

impl Default for EllipseProps {
    fn default() -> Self {
        Self {
            border_style: BorderStyle::Solid,
        }
    }
}

/// `LineAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LineProps {
    pub border_style: BorderStyle,
    pub curve_point: Point,
    pub curve_point_activated: bool,
}

impl Default for LineProps {
    fn default() -> Self {
        Self {
            border_style: BorderStyle::Solid,
            curve_point: Point::default(),
            curve_point_activated: false,
        }
    }
}

/// `ArrowAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ArrowProps {
    pub curve_point: Point,
    pub curve_point_activated: bool,
    pub rounded_corners: bool,
    pub style: ArrowStyle,
}

impl Default for ArrowProps {
    fn default() -> Self {
        Self {
            curve_point: Point::default(),
            curve_point_activated: false,
            rounded_corners: true,
            style: ArrowStyle::Classic,
        }
    }
}

/// `FreehandAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct FreehandProps {
    pub points: Vec<Point>,
    pub border_style: BorderStyle,
    pub show_arrow_head: bool,
    pub smoothing_tolerance: f32,
}

impl Default for FreehandProps {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            border_style: BorderStyle::Solid,
            show_arrow_head: false,
            smoothing_tolerance: 2.0,
        }
    }
}

/// `BaseEffectAnnotation`: highlight, blur, pixelate (and magnify).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct EffectProps {
    pub amount: f32,
    pub is_freehand: bool,
    pub is_ellipse: bool,
}

impl Default for EffectProps {
    fn default() -> Self {
        Self {
            amount: 10.0,
            is_freehand: false,
            is_ellipse: false,
        }
    }
}

/// `MagnifyAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MagnifyProps {
    pub amount: f32,
    pub is_freehand: bool,
    pub is_ellipse: bool,
    pub border_style: BorderStyle,
}

impl Default for MagnifyProps {
    fn default() -> Self {
        Self {
            amount: 2.0,
            is_freehand: false,
            is_ellipse: false,
            border_style: BorderStyle::Solid,
        }
    }
}

/// `SpotlightAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SpotlightProps {
    pub darken_opacity: u8,
    pub blur_amount: f32,
    pub is_ellipse: bool,
}

impl Default for SpotlightProps {
    fn default() -> Self {
        Self {
            darken_opacity: 180,
            blur_amount: 0.0,
            is_ellipse: false,
        }
    }
}

/// `NumberAnnotation` (the Step tool).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct NumberProps {
    pub number: i32,
    pub step_type: StepType,
    pub font_size: f32,
    pub text_color: String,
    pub is_bold: bool,
    pub tail_point: Point,
    pub tail_point_initialized: bool,
    pub tail_enabled: bool,
}

impl Default for NumberProps {
    fn default() -> Self {
        Self {
            number: 1,
            step_type: StepType::Numeric,
            font_size: 24.0,
            text_color: "#FFFFFFFF".into(),
            is_bold: true,
            tail_point: Point::default(),
            tail_point_initialized: false,
            tail_enabled: true,
        }
    }
}

/// `TextAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct TextProps {
    pub text: String,
    pub text_color: String,
    pub font_size: f32,
    pub font_family: String,
    pub is_bold: bool,
    pub is_italic: bool,
    pub horizontal_alignment: TextHorizontalAlignment,
}

impl Default for TextProps {
    fn default() -> Self {
        Self {
            text: String::new(),
            text_color: "#FF000000".into(),
            font_size: 48.0,
            font_family: "Segoe UI".into(),
            is_bold: false,
            is_italic: false,
            horizontal_alignment: TextHorizontalAlignment::Center,
        }
    }
}

/// `SpeechBalloonAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SpeechBalloonProps {
    pub tail_point: Point,
    pub tail_point_initialized: bool,
    pub tail_point_placed: bool,
    pub tail_enabled: bool,
    pub auto_size: bool,
    pub text: String,
    pub font_size: f32,
    pub font_family: String,
    pub text_color: String,
    pub is_bold: bool,
    pub is_italic: bool,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub corner_radius: i32,
}

impl Default for SpeechBalloonProps {
    fn default() -> Self {
        Self {
            tail_point: Point::default(),
            tail_point_initialized: false,
            tail_point_placed: false,
            tail_enabled: true,
            auto_size: false,
            text: String::new(),
            font_size: 20.0,
            font_family: "Segoe UI".into(),
            text_color: "#FF000000".into(),
            is_bold: false,
            is_italic: false,
            horizontal_alignment: TextHorizontalAlignment::Center,
            corner_radius: 10,
        }
    }
}

/// `ImageAnnotation` (an inserted image, or a region cloned with the
/// Clone tool, whose `ToolType` is `Clone`).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ImageProps {
    pub image_path: String,
    /// `ImageBitmap`: the image the annotation shows (`LoadImage`,
    /// `SetImage`). ShareX doesn't serialise it, so JSON leaves it empty
    /// and nothing is drawn until it's set.
    #[serde(skip)]
    pub bitmap: Option<std::sync::Arc<sharex_skia::Pixmap>>,
}

impl ImageProps {
    /// `SetImage`.
    pub fn set_image(&mut self, bitmap: sharex_skia::Pixmap) {
        self.bitmap = Some(std::sync::Arc::new(bitmap));
    }
}

/// `EmojiAnnotation`.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct EmojiProps {
    pub unicode_sequence: String,
    pub display_name: String,
}

/// `CursorAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct CursorProps {
    pub cursor_type: CursorType,
}

impl Default for CursorProps {
    fn default() -> Self {
        Self {
            cursor_type: CursorType::Default,
        }
    }
}

/// `SmartEraserAnnotation`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SmartEraserProps {
    pub border_style: BorderStyle,
    pub corner_radius: i32,
    pub fill_mode: SmartEraserFillMode,
    pub edge_pixels: Option<Vec<u32>>,
}

impl Default for SmartEraserProps {
    fn default() -> Self {
        Self {
            border_style: BorderStyle::Solid,
            corner_radius: 0,
            fill_mode: SmartEraserFillMode::SolidColor,
            edge_pixels: None,
        }
    }
}

/// The type-specific part of an annotation (the `$type` discriminators
/// of `Annotation`'s `[JsonDerivedType]`s).
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Arrow(ArrowProps),
    Blur(EffectProps),
    Crop,
    Cursor(CursorProps),
    Ellipse(EllipseProps),
    Emoji(EmojiProps),
    Freehand(FreehandProps),
    Highlight(EffectProps),
    Image(ImageProps),
    Line(LineProps),
    Magnify(MagnifyProps),
    Number(NumberProps),
    Pixelate(EffectProps),
    Rectangle(RectangleProps),
    SmartEraser(SmartEraserProps),
    SpeechBalloon(SpeechBalloonProps),
    Spotlight(SpotlightProps),
    Text(TextProps),
}

impl Kind {
    pub fn type_name(&self) -> &'static str {
        match self {
            Kind::Arrow(_) => "Arrow",
            Kind::Blur(_) => "Blur",
            Kind::Crop => "Crop",
            Kind::Cursor(_) => "Cursor",
            Kind::Ellipse(_) => "Ellipse",
            Kind::Emoji(_) => "Emoji",
            Kind::Freehand(_) => "Freehand",
            Kind::Highlight(_) => "Highlight",
            Kind::Image(_) => "Image",
            Kind::Line(_) => "Line",
            Kind::Magnify(_) => "Magnify",
            Kind::Number(_) => "Number",
            Kind::Pixelate(_) => "Pixelate",
            Kind::Rectangle(_) => "Rectangle",
            Kind::SmartEraser(_) => "SmartEraser",
            Kind::SpeechBalloon(_) => "SpeechBalloon",
            Kind::Spotlight(_) => "Spotlight",
            Kind::Text(_) => "Text",
        }
    }
}

/// An annotation: `Annotation`'s properties and its type's.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub common: Common,
    pub kind: Kind,
}

impl Annotation {
    /// A new annotation of the type named `type_name` (a `$type`), with
    /// the defaults its ShareX constructor sets.
    pub fn new(type_name: &str) -> Option<Self> {
        let mut c = Common::default();
        let kind = match type_name {
            "Arrow" => {
                c.tool_type = EditorTool::Arrow;
                Kind::Arrow(ArrowProps::default())
            }
            "Blur" => {
                c.tool_type = EditorTool::Blur;
                c.stroke_color = "#00000000".into();
                c.stroke_width = 0.0;
                Kind::Blur(EffectProps {
                    amount: 10.0,
                    ..Default::default()
                })
            }
            "Crop" => {
                c.tool_type = EditorTool::Crop;
                Kind::Crop
            }
            "Cursor" => {
                c.tool_type = EditorTool::Cursor;
                c.stroke_width = 0.0;
                Kind::Cursor(CursorProps::default())
            }
            "Ellipse" => {
                c.tool_type = EditorTool::Ellipse;
                Kind::Ellipse(EllipseProps::default())
            }
            "Emoji" => {
                c.tool_type = EditorTool::Emoji;
                Kind::Emoji(EmojiProps::default())
            }
            "Freehand" => {
                c.tool_type = EditorTool::Freehand;
                Kind::Freehand(FreehandProps::default())
            }
            "Highlight" => {
                c.tool_type = EditorTool::Highlight;
                c.stroke_color = "Transparent".into();
                c.fill_color = "#FFFF00".into();
                c.stroke_width = 0.0;
                Kind::Highlight(EffectProps::default())
            }
            "Image" => {
                c.tool_type = EditorTool::Image;
                c.stroke_width = 0.0;
                Kind::Image(ImageProps::default())
            }
            "Line" => {
                c.tool_type = EditorTool::Line;
                Kind::Line(LineProps::default())
            }
            "Magnify" => {
                c.tool_type = EditorTool::Magnify;
                c.stroke_color = "#00000000".into();
                c.stroke_width = 0.0;
                Kind::Magnify(MagnifyProps::default())
            }
            "Number" => {
                c.tool_type = EditorTool::Step;
                Kind::Number(NumberProps::default())
            }
            "Pixelate" => {
                c.tool_type = EditorTool::Pixelate;
                c.stroke_color = "#00000000".into();
                c.stroke_width = 0.0;
                Kind::Pixelate(EffectProps {
                    amount: 10.0,
                    ..Default::default()
                })
            }
            "Rectangle" => {
                c.tool_type = EditorTool::Rectangle;
                Kind::Rectangle(RectangleProps::default())
            }
            "SmartEraser" => {
                c.tool_type = EditorTool::SmartEraser;
                c.stroke_color = "#80FF0000".into();
                c.fill_color = "#80FF0000".into();
                c.stroke_width = 0.0;
                Kind::SmartEraser(SmartEraserProps::default())
            }
            "SpeechBalloon" => {
                c.tool_type = EditorTool::SpeechBalloon;
                c.stroke_width = 2.0;
                c.stroke_color = "#FF000000".into();
                c.fill_color = "#FFFFFFFF".into();
                Kind::SpeechBalloon(SpeechBalloonProps::default())
            }
            "Spotlight" => {
                c.tool_type = EditorTool::Spotlight;
                Kind::Spotlight(SpotlightProps::default())
            }
            "Text" => {
                c.tool_type = EditorTool::Text;
                Kind::Text(TextProps::default())
            }
            _ => return None,
        };
        Some(Self { common: c, kind })
    }

    /// Reads ShareX's JSON for an annotation (System.Text.Json's names,
    /// matched case-insensitively as the harness reads them).
    pub fn from_json(v: &Value) -> Result<Self, String> {
        let obj = v.as_object().ok_or("an annotation must be an object")?;
        let ty = obj.get("$type").and_then(Value::as_str).ok_or("no $type")?;
        let mut a = Annotation::new(ty).ok_or_else(|| format!("unknown annotation type {ty}"))?;
        a.common = overlay(&a.common, obj)?;
        a.kind = match &a.kind {
            Kind::Arrow(p) => Kind::Arrow(overlay(p, obj)?),
            Kind::Blur(p) => Kind::Blur(overlay(p, obj)?),
            Kind::Crop => Kind::Crop,
            Kind::Cursor(p) => Kind::Cursor(overlay(p, obj)?),
            Kind::Ellipse(p) => Kind::Ellipse(overlay(p, obj)?),
            Kind::Emoji(p) => Kind::Emoji(overlay(p, obj)?),
            Kind::Freehand(p) => Kind::Freehand(overlay(p, obj)?),
            Kind::Highlight(p) => Kind::Highlight(overlay(p, obj)?),
            Kind::Image(p) => Kind::Image(overlay(p, obj)?),
            Kind::Line(p) => Kind::Line(overlay(p, obj)?),
            Kind::Magnify(p) => Kind::Magnify(overlay(p, obj)?),
            Kind::Number(p) => Kind::Number(overlay(p, obj)?),
            Kind::Pixelate(p) => Kind::Pixelate(overlay(p, obj)?),
            Kind::Rectangle(p) => Kind::Rectangle(overlay(p, obj)?),
            Kind::SmartEraser(p) => Kind::SmartEraser(overlay(p, obj)?),
            Kind::SpeechBalloon(p) => Kind::SpeechBalloon(overlay(p, obj)?),
            Kind::Spotlight(p) => Kind::Spotlight(overlay(p, obj)?),
            Kind::Text(p) => Kind::Text(overlay(p, obj)?),
        };
        Ok(a)
    }

    /// ShareX's JSON for this annotation.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("$type".into(), Value::String(self.kind.type_name().into()));
        let mut put = |v: Value| {
            if let Value::Object(o) = v {
                m.extend(o);
            }
        };
        put(serde_json::to_value(&self.common).unwrap_or_default());
        put(match &self.kind {
            Kind::Arrow(p) => serde_json::to_value(p),
            Kind::Blur(p) | Kind::Highlight(p) | Kind::Pixelate(p) => serde_json::to_value(p),
            Kind::Crop => Ok(Value::Object(Map::new())),
            Kind::Cursor(p) => serde_json::to_value(p),
            Kind::Ellipse(p) => serde_json::to_value(p),
            Kind::Emoji(p) => serde_json::to_value(p),
            Kind::Freehand(p) => serde_json::to_value(p),
            Kind::Image(p) => serde_json::to_value(p),
            Kind::Line(p) => serde_json::to_value(p),
            Kind::Magnify(p) => serde_json::to_value(p),
            Kind::Number(p) => serde_json::to_value(p),
            Kind::Rectangle(p) => serde_json::to_value(p),
            Kind::SmartEraser(p) => serde_json::to_value(p),
            Kind::SpeechBalloon(p) => serde_json::to_value(p),
            Kind::Spotlight(p) => serde_json::to_value(p),
            Kind::Text(p) => serde_json::to_value(p),
        }
        .unwrap_or_default());
        Value::Object(m)
    }
}

/// `default`'s JSON with the properties `obj` sets (names matched
/// ignoring case), read back.
fn overlay<T: Serialize + for<'de> Deserialize<'de>>(
    default: &T,
    obj: &Map<String, Value>,
) -> Result<T, String> {
    let Value::Object(mut base) = serde_json::to_value(default).map_err(|e| e.to_string())? else {
        return Err("not an object".into());
    };
    for (k, v) in base.iter_mut() {
        if let Some((_, nv)) = obj.iter().find(|(ok, _)| ok.eq_ignore_ascii_case(k)) {
            *v = nv.clone();
        }
    }
    serde_json::from_value(Value::Object(base)).map_err(|e| e.to_string())
}
