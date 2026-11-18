//! `src/pathops`: Skia m119's path operations (`SkPathOps.h`: `Op` and
//! `Simplify`), ported file by file so the result path is Skia's, contour
//! order and starting points included: the stroker's joins, and so the
//! pixels, depend on them. ShareX unions a step's or a speech balloon's body
//! with its tail through `SKPath.Op(SKPathOp.Union)` (Avalonia's
//! `CombinedGeometry`).
//!
//! The port keeps Skia's arithmetic (doubles where Skia uses doubles, floats
//! where it uses floats, no fused multiply-adds) and its control flow, with
//! release-build behaviour (asserts off). Debug-only code is left out.
//! `SkOpBuilder`, `AsWinding` and `TightBounds` aren't ported.

// Skia's API is ported whole, including the parts `Op` doesn't reach.
#![allow(dead_code, clippy::wrong_self_convention, clippy::ptr_eq)]

mod add_intersections;
mod angle;
mod arena;
mod coincidence;
mod common;
pub mod conic;
mod contour;
pub mod cubic;
mod curve;
mod edge_builder;
mod geometry;
mod graph;
pub mod intersections;
pub mod line;
mod line_intersections;
mod line_parameters;
mod op;
pub mod point;
pub mod quad;
mod rect;
mod reduce_order;
mod segment;
mod sort;
mod span;
mod tsect;
mod types;
mod winding;
mod writer;

pub use op::{op, simplify};

/// `SkPathOp`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathOp {
    /// Subtract the op path from the first path.
    Difference = 0,
    /// Intersect the two paths.
    Intersect = 1,
    /// Union (inclusive-or) the two paths.
    Union = 2,
    /// Exclusive-or the two paths.
    Xor = 3,
    /// Subtract the first path from the op path.
    ReverseDifference = 4,
}
