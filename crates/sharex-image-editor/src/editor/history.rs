//! `EditorHistory`: undo and redo as snapshots of the annotation list
//! (`EditorMemento`), with the image itself for the operations that change
//! it (crop, cut out, flatten: `CreateCanvasMemento`).

use sharex_skia::Pixmap;

use crate::model::Annotation;

/// `EditorHistory.MaxMementos`.
const MAX_MEMENTOS: usize = 50;
/// `EditorHistory.MaxCanvasMementos`: mementos holding a copy of the image.
const MAX_CANVAS_MEMENTOS: usize = 5;

#[derive(Clone, Debug)]
pub struct Memento {
    pub annotations: Vec<Annotation>,
    pub selected: Option<String>,
    /// The image, for a canvas memento.
    pub canvas: Option<Pixmap>,
}

#[derive(Default)]
pub struct History {
    undo: Vec<Memento>,
    redo: Vec<Memento>,
}

impl History {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// `AddMemento`: pushed, trimmed (`TrimUndoHistory`: the newest 50, no
    /// more than 5 of them canvas mementos, kept contiguous), redo cleared.
    pub fn add(&mut self, m: Memento) {
        self.undo.push(m);
        let mut keep = 0;
        let mut canvases = 0;
        for m in self.undo.iter().rev() {
            if keep >= MAX_MEMENTOS {
                break;
            }
            if m.canvas.is_some() {
                canvases += 1;
                if canvases > MAX_CANVAS_MEMENTOS {
                    break;
                }
            }
            keep += 1;
        }
        let extra = self.undo.len() - keep;
        self.undo.drain(..extra);
        self.redo.clear();
    }

    /// `Undo`: the popped memento is restored and the current state goes
    /// on the redo stack (with the image when the popped one had it).
    pub fn undo(&mut self, current: impl FnOnce(bool) -> Memento) -> Option<Memento> {
        let m = self.undo.pop()?;
        self.redo.push(current(m.canvas.is_some()));
        Some(m)
    }

    pub fn redo(&mut self, current: impl FnOnce(bool) -> Memento) -> Option<Memento> {
        let m = self.redo.pop()?;
        self.undo.push(current(m.canvas.is_some()));
        Some(m)
    }

    /// `ClearAll(resetHistory: true)`: a new history.
    pub fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(canvas: bool) -> Memento {
        Memento { annotations: Vec::new(), selected: None, canvas: canvas.then(|| Pixmap::new(1, 1)) }
    }

    #[test]
    fn only_five_canvas_mementos_are_kept_and_the_history_stays_contiguous() {
        let mut h = History::default();
        for i in 0..12 {
            h.add(m(i % 2 == 0));
        }
        // Six canvas mementos (0, 2, ..., 10): the oldest goes.
        let canvases = h.undo.iter().filter(|m| m.canvas.is_some()).count();
        assert_eq!(canvases, 5);
        assert_eq!(h.undo.len(), 11);
        for _ in 0..10 {
            h.add(m(true));
        }
        assert_eq!(h.undo.len(), 5);
    }
}
