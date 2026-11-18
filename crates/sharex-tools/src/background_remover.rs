//! Background remover: `BackgroundRemovalService` (an ONNX matting or
//! segmentation model the user puts in the models folder: the image resized
//! to the model's input with Catmull-Rom, ImageNet-normalised, the output
//! turned into an alpha mask, resized back and applied with `DstIn`),
//! `BackgroundRemovalModel`, `BackgroundRemoverOptions` and the view
//! model's model list. No model ships with ShareX or ShareXrs.
//!
//! ONNX Runtime is `sharex_ocr::runtime`'s (loaded at run time). ShareX
//! runs the GPU and Auto devices on DirectML; ShareXrs's ONNX Runtime has
//! no DirectML, so Auto runs on the CPU ("CPU (GPU unavailable)", as
//! ShareX says when DirectML fails) and GPU fails.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Value};
use sharex_skia::raster::{Blend, Canvas, Paint, Rect, Sampling, Shader};

use crate::imaging::Bitmap;

/// `BackgroundRemovalDevice`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Device {
    #[default]
    Auto,
    Gpu,
    Cpu,
}

impl Device {
    pub const ALL: [Device; 3] = [Device::Auto, Device::Gpu, Device::Cpu];

    /// The enum name (the device combo box's text and the JSON value).
    pub fn name(self) -> &'static str {
        match self {
            Device::Auto => "Auto",
            Device::Gpu => "GPU",
            Device::Cpu => "CPU",
        }
    }
}

/// `BackgroundRemoverOptions` (`TaskSettingsTools.BackgroundRemoverOptions`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackgroundRemoverOptions {
    pub selected_model_file_name: String,
    pub selected_device: Device,
}

impl BackgroundRemoverOptions {
    /// Newtonsoft's reading: an unknown device keeps the default, and the
    /// window resets an undefined number to Auto (`Enum.IsDefined`).
    pub fn from_json(v: &Value) -> Self {
        let names: Vec<&str> = Device::ALL.iter().map(|d| d.name()).collect();
        BackgroundRemoverOptions {
            selected_model_file_name: v.get("SelectedModelFileName").map_or(String::new(), |x| crate::options::json_str_or(x, "")),
            selected_device: v
                .get("SelectedDevice")
                .and_then(|x| crate::options::enum_name(x, &names))
                .map_or(Device::Auto, |i| Device::ALL[i]),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({ "SelectedModelFileName": self.selected_model_file_name, "SelectedDevice": self.selected_device.name() })
    }
}

/// `BackgroundRemovalModel`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub path: PathBuf,
    pub file_name: String,
    pub file_size: u64,
}

impl Model {
    /// `DisplayName`: `name.onnx (176.3 MB)`.
    pub fn display_name(&self) -> String {
        format!("{} ({})", self.file_name, format_file_size(self.file_size))
    }
}

/// `BackgroundRemovalModel.FormatFileSize`: B, KB, MB or GB (1024),
/// `0` bytes, `0.#` otherwise.
pub fn format_file_size(size: u64) -> String {
    let units = ["B", "KB", "MB", "GB"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let spec = if unit == 0 { "0" } else { "0.#" };
    format!("{} {}", crate::text::format_number(value, spec, false), units[unit])
}

/// `RefreshModels`: the folder (created if missing) scanned for `*.onnx`
/// files, top level only, ordered by file name.
pub fn scan_models(folder: &Path) -> Vec<Model> {
    let _ = std::fs::create_dir_all(folder);
    let Ok(dir) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut models: Vec<Model> = dir
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            // Directory.EnumerateFiles("*.onnx"): the extension, any case.
            let is_onnx = Path::new(&name).extension().is_some_and(|x| x.eq_ignore_ascii_case("onnx"));
            (meta.is_file() && is_onnx).then(|| Model {
                path: std::path::absolute(e.path()).unwrap_or(e.path()),
                file_name: name,
                file_size: meta.len(),
            })
        })
        .collect();
    // OrderBy(Path.GetFileName): the current culture's comparison.
    models.sort_by(|a, b| sharex_helpers::culture::current_culture_cmp(&a.file_name, &b.file_name));
    models
}

