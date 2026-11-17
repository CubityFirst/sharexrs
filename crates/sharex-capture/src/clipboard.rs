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

#[cfg(test)]
mod tests {
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
