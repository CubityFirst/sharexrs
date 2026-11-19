//! Clipboard viewer: `ClipboardViewerData` (a snapshot of every clipboard
//! format and its preview) and `ClipboardViewerViewModel`'s texts. Reading
//! the clipboard itself is the window's job (`AvaloniaClipboard.Capture`:
//! text as `UnicodeText`, files as `FileDrop`, a bitmap as PNG bytes under
//! `Bitmap`, `CF_DIB` as `DeviceIndependentBitmap`, `CF_DIBV5` as
//! `Format17`, anything else as its raw bytes under its own name).

use crate::imaging::{self, Bitmap};
use crate::text::{format_dotnet, Arg};

/// `ClipboardHelpers.FORMAT_PNG`.
pub const FORMAT_PNG: &str = "PNG";
/// `ClipboardHelpers.FORMAT_17` (`CF_DIBV5`).
pub const FORMAT_17: &str = "Format17";
/// `ClipboardDataFormats`.
pub const FORMAT_TEXT: &str = "Text";
pub const FORMAT_UNICODE_TEXT: &str = "UnicodeText";
pub const FORMAT_FILE_DROP: &str = "FileDrop";
pub const FORMAT_BITMAP: &str = "Bitmap";
pub const FORMAT_DIB: &str = "DeviceIndependentBitmap";
pub const FORMAT_HTML: &str = "HTML Format";

/// A format's value in the snapshot, as `ClipboardData` holds it.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardValue {
    Text(String),
    Files(Vec<String>),
    Bytes(Vec<u8>),
}

/// `ClipboardData`: formats in the order they were added, looked up
/// ignoring case (a later value for the same format replaces the earlier
/// one and keeps its place and spelling, as `Dictionary` does).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipboardSnapshot {
    entries: Vec<(String, ClipboardValue)>,
}

/// What the preview pane shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Preview {
    Text(String),
    Image(Bitmap),
}

impl ClipboardSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    /// `SetData`.
    pub fn set(&mut self, format: &str, value: ClipboardValue) {
        match self.entries.iter_mut().find(|(f, _)| sharex_helpers::culture::ordinal_ignore_case_eq(f, format)) {
            Some(e) => e.1 = value,
            None => self.entries.push((format.to_string(), value)),
        }
    }

    /// `GetFormats`.
    pub fn formats(&self) -> Vec<&str> {
        self.entries.iter().map(|(f, _)| f.as_str()).collect()
    }

    /// `GetData`.
    pub fn get(&self, format: &str) -> Option<&ClipboardValue> {
        self.entries
            .iter()
            .find(|(f, _)| sharex_helpers::culture::ordinal_ignore_case_eq(f, format))
            .map(|(_, v)| v)
    }

    /// `ClipboardViewerData.GetPreview(format)`: PNG, `image/png` and
    /// `Bitmap` bytes decoded as images, `CF_DIB`/`CF_DIBV5` through
    /// [`image_from_clipboard_dib`], files one per line, other bytes as
    /// upper-case hex, text as it is. An error is the exception's message,
    /// which the window shows as `ClipboardViewerViewModel_Preview_failed`.
    pub fn preview(&self, format: &str) -> Result<Preview, String> {
        let Some(data) = self.get(format) else {
            return Ok(Preview::Text(String::new()));
        };
        let is = |name: &str| sharex_helpers::culture::ordinal_ignore_case_eq(format, name);
        match data {
            ClipboardValue::Bytes(bytes) => {
                if is(FORMAT_PNG) || is("image/png") || is(FORMAT_BITMAP) {
                    return imaging::decode(bytes, true)
                        .map(Preview::Image)
                        .map_err(|_| "The image format is not supported.".to_string());
                }
                if is(FORMAT_DIB) || is(FORMAT_17) {
                    // A null bitmap: FromImage(null) throws.
                    return image_from_clipboard_dib(bytes)
                        .map(Preview::Image)
                        .ok_or_else(|| "Value cannot be null.".to_string());
                }
                Ok(Preview::Text(bytes.iter().map(|b| format!("{b:02X}")).collect()))
            }
            ClipboardValue::Files(files) => Ok(Preview::Text(files.join(crate::system::NEW_LINE))),
            ClipboardValue::Text(text) => Ok(Preview::Text(text.clone())),
        }
    }
}

