//! `ShareX.OCR/TextRecognizer.cs`: reads the text of each line image with a CTC
//! model. Each line is scaled to the model height and the model returns a character
//! probability for every horizontal step.

use std::path::Path;
use std::sync::Mutex;

use sharex_skia::raster::Bitmap;

use crate::config::ModelConfig;
use crate::detector::model_path;
use crate::geometry::{Point, cs_max};
use crate::{OcrError, image_tensor, runtime};

const BATCH_SIZE: usize = 6;
const MAX_INPUT_WIDTH: i32 = 3200;
/// Lines this much taller than wide are vertical text and are turned to read left to right.
const VERTICAL_TEXT_RATIO: f32 = 1.5;
const MEAN: [f32; 3] = [0.5, 0.5, 0.5];
const STD: [f32; 3] = [0.5, 0.5, 0.5];

/// What recognition saw for one line, for comparing with ShareX.
#[derive(Debug, Clone, Default)]
pub struct LineTrace {
    /// The line's position in the batch order (sorted by aspect ratio).
    pub batch: usize,
    /// The batch tensor's width and this line's crop width.
    pub input_width: u32,
    pub crop_width: u32,
    /// The upright crop (premultiplied RGBA, `crop_width` x the model height).
    pub crop: Vec<u8>,
}

pub(crate) struct TextRecognizer {
    session: Mutex<ort::session::Session>,
    input_name: String,
    input_height: i32,
    min_input_width: i32,
    /// Index 0 is the CTC blank, followed by the dictionary and a space.
    characters: Vec<String>,
}

/// Where to sample a box from: its origin corner, the edges that become the line's
/// x and y axes, and its aspect ratio.
#[derive(Debug, Clone, Copy)]
struct LineImage {
    origin: Point,
    x_edge: Point,
    y_edge: Point,
    aspect_ratio: f32,
}

impl TextRecognizer {
    pub fn new(model_folder: &Path) -> Result<Self, OcrError> {
        let config = ModelConfig::load(&model_folder.join("inference.yml"))?;
        let dictionary = config.get_list("character_dict");
        if dictionary.is_empty() {
            return Err(OcrError::InvalidModel(
                "The OCR recognition model configuration has no character dictionary.".into(),
            ));
        }
        let mut characters = Vec::with_capacity(dictionary.len() + 2);
        characters.push(String::new());
        characters.extend(dictionary);
        characters.push(" ".into());
        let shape = config.get_list("image_shape");
        let item = |i: usize, default: i32| {
            if shape.len() == 3 {
                crate::config::parse_int(&shape[i]).unwrap_or(default)
            } else {
                default
            }
        };
        let input_height = item(1, 48);
        let min_input_width = item(2, 320);
        let session = runtime::load_session(&model_path(model_folder)?)?;
        let input_name = session
            .inputs()
            .first()
            .map(|i| i.name().to_owned())
            .unwrap_or_default();
        Ok(Self {
            session: Mutex::new(session),
            input_name,
            input_height,
            min_input_width,
            characters,
        })
    }

    /// `Recognize`: the text and score of each box, in the boxes' order.
    pub fn recognize(
        &self,
        image: &Bitmap,
        boxes: &[[Point; 4]],
        mut trace: Option<&mut Vec<LineTrace>>,
    ) -> Result<Vec<(String, f32)>, OcrError> {
        let mut results = vec![(String::new(), 0.0f32); boxes.len()];
        if let Some(t) = trace.as_deref_mut() {
            *t = vec![LineTrace::default(); boxes.len()];
        }
        let (h, input_height) = (self.input_height as usize, self.input_height);
        let mut first = 0;
        for batch in line_crops(image, boxes, input_height, self.min_input_width) {
            let w = batch.input_width as usize;
            let mut input = vec![0.0f32; batch.lines.len() * 3 * h * w];
            for (i, crop) in batch.crops.iter().enumerate() {
                image_tensor::fill(&mut input, i, w, h, crop, MEAN, STD);
            }
            let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
            let tensor = ort::value::Tensor::from_array(([batch.lines.len(), 3, h, w], input))?;
            let outputs = session.run(ort::inputs![self.input_name.as_str() => tensor])?;
            let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
            let steps = shape[1] as usize;
            let classes = shape[2] as usize;
            for (i, &li) in batch.lines.iter().enumerate() {
                results[li] = self.decode(
                    &data[i * steps * classes..(i + 1) * steps * classes],
                    steps,
                    classes,
                );
            }
            if let Some(t) = trace.as_deref_mut() {
                for (i, (&li, crop)) in batch.lines.iter().zip(batch.crops).enumerate() {
                    t[li] = LineTrace {
                        batch: first + i,
                        input_width: batch.input_width,
                        crop_width: crop.width(),
                        crop: crop.into_data(),
                    };
                }
            }
            first += batch.lines.len();
        }
        Ok(results)
    }

