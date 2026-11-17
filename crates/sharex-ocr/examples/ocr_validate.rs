//! Runs every OCR harness case through ShareXrs's OCR and compares it with what
//! ShareX produced (tools/ocr-harness): the detection input image, the probability
//! map, the boxes, the line crops, the raw texts and scores, the regions, lines and
//! the final text. Writes `<work>/report.md`.
//!
//!     cargo run --release -p sharex-ocr --example ocr_validate -- <cases dir> <models dir> <work dir> [case prefix]
//!
//! ONNX Runtime comes from `ORT_DYLIB_PATH` (point it at the reference build's
//! `onnxruntime.dll`, the one ShareX ran).

use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;
use sharex_ocr::{Bitmap, DEFAULT_MIN_SCORE, PaddleOcrEngine, Point};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: validate <cases dir> <models dir> <work dir> [case prefix]");
        std::process::exit(2);
    }
    let (cases_dir, models, work) = (
        Path::new(&args[0]),
        Path::new(&args[1]),
        Path::new(&args[2]),
    );
    let only = args.get(3);
    let cases: Value =
        serde_json::from_str(&std::fs::read_to_string(cases_dir.join("cases.json")).unwrap())
            .unwrap();
    let engine = PaddleOcrEngine::new(models).unwrap_or_else(|e| panic!("loading the models: {e}"));

    let mut report = String::from(
        "# OCR: ShareXrs vs ShareX\n\n| case | text | det input | prob map | boxes | crops | raw texts/scores | regions/lines |\n|---|---|---|---|---|---|---|---|\n",
    );
    let mut all_text = 0;
    let mut total = 0;
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        if only.is_some_and(|p| !name.starts_with(p.as_str())) {
            continue;
        }
        total += 1;
        let dir = work.join("sharex").join(name);
        let sx: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("result.json")).unwrap())
                .unwrap();
        let bitmap = load(&cases_dir.join(c["file"].as_str().unwrap()));
        let started = std::time::Instant::now();
        let (result, trace) = engine.recognize_traced(&bitmap, DEFAULT_MIN_SCORE).unwrap();
        let elapsed = started.elapsed();

        // Final text, three ways.
        let texts_ok = result.text(false, true) == sx["text"].as_str().unwrap()
            && result.text(true, true) == sx["textSingleLine"].as_str().unwrap()
            && result.text(false, false) == sx["textNoParagraphs"].as_str().unwrap();
        if texts_ok {
            all_text += 1;
        }

        // Detection input.
        let (iw, ih) = (
            sx["detInputWidth"].as_u64().unwrap() as u32,
            sx["detInputHeight"].as_u64().unwrap() as u32,
        );
        let det_size_ok = (trace.detection.input_width, trace.detection.input_height) == (iw, ih);
        let resized = pixel_diff(
            &trace.detection.resized,
            &std::fs::read(dir.join("resized.bgra")).unwrap(),
        );

        // Probability map.
        let sx_prob: Vec<f32> = std::fs::read(dir.join("prob.f32"))
            .unwrap()
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let prob = float_diff(&trace.detection.probabilities, &sx_prob);

        // Boxes (bit for bit).
        let sx_boxes: Vec<Vec<u32>> = sx["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| bits(&b["bits"]))
            .collect();
        let my_boxes: Vec<Vec<u32>> = trace.boxes.iter().map(box_bits).collect();
        let boxes_ok = sx_boxes == my_boxes;

        // Crops and raw texts.
        let mut crops_exact = 0;
        let mut crops_note = String::new();
        let mut raw_ok = true;
        if let Some(sx_texts) = sx["texts"].as_array() {
            for (i, t) in sx_texts.iter().enumerate() {
                let Some(mine) = trace.texts.get(i) else {
                    raw_ok = false;
                    continue;
                };
                if mine.0 != t["text"].as_str().unwrap()
                    || mine.1.to_bits() as u64 != t["scoreBits"].as_u64().unwrap()
                {
                    raw_ok = false;
                }
                if let (Some(line), Ok(bytes)) = (
                    trace.lines.get(i),
                    std::fs::read(dir.join(format!("crop_{i}.bgra"))),
                ) {
                    let d = pixel_diff(&line.crop, &bytes);
                    if d.differing == 0 && d.same_size {
                        crops_exact += 1;
                    } else if crops_note.is_empty() {
                        crops_note = format!(" (#{i}: {})", d.describe());
                    }
                }
            }
            raw_ok &= sx_texts.len() == trace.texts.len();
        } else {
            raw_ok = trace.texts.is_empty();
        }
        let n_crops = sx["texts"].as_array().map_or(0, Vec::len);

        // Regions and lines.
        let sx_regions = sx["regions"].as_array().unwrap();
        let regions_ok = sx_regions.len() == result.regions.len()
            && sx_regions.iter().zip(&result.regions).all(|(s, r)| {
                s["text"].as_str().unwrap() == r.text
                    && s["scoreBits"].as_u64().unwrap() == r.score.to_bits() as u64
                    && bits(&s["pointBits"]) == box_bits(&r.points)
            });
        let sx_lines = sx["lines"].as_array().unwrap();
        let lines_ok = sx_lines.len() == result.lines.len()
            && sx_lines.iter().zip(&result.lines).all(|(s, l)| {
                s["text"].as_str().unwrap() == l.text
                    && s["startsParagraph"].as_bool().unwrap() == l.starts_paragraph
                    && s["heightBits"].as_u64().unwrap() == l.height.to_bits() as u64
                    && bits(&s["bounds"]["bits"])
                        == [l.bounds.left, l.bounds.top, l.bounds.right, l.bounds.bottom]
                            .map(f32::to_bits)
                            .to_vec()
            });

        let mark = |ok: bool| if ok { "same" } else { "**differs**" };
        let _ = writeln!(
            report,
            "| {name} | {} | {} {}x{}, pixels {} | {} | {} {} | {}/{} exact{} | {} | {} |",
            mark(texts_ok),
            if det_size_ok {
                "same size"
            } else {
                "**size differs**"
            },
            iw,
            ih,
            resized.describe(),
            prob.describe(),
            mark(boxes_ok),
            sx_boxes.len(),
            crops_exact,
            n_crops,
            crops_note,
            mark(raw_ok),
            mark(regions_ok && lines_ok),
        );
        println!(
            "{name}: text {} | det {} | prob {} | boxes {} | crops {crops_exact}/{n_crops} | raw {} | regions/lines {} | {:.0} ms",
            mark(texts_ok),
            resized.describe(),
            prob.describe(),
            mark(boxes_ok),
            mark(raw_ok),
            mark(regions_ok && lines_ok),
            elapsed.as_secs_f64() * 1000.0
        );
        if !texts_ok {
            println!(
                "  sharex:   {:?}\n  sharexrs: {:?}",
                sx["text"].as_str().unwrap(),
                result.text(false, true)
            );
        }
    }
    let _ = writeln!(report, "\n{all_text} of {total} cases give the same text.");
    std::fs::write(work.join("report.md"), &report).unwrap();
    println!(
        "{all_text}/{total} cases give the same text; report in {}",
        work.join("report.md").display()
    );
}

