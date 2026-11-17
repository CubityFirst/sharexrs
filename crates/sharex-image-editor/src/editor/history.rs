//! `EditorHistory`: undo and redo as snapshots of the annotation list
//! (`EditorMemento` without a canvas: region capture and the annotation
//! tools never change the image itself).

use crate::model::Annotation;

/// `EditorHistory.MaxMementos`.
const MAX_MEMENTOS: usize = 50;

#[derive(Clone, Debug)]
pub struct Memento {
    pub annotations: Vec<Annotation>,
    pub selected: Option<String>,
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

    /// `AddMemento`: pushed, trimmed to the newest 50, redo cleared.
    pub fn add(&mut self, m: Memento) {
        self.undo.push(m);
        if self.undo.len() > MAX_MEMENTOS {
            let extra = self.undo.len() - MAX_MEMENTOS;
            self.undo.drain(..extra);
        }
        self.redo.clear();
    }

    /// `Undo`: the current state goes on the redo stack.
    pub fn undo(&mut self, current: Memento) -> Option<Memento> {
        let m = self.undo.pop()?;
        self.redo.push(current);
        Some(m)
    }

    pub fn redo(&mut self, current: Memento) -> Option<Memento> {
        let m = self.redo.pop()?;
        self.undo.push(current);
        Some(m)
    }
}
