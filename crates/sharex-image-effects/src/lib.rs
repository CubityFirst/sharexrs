//! ShareX's image effects (`ShareX.ImageEffectsLib`), ported to produce the same pixels.
//!
//! - [`effects`]: every effect, with ShareX's properties, defaults, clamps and
//!   summaries, and a generic property API ([`EffectKind::get_property`],
//!   [`EffectKind::set_property`], [`effects::CATALOG`]) for the Image effects window.
//! - [`preset`]: presets in ShareX's JSON (settings and export forms) and
//!   [`preset::apply_image_effects`], what the after-capture "Add image effects" task runs.
//! - [`package`]: `.sxie` packages.
//! - [`skia`]: the Skia raster operations ShareX's effects go through, ported with
//!   Skia's own arithmetic; [`ops`]: ShareX's `SkiaImageHelpers` on top of them.
//!
//! See `docs/IMAGE-EFFECTS.md` for the map and the validation against ShareX.

pub mod bitmap;
pub mod dotnet;
pub mod effects;
pub mod ops;
pub mod package;
pub mod paths;
pub mod pixels;
pub mod preset;
pub mod random;
pub mod skia;
pub mod text;

pub use bitmap::Bitmap;
pub use effects::{CATALOG, Category, Context, EffectKind};
pub use preset::{
    ImageEffect, ImageEffectPreset, ImageEffectsSettings, TypeNames, apply_image_effects,
};
pub use random::RandomFast;
