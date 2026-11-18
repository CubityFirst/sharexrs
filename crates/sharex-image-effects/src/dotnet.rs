//! The .NET values ShareX's effect presets hold, read and written as Newtonsoft does
//! with ShareX's settings (`JsonHelpers`: `StringEnumConverter`, `System.Drawing` types
//! through their `TypeConverter`s, invariant culture), so presets round-trip between
//! ShareX and ShareXrs unchanged.

use serde_json::{Number, Value};

/// A property value as Newtonsoft reads (`None`: the JSON doesn't convert, and
/// ShareX's error handler skips the property) and writes it.
pub trait Prop: Sized {
    fn to_json(&self) -> Value;
    fn from_json(v: &Value) -> Option<Self>;
}

/// The editor ShareX's `ImageEffectOptionsPanel.CreateEditor` picks for a
/// property from its CLR type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    /// `bool`: a `ToggleSwitch`.
    Bool,
    /// An integer type: a `NumericUpDown` (`Increment` 1, `FormatString="0"`).
    Int,
    /// `float`/`double`: a `NumericUpDown` (`Increment` 0.1, `"0.###"`).
    Float,
    /// A non-flags enum: a `ComboBox` of `ImageEffectsLocalization.GetEnumValue`.
    /// `names` in `Enum.GetValues` order; `resource` the type's name in the
    /// `ImageEffectEnum_*` keys.
    Enum { names: &'static [&'static str], resource: &'static str },
    /// `Color`: the colour button and its `ColorView` flyout.
    Color,
    /// `Point`: X and Y boxes.
    Point,
    /// `Size`: width and height boxes.
    Size,
    /// `ImageMargins`: left, top, right and bottom boxes.
    Margins,
    /// `GradientInfo`: Edit gradient... with the `GradientOptionsPanel` flyout.
    Gradient,
    /// Anything else (strings, fonts, flags): a `TextBox` through the
    /// property's `TypeConverter`.
    Text,
}

/// A property type's [`PropKind`].
pub trait PropMeta {
    const KIND: PropKind;
}

macro_rules! prop_meta {
    ($($t:ty => $k:expr),* $(,)?) => { $(impl PropMeta for $t { const KIND: PropKind = $k; })* };
}

/// An `f32` written as .NET's shortest round-trip form (`0.6`, not `0.6000000238…`).
pub fn f32_json(v: f32) -> Value {
    let f: f64 = format!("{v}").parse().unwrap_or(v as f64);
    Number::from_f64(f)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(*b as i32 as f64),
        _ => None,
    }
}

impl Prop for f32 {
    fn to_json(&self) -> Value {
        f32_json(*self)
    }
    fn from_json(v: &Value) -> Option<Self> {
        number(v).map(|f| f as f32)
    }
}

impl Prop for f64 {
    fn to_json(&self) -> Value {
        Number::from_f64(*self)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
    fn from_json(v: &Value) -> Option<Self> {
        number(v)
    }
}

impl Prop for i32 {
    fn to_json(&self) -> Value {
        Value::from(*self)
    }
    fn from_json(v: &Value) -> Option<Self> {
        match v {
            Value::Number(n) => n.as_i64().and_then(|i| i32::try_from(i).ok()).or_else(|| {
                // Newtonsoft converts a float to an int (rounding is Convert.ToInt32's).
                n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i32)
            }),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
}

impl Prop for u8 {
    fn to_json(&self) -> Value {
        Value::from(*self)
    }
    fn from_json(v: &Value) -> Option<Self> {
        i32::from_json(v).and_then(|i| u8::try_from(i).ok())
    }
}

impl Prop for bool {
    fn to_json(&self) -> Value {
        Value::Bool(*self)
    }
    fn from_json(v: &Value) -> Option<Self> {
        match v {
            Value::Bool(b) => Some(*b),
            Value::String(s) => s.trim().to_ascii_lowercase().parse().ok(),
            Value::Number(n) => n.as_i64().map(|i| i != 0),
            _ => None,
        }
    }
}

impl Prop for String {
    fn to_json(&self) -> Value {
        Value::String(self.clone())
    }
    fn from_json(v: &Value) -> Option<Self> {
        match v {
            Value::String(s) => Some(s.clone()),
            Value::Null => Some(String::new()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(if *b { "True".into() } else { "False".into() }),
            _ => None,
        }
    }
}

/// `System.Drawing.Color`: ARGB plus, for colours made from a name (`Color.Red`,
/// "Red" in JSON), that name; `ColorConverter` writes the name back, else
/// "R, G, B" (opaque) or "A, R, G, B".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub name: Option<&'static str>,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color {
            a: 255,
            r,
            g,
            b,
            name: None,
        }
    }

