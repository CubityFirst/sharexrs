//! The parts of Skia's CPU renderer that ShareX's image editor output
//! depends on, ported from Skia milestone 119 (what SkiaSharp 3.119, and
//! so Avalonia 12 and ShareX, ship), so ShareXrs draws the same pixels:
//!
//! - [`path`]: `SkPath` (conics, `addOval`, the SVG `arcTo`, convexity),
//! - [`aaa`]: the analytic anti-aliasing scan converter Skia uses for most
//!   paths (`SkScan_AAAPath.cpp`), bit for bit,
//! - [`scan`]: `SkScan::AntiFillPath`'s choice between analytic AA and
//!   supersampling (the latter through tiny-skia, a port of Skia's),
//! - [`blitter`]: the legacy N32 blitters that blend a solid colour,
//! - [`draw`]: `drawPath` for fills and strokes.
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
    clippy::needless_range_loop
)]

pub mod aaa;
pub mod blitter;
pub mod draw;
pub mod fixed;
pub mod geom;
pub mod path;
pub mod pixmap;
pub mod scan;
pub mod stroke;

pub use draw::{Cap, Color, Join, Stroke};
pub use geom::{IRect, Point, Rect};
pub use path::{Direction, FillType, Matrix, Path};
pub use pixmap::Pixmap;
