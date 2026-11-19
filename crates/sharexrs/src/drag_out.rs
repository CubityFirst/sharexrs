//! Dragging files out of ShareXrs: Avalonia's
//! `DragDrop.DoDragDropAsync(trigger, data, DragDropEffects.Copy)` with
//! the files (`DataTransferItem.CreateFile`), as the task list, the history
//! windows and the toast start it.
//!
//! On Windows that's OLE's `DoDragDrop` (Avalonia's `Win32Platform`
//! `DoDragDrop`): a shell data object for the files (`CF_HDROP` and the
//! shell's ID lists, `SHCreateDataObject`) and a drop source that drops on
//! the left button's release and cancels on Escape. The call runs its own
//! loop on the window's thread until the drop; [`DragSource::drag_files`]
//! reports the pointer as it moves so the app can redraw its own window
//! (the combine drop zones) and show "no drop" over it where Avalonia's
//! drop handlers refuse the drag.
//!
//! Behind [`DragSource`] so tests never start a real drag.

use std::path::PathBuf;

/// What the drag ended with (`DragDropEffects`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Effect {
    None,
    Copy,
    Move,
}

/// The cursor the app wants where the pointer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    /// The drop target's (OLE's default cursors).
    Default,
    /// `DragDropEffects.None`: ShareXrs's own window refuses the drop here.
    NoDrop,
}

/// How a drag ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub effect: Effect,
    /// Dropped (the button released), not cancelled (Escape).
    pub dropped: bool,
    /// The pointer's last position, desktop pixels.
    pub last: Option<(i32, i32)>,
}

pub trait DragSource {
    /// Drags `files` until they're dropped or the drag is cancelled,
    /// offering Copy (and Move with `allow_move`: the toast's
    /// `DragDropEffects.Copy | Move`). `feedback` sees the pointer (desktop
    /// pixels) each time it moves.
    fn drag_files(&mut self, files: &[PathBuf], allow_move: bool, feedback: &mut dyn FnMut((i32, i32)) -> Cursor) -> Outcome;
}

/// The platform's drag source.
pub fn system() -> Box<dyn DragSource> {
    #[cfg(windows)]
    {
        Box::new(windows::OleDragSource)
    }
    #[cfg(not(windows))]
    {
        Box::new(Unsupported)
    }
}

/// Where ShareXrs can't start a drag yet (macOS's `NSDraggingSession`,
/// X11/Wayland): logged.
#[cfg(not(windows))]
struct Unsupported;

#[cfg(not(windows))]
impl DragSource for Unsupported {
    fn drag_files(&mut self, files: &[PathBuf], _allow_move: bool, _feedback: &mut dyn FnMut((i32, i32)) -> Cursor) -> Outcome {
        eprintln!("Dragging files out isn't available on this platform yet ({} file(s)).", files.len());
        Outcome { effect: Effect::None, dropped: false, last: None }
    }
}