    pub const fn argb(a: u8, r: u8, g: u8, b: u8) -> Self {
        Color {
            a,
            r,
            g,
            b,
            name: None,
        }
    }

    /// `Color.FromKnownColor` by name (case-insensitive); panics for an unknown name.
    pub fn named(name: &str) -> Self {
        Self::known(name).unwrap_or_else(|| panic!("unknown colour {name}"))
    }

    pub fn known(name: &str) -> Option<Self> {
        KNOWN_COLORS
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(n, argb)| Color {
                a: (argb >> 24) as u8,
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: *argb as u8,
                name: Some(n),
            })
    }

    pub fn transparent() -> Self {
        Self::named("Transparent")
    }

    /// Straight RGBA, as `ToSKColor` passes it to Skia.
    pub fn rgba(&self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// `ColorConverter.ConvertFromInvariantString`.
    pub fn parse(text: &str) -> Option<Self> {
        let t = text.trim();
        if t.is_empty() {
            return Some(Color::argb(0, 0, 0, 0));
        }
        if let Some(c) = Self::known(t) {
            return Some(c);
        }
        let hex = t
            .strip_prefix('#')
            .or_else(|| t.strip_prefix("0x"))
            .or_else(|| t.strip_prefix("0X"))
            .or_else(|| t.strip_prefix("&h"))
            .or_else(|| t.strip_prefix("&H"));
        if let Some(h) = hex {
            let v = u32::from_str_radix(h, 16).ok()?;
            let v = if t.starts_with('#') && (h.len() == 6 || h.len() == 3) {
                let v = if h.len() == 3 {
                    let (r, g, b) = ((v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF);
                    (r * 17) << 16 | (g * 17) << 8 | (b * 17)
                } else {
                    v
                };
                v | 0xFF00_0000
            } else {
                v
            };
            return Some(
                Color::argb((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8)
                    .possible_known_color(),
            );
        }
        let parts: Vec<&str> = t.split(',').map(str::trim).collect();
        let ints: Option<Vec<i64>> = parts.iter().map(|p| p.parse::<i64>().ok()).collect();
        let ints = ints?;
        let byte = |v: i64| u8::try_from(v).ok();
        let color = match ints.as_slice() {
            [v] => {
                let v = *v as u32;
                Some(Color::argb(
                    (v >> 24) as u8,
                    (v >> 16) as u8,
                    (v >> 8) as u8,
                    v as u8,
                ))
            }
            [r, g, b] => Some(Color::rgb(byte(*r)?, byte(*g)?, byte(*b)?)),
            [a, r, g, b] => Some(Color::argb(byte(*a)?, byte(*r)?, byte(*g)?, byte(*b)?)),
            _ => None,
        }?;
        Some(color.possible_known_color())
    }

    /// `ColorConverter`'s `PossibleKnownColor`: a colour given by numbers that equals a
    /// `Color` constant (web colours and Transparent, not system colours) becomes it, so
    /// "0, 0, 128" reads back as "Navy".
    fn possible_known_color(self) -> Self {
        let argb =
            (self.a as u32) << 24 | (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32;
        let web = KNOWN_COLORS
            .iter()
            .take_while(|(n, _)| *n != "ActiveBorder");
        for (name, v) in web {
            if *v == argb {
                return Color {
                    name: Some(name),
                    ..self
                };
            }
        }
        self
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.name {
            Some(n) => f.write_str(n),
            None if self.a == 255 => write!(f, "{}, {}, {}", self.r, self.g, self.b),
            None => write!(f, "{}, {}, {}, {}", self.a, self.r, self.g, self.b),
        }
    }
}

impl Prop for Color {
    fn to_json(&self) -> Value {
        Value::String(self.to_string())
    }
    fn from_json(v: &Value) -> Option<Self> {
        match v {
            Value::String(s) => Color::parse(s),
            _ => None,
        }
    }
}

fn parse_ints<const N: usize>(text: &str) -> Option<[i32; N]> {
    let mut out = [0; N];
    let mut parts = text.split(',');
    for v in &mut out {
        *v = parts.next()?.trim().parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

/// `System.Drawing.Point`: "X, Y".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }
}

impl Prop for Point {
    fn to_json(&self) -> Value {
        Value::String(format!("{}, {}", self.x, self.y))
    }
    fn from_json(v: &Value) -> Option<Self> {
        let [x, y] = parse_ints(v.as_str()?)?;
        Some(Point { x, y })
    }
}

/// `System.Drawing.Size`: "Width, Height".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    pub const fn new(width: i32, height: i32) -> Self {
        Size { width, height }
    }
}

impl Prop for Size {
    fn to_json(&self) -> Value {
        Value::String(format!("{}, {}", self.width, self.height))
    }
    fn from_json(v: &Value) -> Option<Self> {
        let [width, height] = parse_ints(v.as_str()?)?;
        Some(Size { width, height })
    }
}

/// `ImageMargins` (HelpersLib/ImageGeometry.cs): "Left, Top, Right, Bottom", or one
/// value for all four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Margins {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Margins {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Margins {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn all(v: i32) -> Self {
        Margins {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }

    pub fn horizontal(&self) -> i32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> i32 {
        self.top + self.bottom
    }

    /// `All`: the common value, or -1.
    pub fn all_value(&self) -> i32 {
        if self.left == self.top && self.top == self.right && self.right == self.bottom {
            self.left
        } else {
            -1
        }
    }
}

impl Prop for Margins {
    fn to_json(&self) -> Value {
        Value::String(format!(
            "{}, {}, {}, {}",
            self.left, self.top, self.right, self.bottom
        ))
    }
    fn from_json(v: &Value) -> Option<Self> {
        let s = v.as_str()?;
        if let Some([a]) = parse_ints::<1>(s) {
            return Some(Margins::all(a));
        }
        let [l, t, r, b] = parse_ints(s)?;
        Some(Margins::new(l, t, r, b))
    }
}

/// `ImageFontStyle` flags.
pub mod font_style {
    pub const BOLD: u8 = 1;
    pub const ITALIC: u8 = 2;
    pub const UNDERLINE: u8 = 4;
    pub const STRIKEOUT: u8 = 8;
}

/// `ImageFontUnit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontUnit {
    World,
    Display,
    Pixel,
    Point,
    Inch,
    Document,
    Millimeter,
}

