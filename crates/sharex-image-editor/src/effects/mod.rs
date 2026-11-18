//! The image editor's own image effects (`ShareX.ImageEditor/Core/ImageEffects`,
//! ShareX 22.4): every effect of the editor's effect browser, its
//! parameters as the effect dialog builds them, and how each changes the
//! image. A separate catalogue from the task presets' effects
//! (`ShareX.ImageEffectsLib`, ported in `sharex-image-effects`): ShareX's
//! editor shares no code with it, only Skia (`sharex-skia`).
//!
//! - [`catalog`] lists the effects as `ImageEffectCatalog.Definitions` does
//!   (by category, then by name, ignoring case); [`find`] looks one up by
//!   id (`ImageEffectCatalog.TryGetDefinition`), [`by_category`] groups
//!   them (`GetByCategory`).
//! - An [`EffectDefinition`] has the browser's label, icon key, description
//!   and category, whether it applies at once (no dialog), and its
//!   [`ParamDef`]s: the dialog's controls with their ranges and defaults.
//! - [`EffectValues`] is a dialog's state; [`EffectDefinition::apply`] runs
//!   the effect on an image (`CreateConfiguredEffect(states).Apply(img)`),
//!   which is also the live preview; [`apply_to_core`] commits it to an
//!   [`EditorCore`] with undo, routing the operations that move annotations
//!   (crop, resize, rotate, flip, auto crop, resize canvas) as ShareX's
//!   `EditorOperationCatalog` does; [`EffectPreview`] is
//!   `MainViewModel`'s preview session (start, preview, apply, cancel).
//!
//! Images are premultiplied RGBA [`Bitmap`]s (`SKBitmap`, `Bgra8888`,
//! premultiplied); [`to_bitmap`] and [`to_pixmap`] convert from and to the
//! editor's [`Pixmap`].

pub mod color;
pub mod params;
pub mod random;

pub(crate) mod draw;
pub(crate) mod helpers;

mod adjustments;
mod drawings;
mod filters;
mod manipulations;

use std::any::Any;
use std::sync::LazyLock;

pub use color::SkColor;
pub use params::{EffectValues, ParamDef, ParamKind, ParamValue};
pub use sharex_skia::raster::Bitmap;
use sharex_skia::Pixmap;

use crate::editor::EditorCore;

/// What the effects' modules share.
pub(crate) mod prelude {
    pub(crate) use super::draw::{self as sk, Pen, solid};
    pub(crate) use sharex_skia::{Cap, Join, Path as SkPath};
    pub(crate) use super::color::{self, SkColor, from_pixels, get_pixel, pixels, set_pixels};
    pub(crate) use super::helpers::*;
    pub(crate) use super::params::{
        bool_param, color_param, double_numeric, enum_param, file_param, float_slider, int_numeric, int_slider,
        text_param,
    };
    pub(crate) use super::random;
    pub(crate) use super::{Bitmap, EffectCategory, EffectError, EffectImpl, EffectResult, Info, ParamDef};
    pub(crate) use sharex_skia::raster::{Blend, Canvas, Matrix, Paint, Rect as RRect, Sampling, Shader, TileMode};

    /// `SKPointI`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct PointI {
        pub x: i32,
        pub y: i32,
    }

    impl PointI {
        pub const fn new(x: i32, y: i32) -> Self {
            PointI { x, y }
        }
    }

    /// `SKSizeI`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct SizeI {
        pub width: i32,
        pub height: i32,
    }

    impl SizeI {
        pub const fn new(width: i32, height: i32) -> Self {
            SizeI { width, height }
        }
    }

    /// What an effect not ported yet returns.
    pub(crate) fn not_ported() -> EffectResult {
        Err(EffectError(super::NOT_PORTED.to_string()))
    }

    /// `ArgumentNullException`/`ArgumentException` messages as .NET words them.
    pub(crate) fn throw(message: &str) -> EffectResult {
        Err(EffectError(message.to_string()))
    }
}

/// The error of an effect whose `Apply` isn't ported yet.
pub const NOT_PORTED: &str = "not ported yet";

/// `ImageEffectCategory`, in ShareX's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectCategory {
    Manipulations,
    Adjustments,
    Filters,
    Drawings,
}

impl EffectCategory {
    pub const ALL: [EffectCategory; 4] = [
        EffectCategory::Manipulations,
        EffectCategory::Adjustments,
        EffectCategory::Filters,
        EffectCategory::Drawings,
    ];

    pub fn name(self) -> &'static str {
        match self {
            EffectCategory::Manipulations => "Manipulations",
            EffectCategory::Adjustments => "Adjustments",
            EffectCategory::Filters => "Filters",
            EffectCategory::Drawings => "Drawings",
        }
    }
}

