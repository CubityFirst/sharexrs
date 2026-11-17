//! `ShareX.OCR/ImageTensor.cs`: the Skia drawing that prepares images for the models
//! (through `sharex-skia`, Skia m119's CPU pipeline as SkiaSharp 3.119 runs it) and
//! the tensor fill.
//!
//! [`Bitmap`] keeps RGBA where ShareX's `SKBitmap` is BGRA; drawing treats the
//! channels alike, and [`fill`] reads them in ShareX's BGR order.

use sharex_skia::raster::{Bitmap, Canvas, Matrix, Paint, Rect, Sampling, Shader, TileMode};

use crate::geometry::Point;

/// `ToOpaqueBgra`: the image drawn on white, so transparent pixels read as background.
pub fn to_opaque(source: &Bitmap) -> Bitmap {
    let mut result = Bitmap::new(source.width(), source.height());
    let mut canvas = Canvas::new(&mut result);
    canvas.clear([255, 255, 255, 255]);
    canvas.draw_bitmap(source, 0.0, 0.0, None);
    result
}

/// `GetSampling`: linear between mipmaps to shrink, Catmull-Rom otherwise.
fn sampling(downscale: bool) -> Sampling {
    if downscale {
        Sampling::LinearMipmapLinear
    } else {
        Sampling::CatmullRom
    }
}

/// `Resize`: the whole image drawn into `width` x `height`.
pub fn resize(source: &Bitmap, width: u32, height: u32) -> Bitmap {
    let mut result = Bitmap::new(width, height);
    let downscale = width < source.width() || height < source.height();
    let mut canvas = Canvas::new(&mut result);
    canvas.draw_image_rect(
        source,
        None,
        Rect::xywh(0.0, 0.0, width as f32, height as f32),
        sampling(downscale),
        None,
    );
    result
}

/// `Crop`: the area inside a rectangle (its `origin` corner and the edges that become
/// the x and y axes) drawn upright at `width` x `height`; outside the image the edge
/// pixels repeat.
pub fn crop(
    source: &Bitmap,
    origin: Point,
    x_edge: Point,
    y_edge: Point,
    width: u32,
    height: u32,
) -> Bitmap {
    let mut result = Bitmap::new(width, height);
    let (w, h) = (width as f32, height as f32);
    // new SKMatrix(scaleX, skewX, transX, skewY, scaleY, transY, 0, 0, 1)
    let target_to_source = Matrix([
        x_edge.x / w,
        y_edge.x / h,
        origin.x,
        x_edge.y / w,
        y_edge.y / h,
        origin.y,
        0.0,
        0.0,
        1.0,
    ]);
    let Some(source_to_target) = target_to_source.invert() else {
        return result;
    };
    let downscale = x_edge.length() > w * 1.01 || y_edge.length() > h * 1.01;
    let paint = Paint::shader(Shader::Image {
        image: source,
        matrix: source_to_target,
        sampling: sampling(downscale),
        tile_x: TileMode::Clamp,
        tile_y: TileMode::Clamp,
    });
    let mut canvas = Canvas::new(&mut result);
    canvas.draw_rect(Rect::xywh(0.0, 0.0, w, h), &paint);
    result
}

/// `Fill`: one image of an NCHW tensor in BGR order, `(value / 255 - mean) / std` as
/// `value * (1 / (255 * std)) - mean / std`. Columns past the bitmap stay as they are
/// (zero), which is how PaddleOCR pads recognition batches.
pub fn fill(
    tensor: &mut [f32],
    batch_index: usize,
    tensor_width: usize,
    tensor_height: usize,
    bitmap: &Bitmap,
    mean: [f32; 3],
    std: [f32; 3],
) {
    let plane = tensor_width * tensor_height;
    let image_offset = batch_index * plane * 3;
    let width = (bitmap.width() as usize).min(tensor_width);
    let height = (bitmap.height() as usize).min(tensor_height);
    let scale = [
        1.0 / (255.0 * std[0]),
        1.0 / (255.0 * std[1]),
        1.0 / (255.0 * std[2]),
    ];
    let offset = [mean[0] / std[0], mean[1] / std[1], mean[2] / std[2]];
    let data = bitmap.data();
    let row_bytes = bitmap.width() as usize * 4;
    for y in 0..height {
        let row = y * row_bytes;
        let tensor_row = image_offset + y * tensor_width;
        for x in 0..width {
            let p = row + x * 4;
            let i = tensor_row + x;
            // BGRA bytes 0, 1, 2 are this bitmap's 2, 1, 0.
            tensor[i] = data[p + 2] as f32 * scale[0] - offset[0];
            tensor[i + plane] = data[p + 1] as f32 * scale[1] - offset[1];
            tensor[i + plane * 2] = data[p] as f32 * scale[2] - offset[2];
        }
    }
}
