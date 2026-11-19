//! ShareX's look, in egui.
//!
//! Every value here comes from ShareX's Avalonia theme
//! (`ShareX.Avalonia/Theming/*.axaml`) or a window's `.axaml`, named after
//! the resource key it ports, so a mismatch with ShareX can be traced to
//! the line it came from:
//!
//! - [`theme`]: the `ShareX.Color.*` palettes (`ShareXTheme.axaml`) and the
//!   `ShareX.Density.*` spacing (`ControlStyles.axaml`, `CompactMode.axaml`).
//! - [`fonts`]: `ShareX.FontFamily.Default` / `.Monospace`, with weights.
//! - [`icons`]: the Modern and Legacy icon packs (`ShareX.Icons`), as the
//!   same SVG files.
//! - [`menu`]: menus and flyouts (compact context menus, the tool grid).
//! - [`border`]: Avalonia's `Border` as Skia draws it (centred border,
//!   exact corner coverage), and [`focus`]: keyboard focus and its adorner.
//! - [`widgets`]: ShareX's shared controls (caption buttons, nav buttons,
//!   cards, toggles, accent buttons).
//! - [`gpu`] and [`offscreen`]: one wgpu device for every window, and
//!   rendering any UI to a PNG for comparing against ShareX.

pub mod accent;
pub mod border;
pub mod color_view;
pub mod controls;
pub mod focus;
pub mod fonts;
pub mod gpu;
pub mod keys;
pub mod menu;
pub mod icons;
pub mod offscreen;
pub mod popup;
pub mod theme;
pub mod tooltip;
pub mod widgets;

pub use theme::{Density, Palette, palette};

/// Applies ShareX's fonts and palettes to a window's context.
pub fn install(ctx: &egui::Context) {
    fonts::install(ctx);
    theme::apply(ctx);
    focus::install(ctx);
}