/// An exception ShareX's effect would throw (the preview reports it and
/// shows nothing new; applying does nothing).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectError(pub String);

impl std::fmt::Display for EffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EffectError {}

pub type EffectResult = Result<Bitmap, EffectError>;

/// The browser's metadata of an effect (`ImageEffectBase`'s `Id`, `Name`,
/// `Category`, `IconKey`, `Description`, `ExecutionMode`).
#[derive(Clone, Copy, Debug)]
pub struct Info {
    pub id: &'static str,
    pub name: &'static str,
    pub category: EffectCategory,
    pub icon: &'static str,
    pub description: &'static str,
    /// `EffectExecutionMode.Immediate`: applied from the browser at once.
    pub immediate: bool,
    /// A `BrowserLabel` override.
    pub browser_label: Option<&'static str>,
}

impl Info {
    pub const fn new(id: &'static str, name: &'static str, category: EffectCategory, icon: &'static str) -> Self {
        Info {
            id,
            name,
            category,
            icon,
            description: "",
            immediate: false,
            browser_label: None,
        }
    }

    pub const fn desc(mut self, d: &'static str) -> Self {
        self.description = d;
        self
    }

    pub const fn immediate(mut self) -> Self {
        self.immediate = true;
        self
    }
}

/// One effect type: its defaults (`Default`, ShareX's property
/// initialisers), metadata, parameters and `Apply`.
pub(crate) trait EffectImpl: Any + Default + Send + Sync {
    fn info() -> Info;
    fn params() -> Vec<ParamDef> {
        Vec::new()
    }
    fn apply(&self, source: &Bitmap) -> EffectResult;
}

/// A configured effect (`ImageEffect`).
pub trait ImageEffect: Any + Send + Sync {
    /// `Apply(SKBitmap)`.
    fn apply(&self, source: &Bitmap) -> EffectResult;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: EffectImpl> ImageEffect for T {
    fn apply(&self, source: &Bitmap) -> EffectResult {
        EffectImpl::apply(self, source)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// `EffectDefinition`: an entry of the effect browser.
pub struct EffectDefinition {
    pub id: &'static str,
    pub name: &'static str,
    /// The name, with "..." when it opens a dialog.
    pub browser_label: String,
    /// The icon key (`ShareX.ImageEditor`'s icon resources).
    pub icon: &'static str,
    pub description: &'static str,
    pub category: EffectCategory,
    /// Applied at once from the browser (no dialog).
    pub apply_immediately: bool,
    /// The dialog's controls, in order.
    pub parameters: Vec<ParamDef>,
    create: fn() -> Box<dyn ImageEffect>,
}

impl std::fmt::Debug for EffectDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EffectDefinition({})", self.id)
    }
}

fn create<T: EffectImpl>() -> Box<dyn ImageEffect> {
    Box::new(T::default())
}

pub(crate) fn def<T: EffectImpl>() -> EffectDefinition {
    let info = T::info();
    let browser_label = info.browser_label.map(str::to_string).unwrap_or_else(|| {
        if info.immediate {
            info.name.to_string()
        } else {
            format!("{}...", info.name)
        }
    });
    EffectDefinition {
        id: info.id,
        name: info.name,
        browser_label,
        icon: info.icon,
        description: info.description,
        category: info.category,
        apply_immediately: info.immediate,
        parameters: T::params(),
        create: create::<T>,
    }
}

impl EffectDefinition {
    /// The dialog's starting state.
    pub fn default_values(&self) -> EffectValues {
        EffectValues::defaults(&self.parameters)
    }

    /// The effect with its property defaults, no parameter applied.
    pub fn create(&self) -> Box<dyn ImageEffect> {
        (self.create)()
    }

    /// `CreateConfiguredEffect`: every parameter's value applied in order.
    pub fn configure(&self, values: &EffectValues) -> Box<dyn ImageEffect> {
        let mut effect = self.create();
        for (i, p) in self.parameters.iter().enumerate() {
            let v = if i < values.len() {
                values.value_at(i).clone()
            } else {
                p.default_value()
            };
            p.apply(effect.as_any_mut(), &v);
        }
        if let Some(resize) = effect
            .as_any_mut()
            .downcast_mut::<manipulations::resize_image::ResizeImageEffect>()
        {
            let (w, h) = (values.last_changed("width"), values.last_changed("height"));
            resize.aspect_ratio_anchor = if w == h {
                manipulations::resize_image::ResizeImageEffectAspectRatioAnchor::LargestDimension
            } else if w > h {
                manipulations::resize_image::ResizeImageEffectAspectRatioAnchor::Width
            } else {
                manipulations::resize_image::ResizeImageEffectAspectRatioAnchor::Height
            };
        }
        effect
    }