impl FontUnit {
    const ALL: [FontUnit; 7] = [
        FontUnit::World,
        FontUnit::Display,
        FontUnit::Pixel,
        FontUnit::Point,
        FontUnit::Inch,
        FontUnit::Document,
        FontUnit::Millimeter,
    ];

    fn suffix(self) -> &'static str {
        match self {
            FontUnit::World => "world",
            FontUnit::Display => "display",
            FontUnit::Pixel => "px",
            FontUnit::Inch => "in",
            FontUnit::Document => "doc",
            FontUnit::Millimeter => "mm",
            FontUnit::Point => "pt",
        }
    }
}

/// `ImageFont` (HelpersLib/ImageFont.cs), kept by `FontSafe` as its string: "Arial, 11.25pt",
/// "Arial, 20pt, style=Bold".
#[derive(Debug, Clone, PartialEq)]
pub struct ImageFont {
    pub name: String,
    pub size: f32,
    pub style: u8,
    pub unit: FontUnit,
}

impl ImageFont {
    pub fn new(name: &str, size: f32) -> Self {
        ImageFont {
            name: name.into(),
            size,
            style: 0,
            unit: FontUnit::Point,
        }
    }

    /// `PixelSize`: the size Skia draws at (96 DPI).
    pub fn pixel_size(&self) -> f32 {
        match self.unit {
            FontUnit::Point => self.size * 96.0 / 72.0,
            FontUnit::Inch => self.size * 96.0,
            FontUnit::Document => self.size * 96.0 / 300.0,
            FontUnit::Millimeter => self.size * 96.0 / 25.4,
            _ => self.size,
        }
    }

    pub fn bold(&self) -> bool {
        self.style & font_style::BOLD != 0
    }

    pub fn italic(&self) -> bool {
        self.style & font_style::ITALIC != 0
    }

