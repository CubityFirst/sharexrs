//! ONNX Runtime, loaded at run time. ShareX links `Microsoft.ML.OnnxRuntime.DirectML`
//! 1.24.4 but never adds the DirectML execution provider, so every session runs on
//! the CPU provider with default options plus `ORT_ENABLE_ALL`. ShareXrs does the
//! same: no execution providers, no automatic device selection, the CPU provider's
//! default thread pool.
//!
//! The library is `onnxruntime.dll` / `libonnxruntime.dylib` / `libonnxruntime.so`,
//! from `ORT_DYLIB_PATH` or beside the executable (in a macOS bundle also its
//! `Contents/Frameworks` folder), where the release packages put it
//! (docs/RELEASING.md). ShareX finds its `onnxruntime.dll` beside `ShareX.exe`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ort::environment::Environment;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;

use crate::OcrError;

/// The library's file names on `os` (`std::env::consts::OS`), most specific first.
pub fn library_names(os: &str) -> &'static [&'static str] {
    match os {
        "windows" => &["onnxruntime.dll"],
        "macos" => &["libonnxruntime.dylib"],
        _ => &["libonnxruntime.so", "libonnxruntime.so.1"],
    }
}

/// Where the library is looked for, in order.
pub fn library_candidates() -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    library_candidates_for(
        std::env::consts::OS,
        std::env::var_os("ORT_DYLIB_PATH"),
        exe_dir.as_deref(),
    )
}

/// [`library_candidates`] for a platform, an `ORT_DYLIB_PATH` value and the
/// executable's folder: the override, then beside the executable, then (macOS, in
/// `X.app/Contents/MacOS`) `X.app/Contents/Frameworks`.
pub fn library_candidates_for(
    os: &str,
    override_path: Option<OsString>,
    exe_dir: Option<&Path>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = override_path.filter(|p| !p.is_empty()) {
        out.push(PathBuf::from(p));
    }
    if let Some(dir) = exe_dir {
        let names = library_names(os);
        out.extend(names.iter().map(|name| dir.join(name)));
        if os == "macos" && dir.file_name().is_some_and(|n| n == "MacOS") {
            let frameworks = dir.with_file_name("Frameworks");
            out.extend(names.iter().map(|name| frameworks.join(name)));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch folder with empty files at `files` (relative paths).
    fn layout(name: &str, files: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("sxr-ort-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for f in files {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"").unwrap();
        }
        root
    }

    fn found(os: &str, override_path: Option<OsString>, exe_dir: &Path) -> Option<PathBuf> {
        library_candidates_for(os, override_path, Some(exe_dir))
            .into_iter()
            .find(|p| p.is_file())
    }

    #[test]
    fn windows_beside_the_exe() {
        // The portable zip and the installed folder: sharexrs.exe, onnxruntime.dll.
        let root = layout("win", &["sharexrs.exe", "onnxruntime.dll"]);
        assert_eq!(
            found("windows", None, &root),
            Some(root.join("onnxruntime.dll"))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn macos_bundle_frameworks() {
        let root = layout(
            "mac",
            &[
                "ShareXrs.app/Contents/MacOS/sharexrs",
                "ShareXrs.app/Contents/Frameworks/libonnxruntime.dylib",
            ],
        );
        let exe_dir = root.join("ShareXrs.app/Contents/MacOS");
        assert_eq!(
            found("macos", None, &exe_dir),
            Some(root.join("ShareXrs.app/Contents/Frameworks/libonnxruntime.dylib"))
        );
        // Beside the executable wins over Frameworks.
        std::fs::write(exe_dir.join("libonnxruntime.dylib"), b"").unwrap();
        assert_eq!(
            found("macos", None, &exe_dir),
            Some(exe_dir.join("libonnxruntime.dylib"))
        );
        // Frameworks is only a bundle's.
        assert_eq!(
            library_candidates_for("macos", None, Some(&root)),
            vec![root.join("libonnxruntime.dylib")]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn linux_package_folder() {
        // The .deb and the tarball: /usr/lib/sharexrs/{sharexrs,libonnxruntime.so}.
        let root = layout(
            "linux",
            &[
                "usr/lib/sharexrs/sharexrs",
                "usr/lib/sharexrs/libonnxruntime.so.1",
            ],
        );
        let exe_dir = root.join("usr/lib/sharexrs");
        assert_eq!(
            found("linux", None, &exe_dir),
            Some(exe_dir.join("libonnxruntime.so.1"))
        );
        std::fs::write(exe_dir.join("libonnxruntime.so"), b"").unwrap();
        assert_eq!(
            found("linux", None, &exe_dir),
            Some(exe_dir.join("libonnxruntime.so"))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn override_comes_first() {
        let root = layout("override", &["onnxruntime.dll"]);
        let custom = root.join("custom.dll");
        let c = library_candidates_for("windows", Some(custom.clone().into()), Some(&root));
        assert_eq!(c[0], custom);
        // An empty ORT_DYLIB_PATH is ignored.
        assert_eq!(
            library_candidates_for("windows", Some(OsString::new()), Some(&root)),
            vec![root.join("onnxruntime.dll")]
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