/// The model `RefreshModels` selects: the saved one (ignoring case), else
/// the first.
pub fn select_model(models: &[Model], saved_file_name: &str) -> Option<usize> {
    models
        .iter()
        .position(|m| sharex_helpers::culture::ordinal_ignore_case_eq(&m.file_name, saved_file_name))
        .or(if models.is_empty() { None } else { Some(0) })
}

/// `ResolveInputShape`: the input's width, height and layout (1024x1024
/// NCHW unless the model's 4-D input says otherwise; dynamic dimensions are
/// -1).
pub fn resolve_input_shape(dims: &[i64]) -> (i32, i32, bool) {
    let (mut width, mut height) = (1024, 1024);
    if dims.len() != 4 {
        return (width, height, false);
    }
    let channel = if dims[1] == 3 { 1 } else if dims[3] == 3 { 3 } else { 1 };
    let channels_last = channel == 3;
    let (hi, wi) = if channels_last { (1, 2) } else { (2, 3) };
    if dims[hi] > 0 {
        height = dims[hi] as i32;
    }
    if dims[wi] > 0 {
        width = dims[wi] as i32;
    }
    (width, height, channels_last)
}

/// `ResizeBitmap`: a copy at the same size, else drawn with Catmull-Rom
/// (an anti-aliased paint) into a transparent bitmap.
pub fn resize_bitmap(source: &Bitmap, width: u32, height: u32) -> Bitmap {
    if source.width() == width && source.height() == height {
        return source.clone();
    }
    let mut out = Bitmap::new(width, height);
    let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
    paint.anti_alias = true;
    Canvas::new(&mut out).draw_image_rect(
        source,
        None,
        Rect::xywh(0.0, 0.0, width as f32, height as f32),
        Sampling::CatmullRom,
        Some(&paint),
    );
    out
}

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// `CreateInputTensor`: RGB, unpremultiplied (`255f / a`), scaled to 0-1
/// and ImageNet-normalised, planar (`[1, 3, h, w]`) or interleaved
/// (`[1, h, w, 3]`).
pub fn create_input_tensor(bitmap: &Bitmap, channels_last: bool) -> Vec<f32> {
    let (w, h) = (bitmap.width() as usize, bitmap.height() as usize);
    let n = w * h;
    let mut t = vec![0f32; n * 3];
    let data = bitmap.data();
    for i in 0..n {
        let p = &data[i * 4..i * 4 + 4];
        let a = p[3];
        let scale = if a > 0 && a < 255 { 255f32 / a as f32 } else { 1f32 };
        let c = |v: u8, k: usize| (((v as f32 * scale).min(255f32) / 255f32) - MEAN[k]) / STD[k];
        let (r, g, b) = (c(p[0], 0), c(p[1], 1), c(p[2], 2));
        if channels_last {
            t[i * 3] = r;
            t[i * 3 + 1] = g;
            t[i * 3 + 2] = b;
        } else {
            t[i] = r;
            t[n + i] = g;
            t[2 * n + i] = b;
        }
    }
    t
}

/// `ResolveOutputShape`: the mask's width and height.
pub fn resolve_output_shape(dims: &[i64]) -> (i64, i64) {
    let n = dims.len();
    if n >= 4 {
        let channels_last = dims[n - 1] == 1 && dims[n - 3] > 1;
        if channels_last { (dims[n - 2], dims[n - 3]) } else { (dims[n - 1], dims[n - 2]) }
    } else if n >= 2 {
        (dims[n - 1], dims[n - 2])
    } else {
        (1, 1)
    }
}