/// `ClipboardHelpersEx.ImageFromClipboardDib` (also `DIBV5ToBitmap`): a
/// 32 bpp DIB with the standard masks is copied as premultiplied BGRA
/// (opaque unless the header's alpha mask is set or, with bit fields, an
/// alpha byte isn't zero), anything else goes to Skia's BMP decoder behind a
/// file header. `None` where ShareX gets null.
pub fn image_from_clipboard_dib(data: &[u8]) -> Option<Bitmap> {
    let i32_at = |o: usize| data.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
    let u32_at = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    if data.len() < 40 {
        return None;
    }
    let header = i32_at(0)?;
    if header < 40 || header as usize > data.len() {
        return None;
    }
    let width = i32_at(4)?;
    let signed_height = i32_at(8)?;
    // Math.Abs(int.MinValue) throws OverflowException: null.
    let height = signed_height.checked_abs()?;
    let bit_count = u16::from_le_bytes([data[14], data[15]]) as i32;
    let compression = i32_at(16)?;
    let color_count = u32_at(32)?;
    let palette_size = if bit_count <= 8 {
        let n = if color_count == 0 { 1u32 << bit_count } else { color_count };
        i32::try_from(n).ok()?.checked_mul(4)?
    } else {
        0
    };
    let masks = match (header, compression) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let pixel_offset = header.checked_add(palette_size)?.checked_add(masks)?;
    let row_bytes = width.checked_mul(bit_count)?.checked_add(31)? / 32 * 4;
    if width <= 0
        || height <= 0
        || pixel_offset as usize > data.len()
        || row_bytes as i64 * height as i64 > (data.len() - pixel_offset as usize) as i64
    {
        return None;
    }
    let (w, h, row, off) = (width as usize, height as usize, row_bytes as usize, pixel_offset as usize);
    if bit_count == 32 && matches!(compression, 0 | 3 | 6) {
        let standard = compression == 0 || (u32_at(40)? == 0x00FF_0000 && u32_at(44)? == 0x0000_FF00 && u32_at(48)? == 0x0000_00FF);
        if standard {
            let mut alpha = header >= 56 && u32_at(52)? == 0xFF00_0000;
            if !alpha && compression != 0 {
                alpha = (off + 3..off + row * h).step_by(4).any(|i| data[i] != 0);
            }
            let mut px = vec![0u8; w * h * 4];
            for y in 0..h {
                let src = off + if signed_height > 0 { h - y - 1 } else { y } * row;
                for x in 0..w {
                    let p = &data[src + x * 4..src + x * 4 + 4];
                    // Bgra8888, Premul or Opaque: the bytes as they are.
                    px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[p[2], p[1], p[0], if alpha { p[3] } else { 255 }]);
                }
            }
            return Some(Bitmap::from_premultiplied(w as u32, h as u32, px));
        }
    }
    let mut file = Vec::with_capacity(14 + data.len());
    file.extend_from_slice(&0x4D42u16.to_le_bytes());
    file.extend_from_slice(&((14 + data.len()) as u32).to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    file.extend_from_slice(&((14 + off) as u32).to_le_bytes());
    file.extend_from_slice(data);
    imaging::decode(&file, true).ok()
}

/// `FormatCountText`.
pub fn format_count_text(count: usize) -> String {
    if count == 1 {
        sharex_strings::tools::ClipboardViewerViewModel_One_format().to_string()
    } else {
        format_dotnet(sharex_strings::tools::ClipboardViewerViewModel_Format_count(), &[Arg::I64(count as i64)])
    }
}

/// `PreviewDetails` for a preview: the image's size, or the text's length
/// in UTF-16 code units.
pub fn preview_details(preview: &Preview) -> String {
    match preview {
        Preview::Image(b) => format_dotnet(
            sharex_strings::tools::ClipboardViewerViewModel_Image_details(),
            &[Arg::I64(b.width() as i64), Arg::I64(b.height() as i64)],
        ),
        Preview::Text(t) => format_dotnet(
            sharex_strings::tools::ClipboardViewerViewModel_Text_details(),
            &[Arg::I64(t.encode_utf16().count() as i64)],
        ),
    }
}

/// `PreviewText` when the preview failed.
pub fn preview_failed_text(message: &str) -> String {
    format_dotnet(sharex_strings::tools::ClipboardViewerViewModel_Preview_failed(), &[message.into()])
}

/// `Refresh`'s choice of the selected format: the previous one if it's
/// still there (ordinal), else the first.
pub fn reselect<'a>(formats: &[&'a str], previous: Option<&str>) -> Option<&'a str> {
    formats.iter().copied().find(|f| Some(*f) == previous).or_else(|| formats.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_and_texts() {
        let mut s = ClipboardSnapshot::new();
        s.set("UnicodeText", ClipboardValue::Text("hi".into()));
        s.set("FileDrop", ClipboardValue::Files(vec!["a".into(), "b".into()]));
        s.set("unicodetext", ClipboardValue::Text("again".into()));
        s.set("Custom", ClipboardValue::Bytes(vec![0, 0xAB, 0x10]));
        assert_eq!(s.formats(), ["UnicodeText", "FileDrop", "Custom"]);
        assert_eq!(s.preview("UNICODETEXT"), Ok(Preview::Text("again".into())));
        assert_eq!(s.preview("Custom"), Ok(Preview::Text("00AB10".into())));
        assert_eq!(s.preview("Missing"), Ok(Preview::Text(String::new())));
        assert_eq!(s.preview("FileDrop"), Ok(Preview::Text(format!("a{}b", crate::system::NEW_LINE))));
        assert_eq!(format_count_text(1), "1 format");
        assert_eq!(format_count_text(3), "3 formats");
        assert_eq!(preview_details(&Preview::Text("a😀".into())), "Text  |  3 characters");
        assert_eq!(reselect(&["A", "B"], Some("B")), Some("B"));
        assert_eq!(reselect(&["A", "B"], Some("b")), Some("A"));
    }
}
