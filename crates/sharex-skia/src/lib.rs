//! The parts of Skia's CPU renderer that ShareX's drawing depends on, ported
//! from Skia milestone 119 (what SkiaSharp 3.119, and so Avalonia 12 and
//! ShareX, ship), so ShareXrs draws the same pixels. Shared by the image
//! editor (`sharex-image-editor`) and the image effects
//! (`sharex-image-effects`):
//!
//! - [`path`]: `SkPath` (conics, `addOval`, the SVG `arcTo`, convexity),
//! - [`aaa`]: the analytic anti-aliasing scan converter Skia uses for most
//!   paths (`SkScan_AAAPath.cpp`), bit for bit, and [`saa`]: the
//!   supersampler it uses for the rest (`SkScan_AntiPath.cpp`),
//! - [`scan`]: `SkScan::AntiFillPath`'s choice between them,
//! - [`blitter`]: the legacy N32 blitters that blend a solid colour,
//! - [`stroke`], [`draw`]: `SkStroke` and `drawPath` for fills and strokes,
//! - [`blur`]: `SkBlurImageFilter`'s Gaussian passes; [`resample`]: image
//!   scaling (`scalePixels`, Mitchell `drawImageRect`),
//! - [`raster`]: the raster pipeline's float and 16-bit stages, the sprite
//!   blitter, pixel conversions, matrices, image shaders and gradients over a
//!   premultiplied RGBA [`raster::Bitmap`] (`SKCanvas` for the image effects).
//!
//! Skia is BSD-licensed (Copyright Google Inc. and The Android Open Source
//! Project); this is a translation of its code.

// A transcription of Skia's C++: keep its expressions as Skia writes them
// (so the port can be checked line by line) rather than as clippy prefers.
#![allow(
    clippy::int_plus_one,
    clippy::manual_range_contains,
    clippy::nonminimal_bool,
    clippy::needless_return,
    clippy::implicit_saturating_sub,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::excessive_precision,
    clippy::too_many_arguments,
    clippy::collapsible_else_if,
    clippy::needless_range_loop,
    clippy::manual_div_ceil
)]

pub mod aaa;
pub mod blitter;
pub mod blur;
pub mod clip;
pub mod cubics;
pub mod draw;
pub mod fixed;
pub mod geom;
pub mod hair;
pub mod path;
pub mod pixmap;
pub mod raster;
pub mod resample;
pub mod saa;
pub mod scan;
pub mod stroke;

pub use draw::{Cap, Color, Join, Stroke};
pub use geom::{IRect, Point, Rect};
pub use path::{Direction, FillType, Matrix, Path};
pub use pixmap::Pixmap;