    /// `ImageFontConverter.ConvertFrom`.
    pub fn parse(text: &str) -> Option<Self> {
        let pieces: Vec<&str> = text.split(',').collect();
        let mut size_text = pieces
            .get(1)
            .map(|s| s.trim())
            .unwrap_or("8.25pt")
            .to_string();
        let mut unit = FontUnit::Point;
        for candidate in FontUnit::ALL {
            let suffix = candidate.suffix();
            if size_text.len() >= suffix.len()
                && size_text[size_text.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
            {
                unit = candidate;
                size_text = size_text[..size_text.len() - suffix.len()]
                    .trim_end()
                    .to_string();
                break;
            }
        }
        let size: f32 = size_text.parse().ok()?;
        let mut style = 0;
        if pieces.len() > 2 {
            let joined = pieces[2..].join(",").replace("style=", "");
            for flag in joined.split(',') {
                style |= match flag.trim().to_ascii_lowercase().as_str() {
                    "regular" => 0,
                    "bold" => font_style::BOLD,
                    "italic" => font_style::ITALIC,
                    "underline" => font_style::UNDERLINE,
                    "strikeout" => font_style::STRIKEOUT,
                    // Enum.TryParse fails as a whole: Regular.
                    _ => {
                        style = 0;
                        break;
                    }
                };
            }
        }
        Some(ImageFont {
            name: pieces[0].trim().to_string(),
            size,
            style,
            unit,
        })
    }
}

impl std::fmt::Display for ImageFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}{}", self.name, self.size, self.unit.suffix())?;
        if self.style != 0 {
            let names = [
                (font_style::BOLD, "Bold"),
                (font_style::ITALIC, "Italic"),
                (font_style::UNDERLINE, "Underline"),
                (font_style::STRIKEOUT, "Strikeout"),
            ];
            let parts: Vec<&str> = names
                .iter()
                .filter(|(b, _)| self.style & b != 0)
                .map(|(_, n)| *n)
                .collect();
            write!(f, ", style={}", parts.join(", "))?;
        }
        Ok(())
    }
}

/// `FontSafe`: null when not set.
impl Prop for Option<ImageFont> {
    fn to_json(&self) -> Value {
        match self {
            Some(f) => Value::String(f.to_string()),
            None => Value::Null,
        }
    }
    fn from_json(v: &Value) -> Option<Self> {
        match v {
            Value::Null => Some(None),
            Value::String(s) => ImageFont::parse(s).map(Some),
            _ => None,
        }
    }
}

/// An enum serialised by name (`StringEnumConverter`), read by name (case-insensitive)
/// or number.
macro_rules! dotnet_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident = $value:expr),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];
            /// The names in `ALL`'s order.
            pub const NAMES: &'static [&'static str] = &[$(stringify!($variant)),+];

            pub fn name(self) -> &'static str {
                match self { $($name::$variant => stringify!($variant)),+ }
            }

            pub fn value(self) -> i32 {
                match self { $($name::$variant => $value),+ }
            }

            pub fn from_name(s: &str) -> Option<Self> {
                let s = s.trim();
                $(if s.eq_ignore_ascii_case(stringify!($variant)) { return Some($name::$variant); })+
                let n: i32 = s.parse().ok()?;
                Self::from_value(n)
            }

            pub fn from_value(n: i32) -> Option<Self> {
                $(if n == $value { return Some($name::$variant); })+
                None
            }
        }

        impl $crate::dotnet::Prop for $name {
            fn to_json(&self) -> serde_json::Value {
                serde_json::Value::String(self.name().into())
            }
            fn from_json(v: &serde_json::Value) -> Option<Self> {
                match v {
                    serde_json::Value::String(s) => Self::from_name(s),
                    serde_json::Value::Number(n) => Self::from_value(n.as_i64()? as i32),
                    _ => None,
                }
            }
        }
    };
}

