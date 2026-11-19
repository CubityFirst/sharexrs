//! The engines behind ShareX's Tools menu (`ShareX.Tools`, plus the parts of
//! `ShareX.HelpersLib` and `TaskHelpers` the tools call), without their
//! windows. One module per tool:
//!
//! - Images: [`combiner`], [`splitter`], [`thumbnailer`], [`resizer`],
//!   [`converter`], [`watermark`], [`icon`] (`.ico` writing), [`comparer`],
//!   [`background_remover`] (an ONNX model on ONNX Runtime).
//! - Data: [`hash`], [`qr`], [`indexer`], [`metadata`], [`analyze_image`].
//! - Video: [`video`] (the FFmpeg command lines and output parsing of the
//!   video converter, thumbnailer, trimmer and GIF tools; [`gif`] for the GIF
//!   maker's encoder).
//! - Screen: [`color`] (the colour pickers' maths and texts), [`ruler`]
//!   (the Ruler's pixel runs and content bounds), [`mouse_highlighter`],
//!   [`pin`] (Pin to screen's options and sizes).
//! - System: [`monitor_test`] (the test patterns as images), [`system`]
//!   (the clipboard viewer, inspect window, network monitor and borderless
//!   window without their windows).
//!
//! Shared: [`imaging`] (`SkiaImageHelpers`: loading, drawing, ShareX's
//! encoders), [`gif`] (ShareX's GIF quantizers and writer), [`dotnet`] (the
//! .NET behaviour outputs depend on), [`text`] (`string.Format`).
//!
//! docs/TOOLS.md maps every tool, says what matches ShareX (validated with
//! `tools/tools-harness`) and plans the windows.

// Much of this transcribes C/C# (libjpeg-turbo, ShareX's quantizers): keep their
// loops and branches as written so the port can be checked line by line.
#![allow(
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::manual_is_multiple_of
)]

pub mod analyze_image;
pub mod background_remover;
pub mod capture_image;
pub mod color;
pub mod combiner;
pub mod comparer;
pub mod converter;
pub mod dotnet;
pub mod gif;
pub mod hash;
pub mod icon;
pub mod imaging;
pub mod indexer;
pub mod jpeg;
pub mod jpeg_decode;
pub mod metadata;
pub mod monitor_test;
pub mod mouse_highlighter;
pub mod options;
pub mod pin;
pub mod system;
pub mod qr;
pub mod resizer;
pub mod ruler;
pub mod splitter;
pub mod text;
pub mod thumbnailer;
pub mod video;
pub mod watermark;
pub mod webp;

/// `Links.Website`.
pub const WEBSITE: &str = "https://getsharex.com";
