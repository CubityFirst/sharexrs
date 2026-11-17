//! `ShareX.OCR/PaddleOCREngine.cs`: a detection model finds the text lines and a
//! recognition model reads each one. The models are multilingual, so there is no
//! language to choose.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sharex_skia::raster::Bitmap;

use crate::detector::{DetectionTrace, TextDetector};
use crate::geometry::Point;
use crate::recognizer::{LineTrace, TextRecognizer};
use crate::result::{OcrResult, OcrTextRegion};
use crate::{OcrError, image_tensor};

/// Models in the order they are preferred, the biggest and most accurate first. The
/// small models ship with ShareX; a user who puts the medium models in the models
/// folder gets them instead, and the tiny ones are used only when the small ones are
/// gone. Detection and recognition are picked separately.
pub const DETECTION_MODEL_NAMES: [&str; 3] = [
    "PP-OCRv6_medium_det",
    "PP-OCRv6_small_det",
    "PP-OCRv6_tiny_det",
];
pub const RECOGNITION_MODEL_NAMES: [&str; 3] = [
    "PP-OCRv6_medium_rec",
    "PP-OCRv6_small_rec",
    "PP-OCRv6_tiny_rec",
];
pub const BUILT_IN_DETECTION_MODEL_NAME: &str = "PP-OCRv6_small_det";
pub const BUILT_IN_RECOGNITION_MODEL_NAME: &str = "PP-OCRv6_small_rec";
/// Hugging Face names its ONNX downloads with this suffix, so a folder copied from
/// there as-is is found too.
pub const DOWNLOAD_FOLDER_SUFFIX: &str = "_onnx";
/// Recognized text below this confidence is usually an icon or noise read as characters.
pub const DEFAULT_MIN_SCORE: f32 = 0.5;

/// Everything one recognition saw, for comparing with ShareX.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    pub detection: DetectionTrace,
    /// The detected boxes, before recognition and the score filter.
    pub boxes: Vec<[Point; 4]>,
    /// Per box: the raw text (untrimmed) and score.
    pub texts: Vec<(String, f32)>,
    pub lines: Vec<LineTrace>,
}

pub struct PaddleOcrEngine {
    detector: TextDetector,
    recognizer: TextRecognizer,
    /// The folders of the models in use.
    pub detection_model_folder: PathBuf,
    pub recognition_model_folder: PathBuf,
}

/// `DefaultModelsFolder`: `Models` beside the application. ShareXrs also looks in a
/// macOS bundle's `Resources`, and `SHAREXRS_OCR_MODELS` overrides both (for
/// development builds, which have no models beside them).
pub fn default_models_folder() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    models_folder_for(
        std::env::consts::OS,
        std::env::var_os("SHAREXRS_OCR_MODELS"),
        &dir,
    )
}

/// [`default_models_folder`] for a platform (`std::env::consts::OS`), a
/// `SHAREXRS_OCR_MODELS` value and the executable's folder: the override, else
/// `Models` beside the executable, else (macOS, in `X.app/Contents/MacOS`, when that
/// isn't there) `X.app/Contents/Resources/Models` if it is.
pub fn models_folder_for(
    os: &str,
    override_folder: Option<std::ffi::OsString>,
    exe_dir: &Path,
) -> PathBuf {
    if let Some(p) = override_folder.filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let beside = exe_dir.join("Models");
    if os == "macos" && !beside.is_dir() && exe_dir.file_name().is_some_and(|n| n == "MacOS") {
        let resources = exe_dir.with_file_name("Resources").join("Models");
        if resources.is_dir() {
            return resources;
        }
    }
    beside
}

/// `FindModelFolder`: the first model in the list that has both its files (under its
/// name or the download name); when none has, the built-in one, so loading it
/// reports that model as missing.
pub fn find_model_folder(models_folder: &Path, model_names: &[&str], built_in: &str) -> PathBuf {
    for name in model_names {
        for folder_name in [name.to_string(), format!("{name}{DOWNLOAD_FOLDER_SUFFIX}")] {
            let folder = models_folder.join(folder_name);
            if folder.join("inference.onnx").is_file() && folder.join("inference.yml").is_file() {
                return folder;
            }
        }
    }
    models_folder.join(built_in)
}

static SHARED: Mutex<Option<Arc<PaddleOcrEngine>>> = Mutex::new(None);

impl PaddleOcrEngine {
    /// Loads the models from `models_folder`.
    pub fn new(models_folder: &Path) -> Result<Self, OcrError> {
        let detection_model_folder = find_model_folder(
            models_folder,
            &DETECTION_MODEL_NAMES,
            BUILT_IN_DETECTION_MODEL_NAME,
        );
        let recognition_model_folder = find_model_folder(
            models_folder,
            &RECOGNITION_MODEL_NAMES,
            BUILT_IN_RECOGNITION_MODEL_NAME,
        );
        let detector = TextDetector::new(&detection_model_folder)?;
        let recognizer = TextRecognizer::new(&recognition_model_folder)?;
        Ok(Self {
            detector,
            recognizer,
            detection_model_folder,
            recognition_model_folder,
        })
    }

