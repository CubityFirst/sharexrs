//! ShareXrs's OCR against ShareX's own (tools/ocr-harness), on the fixtures
//! `make_fixtures.py` copied: everything that runs without ONNX Runtime and the
//! models (detection input sizes and pixels, line crops, boxes from ShareX's
//! probability maps, lines and paragraphs) bit for bit. With ONNX Runtime and the
//! models (`ORT_DYLIB_PATH`, `SHAREXRS_OCR_MODELS`), the whole engine too.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sharex_ocr::{Bitmap, DbParams, NEW_LINE, OcrResult, OcrTextRegion, Point, image_tensor};

/// `PostProcess` in ShareX's `Models/PP-OCRv6_small_det/inference.yml`.
const SMALL_DET: DbParams = DbParams { threshold: 0.2, box_threshold: 0.45, unclip_ratio: 1.4, max_candidates: 3000 };
/// `image_shape` in `Models/PP-OCRv6_small_rec/inference.yml`: 3 x 48 x 320.
const REC_HEIGHT: i32 = 48;
const REC_MIN_WIDTH: i32 = 320;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn inflate(path: &Path) -> Option<Vec<u8>> {
    let z = std::fs::read(path).ok()?;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&z[..]).read_to_end(&mut out).unwrap();
    Some(out)
}

fn results() -> serde_json::Map<String, Value> {
    let bytes = inflate(&fixtures().join("results.json.z")).unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap().as_object().unwrap().clone()
}

fn bits(v: &Value) -> Vec<u32> {
    v.as_array().unwrap().iter().map(|b| b.as_u64().unwrap() as u32).collect()
}

fn points(v: &Value) -> [Point; 4] {
    let b = bits(v);
    std::array::from_fn(|i| Point::new(f32::from_bits(b[i * 2]), f32::from_bits(b[i * 2 + 1])))
}

fn box_bits(b: &[Point; 4]) -> Vec<u32> {
    b.iter().flat_map(|p| [p.x.to_bits(), p.y.to_bits()]).collect()
}

/// A test image as ShareX loads it: premultiplied.
fn load(name: &str) -> Bitmap {
    let img = image::open(fixtures().join("inputs").join(format!("{name}.png"))).unwrap().to_rgba8();
    sharex_ocr::bitmap_from_rgba(img.width(), img.height(), img.as_raw())
}

/// ShareX's BGRA against our RGBA: the number of differing pixels.
fn differing(rgba: &[u8], bgra: &[u8]) -> usize {
    assert_eq!(rgba.len(), bgra.len(), "size");
    rgba.chunks_exact(4).zip(bgra.chunks_exact(4)).filter(|(a, b)| [b[2], b[1], b[0], b[3]] != **a).count()
}

/// ShareX writes `Environment.NewLine`, which is CRLF on Windows.
fn sharex_text(v: &Value) -> String {
    v.as_str().unwrap().replace("\r\n", NEW_LINE)
}

#[test]
fn detection_input_sizes_match_sharex() {
    for (name, r) in results() {
        let size = sharex_ocr::input_size(r["width"].as_i64().unwrap() as i32, r["height"].as_i64().unwrap() as i32);
        let expected = (r["detInputWidth"].as_i64().unwrap() as i32, r["detInputHeight"].as_i64().unwrap() as i32);
        assert_eq!(size, expected, "{name}");
    }
}

#[test]
fn preprocessing_matches_sharex_pixel_for_pixel() {
    let results = results();
    let mut checked = 0;
    for entry in std::fs::read_dir(fixtures().join("inputs")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let r = &results[&name];
        let image = image_tensor::to_opaque(&load(&name));

        // The detection model's input (Catmull-Rom up, mipmapped bilinear down).
        if let Some(expected) = inflate(&fixtures().join(format!("{name}.resized.z"))) {
            let (w, h) = (r["detInputWidth"].as_u64().unwrap() as u32, r["detInputHeight"].as_u64().unwrap() as u32);
            let resized = image_tensor::resize(&image, w, h);
            assert_eq!(differing(resized.data(), &expected), 0, "{name}: detection input");
            checked += 1;
        }

        // The line crops, batched as ShareX batches them.
        let boxes: Vec<[Point; 4]> = r["boxes"].as_array().unwrap().iter().map(points).collect();
        let crops = r["crops"].as_array().unwrap();
        for batch in sharex_ocr::line_crops(&image, &boxes, REC_HEIGHT, REC_MIN_WIDTH) {
            for (k, (&i, crop)) in batch.lines.iter().zip(&batch.crops).enumerate() {
                let meta = &crops[i];
                assert_eq!(batch.input_width as u64, meta["inputWidth"].as_u64().unwrap(), "{name} #{i}: batch width");
                assert_eq!(crop.width() as u64, meta["width"].as_u64().unwrap(), "{name} #{i}: crop width");
                assert_eq!(meta["batch"].as_u64().unwrap() % 6, k as u64, "{name} #{i}: batch order");
                if let Some(expected) = inflate(&fixtures().join(format!("{name}.crop_{i}.z"))) {
                    assert_eq!(differing(crop.data(), &expected), 0, "{name} #{i}: crop");
                    checked += 1;
                }
            }
        }
    }
    assert!(checked >= 14, "only {checked} images compared");
}

