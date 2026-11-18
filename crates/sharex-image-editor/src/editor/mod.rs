//! The editor workspace that ShareX's `EditorView` is, shared by its hosts
//! (the image editor window and region capture): the image and its
//! annotations (`EditorCore`), drawing them with the tools
//! (`EditorInputController`), selecting, moving, resizing and rotating
//! them (`EditorSelectionController`), undo and redo (`EditorHistory`), the
//! tool options (`MainViewModel.ToolOptions`) and the keys
//! (`EditorView.OnKeyDown`/`OnKeyUp`).
//!
//! It works in image pixels and knows no window: a host feeds it input and
//! draws [`Workspace::composite`] (the image with its annotations, rendered
//! by [`crate::render`] exactly as they export) and the selection chrome
//! ([`Workspace::handles`], [`Workspace::outline`]).

pub mod editor_core;
pub mod geometry;
mod history;
pub mod image_ops;
pub mod tool_state;
mod workspace;

pub use editor_core::{CoreMemento, EditorCore};
pub use geometry::Rect;
pub use tool_state::{ToolState, Visible, max_effect_strength};
pub use workspace::*;