dotnet_enum!(
    /// `ImageGradientMode`.
    GradientMode { Horizontal = 0, Vertical = 1, ForwardDiagonal = 2, BackwardDiagonal = 3 }
);
dotnet_enum!(
    /// `ImageContentAlignment`.
    ContentAlignment {
        TopLeft = 1, TopCenter = 2, TopRight = 4, MiddleLeft = 16, MiddleCenter = 32, MiddleRight = 64,
        BottomLeft = 256, BottomCenter = 512, BottomRight = 1024,
    }
);
dotnet_enum!(
    /// `ImageDashStyle`.
    DashStyle { Solid = 0, Dash = 1, Dot = 2, DashDot = 3, DashDotDot = 4, Custom = 5 }
);
dotnet_enum!(
    /// `BorderType`.
    BorderType { Outside = 0, Inside = 1 }
);
dotnet_enum!(
    /// `ImageInterpolationMode`.
    InterpolationMode { HighQualityBicubic = 0, Bicubic = 1, HighQualityBilinear = 2, Bilinear = 3, NearestNeighbor = 4 }
);
dotnet_enum!(
    /// `ImageCompositingMode`.
    CompositingMode { SourceOver = 0, SourceCopy = 1 }
);
dotnet_enum!(
    /// `ImageTextRenderingMode`.
    TextRenderingMode { SystemDefault = 0, SingleBitPerPixelGridFit = 1, SingleBitPerPixel = 2, AntiAliasGridFit = 3, AntiAlias = 4, ClearTypeGridFit = 5 }
);
dotnet_enum!(
    /// `ResizeMode`.
    ResizeMode { ResizeAll = 0, ResizeIfBigger = 1, ResizeIfSmaller = 2 }
);
dotnet_enum!(
    /// `DrawImageSizeMode`.
    DrawImageSizeMode { DontResize = 0, AbsoluteSize = 1, PercentageOfWatermark = 2, PercentageOfCanvas = 3 }
);
dotnet_enum!(
    /// `ImageRotateFlipType`.
    RotateFlipType { None = 0, Rotate90 = 1, Rotate180 = 2, Rotate270 = 3, FlipX = 4, Rotate90FlipX = 5, FlipY = 6, Rotate90FlipY = 7 }
);
dotnet_enum!(
    /// `Canvas.CanvasMarginMode`.
    CanvasMarginMode { AbsoluteSize = 0, PercentageOfCanvas = 1 }
);
dotnet_enum!(
    /// `ForceProportions.ForceProportionsMethod`.
    ForceProportionsMethod { Grow = 0, Crop = 1 }
);

/// `ImageSides` flags, written as .NET formats flags: "Top, Bottom, Left, Right".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sides(pub u8);

