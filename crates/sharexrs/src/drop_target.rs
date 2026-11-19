//! Dropping onto the main window: `MainWindow`'s `DragDrop.DragEnter`,
//! `DragOver`, `DragLeave` and `Drop` (`UpdateDropOverlay`, `OnDrop`) and
//! `UploadManager.DragDropUpload`.
//!
//! ShareX accepts dragged files or text (Copy, with its drop overlay
//! naming which), and on the drop uploads the files, else a bitmap
//! (`RunImageTask`), else the text (`UploadText(text, taskSettings, true)`).
//! Avalonia reads the drag's data as it reads the clipboard; on Windows
//! that's OLE's data object, read here by the same code as the clipboard
//! viewer (`system_tools::native::read_data_object`) through an OLE drop
//! target of our own (winit's only reports files, one at a time).
//! Elsewhere, winit's file drops stand in (files only).
//!
//! The OS side is behind [`DropTarget`] so tests never register a real one.

use std::path::PathBuf;

use image::RgbaImage;
use sharex_tools::system::clipboard_viewer::{self as cv, ClipboardSnapshot, ClipboardValue};

/// What's dragged over the window (`DragEventArgs.DataTransfer`), as
/// `DragDropUpload` reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DropData {
    /// `TryGetFiles()`, local paths only.
    pub files: Vec<PathBuf>,
    /// `TryGetBitmap()`.
    pub bitmap: Option<RgbaImage>,
    /// `TryGetText()`.
    pub text: Option<String>,
}

/// Which of the overlay's texts shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropKind {
    /// `MainWindow_ReleaseToUploadFiles`.
    Files,
    /// `MainWindow_ReleaseToUploadText`.
    Text,
}

impl DropData {
    /// The data transfer read from a data object's formats.
    pub fn from_snapshot(snapshot: &ClipboardSnapshot) -> Self {
        let files = match snapshot.get(cv::FORMAT_FILE_DROP) {
            Some(ClipboardValue::Files(f)) => f.iter().filter(|p| !p.is_empty()).map(PathBuf::from).collect(),
            _ => Vec::new(),
        };
        let bitmap = match snapshot.get(cv::FORMAT_BITMAP) {
            Some(ClipboardValue::Bytes(png)) => image::load_from_memory(png).ok().map(|i| i.to_rgba8()),
            _ => None,
        };
        let text = match snapshot.get(cv::FORMAT_UNICODE_TEXT) {
            Some(ClipboardValue::Text(t)) => Some(t.clone()),
            _ => None,
        };
        Self { files, bitmap, text }
    }

    /// Files only (winit's drops).
    pub fn files(files: Vec<PathBuf>) -> Self {
        Self { files, ..Self::default() }
    }

    /// `UpdateDropOverlay`: files first, then non-empty text; `None` refuses
    /// the drag (`DragDropEffects.None`, no overlay).
    pub fn overlay(&self) -> Option<DropKind> {
        if !self.files.is_empty() {
            Some(DropKind::Files)
        } else if self.text.as_deref().is_some_and(|t| !t.is_empty()) {
            Some(DropKind::Text)
        } else {
            None
        }
    }
}

/// What a drop target reports.
#[derive(Debug)]
pub enum DropEvent {
    /// `DragEnter` (and `DragOver`, which has the same data): the overlay.
    Enter(Option<DropKind>),
    /// `DragLeave`.
    Leave,
    /// `Drop`: the data (only ever sent for a drag that was accepted).
    Drop(Box<DropData>),
}

/// What `UploadManager.DragDropUpload` starts.
#[derive(Clone, Debug, PartialEq)]
pub enum DropUpload {
    /// `UploadFile(files, taskSettings)`.
    Files(Vec<PathBuf>),
    /// `RunImageTask(bitmap, taskSettings)`.
    Image(RgbaImage),
    /// `UploadText(text, taskSettings, true)`.
    Text(String),
}

/// `UploadManager.DragDropUpload`: files, else a bitmap, else text.
pub fn drag_drop_upload(data: DropData) -> Option<DropUpload> {
    if !data.files.is_empty() {
        Some(DropUpload::Files(data.files))
    } else if let Some(image) = data.bitmap {
        Some(DropUpload::Image(image))
    } else {
        data.text.map(DropUpload::Text)
    }
}

/// `MainWindow`'s handlers for one event: the overlay to show
/// (`UpdateDropOverlay`; none after a leave or drop) and, for a drop, what
/// `DragDropUpload` starts. During the window's own drag (a task's file
/// dragged out, `_thumbnailDragSource != null`) neither.
pub fn handle(event: DropEvent, own_drag: bool) -> (Option<DropKind>, Option<DropUpload>) {
    match event {
        _ if own_drag => (None, None),
        DropEvent::Enter(kind) => (kind, None),
        DropEvent::Leave => (None, None),
        DropEvent::Drop(data) => (None, drag_drop_upload(*data)),
    }
}

