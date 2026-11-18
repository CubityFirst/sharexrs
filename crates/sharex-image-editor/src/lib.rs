//! `ShareX.ImageEditor`, ported: the annotation model ([`model`]), the
//! tool settings ShareX keeps for it ([`options`]), and how each
//! annotation renders ([`render`]), drawn through `sharex-skia` so the
//! pixels match ShareX's (Avalonia on Skia m119).
//!
//! See `docs/IMAGE-EDITOR.md` for the map of ShareX's editor and what is
//! ported, and `tools/editor-harness` for the headless .NET harness that
//! renders the same annotations with ShareX's own code.

pub mod editor;
pub mod emoji_catalog;
pub mod model;
pub mod options;
pub mod render;

pub use model::{Annotation, Kind};
pub use options::ImageEditorOptions;
pub use render::render;