    /// `Shared`: loading the models takes a moment, so one engine is kept for the
    /// life of the process (a failed load is tried again next time).
    pub fn shared() -> Result<Arc<PaddleOcrEngine>, OcrError> {
        let mut shared = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(engine) = shared.as_ref() {
            return Ok(engine.clone());
        }
        let engine = Arc::new(PaddleOcrEngine::new(&default_models_folder())?);
        *shared = Some(engine.clone());
        Ok(engine)
    }

    /// `Recognize`: the text regions of a (premultiplied) image whose score reaches
    /// `min_score`.
    pub fn recognize(&self, bitmap: &Bitmap, min_score: f32) -> Result<OcrResult, OcrError> {
        self.run(bitmap, min_score, None)
    }

    /// [`Self::recognize`], also returning the intermediate results.
    pub fn recognize_traced(
        &self,
        bitmap: &Bitmap,
        min_score: f32,
    ) -> Result<(OcrResult, Trace), OcrError> {
        let mut trace = Trace::default();
        let result = self.run(bitmap, min_score, Some(&mut trace))?;
        Ok((result, trace))
    }

    fn run(
        &self,
        bitmap: &Bitmap,
        min_score: f32,
        mut trace: Option<&mut Trace>,
    ) -> Result<OcrResult, OcrError> {
        if bitmap.width() == 0 || bitmap.height() == 0 {
            return Ok(OcrResult::new(Vec::new()));
        }
        let image = image_tensor::to_opaque(bitmap);
        let boxes = self
            .detector
            .detect(&image, trace.as_deref_mut().map(|t| &mut t.detection))?;
        if let Some(t) = trace.as_deref_mut() {
            t.boxes = boxes.clone();
        }
        if boxes.is_empty() {
            return Ok(OcrResult::new(Vec::new()));
        }
        let texts = self.recognizer.recognize(
            &image,
            &boxes,
            trace.as_deref_mut().map(|t| &mut t.lines),
        )?;
        let mut regions = Vec::new();
        for (b, (text, score)) in boxes.iter().zip(&texts) {
            // `string.Trim()`: Unicode white space, as `char.IsWhiteSpace` has it.
            let text = text.trim();
            if !text.is_empty() && *score >= min_score {
                regions.push(OcrTextRegion::new(*b, text.to_owned(), *score));
            }
        }
        if let Some(t) = trace {
            t.texts = texts;
        }
        Ok(OcrResult::new(regions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(name: &str, dirs: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("sxr-models-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in dirs {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        root
    }

    #[test]
    fn windows_and_linux_beside_the_exe() {
        // Portable zip / install folder, and /usr/lib/sharexrs: Models beside the exe.
        let root = dirs("beside", &["Models/PP-OCRv6_small_det"]);
        for os in ["windows", "linux"] {
            assert_eq!(models_folder_for(os, None, &root), root.join("Models"));
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn macos_bundle_resources() {
        let root = dirs(
            "mac",
            &[
                "ShareXrs.app/Contents/MacOS",
                "ShareXrs.app/Contents/Resources/Models",
            ],
        );
        let exe_dir = root.join("ShareXrs.app/Contents/MacOS");
        assert_eq!(
            models_folder_for("macos", None, &exe_dir),
            root.join("ShareXrs.app/Contents/Resources/Models")
        );
        // Only macOS looks in Resources.
        assert_eq!(
            models_folder_for("linux", None, &exe_dir),
            exe_dir.join("Models")
        );
        // Models beside the executable win.
        std::fs::create_dir_all(exe_dir.join("Models")).unwrap();
        assert_eq!(
            models_folder_for("macos", None, &exe_dir),
            exe_dir.join("Models")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_is_beside_the_exe() {
        // Nothing anywhere: the folder ShareX would report missing.
        let root = dirs("missing", &["ShareXrs.app/Contents/MacOS"]);
        let exe_dir = root.join("ShareXrs.app/Contents/MacOS");
        assert_eq!(
            models_folder_for("macos", None, &exe_dir),
            exe_dir.join("Models")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn override_wins() {
        let exe_dir = Path::new("/opt/sharexrs");
        assert_eq!(
            models_folder_for("linux", Some("/tmp/models".into()), exe_dir),
            PathBuf::from("/tmp/models")
        );
        assert_eq!(
            models_folder_for("linux", Some("".into()), exe_dir),
            exe_dir.join("Models")
        );
    }
}