    /// The effect applied to `source`: the dialog's preview, or what
    /// applying commits.
    pub fn apply(&self, source: &Bitmap, values: &EffectValues) -> EffectResult {
        self.configure(values).apply(source)
    }

    /// [`apply`](Self::apply) on the editor's pixmap.
    pub fn apply_pixmap(&self, source: &Pixmap, values: &EffectValues) -> Result<Pixmap, EffectError> {
        self.apply(&to_bitmap(source), values).map(|b| to_pixmap(&b))
    }

    /// The `EditorOperationCatalog` routing of this effect, if any.
    pub fn editor_operation(&self) -> Option<EditorOperation> {
        EditorOperation::from_id(self.id)
    }
}

static CATALOG: LazyLock<Vec<EffectDefinition>> = LazyLock::new(|| {
    let mut list = Vec::new();
    manipulations::register(&mut list);
    adjustments::register(&mut list);
    filters::register(&mut list);
    drawings::register(&mut list);
    list.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then_with(|| ordinal_ignore_case(a.name, b.name))
    });
    list
});

/// `StringComparer.OrdinalIgnoreCase`: UTF-16 code units, upper-cased.
fn ordinal_ignore_case(a: &str, b: &str) -> std::cmp::Ordering {
    let up = |s: &str| -> Vec<u16> {
        s.encode_utf16()
            .map(|c| {
                if (b'a' as u16..=b'z' as u16).contains(&c) {
                    c - 32
                } else {
                    c
                }
            })
            .collect()
    };
    up(a).cmp(&up(b))
}

/// Every effect, in the browser's order.
pub fn catalog() -> &'static [EffectDefinition] {
    &CATALOG
}

/// An effect by id, ignoring case.
pub fn find(id: &str) -> Option<&'static EffectDefinition> {
    CATALOG.iter().find(|d| d.id.eq_ignore_ascii_case(id))
}

/// The effects of one category, in the browser's order.
pub fn by_category(category: EffectCategory) -> Vec<&'static EffectDefinition> {
    CATALOG.iter().filter(|d| d.category == category).collect()
}

/// The editor's pixmap as an effect's bitmap.
pub fn to_bitmap(p: &Pixmap) -> Bitmap {
    Bitmap::from_premultiplied(p.width(), p.height(), p.to_premul_rgba())
}

/// An effect's bitmap as the editor's pixmap.
pub fn to_pixmap(b: &Bitmap) -> Pixmap {
    Pixmap::from_premul_rgba(b.width(), b.height(), b.data())
}

/// `EditorOperationKind`: the effects ShareX applies through `EditorCore`'s
/// own operations (they move annotations) rather than as image effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorOperation {
    AutoCropImage,
    CropImage,
    ResizeImage,
    ResizeCanvas,
    Rotate90Clockwise,
    Rotate90CounterClockwise,
    Rotate180,
    RotateCustomAngle,
    FlipHorizontal,
    FlipVertical,
}

impl EditorOperation {
    /// `EditorOperationCatalog._operationMappings`.
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id.to_ascii_lowercase().as_str() {
            "auto_crop_image" => EditorOperation::AutoCropImage,
            "crop_image" => EditorOperation::CropImage,
            "resize_image" => EditorOperation::ResizeImage,
            "resize_canvas" => EditorOperation::ResizeCanvas,
            "rotate_90_clockwise" => EditorOperation::Rotate90Clockwise,
            "rotate_90_counter_clockwise" => EditorOperation::Rotate90CounterClockwise,
            "rotate_180" => EditorOperation::Rotate180,
            "rotate_custom_angle" => EditorOperation::RotateCustomAngle,
            "flip_horizontal" => EditorOperation::FlipHorizontal,
            "flip_vertical" => EditorOperation::FlipVertical,
            _ => return None,
        })
    }
}

fn number(values: &EffectValues, key: &str) -> f64 {
    values.get(key).and_then(ParamValue::as_number).unwrap_or(0.0)
}

