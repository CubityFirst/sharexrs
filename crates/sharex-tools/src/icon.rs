//! Icon converter: `IconConverterService` (`.ico` writing, byte for byte).

use sharex_skia::raster::{Canvas, Paint, Rect, Sampling, Shader};

use crate::imaging::{self, Bitmap, Rgba};

/// `IconBitDepth`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconBitDepth {
    Palette8 = 8,
    TrueColor32 = 32,
}

/// `IconConverterService.SupportedSizes`.
pub const SUPPORTED_SIZES: [i32; 6] = [16, 32, 48, 64, 128, 256];

/// `IconConverterService.Convert`: one entry per distinct size, smallest first;
/// 256 is stored as PNG, the others as BMP (XOR image plus AND mask).
pub fn convert(source: &Bitmap, sizes: &[i32], depth: IconBitDepth) -> Result<Vec<u8>, String> {
    if source.width() == 0 || source.height() == 0 {
        return Err(sharex_strings::tools::IconConverterService_Source_image_empty().into());
    }
    let mut selected: Vec<i32> = sizes.to_vec();
    selected.sort();
    selected.dedup();
    if selected.is_empty() {
        return Err(sharex_strings::tools::IconConverterService_Select_icon_size().into());
    }
    if selected.iter().any(|s| !SUPPORTED_SIZES.contains(s)) {
        return Err(sharex_strings::tools::IconConverterService_Unsupported_icon_size().into());
    }
    let mut entries = Vec::new();
    for size in selected {
        let bitmap = resize_to_square(source, size as u32);
        let data = if size == 256 { encode_png(&bitmap, depth) } else { encode_bitmap(&bitmap, depth) };
        entries.push((size, data));
    }
    Ok(write_icon(&entries, depth))
}

/// `ResizeToSquare`: the image fitted and centred in a transparent square,
/// Catmull-Rom, anti-aliased.
pub fn resize_to_square(source: &Bitmap, size: u32) -> Bitmap {
    let mut out = imaging::create_bitmap(size, size);
    let s = size as f32;
    let scale = (s / source.width() as f32).min(s / source.height() as f32);
    let w = source.width() as f32 * scale;
    let h = source.height() as f32 * scale;
    let left = (s - w) / 2.0;
    let top = (s - h) / 2.0;
    let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
    paint.anti_alias = true;
    Canvas::new(&mut out).draw_image_rect(
        source,
        None,
        Rect::ltrb(left, top, left + w, top + h),
        Sampling::CatmullRom,
        Some(&paint),
    );
    out
}

fn encode_png(bmp: &Bitmap, depth: IconBitDepth) -> Vec<u8> {
    match depth {
        IconBitDepth::Palette8 => encode_indexed_png(bmp),
        IconBitDepth::TrueColor32 => imaging::encode_png(bmp),
    }
}

fn align4(v: usize) -> usize {
    (v + 3) & !3
}