/// `CreateMaskBitmap`: the output's first `w * h` values, through a sigmoid
/// if any is negative, stretched to 0-1 and rounded (to even) to alpha.
pub fn create_mask(values: &[f32], dims: &[i64]) -> Result<Bitmap, String> {
    let (w, h) = resolve_output_shape(dims);
    if w <= 0 || h <= 0 {
        return Err("The ONNX model returned an invalid mask size.".into());
    }
    let count = (w * h) as usize;
    if values.len() < count {
        return Err("The ONNX model returned an incomplete mask.".into());
    }
    let mut v: Vec<f32> = values[..count].to_vec();
    let (mut min, mut max) = (f32::MAX, f32::MIN);
    for &x in &v {
        if x < min {
            min = x;
        }
        if x > max {
            max = x;
        }
    }
    if min < 0.0 {
        min = f32::MAX;
        max = f32::MIN;
        for x in v.iter_mut() {
            *x = 1f32 / (1f32 + (-*x).exp());
            if *x < min {
                min = *x;
            }
            if *x > max {
                max = *x;
            }
        }
    }
    let range = (max - min).max(0.000001f32);
    let mut px = vec![0u8; count * 4];
    for (i, x) in v.iter().enumerate() {
        let normalized = ((x - min) / range).clamp(0.0, 1.0);
        px[i * 4 + 3] = (normalized * 255f32).round_ties_even() as u8;
    }
    Ok(Bitmap::from_premultiplied(w as u32, h as u32, px))
}

/// `ApplyAlphaMask`: the source, then the mask drawn over it with `DstIn`.
pub fn apply_alpha_mask(source: &Bitmap, mask: &Bitmap) -> Bitmap {
    let mut out = Bitmap::new(source.width(), source.height());
    let mut c = Canvas::new(&mut out);
    c.draw_bitmap(source, 0.0, 0.0, None);
    let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
    paint.blend = Blend::DstIn;
    c.draw_bitmap(mask, 0.0, 0.0, Some(&paint));
    out
}

/// `BackgroundRemovalResult`.
#[derive(Debug, Clone)]
pub struct RemovalResult {
    pub image: Bitmap,
    pub session_cached: bool,
    pub execution_device: String,
    pub session_ms: u128,
    pub preprocessing_ms: u128,
    pub inference_ms: u128,
    pub postprocessing_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CacheKey {
    path: PathBuf,
    size: u64,
    modified: Option<std::time::SystemTime>,
    device: Device,
}

/// `BackgroundRemovalService`: keeps the last model's session while the
/// model file (path, size, time) and device stay the same.
#[derive(Default)]
pub struct BackgroundRemovalService {
    cache: Option<(CacheKey, ort::session::Session, String)>,
}

impl BackgroundRemovalService {
    pub fn new() -> Self {
        Self::default()
    }

