//! `ShareX.OCR/TextDetector.cs`: finds text lines with a DB (differentiable
//! binarization) model. The model returns a text probability for every pixel, and
//! each connected area above the threshold becomes one rotated box.

use std::path::Path;
use std::sync::Mutex;

use sharex_skia::raster::Bitmap;

use crate::config::ModelConfig;
use crate::geometry::{self, Point, cs_clamp};
use crate::{OcrError, image_tensor, runtime};

/// Images are scaled up until their short side reaches this, and down until their
/// long side fits the max.
const MIN_SIDE_LENGTH: i32 = 64;
const MAX_SIDE_LENGTH: i32 = 4000;
/// The model works on sizes that are a multiple of its downsampling stride.
const SIZE_MULTIPLE: i32 = 32;
const MIN_BOX_SIZE: f32 = 3.0;

/// What one detection pass saw, for comparing with ShareX.
#[derive(Debug, Clone, Default)]
pub struct DetectionTrace {
    /// The model input size (`GetInputSize`).
    pub input_width: u32,
    pub input_height: u32,
    /// The resized image fed to the model (premultiplied RGBA).
    pub resized: Vec<u8>,
    /// The probability map and its size.
    pub map_width: usize,
    pub map_height: usize,
    pub probabilities: Vec<f32>,
}

/// The DB post-processing values from the model's `inference.yml` (`PostProcess`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DbParams {
    /// `thresh`: a pixel is text above this probability.
    pub threshold: f32,
    /// `box_thresh`: a box's mean probability must reach this.
    pub box_threshold: f32,
    /// `unclip_ratio`: how far a box grows (area * ratio / perimeter).
    pub unclip_ratio: f32,
    /// `max_candidates`: regions looked at, at most.
    pub max_candidates: i32,
}

pub(crate) struct TextDetector {
    session: Mutex<ort::session::Session>,
    input_name: String,
    mean: [f32; 3],
    std: [f32; 3],
    params: DbParams,
}

impl TextDetector {
    pub fn new(model_folder: &Path) -> Result<Self, OcrError> {
        let config = ModelConfig::load(&model_folder.join("inference.yml"))?;
        let mean = config.get_floats("mean", [0.485, 0.456, 0.406]);
        let std = config.get_floats("std", [0.229, 0.224, 0.225]);
        let params = DbParams {
            threshold: config.get_float("thresh", 0.3),
            box_threshold: config.get_float("box_thresh", 0.6),
            unclip_ratio: config.get_float("unclip_ratio", 1.5),
            max_candidates: config.get_int("max_candidates", 1000),
        };
        let session = runtime::load_session(&model_path(model_folder)?)?;
        let input_name = session
            .inputs()
            .first()
            .map(|i| i.name().to_owned())
            .unwrap_or_default();
        Ok(Self {
            session: Mutex::new(session),
            input_name,
            mean,
            std,
            params,
        })
    }

    /// `Detect`: the boxes in image coordinates, each top-left, top-right,
    /// bottom-right, bottom-left.
    pub fn detect(
        &self,
        image: &Bitmap,
        trace: Option<&mut DetectionTrace>,
    ) -> Result<Vec<[Point; 4]>, OcrError> {
        let (input_width, input_height) = input_size(image.width() as i32, image.height() as i32);
        let (w, h) = (input_width as usize, input_height as usize);
        let mut input = vec![0.0f32; 3 * w * h];
        let resized = image_tensor::resize(image, input_width as u32, input_height as u32);
        image_tensor::fill(&mut input, 0, w, h, &resized, self.mean, self.std);

        let (map_width, map_height, probabilities) = {
            let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
            let tensor = ort::value::Tensor::from_array(([1usize, 3, h, w], input))?;
            let outputs = session.run(ort::inputs![self.input_name.as_str() => tensor])?;
            let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
            let n = shape.len();
            (shape[n - 1] as usize, shape[n - 2] as usize, data.to_vec())
        };

        let boxes = boxes_from_map(
            &probabilities,
            map_width,
            map_height,
            image.width(),
            image.height(),
            &self.params,
        );
        if let Some(t) = trace {
            *t = DetectionTrace {
                input_width: input_width as u32,
                input_height: input_height as u32,
                resized: resized.into_data(),
                map_width,
                map_height,
                probabilities,
            };
        }
        Ok(boxes)
    }
}

/// The second half of `Detect`: the boxes in a probability map (`FindBoxes`), scaled
/// to an `image_width` x `image_height` image and clamped to it, without the ones
/// 3 px or less across.
pub fn boxes_from_map(
    probabilities: &[f32],
    map_width: usize,
    map_height: usize,
    image_width: u32,
    image_height: u32,
    params: &DbParams,
) -> Vec<[Point; 4]> {
    let scale_x = image_width as f32 / map_width as f32;
    let scale_y = image_height as f32 / map_height as f32;
    let (iw, ih) = (image_width as i32, image_height as i32);
    let mut boxes = Vec::new();
    for b in find_boxes(probabilities, map_width, map_height, params) {
        let mut scaled = [Point::default(); 4];
        for i in 0..4 {
            scaled[i] = Point::new(
                cs_clamp(b[i].x * scale_x, 0.0, (iw - 1) as f32),
                cs_clamp(b[i].y * scale_y, 0.0, (ih - 1) as f32),
            );
        }
        if Point::distance(scaled[0], scaled[1]) > MIN_BOX_SIZE
            && Point::distance(scaled[0], scaled[3]) > MIN_BOX_SIZE
        {
            boxes.push(scaled);
        }
    }
    boxes
}

