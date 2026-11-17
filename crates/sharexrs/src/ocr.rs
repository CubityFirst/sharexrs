//! ShareX's OCR task helpers over `sharex-ocr`: `TaskHelpers.OCRImage` (the
//! after capture task), `AsyncOCRImage` (its silent path) and
//! `RecognizeTextForHistoryAsync` (`HistorySaveOCRText`). The OCR window (the
//! non-silent path) isn't ported yet; docs/OCR.md has the plan.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use image::RgbaImage;
use serde_json::Value;
use sharex_settings::TaskSettings;

use crate::sound;
use crate::upload::TaskUi;

/// Text being recognized on another thread (`Task<string>`); cloning shares it.
#[derive(Clone, Default)]
pub struct PendingText(Arc<Mutex<Option<JoinHandle<Option<String>>>>>);

impl std::fmt::Debug for PendingText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PendingText")
    }
}

impl PendingText {
    fn spawn(work: impl FnOnce() -> Option<String> + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Some(std::thread::spawn(work)))))
    }

    /// `await`: the text, or `None` when recognition failed (the first caller
    /// gets it; later ones get `None`).
    pub fn wait(&self) -> Option<String> {
        let handle = self.0.lock().unwrap_or_else(|e| e.into_inner()).take()?;
        handle.join().ok().flatten()
    }
}

fn bitmap(image: &RgbaImage) -> sharex_ocr::Bitmap {
    sharex_ocr::bitmap_from_rgba(image.width(), image.height(), image.as_raw())
}

/// `TaskHelpers.RecognizeTextForHistoryAsync`: takes the image; the text is kept
/// on one line, so it stays one line in the history tag editor. A failure (such
/// as missing models) is logged and gives no text.
pub fn recognize_text_for_history(image: std::sync::Arc<RgbaImage>) -> PendingText {
    PendingText::spawn(move || match sharex_ocr::ocr(&bitmap(&image), true, true) {
        Ok(text) => Some(text),
        Err(e) => {
            eprintln!("OCR for history failed: {e}");
            None
        }
    })
}

/// `OCROptions`, as far as the after capture task reads it (kept as JSON in the
/// task settings until the OCR window is ported).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcrOptions {
    pub single_line: bool,
    pub separate_paragraphs: bool,
    pub silent: bool,
}

impl OcrOptions {
    pub fn from_task(task: &TaskSettings) -> Self {
        let options = task.CaptureSettings.as_ref().map(|c| &c.OCROptions);
        let flag = |key: &str, default: bool| options.and_then(|o| o.get(key)).and_then(Value::as_bool).unwrap_or(default);
        Self { single_line: flag("SingleLine", false), separate_paragraphs: flag("SeparateParagraphs", true), silent: flag("Silent", false) }
    }
}

/// `TaskHelpers.OCRImage(bmp, taskSettings)` from `WorkerTask.DoOCR` (no file
/// path, so no text file is written).
pub fn ocr_image(image: &RgbaImage, task: &TaskSettings, ui: &dyn TaskUi) {
    let options = OcrOptions::from_task(task);
    if !options.silent {
        // `ToolsIntegration.ShowOCRWindowAsync`.
        eprintln!("The OCR window isn't ported yet; turn on \"Process OCR silently\" to copy the text.");
        return;
    }
    // `AsyncOCRImage`; ShareX shows an error box for exceptions (`ShowError`).
    match sharex_ocr::ocr(&bitmap(image), options.single_line, options.separate_paragraphs) {
        Ok(text) => {
            if !text.is_empty() {
                ui.copy_text(&text);
            } else {
                ui.clear_clipboard();
            }
            // `PlayNotificationSoundAsync(NotificationSound.ActionCompleted, taskSettings)`.
            if task.GeneralSettings.as_ref().is_none_or(|g| g.PlaySoundAfterAction) {
                sound::play_action_completed();
            }
        }
        Err(e) => eprintln!("OCR failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_come_from_the_capture_settings_json() {
        let mut task = TaskSettings::default();
        assert_eq!(OcrOptions::from_task(&task), OcrOptions { single_line: false, separate_paragraphs: true, silent: false });
        let capture = task.CaptureSettings.get_or_insert_with(Default::default);
        capture.OCROptions = serde_json::json!({ "SingleLine": true, "SeparateParagraphs": false, "Silent": true });
        assert_eq!(OcrOptions::from_task(&task), OcrOptions { single_line: true, separate_paragraphs: false, silent: true });
    }

    #[test]
    fn a_pending_text_is_taken_once() {
        let p = PendingText::spawn(|| Some("hi".into()));
        let q = p.clone();
        assert_eq!(p.wait().as_deref(), Some("hi"));
        assert_eq!(q.wait(), None);
        assert_eq!(PendingText::default().wait(), None);
    }
}