    /// `Decode`: greedy CTC decoding: the likeliest class at each step, repeats merged,
    /// blanks dropped; the score is the mean probability of the kept characters.
    fn decode(&self, p: &[f32], steps: usize, classes: usize) -> (String, f32) {
        let mut text = String::new();
        let mut score_sum = 0.0f32;
        let mut score_count = 0;
        let mut previous = usize::MAX;
        for step in 0..steps {
            let sp = &p[step * classes..(step + 1) * classes];
            let mut best = 0;
            for c in 1..classes {
                if sp[c] > sp[best] {
                    best = c;
                }
            }
            if best != 0 && best != previous && best < self.characters.len() {
                text.push_str(&self.characters[best]);
                score_sum += sp[best];
                score_count += 1;
            }
            previous = best;
        }
        (
            text,
            if score_count > 0 {
                score_sum / score_count as f32
            } else {
                0.0
            },
        )
    }
}

/// One recognition batch: the boxes in it (indices), the tensor width and each
/// box's upright line image.
#[derive(Debug, Clone)]
pub struct CropBatch {
    pub lines: Vec<usize>,
    pub input_width: u32,
    pub crops: Vec<Bitmap>,
}

/// The line images `Recognize` feeds the model: boxes sorted by aspect ratio
/// (`OrderBy` is stable) so lines of similar width share a batch, six to a batch;
/// each batch as wide as its widest line (at least `min_input_width`, at most
/// 3200), and each line scaled to `input_height` (`RecognizeBatch`).
pub fn line_crops(
    image: &Bitmap,
    boxes: &[[Point; 4]],
    input_height: i32,
    min_input_width: i32,
) -> Vec<CropBatch> {
    let lines: Vec<LineImage> = boxes.iter().map(line_image).collect();
    let mut order: Vec<usize> = (0..lines.len()).collect();
    order.sort_by(|&a, &b| {
        lines[a]
            .aspect_ratio
            .partial_cmp(&lines[b].aspect_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    order
        .chunks(BATCH_SIZE)
        .map(|batch| {
            // Enumerable.Max over floats: the first largest.
            let batch_max = batch
                .iter()
                .map(|&i| lines[i].aspect_ratio)
                .fold(f32::NEG_INFINITY, |m, v| if v > m { v } else { m });
            let max_aspect = cs_max(min_input_width as f32 / input_height as f32, batch_max);
            let input_width = ((input_height as f32 * max_aspect) as i32).min(MAX_INPUT_WIDTH);
            let crops = batch
                .iter()
                .map(|&li| {
                    let line = lines[li];
                    let width = (((input_height as f32 * line.aspect_ratio) as f64).ceil() as i32)
                        .clamp(1, input_width);
                    image_tensor::crop(
                        image,
                        line.origin,
                        line.x_edge,
                        line.y_edge,
                        width as u32,
                        input_height as u32,
                    )
                })
                .collect();
            CropBatch {
                lines: batch.to_vec(),
                input_width: input_width as u32,
                crops,
            }
        })
        .collect()
}

/// `GetLineImage`.
fn line_image(b: &[Point; 4]) -> LineImage {
    let width = cs_max(Point::distance(b[0], b[1]), Point::distance(b[3], b[2]));
    let height = cs_max(Point::distance(b[0], b[3]), Point::distance(b[1], b[2]));
    let width = cs_max(width, 1.0);
    let height = cs_max(height, 1.0);
    if height / width >= VERTICAL_TEXT_RATIO {
        // Turned a quarter counter-clockwise: the top-right corner becomes the origin
        // and the right edge the x axis.
        return LineImage {
            origin: b[1],
            x_edge: b[2] - b[1],
            y_edge: b[0] - b[1],
            aspect_ratio: height / width,
        };
    }
    LineImage {
        origin: b[0],
        x_edge: b[1] - b[0],
        y_edge: b[3] - b[0],
        aspect_ratio: width / height,
    }
}