fn find_boxes(
    probabilities: &[f32],
    width: usize,
    height: usize,
    params: &DbParams,
) -> Vec<[Point; 4]> {
    let mut visited = vec![false; probabilities.len()];
    let mut pending = Vec::new();
    let mut candidates = 0;
    let mut boxes = Vec::new();
    for start in 0..probabilities.len() {
        if candidates >= params.max_candidates {
            break;
        }
        if visited[start] || probabilities[start] <= params.threshold {
            continue;
        }
        candidates += 1;
        let mut edge = collect_region(
            params.threshold,
            probabilities,
            &mut visited,
            &mut pending,
            start,
            width,
            height,
        );
        let (b, short_side) = geometry::min_area_rect(&geometry::convex_hull(&mut edge));
        if short_side < MIN_BOX_SIZE
            || box_score(probabilities, width, height, &b) < params.box_threshold
        {
            continue;
        }
        let (expanded, expanded_short) = geometry::unclip(&b, params.unclip_ratio);
        if expanded_short < MIN_BOX_SIZE + 2.0 {
            continue;
        }
        boxes.push(expanded);
    }
    boxes
}

/// `CollectRegion`: flood fills one 8-connected region of text pixels and returns
/// its edge pixels (they have the region's convex hull).
fn collect_region(
    threshold: f32,
    p: &[f32],
    visited: &mut [bool],
    pending: &mut Vec<usize>,
    start: usize,
    width: usize,
    height: usize,
) -> Vec<Point> {
    let mut edge = Vec::new();
    visited[start] = true;
    pending.push(start);
    let is_text = |x: isize, y: isize| {
        x >= 0
            && y >= 0
            && (x as usize) < width
            && (y as usize) < height
            && p[y as usize * width + x as usize] > threshold
    };
    while let Some(index) = pending.pop() {
        let x = (index % width) as isize;
        let y = (index / width) as isize;
        if !is_text(x - 1, y) || !is_text(x + 1, y) || !is_text(x, y - 1) || !is_text(x, y + 1) {
            edge.push(Point::new(x as f32, y as f32));
        }
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if (dx != 0 || dy != 0) && is_text(nx, ny) {
                    let n = ny as usize * width + nx as usize;
                    if !visited[n] {
                        visited[n] = true;
                        pending.push(n);
                    }
                }
            }
        }
    }
    edge
}

/// `GetInputSize`: the detection model input size for an image.
pub fn input_size(width: i32, height: i32) -> (i32, i32) {
    let mut ratio = 1.0f32;
    if width.min(height) < MIN_SIDE_LENGTH {
        ratio = MIN_SIDE_LENGTH as f32 / width.min(height) as f32;
    }
    if width.max(height) as f32 * ratio > MAX_SIDE_LENGTH as f32 {
        ratio = MAX_SIDE_LENGTH as f32 / width.max(height) as f32;
    }
    // `Math.Round` takes the float as a double and rounds halves to even.
    let round = |v: f32| {
        (((v / SIZE_MULTIPLE as f32) as f64).round_ties_even() as i32 * SIZE_MULTIPLE)
            .max(SIZE_MULTIPLE)
    };
    (round(width as f32 * ratio), round(height as f32 * ratio))
}

/// `GetBoxScore`: the mean probability of the pixels inside the box.
fn box_score(p: &[f32], width: usize, height: usize, b: &[Point; 4]) -> f32 {
    let bounds = geometry::bounds(b);
    let clamp = |v: f32, max: usize| (v as i32).clamp(0, max as i32 - 1);
    let left = clamp(bounds.left.floor(), width);
    let right = clamp(bounds.right.ceil(), width);
    let top = clamp(bounds.top.floor(), height);
    let bottom = clamp(bounds.bottom.ceil(), height);
    let mut sum = 0.0f64;
    let mut count = 0;
    for y in top..=bottom {
        for x in left..=right {
            if is_inside(b, x as f32, y as f32) {
                sum += p[y as usize * width + x as usize] as f64;
                count += 1;
            }
        }
    }
    if count > 0 {
        (sum / count as f64) as f32
    } else {
        0.0
    }
}

/// `IsInside`: a pixel centre inside the convex box, counting pixels within half a
/// pixel of an edge as a filled polygon does.
fn is_inside(b: &[Point; 4], x: f32, y: f32) -> bool {
    let mut sign = 0;
    for i in 0..4 {
        let a = b[i];
        let edge = b[(i + 1) % 4] - a;
        let length = edge.length();
        if length < 1e-6 {
            continue;
        }
        let distance = (edge.x * (y - a.y) - edge.y * (x - a.x)) / length;
        if distance.abs() <= 0.5 {
            continue;
        }
        let side = if distance > 0.0 { 1 } else { -1 };
        if sign == 0 {
            sign = side;
        } else if side != sign {
            return false;
        }
    }
    true
}

/// `GetModelPath`.
pub(crate) fn model_path(model_folder: &Path) -> Result<std::path::PathBuf, OcrError> {
    let path = model_folder.join("inference.onnx");
    if !path.is_file() {
        return Err(OcrError::MissingFile {
            what: "The OCR model file is missing.",
            path,
        });
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_sizes_follow_sharex() {
        // 64 px short side, multiples of 32, halves rounded to even.
        assert_eq!(input_size(100, 20), (320, 64));
        assert_eq!(input_size(1920, 1080), (1920, 1088));
        assert_eq!(input_size(8000, 100), (4000, 64));
        assert_eq!(input_size(48, 48), (64, 64));
        assert_eq!(input_size(16, 16), (64, 64));
        // 80 / 32 = 2.5 rounds to 2.
        assert_eq!(input_size(80, 80), (64, 64));
    }

    #[test]
    fn pixels_on_an_edge_are_inside() {
        let b = [
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        assert!(is_inside(&b, 0.0, 0.0));
        assert!(is_inside(&b, 4.0, 2.0));
        assert!(!is_inside(&b, 5.0, 1.0));
    }
}
