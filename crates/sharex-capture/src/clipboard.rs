//! `ShareX.HelpersLib/Helpers/ClipboardHelpers.CopyImage`: on Windows a
//! "PNG" (with transparency) and an opaque 24 bpp CF_DIB on white, which
//! is what Office and Windows clipboard history expect; elsewhere the
//! system's image format (arboard).

use image::RgbaImage;

/// The image as PNG, as `img.Save(png, ImageFormat.Png)`.
pub fn png_bytes(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(image.as_raw(), image.width(), image.height(), image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

use image::ImageEncoder;

/// `CreateDib`: the image over white (`FillBackground`), as a bottom-up
/// 24 bpp BMP without its file header.
pub fn dib_bytes(image: &RgbaImage) -> Vec<u8> {
    let (w, h) = image.dimensions();
    let stride = (w * 3).div_ceil(4) * 4;
    let size = stride * h;
    let mut out = Vec::with_capacity(40 + size as usize);
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&size.to_le_bytes());
    // 96 DPI, as ShareX's BMP encoder writes it.
    out.extend_from_slice(&3780i32.to_le_bytes());
    out.extend_from_slice(&3780i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for y in (0..h).rev() {
        let start = out.len();
        for x in 0..w {
            let p = image.get_pixel(x, y);
            let a = p[3] as u32;
            let over_white = |c: u8| ((c as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
            out.extend_from_slice(&[over_white(p[2]), over_white(p[1]), over_white(p[0])]);
        }
        out.resize(start + stride as usize, 0);
    }
    out
}

/// Copies `image` to the clipboard. `owner` is a window of ours (Windows
/// needs one to own the clipboard data).
pub fn copy_image(image: &RgbaImage, owner: Option<isize>) -> Result<(), String> {
    #[cfg(windows)]
    {
        let owner = owner.ok_or("no window to own the clipboard")?;
        crate::platform::set_clipboard_image(owner, &png_bytes(image)?, &dib_bytes(image))
    }
    #[cfg(not(windows))]
    {
        let _ = owner;
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard
            .set_image(arboard::ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: std::borrow::Cow::Borrowed(image.as_raw()),
            })
            .map_err(|e| e.to_string())
    }
}

/// `ClipboardHelpers.CopyText`: the text as CF_UNICODETEXT on Windows.
pub fn copy_text(text: &str, owner: Option<isize>) -> Result<(), String> {
    if text.is_empty() {
        return Err("no text".into());
    }
    #[cfg(windows)]
    {
        let owner = owner.ok_or("no window to own the clipboard")?;
        let wide: Vec<u8> = text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
        crate::platform::set_clipboard(owner, &[(crate::platform::ClipboardFormat::Id(13), &wide)]) // CF_UNICODETEXT
    }
    #[cfg(not(windows))]
    {
        let _ = owner;
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard.set_text(text.to_owned()).map_err(|e| e.to_string())
    }
}

/// `ClipboardHelpers.CopyFile`: the files as a file drop list (CF_HDROP on
/// Windows), so pasting in a file manager copies them.
pub fn copy_files(paths: &[std::path::PathBuf], owner: Option<isize>) -> Result<(), String> {
    if paths.is_empty() {
        return Err("no files".into());
    }
    #[cfg(windows)]
    {
        let owner = owner.ok_or("no window to own the clipboard")?;
        crate::platform::set_clipboard(owner, &[(crate::platform::ClipboardFormat::Id(15), &drop_files(paths))]) // CF_HDROP
    }
    #[cfg(not(windows))]
    {
        let _ = owner;
        Err("copying files isn't ported to this platform yet".into())
    }
}

/// A `DROPFILES` header and the paths as UTF-16, each NUL-terminated, with
/// a final NUL.
#[cfg_attr(not(windows), allow(dead_code))]
fn drop_files(paths: &[std::path::PathBuf]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&20u32.to_le_bytes()); // pFiles: the header's size
    out.extend_from_slice(&0i32.to_le_bytes()); // pt
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // fNC
    out.extend_from_slice(&1u32.to_le_bytes()); // fWide
    for p in paths {
        for u in p.to_string_lossy().encode_utf16().chain([0]) {
            out.extend_from_slice(&u.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// `ClipboardHelpers.Clear`: empties the clipboard.
pub fn clear(owner: Option<isize>) -> Result<(), String> {
    #[cfg(windows)]
    {
        let owner = owner.ok_or("no window to own the clipboard")?;
        crate::platform::set_clipboard(owner, &[])
    }
    #[cfg(not(windows))]
    {
        let _ = owner;
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard.clear().map_err(|e| e.to_string())
    }
}

/// `ClipboardHelpers.ContainsText`.
pub fn contains_text() -> bool {
    #[cfg(windows)]
    {
        crate::platform::clipboard_has_format(13) // CF_UNICODETEXT
    }
    #[cfg(not(windows))]
    {
        arboard::Clipboard::new().and_then(|mut c| c.get_text()).is_ok_and(|t| !t.is_empty())
    }
}

/// `ClipboardHelpers.ContainsImage`: a bitmap format (PNG, a DIB or
/// `CF_BITMAP`).
pub fn contains_image() -> bool {
    #[cfg(windows)]
    {
        use crate::platform::{clipboard_format_id, clipboard_has_format};
        // CF_BITMAP, CF_DIB, CF_DIBV5.
        [2, 8, 17].into_iter().any(clipboard_has_format) || clipboard_has_format(clipboard_format_id("PNG"))
    }
    #[cfg(not(windows))]
    {
        arboard::Clipboard::new().and_then(|mut c| c.get_image()).is_ok()
    }
}

/// `ClipboardHelpers.ContainsFileDropList`.
pub fn contains_file_drop_list() -> bool {
    #[cfg(windows)]
    {
        crate::platform::clipboard_has_format(15) // CF_HDROP
    }
    #[cfg(not(windows))]
    {
        arboard::Clipboard::new().and_then(|mut c| c.get().file_list()).is_ok_and(|f| !f.is_empty())
    }
}

/// `ClipboardHelpers.GetText`.
pub fn get_text() -> Option<String> {
    #[cfg(windows)]
    {
        let bytes = crate::platform::get_clipboard_data(13)?; // CF_UNICODETEXT
        let wide: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&u| u != 0).collect();
        Some(String::from_utf16_lossy(&wide))
    }
    #[cfg(not(windows))]
    {
        arboard::Clipboard::new().and_then(|mut c| c.get_text()).ok()
    }
}

/// `ClipboardHelpers.GetFileDropList`.
pub fn get_file_drop_list() -> Option<Vec<std::path::PathBuf>> {
    #[cfg(windows)]
    {
        let bytes = crate::platform::get_clipboard_data(15)?; // CF_HDROP
        parse_drop_files(&bytes)
    }
    #[cfg(not(windows))]
    {
        arboard::Clipboard::new().and_then(|mut c| c.get().file_list()).ok()
    }
}

/// A `DROPFILES` block's paths (wide or ANSI).
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_drop_files(bytes: &[u8]) -> Option<Vec<std::path::PathBuf>> {
    let offset = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
    let wide = u32::from_le_bytes(bytes.get(16..20)?.try_into().ok()?) != 0;
    let list = bytes.get(offset..)?;
    let names: Vec<String> = if wide {
        let units: Vec<u16> = list.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        units.split(|&u| u == 0).take_while(|s| !s.is_empty()).map(String::from_utf16_lossy).collect()
    } else {
        list.split(|&b| b == 0).take_while(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect()
    };
    Some(names.into_iter().map(std::path::PathBuf::from).collect())
}

/// `ClipboardHelpers.GetImage`: "PNG" first, then the DIBs
/// (`ImageFromClipboardDib`).
pub fn get_image() -> Option<RgbaImage> {
    #[cfg(windows)]
    {
        use crate::platform::{clipboard_format_id, get_clipboard_data};
        if let Some(png) = get_clipboard_data(clipboard_format_id("PNG"))
            && let Ok(image) = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        {
            return Some(image.to_rgba8());
        }
        for format in [17, 8] {
            if let Some(image) = get_clipboard_data(format).and_then(|d| image_from_dib(&d)) {
                return Some(image);
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        let data = arboard::Clipboard::new().and_then(|mut c| c.get_image()).ok()?;
        RgbaImage::from_raw(data.width as u32, data.height as u32, data.bytes.into_owned())
    }
}

/// `ClipboardHelpersEx.ImageFromClipboardDib`: 32 bpp DIBs with the
/// standard masks are read as BGRA (premultiplied when they have alpha),
/// anything else is decoded as a BMP file.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn image_from_dib(data: &[u8]) -> Option<RgbaImage> {
    let i32_at = |o: usize| data.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
    let u32_at = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    if data.len() < 40 {
        return None;
    }
    let header = i32_at(0)? as usize;
    if header < 40 || header > data.len() {
        return None;
    }
    let width = i32_at(4)?;
    let signed_height = i32_at(8)?;
    let height = signed_height.unsigned_abs() as usize;
    let bits = u16::from_le_bytes([data[14], data[15]]) as usize;
    let compression = i32_at(16)?;
    let colors = u32_at(32)?;
    let palette = if bits <= 8 { (if colors == 0 { 1u32 << bits } else { colors }) as usize * 4 } else { 0 };
    let masks = match (header, compression) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let offset = header + palette + masks;
    if width <= 0 || height == 0 {
        return None;
    }
    let width = width as usize;
    let row = (width * bits).div_ceil(32) * 4;
    if offset > data.len() || row * height > data.len() - offset {
        return None;
    }
    if bits == 32 && matches!(compression, 0 | 3 | 6) {
        let standard = compression == 0 || (u32_at(40)? == 0x00FF_0000 && u32_at(44)? == 0x0000_FF00 && u32_at(48)? == 0x0000_00FF);
        if standard {
            let mut alpha = header >= 56 && u32_at(52)? == 0xFF00_0000;
            if !alpha && compression != 0 {
                alpha = (offset + 3..offset + row * height).step_by(4).any(|i| data[i] != 0);
            }
            let mut image = RgbaImage::new(width as u32, height as u32);
            for y in 0..height {
                let src = offset + if signed_height > 0 { height - y - 1 } else { y } * row;
                for x in 0..width {
                    let p = &data[src + x * 4..src + x * 4 + 4];
                    let (b, g, r, a) = (p[0], p[1], p[2], if alpha { p[3] } else { 255 });
                    // Skia's premultiplied BGRA read back as straight RGBA.
                    let un = |c: u8| if a == 0 || a == 255 { c } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
                    image.put_pixel(x as u32, y as u32, image::Rgba([un(r), un(g), un(b), a]));
                }
            }
            return Some(image);
        }
    }
    let mut file = Vec::with_capacity(14 + data.len());
    file.extend_from_slice(&0x4D42u16.to_le_bytes());
    file.extend_from_slice(&((14 + data.len()) as u32).to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    file.extend_from_slice(&((14 + offset) as u32).to_le_bytes());
    file.extend_from_slice(data);
    image::load_from_memory_with_format(&file, image::ImageFormat::Bmp).ok().map(|i| i.to_rgba8())
}

#[cfg(test)]
mod tests {
    #[test]
    fn dibs_read_back_as_written() {
        let mut image = RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]));
        image.put_pixel(2, 1, image::Rgba([200, 100, 50, 255]));
        let read = image_from_dib(&dib_bytes(&image)).unwrap();
        assert_eq!(read, image);
    }

    #[test]
    fn drop_lists_parse_back() {
        let paths = [std::path::PathBuf::from("C:\\a.png"), std::path::PathBuf::from("b c.txt")];
        assert_eq!(parse_drop_files(&drop_files(&paths)).unwrap(), paths);
    }

    #[test]
    fn drop_lists_are_wide_and_double_terminated() {
        let d = super::drop_files(&[std::path::PathBuf::from("a"), std::path::PathBuf::from("bc")]);
        assert_eq!(d.len(), 20 + 2 * (2 + 3 + 1));
        assert_eq!(&d[20..24], &[b'a', 0, 0, 0]);
        assert_eq!(&d[d.len() - 4..], &[0, 0, 0, 0]);
    }

    use super::*;

    #[test]
    fn dibs_are_bottom_up_bgr_on_white() {
        let mut image = RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        image.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
        let dib = dib_bytes(&image);
        assert_eq!(dib.len(), 40 + 8 * 2);
        // Row 0 of the image is last; its first pixel is transparent -> white.
        assert_eq!(&dib[48..51], &[255, 255, 255]);
        assert_eq!(&dib[40..43], &[30, 20, 10]);
    }
}
