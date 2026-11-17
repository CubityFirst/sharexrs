//! Image thumbnailer: `ImageThumbnailerViewModel.GenerateThumbnails`.

use crate::dotnet;
use crate::imaging::{self, Bitmap, JpegDownsample, WHITE};

/// The window's defaults (`ImageThumbnailerViewModel`).
pub const DEFAULT_WIDTH: i32 = 150;
pub const DEFAULT_HEIGHT: i32 = 150;
pub const DEFAULT_QUALITY: i32 = 90;
pub const DEFAULT_FILE_NAME: &str = "$filename_th";

/// One thumbnail: the centred crop scaled to `width` x `height`, on white.
pub fn create(source: &Bitmap, width: i32, height: i32) -> Result<Bitmap, String> {
    let thumbnail = imaging::create_thumbnail(source, width, height)?;
    Ok(imaging::fill_background(&thumbnail, WHITE))
}

/// The output path: `$filename` replaced by the source's name, as `.jpg`.
pub fn output_path(output_folder: &str, output_file_name: &str, source_path: &str) -> String {
    let name = dotnet::file_name_without_extension(source_path);
    let path = dotnet::combine(output_folder, &output_file_name.replace("$filename", name));
    dotnet::change_extension(&path, "jpg")
}

/// `GenerateThumbnails`: a JPEG per image that loads, at `quality`.
pub fn generate(
    files: &[String],
    width: i32,
    height: i32,
    quality: i32,
    output_folder: &str,
    output_file_name: &str,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for file in files {
        if !std::path::Path::new(file).is_file() {
            continue;
        }
        let Some(source) = imaging::load_image(file) else { continue };
        let output = create(&source, width, height)?;
        let path = output_path(output_folder, output_file_name, file);
        std::fs::write(&path, imaging::save_jpeg(&output, quality, JpegDownsample::Downsample420))
            .map_err(|e| e.to_string())?;
        out.push(path);
    }
    Ok(out)
}
