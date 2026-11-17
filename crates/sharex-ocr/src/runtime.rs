//! ONNX Runtime, loaded at run time. ShareX links `Microsoft.ML.OnnxRuntime.DirectML`
//! 1.24.4 but never adds the DirectML execution provider, so every session runs on
//! the CPU provider with default options plus `ORT_ENABLE_ALL`. ShareXrs does the
//! same: no execution providers, no automatic device selection, the CPU provider's
//! default thread pool.
//!
//! The library is `onnxruntime.dll` / `libonnxruntime.dylib` / `libonnxruntime.so`,
//! from `ORT_DYLIB_PATH` or beside the executable (on macOS also the bundle's
//! `Frameworks` folder).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ort::environment::Environment;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;

use crate::OcrError;

/// The library's file names, most specific first.
#[cfg(windows)]
const LIBRARY_NAMES: &[&str] = &["onnxruntime.dll"];
#[cfg(target_os = "macos")]
const LIBRARY_NAMES: &[&str] = &["libonnxruntime.dylib"];
#[cfg(all(unix, not(target_os = "macos")))]
const LIBRARY_NAMES: &[&str] = &["libonnxruntime.so", "libonnxruntime.so.1"];

/// Where the library is looked for, in order.
pub fn library_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("ORT_DYLIB_PATH").filter(|p| !p.is_empty()) {
        out.push(PathBuf::from(p));
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        for name in LIBRARY_NAMES {
            out.push(dir.join(name));
        }
        if cfg!(target_os = "macos") {
            for name in LIBRARY_NAMES {
                out.push(dir.join("../Frameworks").join(name));
            }
        }
    }
    out
}

static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();

/// The process's ONNX Runtime environment, loading the library the first time.
pub fn environment() -> Result<&'static Environment, OcrError> {
    let env = ENVIRONMENT.get_or_init(|| {
        let candidates = library_candidates();
        let Some(path) = candidates.iter().find(|p| p.is_file()) else {
            return Err(format!(
                "ONNX Runtime wasn't found. Looked for: {}",
                candidates
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };
        let builder = ort::init_from(path).map_err(|e| e.to_string())?;
        builder
            .with_name("ShareXrs")
            .build()
            .map_err(|e| e.to_string())
    });
    env.as_ref().map_err(|e| OcrError::Runtime(e.clone()))
}

/// `new InferenceSession(modelPath, new SessionOptions { GraphOptimizationLevel = ORT_ENABLE_ALL })`.
pub fn load_session(model_path: &Path) -> Result<Session, OcrError> {
    let env = environment()?;
    let builder = Session::builder(env)?;
    let mut builder = builder
        .with_optimization_level(GraphOptimizationLevel::All)
        .map_err(|e| OcrError::Runtime(e.to_string()))?;
    Ok(builder.commit_from_file(model_path)?)
}