fn encode_bitmap(bmp: &Bitmap, depth: IconBitDepth) -> Vec<u8> {
    let (w, h) = (bmp.width() as usize, bmp.height() as usize);
    let xor_stride = if depth == IconBitDepth::Palette8 { align4(w) } else { w * 4 };
    let mask_stride = align4(w.div_ceil(8));
    let image_size = xor_stride * h + mask_stride * h;
    let mut out = Vec::new();
    out.extend_from_slice(&40i32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&((h * 2) as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(depth as u16).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&(image_size as i32).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&(if depth == IconBitDepth::Palette8 { 256i32 } else { 0 }).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    if depth == IconBitDepth::Palette8 {
        for i in 0..256 {
            let c = palette_color(i);
            out.extend_from_slice(&[c[2], c[1], c[0], 0]);
        }
    }
    let mut row = vec![0u8; xor_stride];
    for y in (0..h).rev() {
        row.fill(0);
        for x in 0..w {
            let c = imaging::get_color(bmp, x as u32, y as u32);
            if depth == IconBitDepth::Palette8 {
                row[x] = palette_index(c);
            } else {
                row[x * 4..x * 4 + 4].copy_from_slice(&[c[2], c[1], c[0], c[3]]);
            }
        }
        out.extend_from_slice(&row);
    }
    let mut mask = vec![0u8; mask_stride];
    for y in (0..h).rev() {
        mask.fill(0);
        for x in 0..w {
            if imaging::get_color(bmp, x as u32, y as u32)[3] < 128 {
                mask[x / 8] |= 0x80 >> (x % 8);
            }
        }
        out.extend_from_slice(&mask);
    }
    out
}

/// `GetPaletteIndex`: 0 for transparent, else the 6x7x6 colour cube.
pub fn palette_index(c: Rgba) -> u8 {
    if c[3] < 128 {
        return 0;
    }
    let r = (c[0] as i32 * 5 + 127) / 255;
    let g = (c[1] as i32 * 6 + 127) / 255;
    let b = (c[2] as i32 * 5 + 127) / 255;
    (1 + r * 42 + g * 6 + b) as u8
}

/// `GetPaletteColor`.
pub fn palette_color(index: i32) -> Rgba {
    if index == 0 {
        // SKColors.Transparent: transparent white.
        return [255, 255, 255, 0];
    }
    if index > 252 {
        let gray = ((index - 252) * 255 / 3) as u8;
        return [gray, gray, gray, 255];
    }
    let v = index - 1;
    let (r, g, b) = (v / 42, v / 6 % 7, v % 6);
    [(r * 255 / 5) as u8, (g * 255 / 6) as u8, (b * 255 / 5) as u8, 255]
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { 0xEDB8_8320 ^ (crc >> 1) } else { crc >> 1 };
        }
    }
    !crc
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut all = kind.to_vec();
    all.extend_from_slice(data);
    out.extend_from_slice(&crc32(&all).to_be_bytes());
}

/// `EncodeIndexedPng`: 8-bit indexed PNG, palette and tRNS (index 0
/// transparent), unfiltered rows through zlib: .NET's `ZLibStream(Optimal)` is
/// zlib-ng at level 6, which zlib-rs reproduces.
fn encode_indexed_png(bmp: &Bitmap) -> Vec<u8> {
    let (w, h) = (bmp.width(), bmp.height());
    let mut out = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 3, 0, 0, 0]);
    png_chunk(&mut out, b"IHDR", &ihdr);
    let palette: Vec<u8> = (0..256).flat_map(|i| {
        let c = palette_color(i);
        [c[0], c[1], c[2]]
    }).collect();
    png_chunk(&mut out, b"PLTE", &palette);
    let mut alpha = vec![255u8; 256];
    alpha[0] = 0;
    png_chunk(&mut out, b"tRNS", &alpha);
    let mut raw = Vec::with_capacity(((w + 1) * h) as usize);
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            raw.push(palette_index(imaging::get_color(bmp, x, y)));
        }
    }
    let compressed = {
        use std::io::Write;
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
        // ZLibStream.Write per row, as ShareX writes them.
        for row in raw.chunks(w as usize + 1) {
            z.write_all(row).expect("zlib");
        }
        z.finish().expect("zlib")
    };
    png_chunk(&mut out, b"IDAT", &compressed);
    png_chunk(&mut out, b"IEND", &[]);
    out
}

fn write_icon(entries: &[(i32, Vec<u8>)], depth: IconBitDepth) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + entries.len() * 16;
    for (size, data) in entries {
        let s = if *size == 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[s, s, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&(depth as u16).to_le_bytes());
        out.extend_from_slice(&(data.len() as i32).to_le_bytes());
        out.extend_from_slice(&(offset as i32).to_le_bytes());
        offset += data.len();
    }
    for (_, data) in entries {
        out.extend_from_slice(data);
    }
    out
}

/// `IconConverterViewModel.LoadImage`: `SKBitmap.Decode` (no EXIF orientation).
pub fn load_image(path: &str) -> Option<Bitmap> {
    imaging::load_image_with(path, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_round_trip() {
        for i in 1..=252 {
            assert_eq!(palette_index(palette_color(i)) as i32, i);
        }
        assert_eq!(palette_index([255, 255, 255, 0]), 0);
    }

    #[test]
    fn icon_directory() {
        let mut b = Bitmap::new(4, 2);
        b.set_pixel(0, 0, [255, 0, 0, 255]);
        let ico = convert(&b, &[32, 16, 16], IconBitDepth::TrueColor32).unwrap();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(ico[6], 16);
        assert_eq!(ico[22], 32);
    }
}
