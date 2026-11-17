//! Image splitter: `ImageSplitterViewModel.SplitImage` and
//! `SkiaImageHelpers.SplitImage`.

use sharex_skia::raster::{Canvas, Rect};

use crate::dotnet;
use crate::imaging::{self, Bitmap};

/// `SkiaImageHelpers.SplitImage(img, rowCount, columnCount)`: row by row, each
/// cell `width / columns` by `height / rows` (the remainder is dropped).
pub fn split_image(img: &Bitmap, row_count: i32, column_count: i32) -> Vec<Bitmap> {
    let width = img.width() as i32 / column_count;
    let height = img.height() as i32 / row_count;
    let mut images = Vec::new();
    for y in 0..row_count {
        for x in 0..column_count {
            let mut bmp = imaging::create_bitmap(width.max(0) as u32, height.max(0) as u32);
            {
                let mut g = Canvas::new(&mut bmp);
                let src = Rect::xywh((x * width) as f32, (y * height) as f32, width as f32, height as f32);
                imaging::draw_image(&mut g, img, src, Rect::xywh(0.0, 0.0, width as f32, height as f32), None);
            }
            images.push(bmp);
        }
    }
    images
}

/// `ImageSplitterViewModel.CanProcess`'s grid condition.
pub fn can_split(row_count: i32, column_count: i32) -> bool {
    row_count > 1 || column_count > 1
}

/// `ImageSplitterViewModel.SplitImage`: `<name><n>.png` in `output_folder`, n
/// from 1. Returns the files written (none when the image doesn't load).
pub fn split_image_file(file_path: &str, row_count: i32, column_count: i32, output_folder: &str) -> std::io::Result<Vec<String>> {
    let Some(source) = imaging::load_image(file_path) else {
        return Ok(Vec::new());
    };
    let images = split_image(&source, row_count, column_count);
    let name = dotnet::file_name_without_extension(file_path);
    let mut paths = Vec::new();
    for (i, image) in images.iter().enumerate() {
        let path = dotnet::combine(output_folder, &format!("{name}{}.png", i + 1));
        std::fs::write(&path, imaging::encode_png(image))?;
        paths.push(path);
    }
    Ok(paths)
}