#[cfg(windows)]
mod windows {
    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;

    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::{CoTaskMemFree, IDataObject};
    use windows::Win32::System::Ole::{
        DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE, DROPEFFECT_NONE, DoDragDrop, IDropSource, IDropSource_Impl, OleInitialize,
    };
    use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{SHCreateDataObject, SHParseDisplayName};
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, IDC_NO, LoadCursorW, SetCursor};
    use windows::core::{BOOL, HRESULT, HSTRING, implement};
    use windows_core::AsImpl;

    use super::{Cursor, DragSource, Effect, Outcome};

    const S_OK: HRESULT = HRESULT(0);
    /// `DRAGDROP_S_DROP`, `DRAGDROP_S_CANCEL`, `DRAGDROP_S_USEDEFAULTCURSORS`.
    const DRAGDROP_S_DROP: HRESULT = HRESULT(0x0004_0100);
    const DRAGDROP_S_CANCEL: HRESULT = HRESULT(0x0004_0101);
    const DRAGDROP_S_USEDEFAULTCURSORS: HRESULT = HRESULT(0x0004_0102);

    pub struct OleDragSource;

    /// The feedback closure, borrowed for the length of `DoDragDrop`.
    type Feedback<'a> = &'a mut dyn FnMut((i32, i32)) -> Cursor;

    #[implement(IDropSource)]
    struct Source {
        // Erased to 'static: the object is released before `drag_files`
        // returns (`DoDragDrop` holds the only other reference).
        feedback: RefCell<Feedback<'static>>,
        last: Cell<Option<(i32, i32)>>,
        dropped: Cell<bool>,
    }

    impl IDropSource_Impl for Source_Impl {
        fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
            if escape.as_bool() {
                DRAGDROP_S_CANCEL
            } else if keys.0 & MK_LBUTTON.0 == 0 {
                self.dropped.set(true);
                DRAGDROP_S_DROP
            } else {
                S_OK
            }
        }

        fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
            let mut p = POINT::default();
            // SAFETY: plain Win32 calls with valid out pointers.
            unsafe {
                if GetCursorPos(&mut p).is_err() {
                    return DRAGDROP_S_USEDEFAULTCURSORS;
                }
            }
            self.last.set(Some((p.x, p.y)));
            let cursor = match self.feedback.try_borrow_mut() {
                Ok(mut f) => f((p.x, p.y)),
                Err(_) => Cursor::Default,
            };
            match cursor {
                Cursor::Default => DRAGDROP_S_USEDEFAULTCURSORS,
                Cursor::NoDrop => {
                    // SAFETY: a system cursor.
                    unsafe {
                        if let Ok(c) = LoadCursorW(None, IDC_NO) {
                            SetCursor(Some(c));
                        }
                    }
                    S_OK
                }
            }
        }
    }

    /// A shell data object for the files (absolute ID lists from the desktop).
    fn data_object(files: &[PathBuf]) -> Option<IDataObject> {
        let mut pidls: Vec<*mut ITEMIDLIST> = Vec::new();
        for f in files {
            let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
            // SAFETY: SHParseDisplayName writes an allocated ID list we free below.
            let ok = unsafe { SHParseDisplayName(&HSTRING::from(f.as_os_str()), None, &mut pidl, 0, None) };
            if ok.is_ok() && !pidl.is_null() {
                pidls.push(pidl);
            }
        }
        let result = if pidls.is_empty() {
            None
        } else {
            let list: Vec<*const ITEMIDLIST> = pidls.iter().map(|p| *p as *const _).collect();
            // SAFETY: the ID lists are valid for the call; the object copies them.
            unsafe { SHCreateDataObject::<_, IDataObject>(None, Some(&list), None).ok() }
        };
        for p in pidls {
            // SAFETY: allocated by the shell's allocator.
            unsafe { CoTaskMemFree(Some(p as *const _)) };
        }
        result
    }

    impl DragSource for OleDragSource {
        fn drag_files(&mut self, files: &[PathBuf], allow_move: bool, feedback: &mut dyn FnMut((i32, i32)) -> Cursor) -> Outcome {
            let none = Outcome { effect: Effect::None, dropped: false, last: None };
            // SAFETY: OLE on the window's thread (winit's drop target has it
            // initialised already; a second call only adds a reference).
            unsafe {
                let _ = OleInitialize(None);
            }
            let Some(data) = data_object(files) else {
                eprintln!("couldn't make a data object for {} file(s)", files.len());
                return none;
            };
            // SAFETY: the closure outlives the source object, which is
            // released (refcount back to ours, then dropped) before this returns.
            let erased: Feedback<'static> = unsafe { std::mem::transmute::<Feedback<'_>, Feedback<'static>>(feedback) };
            let source: IDropSource = Source { feedback: RefCell::new(erased), last: Cell::new(None), dropped: Cell::new(false) }.into();
            let mut effect = DROPEFFECT_NONE;
            // SAFETY: valid COM objects; `DoDragDrop` runs its loop here.
            let allowed = if allow_move { DROPEFFECT(DROPEFFECT_COPY.0 | DROPEFFECT_MOVE.0) } else { DROPEFFECT_COPY };
            let hr = unsafe { DoDragDrop(&data, &source, allowed, &mut effect) };
            // SAFETY: our own implementation behind the interface.
            let inner: &Source = unsafe { source.as_impl() };
            let outcome = Outcome {
                effect: if effect.0 & DROPEFFECT_MOVE.0 != 0 {
                    Effect::Move
                } else if effect.0 & DROPEFFECT_COPY.0 != 0 {
                    Effect::Copy
                } else {
                    Effect::None
                },
                dropped: hr == DRAGDROP_S_DROP && inner.dropped.get(),
                last: inner.last.get(),
            };
            drop(source);
            outcome
        }
    }
}

/// A drag source for tests: replays pointer positions and ends as told.
#[cfg(test)]
pub struct MockDragSource {
    pub moves: Vec<(i32, i32)>,
    pub effect: Effect,
    pub dropped: bool,
    /// What each drag carried and the cursors the app asked for.
    pub dragged: Vec<Vec<PathBuf>>,
    pub cursors: Vec<Cursor>,
}

#[cfg(test)]
impl DragSource for MockDragSource {
    fn drag_files(&mut self, files: &[PathBuf], _allow_move: bool, feedback: &mut dyn FnMut((i32, i32)) -> Cursor) -> Outcome {
        self.dragged.push(files.to_vec());
        for m in &self.moves {
            let c = feedback(*m);
            self.cursors.push(c);
        }
        Outcome { effect: self.effect, dropped: self.dropped, last: self.moves.last().copied() }
    }
}