/// Where drop events go: the event loop.
pub type DropSink = Box<dyn Fn(DropEvent)>;

/// Makes a window a drop target.
pub trait DropTarget {
    /// Registers `window` (again, after it's recreated); events go to `sink`.
    /// False where the platform's own (winit's file drops) stands in.
    fn attach(&mut self, window: &dyn winit::raw_window_handle::HasWindowHandle, sink: DropSink) -> bool;
}

/// The platform's drop target.
pub fn system() -> Box<dyn DropTarget> {
    #[cfg(windows)]
    {
        Box::new(windows::OleDropTarget)
    }
    #[cfg(not(windows))]
    {
        Box::new(WinitFiles)
    }
}

/// Outside Windows, winit's `HoveredFile`/`DroppedFile` stand in (files
/// only; Avalonia's macOS/X11 drops also carry text).
#[cfg(not(windows))]
struct WinitFiles;

#[cfg(not(windows))]
impl DropTarget for WinitFiles {
    fn attach(&mut self, _window: &dyn winit::raw_window_handle::HasWindowHandle, _sink: DropSink) -> bool {
        false
    }
}

#[cfg(windows)]
mod windows {
    use std::cell::Cell;

    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, POINTL};
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl, OleInitialize, RegisterDragDrop, RevokeDragDrop};
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::core::{Ref, implement};

    use super::{DropData, DropEvent, DropSink, DropTarget};

    pub struct OleDropTarget;

    #[implement(IDropTarget)]
    struct Target {
        sink: DropSink,
        /// What `DragEnter` decided, for `DragOver` (which has no data).
        accepted: Cell<bool>,
    }

    fn effect(accepted: bool) -> DROPEFFECT {
        if accepted { DROPEFFECT_COPY } else { DROPEFFECT_NONE }
    }

    fn read(data: Ref<IDataObject>) -> DropData {
        data.as_ref().map(|d| DropData::from_snapshot(&crate::system_tools::native::read_data_object(d))).unwrap_or_default()
    }

    impl IDropTarget_Impl for Target_Impl {
        fn DragEnter(&self, data: Ref<IDataObject>, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect_out: *mut DROPEFFECT) -> windows::core::Result<()> {
            let kind = read(data).overlay();
            self.accepted.set(kind.is_some());
            (self.sink)(DropEvent::Enter(kind));
            if !effect_out.is_null() {
                // SAFETY: OLE's out parameter.
                unsafe { *effect_out = effect(kind.is_some()) };
            }
            Ok(())
        }

        fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect_out: *mut DROPEFFECT) -> windows::core::Result<()> {
            if !effect_out.is_null() {
                // SAFETY: as above.
                unsafe { *effect_out = effect(self.accepted.get()) };
            }
            Ok(())
        }

        fn DragLeave(&self) -> windows::core::Result<()> {
            (self.sink)(DropEvent::Leave);
            Ok(())
        }

        fn Drop(&self, data: Ref<IDataObject>, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect_out: *mut DROPEFFECT) -> windows::core::Result<()> {
            let data = read(data);
            let accepted = data.overlay().is_some();
            (self.sink)(if accepted { DropEvent::Drop(Box::new(data)) } else { DropEvent::Leave });
            if !effect_out.is_null() {
                // SAFETY: as above.
                unsafe { *effect_out = effect(accepted) };
            }
            Ok(())
        }
    }

    impl DropTarget for OleDropTarget {
        fn attach(&mut self, window: &dyn HasWindowHandle, sink: DropSink) -> bool {
            let Ok(handle) = window.window_handle() else { return false };
            let RawWindowHandle::Win32(h) = handle.as_raw() else { return false };
            let hwnd = HWND(h.hwnd.get() as *mut _);
            let target: IDropTarget = Target { sink, accepted: Cell::new(false) }.into();
            // SAFETY: OLE on the window's (UI) thread; winit's own file drop
            // target is replaced by ours (it revokes the window's target
            // when the window goes).
            unsafe {
                let _ = OleInitialize(None);
                let _ = RevokeDragDrop(hwnd);
                match RegisterDragDrop(hwnd, &target) {
                    Ok(()) => true,
                    Err(e) => {
                        eprintln!("Couldn't make the main window a drop target: {e}");
                        false
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::mpsc;

    /// A drop target that keeps the sink, so a test can drag over the window.
    #[derive(Default)]
    struct FakeDropTarget {
        sinks: Rc<RefCell<Vec<DropSink>>>,
    }

    impl DropTarget for FakeDropTarget {
        fn attach(&mut self, _window: &dyn winit::raw_window_handle::HasWindowHandle, sink: DropSink) -> bool {
            self.sinks.borrow_mut().push(sink);
            true
        }
    }

    struct NoWindow;

    impl winit::raw_window_handle::HasWindowHandle for NoWindow {
        fn window_handle(&self) -> Result<winit::raw_window_handle::WindowHandle<'_>, winit::raw_window_handle::HandleError> {
            Err(winit::raw_window_handle::HandleError::Unavailable)
        }
    }

    #[test]
    fn drags_through_a_fake_target() {
        let mut target = FakeDropTarget::default();
        let sinks = target.sinks.clone();
        let (tx, rx) = mpsc::channel();
        let dyn_target: &mut dyn DropTarget = &mut target;
        assert!(dyn_target.attach(&NoWindow, Box::new(move |e| tx.send(e).unwrap())));
        let fire = |e: DropEvent| (sinks.borrow()[0])(e);
        // Text dragged in, out, in again and dropped.
        fire(DropEvent::Enter(Some(DropKind::Text)));
        fire(DropEvent::Leave);
        fire(DropEvent::Enter(Some(DropKind::Text)));
        fire(DropEvent::Drop(Box::new(DropData { text: Some("hello".into()), ..DropData::default() })));
        let results: Vec<_> = rx.try_iter().map(|e| handle(e, false)).collect();
        assert_eq!(
            results,
            vec![(Some(DropKind::Text), None), (None, None), (Some(DropKind::Text), None), (None, Some(DropUpload::Text("hello".into())))]
        );
        // The window's own drag shows nothing and uploads nothing.
        assert_eq!(handle(DropEvent::Enter(Some(DropKind::Files)), true), (None, None));
        assert_eq!(handle(DropEvent::Drop(Box::new(DropData::files(vec!["a.png".into()]))), true), (None, None));
        // Something refused: no overlay.
        assert_eq!(handle(DropEvent::Enter(None), false), (None, None));
    }

    fn snapshot(entries: &[(&str, ClipboardValue)]) -> ClipboardSnapshot {
        let mut s = ClipboardSnapshot::new();
        for (f, v) in entries {
            s.set(f, v.clone());
        }
        s
    }

    #[test]
    fn files_win_over_text_and_bitmaps() {
        let png = sharex_capture::clipboard::png_bytes(&RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255]))).unwrap();
        let s = snapshot(&[
            (cv::FORMAT_UNICODE_TEXT, ClipboardValue::Text("https://example.com/a.png".into())),
            (cv::FORMAT_FILE_DROP, ClipboardValue::Files(vec!["C:\\a.png".into(), String::new(), "C:\\b.txt".into()])),
            (cv::FORMAT_BITMAP, ClipboardValue::Bytes(png)),
        ]);
        let data = DropData::from_snapshot(&s);
        assert_eq!(data.files, vec![PathBuf::from("C:\\a.png"), PathBuf::from("C:\\b.txt")]);
        assert_eq!(data.overlay(), Some(DropKind::Files));
        assert!(data.bitmap.is_some());
        assert_eq!(drag_drop_upload(data), Some(DropUpload::Files(vec![PathBuf::from("C:\\a.png"), PathBuf::from("C:\\b.txt")])));
    }

    #[test]
    fn a_bitmap_is_uploaded_before_text() {
        let image = RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255]));
        let png = sharex_capture::clipboard::png_bytes(&image).unwrap();
        let s = snapshot(&[(cv::FORMAT_UNICODE_TEXT, ClipboardValue::Text("x".into())), (cv::FORMAT_BITMAP, ClipboardValue::Bytes(png))]);
        let data = DropData::from_snapshot(&s);
        // The overlay says text (ShareX doesn't look for bitmaps there).
        assert_eq!(data.overlay(), Some(DropKind::Text));
        assert_eq!(drag_drop_upload(data), Some(DropUpload::Image(image)));
    }

    #[test]
    fn text_and_urls() {
        let data = DropData::from_snapshot(&snapshot(&[(cv::FORMAT_UNICODE_TEXT, ClipboardValue::Text("https://getsharex.com".into()))]));
        assert_eq!(data.overlay(), Some(DropKind::Text));
        assert_eq!(drag_drop_upload(data), Some(DropUpload::Text("https://getsharex.com".into())));
    }

    #[test]
    fn nothing_usable_is_refused() {
        let empty_text = DropData::from_snapshot(&snapshot(&[(cv::FORMAT_UNICODE_TEXT, ClipboardValue::Text(String::new()))]));
        assert_eq!(empty_text.overlay(), None);
        let other = DropData::from_snapshot(&snapshot(&[("HTML Format", ClipboardValue::Bytes(vec![1, 2]))]));
        assert_eq!(other.overlay(), None);
        assert_eq!(drag_drop_upload(other), None);
        // A bitmap alone has no overlay, so ShareX refuses it before the drop.
        let png = sharex_capture::clipboard::png_bytes(&RgbaImage::new(1, 1)).unwrap();
        assert_eq!(DropData::from_snapshot(&snapshot(&[(cv::FORMAT_BITMAP, ClipboardValue::Bytes(png))])).overlay(), None);
    }
}