    /// `RemoveBackground`.
    pub fn remove_background(&mut self, source: &Bitmap, model: &Model, device: Device) -> Result<RemovalResult, String> {
        if source.width() == 0 || source.height() == 0 {
            return Err("Selected image is empty.".into());
        }
        let meta = std::fs::metadata(&model.path).ok().filter(|m| m.is_file());
        let Some(meta) = meta else {
            return Err("The background removal model file is missing.".into());
        };
        let started = Instant::now();
        let key = CacheKey {
            path: std::path::absolute(&model.path).unwrap_or(model.path.clone()),
            size: meta.len(),
            modified: meta.modified().ok(),
            device,
        };
        let cached = self.cache.as_ref().is_some_and(|(k, _, _)| *k == key);
        if !cached {
            self.cache = None;
            let execution = match device {
                Device::Cpu => "CPU",
                Device::Auto => "CPU (GPU unavailable)",
                Device::Gpu => return Err("DirectML isn't available in ShareXrs's ONNX Runtime.".into()),
            };
            let session = sharex_ocr::runtime::load_session(&model.path).map_err(|e| e.to_string())?;
            self.cache = Some((key, session, execution.to_string()));
        }
        let session_ms = started.elapsed().as_millis();
        let (_, session, execution) = self.cache.as_mut().unwrap();
        let execution = execution.clone();
        run_model(session, source, cached, execution, session_ms)
    }
}

fn run_model(
    session: &mut ort::session::Session,
    source: &Bitmap,
    cached: bool,
    execution_device: String,
    session_ms: u128,
) -> Result<RemovalResult, String> {
    let input = session.inputs().first().ok_or("The ONNX model does not expose an input tensor.")?;
    let input_name = input.name().to_string();
    let dims: Vec<i64> = match input.dtype() {
        ort::value::ValueType::Tensor { shape, .. } => shape.iter().copied().collect(),
        _ => Vec::new(),
    };
    let (w, h, channels_last) = resolve_input_shape(&dims);
    let phase = Instant::now();
    let resized = resize_bitmap(source, w as u32, h as u32);
    let tensor = create_input_tensor(&resized, channels_last);
    let preprocessing_ms = phase.elapsed().as_millis();

    let phase = Instant::now();
    let (w, h) = (w as usize, h as usize);
    let shape = if channels_last { [1usize, h, w, 3] } else { [1usize, 3, h, w] };
    let value = ort::value::Tensor::from_array((shape, tensor)).map_err(|e| e.to_string())?;
    let outputs = session.run(ort::inputs![input_name.as_str() => value]).map_err(|e| e.to_string())?;
    if outputs.len() == 0 {
        return Err("The ONNX model did not return a usable output tensor.".into());
    }
    let (out_shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|_| "The ONNX model did not return a usable output tensor.".to_string())?;
    let out_dims: Vec<i64> = out_shape.iter().copied().collect();
    let values = data.to_vec();
    drop(outputs);
    let inference_ms = phase.elapsed().as_millis();

    let phase = Instant::now();
    let mask = create_mask(&values, &out_dims)?;
    let mask = resize_bitmap(&mask, source.width(), source.height());
    let image = apply_alpha_mask(source, &mask);
    Ok(RemovalResult {
        image,
        session_cached: cached,
        execution_device,
        session_ms,
        preprocessing_ms,
        inference_ms,
        postprocessing_ms: phase.elapsed().as_millis(),
    })
}

/// `LoadImage`/`RemoveBackgroundAsync`'s `SKBitmap.Decode(path)`: no EXIF
/// rotation.
pub fn load_image(path: &Path) -> Option<Bitmap> {
    let bytes = std::fs::read(path).ok()?;
    crate::imaging::decode(&bytes, false).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_and_sizes() {
        assert_eq!(resolve_input_shape(&[1, 3, 320, 480]), (480, 320, false));
        assert_eq!(resolve_input_shape(&[1, 320, 480, 3]), (480, 320, true));
        assert_eq!(resolve_input_shape(&[-1, 3, -1, -1]), (1024, 1024, false));
        assert_eq!(resolve_input_shape(&[1, 3, 64]), (1024, 1024, false));
        assert_eq!(resolve_output_shape(&[1, 1, 20, 30]), (30, 20));
        assert_eq!(resolve_output_shape(&[1, 20, 30, 1]), (30, 20));
        assert_eq!(resolve_output_shape(&[20, 30]), (30, 20));
        assert_eq!(resolve_output_shape(&[7]), (1, 1));
        assert_eq!(format_file_size(1000), "1000 B");
        assert_eq!(format_file_size(184_844_582), "176.3 MB");
        assert_eq!(format_file_size(1024), "1 KB");
    }

    #[test]
    fn mask_and_tensor() {
        let m = create_mask(&[-1.0, 0.0, 1.0, 3.0], &[1, 1, 2, 2]).unwrap();
        let a: Vec<u8> = (0..4).map(|i| m.px(i)[3]).collect();
        assert_eq!((a[0], a[3]), (0, 255));
        let m = create_mask(&[0.0, 0.25, 0.5, 1.0], &[2, 2]).unwrap();
        assert_eq!((0..4).map(|i| m.px(i)[3]).collect::<Vec<_>>(), [0, 64, 128, 255]);
        assert!(create_mask(&[0.0], &[1, 1, 2, 2]).is_err());
        let b = Bitmap::from_premultiplied(1, 1, vec![64, 32, 0, 128]);
        let t = create_input_tensor(&b, false);
        assert!((t[0] - ((64.0 * (255.0 / 128.0) / 255.0 - 0.485) / 0.229)).abs() < 1e-6);
        let o = BackgroundRemoverOptions::from_json(&json!({"SelectedModelFileName": "x.onnx", "SelectedDevice": 9}));
        assert_eq!(o.selected_device, Device::Auto);
        assert_eq!(BackgroundRemoverOptions::from_json(&json!({"SelectedDevice": "cpu"})).selected_device, Device::Cpu);
    }
}
