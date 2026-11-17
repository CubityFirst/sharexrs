//! Image comparer: `ImageComparisonService.Compare` (the comparison maths and
//! the diff preview) and the texts `ImageComparerViewModel` shows.

use crate::imaging::{self, Bitmap};

/// `ImageComparisonResult`.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonResult {
    /// Opaque black, white where a pixel differs or only one image has it.
    pub diff: Bitmap,
    pub matching_pixels: i64,
    pub total_pixels: i64,
}

impl ComparisonResult {
    /// `SimilarityPercentage`.
    pub fn similarity(&self) -> f64 {
        if self.total_pixels > 0 { self.matching_pixels as f64 * 100.0 / self.total_pixels as f64 } else { 100.0 }
    }

    /// Whether the window shows the similarity in green (`|s - 100| < 0.0001`).
    pub fn is_identical(&self) -> bool {
        (self.similarity() - 100.0).abs() < 0.0001
    }
}

fn div_round_up(v: i64, d: i64) -> i64 {
    (v + d - 1) / d
}

/// `ImageComparisonService.Compare(image1, image2, maxDiffPreviewDimension)`:
/// premultiplied pixels compared exactly over the common area; everything
/// outside it counts as different.
pub fn compare(a: &Bitmap, b: &Bitmap, max_preview: i32) -> ComparisonResult {
    assert!(max_preview > 0, "maxDiffPreviewDimension");
    let width = a.width().max(b.width()) as i64;
    let height = a.height().max(b.height()) as i64;
    if width <= 0 || height <= 0 {
        return ComparisonResult { diff: Bitmap::new(1, 1), matching_pixels: 1, total_pixels: 1 };
    }
    let common_w = a.width().min(b.width()) as i64;
    let common_h = a.height().min(b.height()) as i64;
    let scale = (max_preview as f64 / width.max(height) as f64).min(1.0);
    let pw = ((width as f64 * scale).round_ties_even() as i64).max(1);
    let ph = ((height as f64 * scale).round_ties_even() as i64).max(1);
    let mut diff = Bitmap::new(pw as u32, ph as u32);
    for i in 0..(pw * ph) as usize {
        diff.set_px(i, [0, 0, 0, 255]);
    }
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    let preview_x: Vec<i64> = (0..common_w).map(|x| x * pw / width).collect();
    let first_extra_x = if common_w < width { common_w * pw / width } else { pw };
    let mut matching = 0i64;
    for py in 0..ph {
        let y0 = div_round_up(py * height, ph);
        let y1 = div_round_up((py + 1) * height, ph);
        if y1 > common_h {
            for x in 0..pw {
                diff.set_pixel(x as u32, py as u32, WHITE);
            }
        } else if first_extra_x < pw {
            for x in first_extra_x..pw {
                diff.set_pixel(x as u32, py as u32, WHITE);
            }
        }
        for sy in y0..y1.min(common_h) {
            for x in 0..common_w {
                if a.pixel(x as u32, sy as u32) == b.pixel(x as u32, sy as u32) {
                    matching += 1;
                } else {
                    diff.set_pixel(preview_x[x as usize] as u32, py as u32, WHITE);
                }
            }
        }
    }
    ComparisonResult { diff, matching_pixels: matching, total_pixels: width * height }
}

/// `ImageComparerViewModel`: images load with `SKBitmap.Decode` (no EXIF rotation).
pub fn load_image(path: &str) -> Option<Bitmap> {
    imaging::load_image_with(path, false)
}

/// `ImageComparerViewModel_Similarity_value` ("{0:0.##}%"-style) and
/// `ImageComparerViewModel_Pixels_match`, formatted as the window shows them.
pub fn texts(r: &ComparisonResult) -> (String, String) {
    let similarity = crate::text::format_dotnet(
        sharex_strings::tools::ImageComparerViewModel_Similarity_value(),
        &[crate::text::Arg::F64(r.similarity())],
    );
    let status = crate::text::format_dotnet(
        sharex_strings::tools::ImageComparerViewModel_Pixels_match(),
        &[crate::text::Arg::I64(r.matching_pixels), crate::text::Arg::I64(r.total_pixels)],
    );
    (similarity, status)
}
