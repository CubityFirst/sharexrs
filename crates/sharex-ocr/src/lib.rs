//! ShareX.OCR, ported: offline OCR with PaddleOCR's PP-OCRv6 ONNX models (the small
//! detection and recognition models ShareX ships in its `Models` folder), with
//! ShareX's preprocessing (Skia's resampling, via `sharex-skia`), thresholds and
//! post-processing, so the same image gives the same text. See docs/OCR.md.
//!
//! - [`PaddleOcrEngine`] (`PaddleOCREngine`): finds the models and runs detection
//!   then recognition; [`PaddleOcrEngine::shared`] keeps one for the process.
//! - [`OcrResult`] (`OCRResult`): the regions, joined into lines and paragraphs.
//! - [`ocr`] / [`recognize`] (`ShareX.Tools.OCRHelper`).
//!
//! ONNX Runtime is loaded at run time ([`runtime`]); without it, or without the
//! models, recognition returns an error (ShareX throws, and its callers log it).

pub mod config;
mod detector;
mod engine;
pub mod geometry;
pub mod image_tensor;
mod recognizer;
mod result;
pub mod runtime;

use std::path::PathBuf;

pub use detector::{DbParams, DetectionTrace, boxes_from_map, input_size};
pub use engine::{
    BUILT_IN_DETECTION_MODEL_NAME, BUILT_IN_RECOGNITION_MODEL_NAME, DEFAULT_MIN_SCORE,
    DETECTION_MODEL_NAMES, DOWNLOAD_FOLDER_SUFFIX, PaddleOcrEngine, RECOGNITION_MODEL_NAMES, Trace,
    default_models_folder, find_model_folder,
};
pub use geometry::{Point, Rect};
pub use recognizer::{CropBatch, LineTrace, line_crops};
pub use result::{NEW_LINE, OcrResult, OcrTextLine, OcrTextRegion};
pub use sharex_skia::raster::Bitmap;

#[derive(Debug)]
pub enum OcrError {
    /// A model file is missing (`FileNotFoundException`).
    MissingFile { what: &'static str, path: PathBuf },
    /// A model's configuration is unusable (`InvalidDataException`).
    InvalidModel(String),
    /// ONNX Runtime couldn't be loaded or failed.
    Runtime(String),
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrError::MissingFile { what, path } => write!(f, "{what} {}", path.display()),
            OcrError::InvalidModel(m) | OcrError::Runtime(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for OcrError {}

impl From<ort::Error> for OcrError {
    fn from(e: ort::Error) -> Self {
        OcrError::Runtime(e.to_string())
    }
}

/// An image from straight (unpremultiplied) RGBA bytes, premultiplied as Skia
/// decodes it.
pub fn bitmap_from_rgba(width: u32, height: u32, rgba: &[u8]) -> Bitmap {
    Bitmap::from_straight(width, height, rgba)
}

/// `OCRHelper.Recognize`: the shared engine on `bitmap`, default minimum score.
pub fn recognize(bitmap: &Bitmap) -> Result<OcrResult, OcrError> {
    PaddleOcrEngine::shared()?.recognize(bitmap, DEFAULT_MIN_SCORE)
}

/// `OCRHelper.OCR`: the recognized text.
pub fn ocr(
    bitmap: &Bitmap,
    single_line: bool,
    separate_paragraphs: bool,
) -> Result<String, OcrError> {
    Ok(recognize(bitmap)?.text(single_line, separate_paragraphs))
}