#[test]
fn boxes_from_sharex_probability_maps_match() {
    let results = results();
    for name in ["tiny_image", "rotated_90"] {
        let r = &results[name];
        let bytes = inflate(&fixtures().join(format!("{name}.prob.z"))).unwrap();
        let map: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
        let boxes = sharex_ocr::boxes_from_map(
            &map,
            r["mapWidth"].as_u64().unwrap() as usize,
            r["mapHeight"].as_u64().unwrap() as usize,
            r["width"].as_u64().unwrap() as u32,
            r["height"].as_u64().unwrap() as u32,
            &SMALL_DET,
        );
        let mine: Vec<Vec<u32>> = boxes.iter().map(box_bits).collect();
        let sharex: Vec<Vec<u32>> = r["boxes"].as_array().unwrap().iter().map(bits).collect();
        assert_eq!(mine, sharex, "{name}");
    }
}

#[test]
fn lines_paragraphs_and_text_match_sharex() {
    for (name, r) in results() {
        let regions = r["regions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| {
                let score = f32::from_bits(x["scoreBits"].as_u64().unwrap() as u32);
                OcrTextRegion::new(points(&x["pointBits"]), x["text"].as_str().unwrap().to_owned(), score)
            })
            .collect();
        let result = OcrResult::new(regions);
        let lines = r["lines"].as_array().unwrap();
        assert_eq!(result.lines.len(), lines.len(), "{name}: lines");
        for (mine, sx) in result.lines.iter().zip(lines) {
            assert_eq!(mine.text, sx["text"].as_str().unwrap(), "{name}");
            assert_eq!(mine.starts_paragraph, sx["startsParagraph"].as_bool().unwrap(), "{name}: {}", mine.text);
            assert_eq!(mine.height.to_bits() as u64, sx["heightBits"].as_u64().unwrap(), "{name}: {}", mine.text);
            let b = [mine.bounds.left, mine.bounds.top, mine.bounds.right, mine.bounds.bottom].map(f32::to_bits);
            assert_eq!(b.to_vec(), bits(&sx["boundsBits"]), "{name}: {}", mine.text);
        }
        assert_eq!(result.text(false, true), sharex_text(&r["text"]), "{name}");
        assert_eq!(result.text(true, true), sharex_text(&r["textSingleLine"]), "{name}");
        assert_eq!(result.text(false, false), sharex_text(&r["textNoParagraphs"]), "{name}");
    }
}

/// The whole engine, when ONNX Runtime (the build ShareX ships) and the models are
/// set up: `ORT_DYLIB_PATH` and `SHAREXRS_OCR_MODELS`. Other CPUs can pick other ONNX
/// Runtime kernels, so this is for Windows on x64, where the fixtures were made.
#[test]
#[cfg(all(windows, target_arch = "x86_64"))]
fn engine_matches_sharex() {
    let (Some(_), Some(models)) = (std::env::var_os("ORT_DYLIB_PATH"), std::env::var_os("SHAREXRS_OCR_MODELS")) else {
        eprintln!("skipped: set ORT_DYLIB_PATH and SHAREXRS_OCR_MODELS to run the models");
        return;
    };
    let engine = sharex_ocr::PaddleOcrEngine::new(Path::new(&models)).unwrap();
    let results = results();
    for entry in std::fs::read_dir(fixtures().join("inputs")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let r = &results[&name];
        let (result, trace) = engine.recognize_traced(&load(&name), sharex_ocr::DEFAULT_MIN_SCORE).unwrap();
        let mine: Vec<Vec<u32>> = trace.boxes.iter().map(box_bits).collect();
        let sharex: Vec<Vec<u32>> = r["boxes"].as_array().unwrap().iter().map(bits).collect();
        assert_eq!(mine, sharex, "{name}: boxes");
        for (m, sx) in trace.texts.iter().zip(r["texts"].as_array().unwrap()) {
            assert_eq!(m.0, sx["text"].as_str().unwrap(), "{name}");
            assert_eq!(m.1.to_bits() as u64, sx["scoreBits"].as_u64().unwrap(), "{name}: score of {}", m.0);
        }
        assert_eq!(result.text(false, true), sharex_text(&r["text"]), "{name}");
    }
}