/// The PNG as ShareX's `SkiaImageHelpers.LoadImage` has it: premultiplied.
fn load(path: &Path) -> Bitmap {
    let img = image::open(path).unwrap().to_rgba8();
    sharex_ocr::bitmap_from_rgba(img.width(), img.height(), img.as_raw())
}

fn box_bits(b: &[Point; 4]) -> Vec<u32> {
    b.iter()
        .flat_map(|p| [p.x.to_bits(), p.y.to_bits()])
        .collect()
}

fn bits(v: &Value) -> Vec<u32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_u64().unwrap() as u32)
        .collect()
}

struct Diff {
    same_size: bool,
    differing: usize,
    max: f64,
}

impl Diff {
    fn describe(&self) -> String {
        if !self.same_size {
            "size differs".into()
        } else if self.differing == 0 {
            "exact".into()
        } else {
            format!("{} differ (max {})", self.differing, self.max)
        }
    }
}

/// Our RGBA against ShareX's BGRA.
fn pixel_diff(rgba: &[u8], bgra: &[u8]) -> Diff {
    if rgba.len() != bgra.len() {
        return Diff {
            same_size: false,
            differing: 0,
            max: 0.0,
        };
    }
    let mut differing = 0;
    let mut max = 0i32;
    for (a, b) in rgba.chunks_exact(4).zip(bgra.chunks_exact(4)) {
        let b = [b[2], b[1], b[0], b[3]];
        let d = (0..4)
            .map(|i| (a[i] as i32 - b[i] as i32).abs())
            .max()
            .unwrap();
        if d > 0 {
            differing += 1;
            max = max.max(d);
        }
    }
    Diff {
        same_size: true,
        differing,
        max: max as f64,
    }
}

fn float_diff(a: &[f32], b: &[f32]) -> Diff {
    if a.len() != b.len() {
        return Diff {
            same_size: false,
            differing: 0,
            max: 0.0,
        };
    }
    let mut differing = 0;
    let mut max = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        if x.to_bits() != y.to_bits() {
            differing += 1;
            max = max.max((x - y).abs() as f64);
        }
    }
    Diff {
        same_size: true,
        differing,
        max,
    }
}