/// Commits an effect to the editor, with undo, as ShareX's effect browser
/// and dialog do (`EditorView.EffectsHost`): the editor operations through
/// `EditorCore` (annotations moved with the image), every other effect
/// through `ApplyImageEffect` (annotations kept). Returns whether the image
/// changed; an effect that throws in ShareX changes nothing.
pub fn apply_to_core(core: &mut EditorCore, def: &EffectDefinition, values: &EffectValues) -> Result<bool, EffectError> {
    match def.editor_operation() {
        Some(EditorOperation::AutoCropImage) => {
            let tolerance = number(values, "tolerance").round_ties_even() as i32;
            Ok(core.auto_crop(tolerance))
        }
        Some(EditorOperation::CropImage) => {
            // (int)(n.Value ?? 0): truncation.
            let (x, y) = (number(values, "x") as i32, number(values, "y") as i32);
            let (w, h) = (number(values, "width") as i32, number(values, "height") as i32);
            Ok(crop_image(core, x, y, w, h))
        }
        Some(EditorOperation::ResizeImage) => {
            let effect = def.configure(values);
            let resize = effect
                .as_any()
                .downcast_ref::<manipulations::resize_image::ResizeImageEffect>()
                .expect("resize_image is a ResizeImageEffect");
            let (w, h) = resize.target_size(core.source().width() as i32, core.source().height() as i32);
            if w <= 0 || h <= 0 {
                return Ok(false);
            }
            Ok(core.resize_image(w, h))
        }
        Some(EditorOperation::ResizeCanvas) => {
            let side = |k: &str| number(values, k) as i32;
            let color = match values.get("background_color") {
                Some(ParamValue::Color(c)) => *c,
                _ => color::TRANSPARENT,
            };
            Ok(core.resize_canvas(side("top"), side("right"), side("bottom"), side("left"), color.rgba_array()))
        }
        Some(EditorOperation::RotateCustomAngle) => {
            let angle = number(values, "angle") as f32;
            let auto = !matches!(values.get("auto_resize"), Some(ParamValue::Bool(false)));
            Ok(core.rotate_custom_angle(angle, auto))
        }
        Some(EditorOperation::Rotate90Clockwise) => Ok(core.rotate_90_clockwise()),
        Some(EditorOperation::Rotate90CounterClockwise) => Ok(core.rotate_90_counter_clockwise()),
        Some(EditorOperation::Rotate180) => Ok(core.rotate_180()),
        Some(EditorOperation::FlipHorizontal) => Ok(core.flip_horizontal()),
        Some(EditorOperation::FlipVertical) => Ok(core.flip_vertical()),
        None => {
            let result = def.apply_pixmap(core.source(), values)?;
            Ok(core.apply_image_effect(|_| Some(result)))
        }
    }
}

/// `MainViewModel.CropImage(x, y, w, h)` → `EditorCore.Crop`.
fn crop_image(core: &mut EditorCore, x: i32, y: i32, w: i32, h: i32) -> bool {
    if w <= 0 || h <= 0 {
        return false;
    }
    // new SKRect(x, y, x + width, y + height)
    core.crop(crate::editor::Rect::new(
        x as f32,
        y as f32,
        (x + w) as f32,
        (y + h) as f32,
    ))
}

/// `MainViewModel`'s effect preview session: the image before the effect
/// (`StartEffectPreview`), the latest preview (`PreviewEffect`), committed
/// with undo (`ApplyEffect`: the latest preview if there is one) or dropped
/// (`CancelEffectPreview`).
pub struct EffectPreview {
    pre_effect: Bitmap,
    latest: Option<Bitmap>,
}

impl EffectPreview {
    /// `StartEffectPreview`: the editor's image as it is now.
    pub fn start(core: &EditorCore) -> Self {
        EffectPreview {
            pre_effect: to_bitmap(core.source()),
            latest: None,
        }
    }

    /// The image the effect runs on.
    pub fn source(&self) -> &Bitmap {
        &self.pre_effect
    }

    /// `PreviewEffect`: the effect on the image before the effect; the
    /// result is what the canvas shows (the editor's image is unchanged).
    pub fn preview(&mut self, def: &EffectDefinition, values: &EffectValues) -> Result<&Bitmap, EffectError> {
        let result = def.apply(&self.pre_effect, values)?;
        self.latest = Some(result);
        Ok(self.latest.as_ref().expect("just set"))
    }

    /// The latest preview, if any.
    pub fn latest(&self) -> Option<&Bitmap> {
        self.latest.as_ref()
    }

    /// Applies (the dialog's Apply): editor operations through
    /// [`apply_to_core`], other effects commit the latest preview (or run
    /// the effect if there's none) with undo.
    pub fn apply(self, core: &mut EditorCore, def: &EffectDefinition, values: &EffectValues) -> Result<bool, EffectError> {
        if def.editor_operation().is_some() {
            return apply_to_core(core, def, values);
        }
        let result = match self.latest {
            Some(b) => b,
            None => def.apply(&self.pre_effect, values)?,
        };
        let pixmap = to_pixmap(&result);
        Ok(core.apply_image_effect(|_| Some(pixmap)))
    }

    /// `CancelEffectPreview`: nothing changes.
    pub fn cancel(self) {}
}
