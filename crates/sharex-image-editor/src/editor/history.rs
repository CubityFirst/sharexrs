//! `EditorHistory`: undo and redo as snapshots of the annotation list
//! (`EditorMemento`), with the image itself for the destructive operations
//! (crop, cut out, rotate, resize...: [`super::editor_core::CoreMemento`]).

/// `EditorHistory.MaxMementos`.
const MAX_MEMENTOS: usize = 50;

/// `EditorHistory.MaxCanvasMementos`: full image copies kept.
const MAX_CANVAS_MEMENTOS: usize = 5;

/// What a history keeps: whether a snapshot holds the image
/// (`EditorMemento.Canvas != null`).
pub trait Snapshot {
    fn has_canvas(&self) -> bool {
        false
    }
}

/// `EditorHistory` of `M` snapshots (`CoreMemento`).
pub struct History<M> {
    undo: Vec<M>,
    redo: Vec<M>,
}

impl<M> Default for History<M> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl<M: Snapshot> History<M> {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// `AddMemento`: pushed, trimmed (`TrimUndoHistory`), redo cleared.
    pub fn add(&mut self, m: M) {
        self.undo.push(m);
        self.trim();
        self.redo.clear();
    }

    /// `TrimUndoHistory`: the newest 50 snapshots, stopping before a sixth
    /// one with an image, so undo never skips a step.
    fn trim(&mut self) {
        let mut keep = 0;
        let mut canvases = 0;
        for m in self.undo.iter().rev() {
            if keep >= MAX_MEMENTOS {
                break;
            }
            if m.has_canvas() {
                canvases += 1;
                if canvases > MAX_CANVAS_MEMENTOS {
                    break;
                }
            }
            keep += 1;
        }
        let extra = self.undo.len() - keep;
        self.undo.drain(..extra);
    }

    /// The snapshot `undo` would restore.
    pub fn peek_undo(&self) -> Option<&M> {
        self.undo.last()
    }

    /// The snapshot `redo` would restore.
    pub fn peek_redo(&self) -> Option<&M> {
        self.redo.last()
    }

    /// `Undo`: the current state goes on the redo stack.
    pub fn undo(&mut self, current: M) -> Option<M> {
        let m = self.undo.pop()?;
        self.redo.push(current);
        Some(m)
    }

    pub fn redo(&mut self, current: M) -> Option<M> {
        let m = self.redo.pop()?;
        self.undo.push(current);
        Some(m)
    }
}
