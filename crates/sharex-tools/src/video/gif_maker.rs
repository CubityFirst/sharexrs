//! Animated GIF maker: `AnimatedGifMakerService` (frames fitted to the first
//! image's size, each quantized adaptively, written by `AnimatedGifCreator`).

use crate::gif::AnimatedGifCreator;
use crate::imaging::{self, Bitmap, GifQuality};
use crate::resizer::{self, ResizeMode};

const MAX_PREVIEW_DIMENSION: u32 = 1200;

fn load(path: &str) -> Result<Bitmap, String> {
    imaging::load_image(path).ok_or_else(|| format!("Unable to load image: {}", crate::dotnet::file_name(path)))
}

fn validate(files: &[String]) -> Result<(), String> {
    if files.len() < 2 {
        return Err("At least two images are required.".into());
    }
    Ok(())
}

/// `CreatePreviewFrames`: every image fitted into the first one's size (at most
/// 1200 pixels), as PNG.
pub fn create_preview_frames(files: &[String]) -> Result<Vec<Vec<u8>>, String> {
    validate(files)?;
    let first = load(&files[0])?;
    let (w, h) = preview_size(first.width(), first.height());
    files
        .iter()
        .map(|f| {
            let src = load(f)?;
            let frame = resizer::resize(&src, w as i32, h as i32, ResizeMode::Fit)?;
            Ok(imaging::encode_png(&frame))
        })
        .collect()
}

fn preview_size(w: u32, h: u32) -> (u32, u32) {
    let largest = w.max(h);
    if largest <= MAX_PREVIEW_DIMENSION {
        return (w, h);
    }
    let scale = MAX_PREVIEW_DIMENSION as f64 / largest as f64;
    (
        ((w as f64 * scale).round_ties_even() as u32).max(1),
        ((h as f64 * scale).round_ties_even() as u32).max(1),
    )
}

/// `AnimatedGifMakerService.Create`, returning the file's bytes.
pub fn create_bytes(files: &[String], delay: i32, looping: bool, repeat_count: i32) -> Result<Vec<u8>, String> {
    validate(files)?;
    if !(10..=655350).contains(&delay) {
        return Err("delay out of range".into());
    }
    if !(0..=u16::MAX as i32).contains(&repeat_count) {
        return Err("repeatCount out of range".into());
    }
    let first = load(&files[0])?;
    let (w, h) = (first.width() as i32, first.height() as i32);
    let mut creator = AnimatedGifCreator::new(delay, repeat_count, looping);
    let mut add = |src: &Bitmap| -> Result<(), String> {
        let frame = resizer::resize(src, w, h, ResizeMode::Fit)?;
        creator.add_bitmap(&frame, GifQuality::Adaptive);
        Ok(())
    };
    add(&first)?;
    for f in &files[1..] {
        add(&load(f)?)?;
    }
    Ok(creator.finish())
}

/// `AnimatedGifMakerService.Create`: written through a temporary file beside
/// the output, then moved over it.
pub fn create(output: &str, files: &[String], delay: i32, looping: bool, repeat_count: i32) -> Result<(), String> {
    let folder = crate::dotnet::directory_name(output).filter(|d| !d.trim().is_empty()).ok_or("An output folder is required.")?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let data = create_bytes(files, delay, looping, repeat_count)?;
    let tmp = crate::dotnet::combine(&folder, &format!(".{}.{:032x}.tmp", crate::dotnet::file_name(output), rand_u128()));
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, output).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

fn rand_u128() -> u128 {
    let mut b = [0u8; 16];
    let _ = getrandom_fill(&mut b);
    u128::from_le_bytes(b)
}

fn getrandom_fill(b: &mut [u8]) -> Result<(), ()> {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_| ())?.as_nanos();
    for (i, x) in b.iter_mut().enumerate() {
        *x = (t >> ((i % 16) * 8)) as u8 ^ (std::process::id() as u8).wrapping_mul(i as u8 + 1);
    }
    Ok(())
}