impl Sides {
    pub const NONE: Sides = Sides(0);
    pub const TOP: u8 = 1;
    pub const BOTTOM: u8 = 2;
    pub const LEFT: u8 = 4;
    pub const RIGHT: u8 = 8;
    pub const ALL: Sides = Sides(15);
    const NAMES: [(u8, &'static str); 4] = [(1, "Top"), (2, "Bottom"), (4, "Left"), (8, "Right")];

    pub fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

impl Prop for Sides {
    fn to_json(&self) -> Value {
        if self.0 == 0 {
            return Value::String("None".into());
        }
        if self.0 & !15 != 0 {
            return Value::String(self.0.to_string());
        }
        let parts: Vec<&str> = Self::NAMES
            .iter()
            .filter(|(b, _)| self.0 & b != 0)
            .map(|(_, n)| *n)
            .collect();
        Value::String(parts.join(", "))
    }
    fn from_json(v: &Value) -> Option<Self> {
        match v {
            Value::Number(n) => Some(Sides(n.as_u64()? as u8)),
            Value::String(s) => {
                let mut out = 0u8;
                for part in s.split(',') {
                    let part = part.trim();
                    if part.eq_ignore_ascii_case("None") {
                        continue;
                    }
                    if let Some((b, _)) = Self::NAMES
                        .iter()
                        .find(|(_, n)| n.eq_ignore_ascii_case(part))
                    {
                        out |= b;
                    } else {
                        out |= part.parse::<u8>().ok()?;
                    }
                }
                Some(Sides(out))
            }
            _ => None,
        }
    }
}

/// `GradientStop`.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    pub color: Color,
    /// 0 to 100 (the setter clamps).
    pub location: f32,
}

/// `GradientInfo` (HelpersLib/Colors/GradientInfo.cs).
#[derive(Debug, Clone, PartialEq)]
pub struct GradientInfo {
    pub kind: GradientMode,
    pub colors: Vec<GradientStop>,
}

impl GradientInfo {
    pub fn new(kind: GradientMode, stops: &[(Color, f32)]) -> Self {
        GradientInfo {
            kind,
            colors: stops
                .iter()
                .map(|(c, l)| GradientStop {
                    color: c.clone(),
                    location: l.clamp(0.0, 100.0),
                })
                .collect(),
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.colors.is_empty()
    }

    pub fn is_visible(&self) -> bool {
        self.is_valid() && self.colors.iter().any(|s| s.color.a > 0)
    }
}

/// `Gradient` properties: null stays null.
impl Prop for Option<GradientInfo> {
    fn to_json(&self) -> Value {
        let Some(g) = self else { return Value::Null };
        let colors: Vec<Value> = g
            .colors
            .iter()
            .map(|s| {
                let mut m = serde_json::Map::new();
                m.insert("Color".into(), s.color.to_json());
                m.insert("Location".into(), f32_json(s.location));
                Value::Object(m)
            })
            .collect();
        let mut m = serde_json::Map::new();
        m.insert("Type".into(), g.kind.to_json());
        m.insert("Colors".into(), Value::Array(colors));
        Value::Object(m)
    }
    fn from_json(v: &Value) -> Option<Self> {
        let obj = match v {
            Value::Null => return Some(None),
            Value::Object(o) => o,
            _ => return None,
        };
        let get = |k: &str| {
            obj.get(k).or_else(|| {
                obj.iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(k))
                    .map(|(_, v)| v)
            })
        };
        let mut g = GradientInfo {
            kind: GradientMode::Vertical,
            colors: Vec::new(),
        };
        if let Some(k) = get("Type").and_then(GradientMode::from_json) {
            g.kind = k;
        }
        if let Some(Value::Array(stops)) = get("Colors") {
            for stop in stops {
                let Value::Object(s) = stop else { continue };
                let sget = |k: &str| {
                    s.get(k).or_else(|| {
                        s.iter()
                            .find(|(n, _)| n.eq_ignore_ascii_case(k))
                            .map(|(_, v)| v)
                    })
                };
                let color = sget("Color")
                    .and_then(Color::from_json)
                    .unwrap_or_else(|| Color::named("Black"));
                let location = sget("Location")
                    .and_then(f32::from_json)
                    .unwrap_or(0.0)
                    .clamp(0.0, 100.0);
                g.colors.push(GradientStop { color, location });
            }
        }
        Some(Some(g))
    }
}

/// `KnownColor` names and their ARGB (web colours as .NET defines them, `Transparent`,
/// and Windows' default system colours).
pub static KNOWN_COLORS: &[(&str, u32)] = &[
    ("Transparent", 0x00FFFFFF),
    ("AliceBlue", 0xFFF0F8FF),
    ("AntiqueWhite", 0xFFFAEBD7),
    ("Aqua", 0xFF00FFFF),
    ("Aquamarine", 0xFF7FFFD4),
    ("Azure", 0xFFF0FFFF),
    ("Beige", 0xFFF5F5DC),
    ("Bisque", 0xFFFFE4C4),
    ("Black", 0xFF000000),
    ("BlanchedAlmond", 0xFFFFEBCD),
    ("Blue", 0xFF0000FF),
    ("BlueViolet", 0xFF8A2BE2),
    ("Brown", 0xFFA52A2A),
    ("BurlyWood", 0xFFDEB887),
    ("CadetBlue", 0xFF5F9EA0),
    ("Chartreuse", 0xFF7FFF00),
    ("Chocolate", 0xFFD2691E),
    ("Coral", 0xFFFF7F50),
    ("CornflowerBlue", 0xFF6495ED),
    ("Cornsilk", 0xFFFFF8DC),
    ("Crimson", 0xFFDC143C),
    ("Cyan", 0xFF00FFFF),
    ("DarkBlue", 0xFF00008B),
    ("DarkCyan", 0xFF008B8B),
    ("DarkGoldenrod", 0xFFB8860B),
    ("DarkGray", 0xFFA9A9A9),
    ("DarkGreen", 0xFF006400),
    ("DarkKhaki", 0xFFBDB76B),
    ("DarkMagenta", 0xFF8B008B),
    ("DarkOliveGreen", 0xFF556B2F),
    ("DarkOrange", 0xFFFF8C00),
    ("DarkOrchid", 0xFF9932CC),
    ("DarkRed", 0xFF8B0000),
    ("DarkSalmon", 0xFFE9967A),
    ("DarkSeaGreen", 0xFF8FBC8B),
    ("DarkSlateBlue", 0xFF483D8B),
    ("DarkSlateGray", 0xFF2F4F4F),
    ("DarkTurquoise", 0xFF00CED1),
    ("DarkViolet", 0xFF9400D3),
    ("DeepPink", 0xFFFF1493),
    ("DeepSkyBlue", 0xFF00BFFF),
    ("DimGray", 0xFF696969),
    ("DodgerBlue", 0xFF1E90FF),
    ("Firebrick", 0xFFB22222),
    ("FloralWhite", 0xFFFFFAF0),
    ("ForestGreen", 0xFF228B22),
    ("Fuchsia", 0xFFFF00FF),
    ("Gainsboro", 0xFFDCDCDC),
    ("GhostWhite", 0xFFF8F8FF),
    ("Gold", 0xFFFFD700),
    ("Goldenrod", 0xFFDAA520),
    ("Gray", 0xFF808080),
    ("Green", 0xFF008000),
    ("GreenYellow", 0xFFADFF2F),
    ("Honeydew", 0xFFF0FFF0),
    ("HotPink", 0xFFFF69B4),
    ("IndianRed", 0xFFCD5C5C),
    ("Indigo", 0xFF4B0082),
    ("Ivory", 0xFFFFFFF0),
    ("Khaki", 0xFFF0E68C),
    ("Lavender", 0xFFE6E6FA),
    ("LavenderBlush", 0xFFFFF0F5),
    ("LawnGreen", 0xFF7CFC00),
    ("LemonChiffon", 0xFFFFFACD),
    ("LightBlue", 0xFFADD8E6),
    ("LightCoral", 0xFFF08080),
    ("LightCyan", 0xFFE0FFFF),
    ("LightGoldenrodYellow", 0xFFFAFAD2),
    ("LightGray", 0xFFD3D3D3),
    ("LightGreen", 0xFF90EE90),
    ("LightPink", 0xFFFFB6C1),
    ("LightSalmon", 0xFFFFA07A),
    ("LightSeaGreen", 0xFF20B2AA),
    ("LightSkyBlue", 0xFF87CEFA),
    ("LightSlateGray", 0xFF778899),
    ("LightSteelBlue", 0xFFB0C4DE),
    ("LightYellow", 0xFFFFFFE0),
    ("Lime", 0xFF00FF00),
    ("LimeGreen", 0xFF32CD32),
    ("Linen", 0xFFFAF0E6),
    ("Magenta", 0xFFFF00FF),
    ("Maroon", 0xFF800000),
    ("MediumAquamarine", 0xFF66CDAA),
    ("MediumBlue", 0xFF0000CD),
    ("MediumOrchid", 0xFFBA55D3),
    ("MediumPurple", 0xFF9370DB),
    ("MediumSeaGreen", 0xFF3CB371),
    ("MediumSlateBlue", 0xFF7B68EE),
    ("MediumSpringGreen", 0xFF00FA9A),
    ("MediumTurquoise", 0xFF48D1CC),
    ("MediumVioletRed", 0xFFC71585),
    ("MidnightBlue", 0xFF191970),
    ("MintCream", 0xFFF5FFFA),
    ("MistyRose", 0xFFFFE4E1),
    ("Moccasin", 0xFFFFE4B5),
    ("NavajoWhite", 0xFFFFDEAD),
    ("Navy", 0xFF000080),
    ("OldLace", 0xFFFDF5E6),
    ("Olive", 0xFF808000),
    ("OliveDrab", 0xFF6B8E23),
    ("Orange", 0xFFFFA500),
    ("OrangeRed", 0xFFFF4500),
    ("Orchid", 0xFFDA70D6),
    ("PaleGoldenrod", 0xFFEEE8AA),
    ("PaleGreen", 0xFF98FB98),
    ("PaleTurquoise", 0xFFAFEEEE),
    ("PaleVioletRed", 0xFFDB7093),
    ("PapayaWhip", 0xFFFFEFD5),
    ("PeachPuff", 0xFFFFDAB9),
    ("Peru", 0xFFCD853F),
    ("Pink", 0xFFFFC0CB),
    ("Plum", 0xFFDDA0DD),
    ("PowderBlue", 0xFFB0E0E6),
    ("Purple", 0xFF800080),
    ("RebeccaPurple", 0xFF663399),
    ("Red", 0xFFFF0000),
    ("RosyBrown", 0xFFBC8F8F),
    ("RoyalBlue", 0xFF4169E1),
    ("SaddleBrown", 0xFF8B4513),
    ("Salmon", 0xFFFA8072),
    ("SandyBrown", 0xFFF4A460),
    ("SeaGreen", 0xFF2E8B57),
    ("SeaShell", 0xFFFFF5EE),
    ("Sienna", 0xFFA0522D),
    ("Silver", 0xFFC0C0C0),
    ("SkyBlue", 0xFF87CEEB),
    ("SlateBlue", 0xFF6A5ACD),
    ("SlateGray", 0xFF708090),
    ("Snow", 0xFFFFFAFA),
    ("SpringGreen", 0xFF00FF7F),
    ("SteelBlue", 0xFF4682B4),
    ("Tan", 0xFFD2B48C),
    ("Teal", 0xFF008080),
    ("Thistle", 0xFFD8BFD8),
    ("Tomato", 0xFFFF6347),
    ("Turquoise", 0xFF40E0D0),
    ("Violet", 0xFFEE82EE),
    ("Wheat", 0xFFF5DEB3),
    ("White", 0xFFFFFFFF),
    ("WhiteSmoke", 0xFFF5F5F5),
    ("Yellow", 0xFFFFFF00),
    ("YellowGreen", 0xFF9ACD32),
    // System colours (Windows 10/11 defaults).
    ("ActiveBorder", 0xFFB4B4B4),
    ("ActiveCaption", 0xFF99B4D1),
    ("ActiveCaptionText", 0xFF000000),
    ("AppWorkspace", 0xFFABABAB),
    ("ButtonFace", 0xFFF0F0F0),
    ("ButtonHighlight", 0xFFFFFFFF),
    ("ButtonShadow", 0xFFA0A0A0),
    ("Control", 0xFFF0F0F0),
    ("ControlDark", 0xFFA0A0A0),
    ("ControlDarkDark", 0xFF696969),
    ("ControlLight", 0xFFE3E3E3),
    ("ControlLightLight", 0xFFFFFFFF),
    ("ControlText", 0xFF000000),
    ("Desktop", 0xFF000000),
    ("GradientActiveCaption", 0xFFB9D1EA),
    ("GradientInactiveCaption", 0xFFD7E4F2),
    ("GrayText", 0xFF6D6D6D),
    ("Highlight", 0xFF0078D7),
    ("HighlightText", 0xFFFFFFFF),
    ("HotTrack", 0xFF0066CC),
    ("InactiveBorder", 0xFFF4F7FC),
    ("InactiveCaption", 0xFFBFCDDB),
    ("InactiveCaptionText", 0xFF000000),
    ("Info", 0xFFFFFFE1),
    ("InfoText", 0xFF000000),
    ("Menu", 0xFFF0F0F0),
    ("MenuBar", 0xFFF0F0F0),
    ("MenuHighlight", 0xFF3399FF),
    ("MenuText", 0xFF000000),
    ("ScrollBar", 0xFFC8C8C8),
    ("Window", 0xFFFFFFFF),
    ("WindowFrame", 0xFF646464),
    ("WindowText", 0xFF000000),
];

prop_meta! {
    bool => PropKind::Bool,
    i32 => PropKind::Int,
    u8 => PropKind::Int,
    f32 => PropKind::Float,
    f64 => PropKind::Float,
    String => PropKind::Text,
    Option<ImageFont> => PropKind::Text,
    Sides => PropKind::Text,
    Color => PropKind::Color,
    Point => PropKind::Point,
    Size => PropKind::Size,
    Margins => PropKind::Margins,
    Option<GradientInfo> => PropKind::Gradient,
}

macro_rules! enum_meta {
    ($($t:ident => $res:literal),* $(,)?) => {
        $(impl PropMeta for $t { const KIND: PropKind = PropKind::Enum { names: $t::NAMES, resource: $res }; })*
    };
}

// `ImageEffectsLocalization.GetEnumValue`'s resource type names.
enum_meta! {
    GradientMode => "LinearGradientMode",
    ContentAlignment => "ContentAlignment",
    DashStyle => "DashStyle",
    BorderType => "BorderType",
    InterpolationMode => "ImageInterpolationMode",
    CompositingMode => "CompositingMode",
    TextRenderingMode => "TextRenderingHint",
    ResizeMode => "ResizeMode",
    DrawImageSizeMode => "DrawImageSizeMode",
    RotateFlipType => "ImageRotateFlipType",
    CanvasMarginMode => "CanvasMarginMode",
    ForceProportionsMethod => "ForceProportionsMethod",
}
