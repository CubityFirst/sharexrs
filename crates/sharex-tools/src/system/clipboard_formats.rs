//! `AvaloniaClipboard.Capture` outside Windows: the formats Avalonia 12.1.6
//! reports for the clipboard on macOS (Avalonia.Native) and Linux (X11),
//! in its order, named as ShareX names them, with the values it reads.
//! The native side (the pasteboard, the X11 selection) is behind
//! [`Pasteboard`] and [`Selection`], so tests use fakes.
//!
//! macOS, from Avalonia.Native's managed code and `libAvaloniaNative.dylib`
//! (`Clipboard::GetFormats`, `ConvertPasteboardTypes`, `IsTextFormat`,
//! `GetItemValueAsBytes`):
//! - the formats are `[NSPasteboard types]`, plus `public.png` at the end
//!   when there's `public.tiff` or `public.jpeg` but no `public.png`; each
//!   item's are its own `types`, converted the same way;
//! - `public.utf8-plain-text` is `DataFormat.Text`, `public.file-url`
//!   `DataFormat.File`, `public.png` `DataFormat.Bitmap`; any other type a
//!   string format when its `UTType` conforms to `public.text`, else bytes,
//!   named by the type (`net.avaloniaui.app.uti.` taken off);
//! - values come from the first item with the format: `stringForType`,
//!   `dataForType` (empty data is none; `public.png` falls back to the TIFF
//!   or JPEG as PNG), a string format's bytes read as UTF-16 and a byte
//!   format's string written as UTF-16.
//!
//! Linux (`Avalonia.X11.Selections.DataFormatHelper`, `SelectionDataReader`):
//! - the formats are the owner's `TARGETS` in order, without `TARGETS`,
//!   `MULTIPLE` and `SAVE_TARGETS`; the text targets (`UTF16_STRING`,
//!   `UTF8_STRING`, `text/plain`, `text/plain;charset=utf-8`, `STRING`) are
//!   one `DataFormat.Text` where the first of them is, `text/uri-list` is
//!   `DataFormat.File`, anything else bytes named by the target
//!   (`application/avn-fmt.` taken off), and `DataFormat.Bitmap` is added
//!   at the end when there's `image/png` or `image/jpeg`;
//! - text is read from the preferred text target in that order, decoded by
//!   the reply's type; the bitmap from `image/png`, else `image/jpeg`; files
//!   from `text/uri-list` (existing local files and folders only).
//!
//! Then `Capture`: `Text` is `UnicodeText`, `Bitmap` `Bitmap` (the bitmap
//! saved as PNG), `File` `FileDrop`; a format without a value, or whose
//! value fails to read, isn't listed.

use crate::imaging;
use crate::system::clipboard_viewer::{self as cv, ClipboardSnapshot, ClipboardValue};

/// A format as Avalonia's `DataFormat` has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    /// `DataFormat.Text`.
    Text,
    /// `DataFormat.File`.
    File,
    /// `DataFormat.Bitmap`.
    Bitmap,
    /// `DataFormat.FromSystemName<T>`: an application format (the prefix
    /// taken off) or a platform one, of strings or bytes.
    Other { identifier: String, application: bool, string: bool },
}

impl Format {
    /// `DataFormat.FromSystemName<T>(systemName, applicationPrefix)`.
    fn from_system_name(name: &str, prefix: &str, string: bool) -> Self {
        if name.len() > prefix.len()
            && name.is_char_boundary(prefix.len())
            && name[..prefix.len()].eq_ignore_ascii_case(prefix)
            && name[prefix.len()..].chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Format::Other { identifier: name[prefix.len()..].to_owned(), application: true, string };
        }
        Format::Other { identifier: name.to_owned(), application: false, string }
    }

    /// `ToSystemName(applicationPrefix)`.
    fn system_name(&self, prefix: &str) -> Option<String> {
        match self {
            Format::Other { identifier, application: true, .. } => Some(format!("{prefix}{identifier}")),
            Format::Other { identifier, .. } => Some(identifier.clone()),
            _ => None,
        }
    }

    /// `DataFormat.Equals`: the kind and the identifier (a string and a
    /// bytes format of the same name are equal).
    fn same(&self, other: &Format) -> bool {
        match (self, other) {
            (Format::Other { identifier: a, application: x, .. }, Format::Other { identifier: b, application: y, .. }) => a == b && x == y,
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }

    /// The name `AvaloniaClipboard.Capture` gives it.
    pub fn name(&self) -> String {
        match self {
            Format::Text => cv::FORMAT_UNICODE_TEXT.to_owned(),
            Format::Bitmap => cv::FORMAT_BITMAP.to_owned(),
            Format::File => cv::FORMAT_FILE_DROP.to_owned(),
            Format::Other { identifier, .. } => match identifier.as_str() {
                "CF_DIB" => cv::FORMAT_DIB.to_owned(),
                "CF_DIBV5" => cv::FORMAT_17.to_owned(),
                other => other.to_owned(),
            },
        }
    }
}

/// A bitmap's bytes as `Capture` keeps them: decoded as Avalonia's
/// `Bitmap` does (or not listed), saved as PNG.
fn bitmap_png(bytes: &[u8]) -> Option<Vec<u8>> {
    imaging::decode(bytes, false).ok().map(|b| imaging::encode_png(&b))
}

/// `Encoding.Unicode.GetString`.
fn utf16_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks(2).map(|c| if c.len() == 2 { u16::from_le_bytes([c[0], c[1]]) } else { 0xFFFD }).collect();
    String::from_utf16_lossy(&units)
}

/// `Encoding.Unicode.GetBytes`.
fn utf16_bytes(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

/// `Uri.TryCreate(s, UriKind.Absolute)` for a `file:` URI, and its
/// `LocalPath` (escapes decoded; a host other than `localhost` makes a
/// `//host/path` path).
pub fn file_uri_local_path(uri: &str) -> Option<String> {
    let rest = uri.get(..5).filter(|s| s.eq_ignore_ascii_case("file:")).map(|_| &uri[5..])?;
    let rest = rest.strip_prefix("//")?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let path = percent_decode(path);
    if host.is_empty() || host.eq_ignore_ascii_case("localhost") {
        Some(path)
    } else {
        Some(format!("//{host}{path}"))
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ----------------------------------------------------------------------
// macOS: Avalonia.Native.

/// `NSPasteboardTypeString`.
pub const PASTEBOARD_STRING: &str = "public.utf8-plain-text";
/// `NSPasteboardTypeFileURL`.
pub const PASTEBOARD_FILE_URL: &str = "public.file-url";
/// `NSPasteboardTypePNG`.
pub const PASTEBOARD_PNG: &str = "public.png";
/// `NSPasteboardTypeTIFF`.
pub const PASTEBOARD_TIFF: &str = "public.tiff";
/// `public.jpeg`.
pub const PASTEBOARD_JPEG: &str = "public.jpeg";
/// Avalonia.Native's `AppPrefix`.
const MAC_APP_PREFIX: &str = "net.avaloniaui.app.uti.";

/// The general pasteboard, as `libAvaloniaNative`'s `Clipboard` reads it.
pub trait Pasteboard {
    /// `[NSPasteboard types]`.
    fn types(&self) -> Vec<String>;
    /// Each of `pasteboardItems`' `types`.
    fn item_types(&self) -> Vec<Vec<String>>;
    /// `GetItemValueAsString`: the item's `stringForType`.
    fn string(&self, item: usize, ty: &str) -> Option<String>;
    /// `GetItemValueAsBytes`: the item's `dataForType` (none when empty);
    /// for `public.png` without one, the TIFF's (else the JPEG's) bitmap
    /// representation as PNG.
    fn bytes(&self, item: usize, ty: &str) -> Option<Vec<u8>>;
    /// `IsTextFormat`: `[UTType typeWithIdentifier:]` conforms to
    /// `UTTypeText`.
    fn is_text_format(&self, ty: &str) -> bool;
    /// `TryGetFilePathUri` and `StorageProviderApi.TryGetStorageItem`'s
    /// `TryGetLocalPath`: a file URL's path (file reference URLs resolved)
    /// when the file or folder is there.
    fn file_path(&self, url: &str) -> Option<String>;
}

/// `Clipboard::ConvertPasteboardTypes`: `public.png` added at the end when
/// there's a TIFF or a JPEG but no PNG.
pub fn convert_pasteboard_types(types: &[String]) -> Vec<String> {
    let mut out = types.to_vec();
    let has = |t: &str| types.iter().any(|x| x == t);
    if !has(PASTEBOARD_PNG) && (has(PASTEBOARD_TIFF) || has(PASTEBOARD_JPEG)) {
        out.push(PASTEBOARD_PNG.to_owned());
    }
    out
}

/// `ClipboardDataFormatHelper.ToDataFormat`.
fn mac_format(ty: &str, is_text: &dyn Fn(&str) -> bool) -> Format {
    match ty {
        PASTEBOARD_STRING => Format::Text,
        PASTEBOARD_FILE_URL => Format::File,
        PASTEBOARD_PNG => Format::Bitmap,
        other => Format::from_system_name(other, MAC_APP_PREFIX, is_text(other)),
    }
}

/// `ClipboardDataFormatHelper.ToDataFormats` over converted types.
pub fn mac_formats(types: &[String], is_text: &dyn Fn(&str) -> bool) -> Vec<Format> {
    convert_pasteboard_types(types).iter().map(|t| mac_format(t, is_text)).collect()
}

/// `AvaloniaClipboard.Capture` on macOS.
pub fn capture_macos(pb: &dyn Pasteboard) -> ClipboardSnapshot {
    let is_text = |t: &str| pb.is_text_format(t);
    let mut snapshot = ClipboardSnapshot::new();
    let formats = mac_formats(&pb.types(), &is_text);
    // `TryGetData`: no formats, no data transfer.
    if formats.is_empty() {
        return snapshot;
    }
    let items: Vec<Vec<Format>> = pb.item_types().iter().map(|t| mac_formats(t, &is_text)).collect();
    for format in &formats {
        let name = format.name();
        if *format == Format::File {
            // `TryGetFilesAsync`: every item's file.
            let paths: Vec<String> = items
                .iter()
                .enumerate()
                .filter(|(_, f)| f.iter().any(|x| x.same(format)))
                .filter_map(|(i, _)| pb.string(i, PASTEBOARD_FILE_URL))
                .filter_map(|url| pb.file_path(&url))
                .collect();
            if !paths.is_empty() {
                snapshot.set(&name, ClipboardValue::Files(paths));
            }
            continue;
        }
        // `transfer.Items.First(x => x.Formats.Contains(format))`: none
        // throws, which `Capture` logs and skips.
        let Some(item) = items.iter().position(|f| f.iter().any(|x| x.same(format))) else { continue };
        let value = match format {
            Format::Text => pb.string(item, PASTEBOARD_STRING).map(ClipboardValue::Text),
            Format::Bitmap => pb.bytes(item, PASTEBOARD_PNG).and_then(|b| bitmap_png(&b)).map(ClipboardValue::Bytes),
            Format::Other { string, .. } => {
                let ty = format.system_name(MAC_APP_PREFIX).unwrap_or_default();
                if *string {
                    pb.string(item, &ty).or_else(|| pb.bytes(item, &ty).map(|b| utf16_string(&b))).map(ClipboardValue::Text)
                } else {
                    pb.bytes(item, &ty).or_else(|| pb.string(item, &ty).map(|s| utf16_bytes(&s))).map(ClipboardValue::Bytes)
                }
            }
            Format::File => None,
        };
        if let Some(v) = value {
            snapshot.set(&name, v);
        }
    }
    snapshot
}

/// The extra item type Avalonia.Native writes with every item it puts on
/// the pasteboard (`GetAvnCustomDataType`: `net.avaloniaui.inproc.uti.n`
/// and the process id), its value an empty string.
pub fn avalonia_inproc_type(pid: u32) -> String {
    format!("net.avaloniaui.inproc.uti.n{pid}")
}

/// What `AvaloniaClipboard.SetImage` puts on the pasteboard on macOS: one
/// item whose `DataFormat.Bitmap` and `image/png` formats both become
/// `public.png` (`TryConvertFormatToUti`: `image/png` isn't a UTI, so
/// `typeWithMIMEType:`), served from the bitmap as PNG
/// (`FindDataFormat` matches `public.png` to the bitmap only), then
/// Avalonia's in-process type. So the raw `image/png` bytes never reach
/// the pasteboard: the types are `public.png` and the marker.
pub fn macos_image_item_types(pid: u32) -> [String; 2] {
    [PASTEBOARD_PNG.to_owned(), avalonia_inproc_type(pid)]
}

// ----------------------------------------------------------------------
// Linux: Avalonia.X11.

/// `X11Atoms.TextFormats`, by preference.
pub const X11_TEXT_TARGETS: [&str; 5] = ["UTF16_STRING", "UTF8_STRING", "text/plain", "text/plain;charset=utf-8", "STRING"];
/// `DataFormatHelper.MimeTypeTextUriList`.
pub const X11_URI_LIST: &str = "text/uri-list";
/// Avalonia.X11's `AppPrefix`.
const X11_APP_PREFIX: &str = "application/avn-fmt.";

/// The `CLIPBOARD` selection's owner, through X11 conversions.
pub trait Selection {
    /// `SendFormatRequest`: the `TARGETS` reply's atoms' names, in order
    /// (unnamed atoms left out).
    fn targets(&self) -> Vec<String>;
    /// `SendDataRequest(target)`: the reply's type atom's name and bytes
    /// (`INCR` transfers joined), or none when the owner refuses.
    fn convert(&self, target: &str) -> Option<(String, Vec<u8>)>;
    /// `StorageProviderHelpers.TryCreateBclStorageItem`: whether a folder
    /// or a file is at the path.
    fn exists(&self, path: &str) -> bool;
}

/// `DataFormatHelper.ToDataFormats`: the formats and the text targets.
pub fn x11_formats(targets: &[String]) -> (Vec<Format>, Vec<String>) {
    let mut formats = Vec::new();
    let mut text: Option<Vec<String>> = None;
    let mut image = false;
    for t in targets {
        if X11_TEXT_TARGETS.contains(&t.as_str()) {
            if text.is_none() {
                formats.push(Format::Text);
                text = Some(Vec::new());
            }
            text.as_mut().unwrap().push(t.clone());
            continue;
        }
        if matches!(t.as_str(), "MULTIPLE" | "TARGETS" | "SAVE_TARGETS") {
            continue;
        }
        let f = if t == X11_URI_LIST { Format::File } else { Format::from_system_name(t, X11_APP_PREFIX, false) };
        if let Format::Other { identifier, .. } = &f
            && matches!(identifier.as_str(), "image/jpeg" | "image/png")
        {
            image = true;
        }
        formats.push(f);
    }
    if image {
        formats.push(Format::Bitmap);
    }
    (formats, text.unwrap_or_default())
}

/// `DataFormatHelper.TryGetStringEncoding` and `GetString`.
fn x11_decode_text(ty: &str, bytes: &[u8]) -> Option<String> {
    match ty {
        "UTF16_STRING" => Some(utf16_string(bytes)),
        "UTF8_STRING" | "text/plain" | "text/plain;charset=utf-8" => Some(String::from_utf8_lossy(bytes).into_owned()),
        // `Encoding.ASCII`: anything above 0x7F is '?'.
        "STRING" => Some(bytes.iter().map(|&b| if b < 0x80 { b as char } else { '?' }).collect()),
        _ => None,
    }
}

/// `UriListHelper.Utf8BytesToFileUriList`: the lines' existing local
/// files and folders.
fn x11_uri_list(sel: &dyn Selection, bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)).into_owned();
    // `StreamReader.ReadLine`: \n, \r\n or \r.
    text.split('\n')
        .flat_map(|l| l.split('\r'))
        .filter_map(file_uri_local_path)
        .filter(|p| !p.trim().is_empty() && sel.exists(p))
        .collect()
}

/// `AvaloniaClipboard.Capture` on Linux (X11).
pub fn capture_x11(sel: &dyn Selection) -> ClipboardSnapshot {
    let mut snapshot = ClipboardSnapshot::new();
    let (formats, text_targets) = x11_formats(&sel.targets());
    if formats.is_empty() {
        return snapshot;
    }
    // `CreateItemsAsync`: the files are read first, into items of their own.
    let files = formats
        .contains(&Format::File)
        .then(|| sel.convert(X11_URI_LIST))
        .flatten()
        .filter(|(ty, _)| ty == X11_URI_LIST)
        .map(|(_, b)| x11_uri_list(sel, &b))
        .unwrap_or_default();
    for format in &formats {
        let name = format.name();
        let value = match format {
            Format::File => (!files.is_empty()).then(|| ClipboardValue::Files(files.clone())),
            // `GetPreferredStringFormatAtom`.
            Format::Text => X11_TEXT_TARGETS
                .iter()
                .find(|t| text_targets.iter().any(|x| x == *t))
                .and_then(|t| sel.convert(t))
                .and_then(|(ty, b)| x11_decode_text(&ty, &b))
                .map(ClipboardValue::Text),
            Format::Bitmap => {
                let has = |id: &str| formats.iter().any(|f| matches!(f, Format::Other { identifier, .. } if identifier == id));
                let target = if has("image/png") { "image/png" } else { "image/jpeg" };
                sel.convert(target).and_then(|(_, b)| bitmap_png(&b)).map(ClipboardValue::Bytes)
            }
            Format::Other { .. } => {
                let target = format.system_name(X11_APP_PREFIX).unwrap_or_default();
                sel.convert(&target).map(|(_, b)| ClipboardValue::Bytes(b))
            }
        };
        if let Some(v) = value {
            snapshot.set(&name, v);
        }
    }
    snapshot
}

/// The targets Avalonia.X11 offers for `AvaloniaClipboard.SetImage`'s
/// item (`ConvertDataTransfer`): `TARGETS`, `MULTIPLE`, then the bitmap's
/// `image/png` and the raw `image/png` format's, both answered with the
/// bitmap as PNG (`WriteTargetToProperty`).
pub const X11_IMAGE_TARGETS: [&str; 4] = ["TARGETS", "MULTIPLE", "image/png", "image/png"];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]));
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[derive(Default)]
    struct FakePasteboard {
        items: Vec<Vec<(String, Option<String>, Option<Vec<u8>>)>>,
        text_types: Vec<String>,
        existing: Vec<String>,
        /// What the native side makes of a TIFF or JPEG for `public.png`.
        png_from_tiff: Option<Vec<u8>>,
    }

    impl FakePasteboard {
        fn item(mut self, entries: &[(&str, Option<&str>, Option<Vec<u8>>)]) -> Self {
            self.items.push(entries.iter().map(|(t, s, b)| (t.to_string(), s.map(str::to_owned), b.clone())).collect());
            self
        }
    }

    impl Pasteboard for FakePasteboard {
        fn types(&self) -> Vec<String> {
            // `[NSPasteboard types]`: the first item's, as AppKit answers.
            self.items.first().map(|i| i.iter().map(|e| e.0.clone()).collect()).unwrap_or_default()
        }
        fn item_types(&self) -> Vec<Vec<String>> {
            self.items.iter().map(|i| i.iter().map(|e| e.0.clone()).collect()).collect()
        }
        fn string(&self, item: usize, ty: &str) -> Option<String> {
            self.items[item].iter().find(|e| e.0 == ty).and_then(|e| e.1.clone())
        }
        fn bytes(&self, item: usize, ty: &str) -> Option<Vec<u8>> {
            let data = self.items[item].iter().find(|e| e.0 == ty).and_then(|e| e.2.clone()).filter(|b| !b.is_empty());
            data.or_else(|| (ty == PASTEBOARD_PNG).then(|| self.png_from_tiff.clone()).flatten())
        }
        fn is_text_format(&self, ty: &str) -> bool {
            self.text_types.iter().any(|t| t == ty)
        }
        fn file_path(&self, url: &str) -> Option<String> {
            file_uri_local_path(url).filter(|p| self.existing.contains(p))
        }
    }

    #[test]
    fn pasteboard_types_get_a_png_for_a_tiff_or_jpeg() {
        assert_eq!(convert_pasteboard_types(&s(&["public.tiff", "NeXT TIFF v4.0 pasteboard type"])), s(&["public.tiff", "NeXT TIFF v4.0 pasteboard type", "public.png"]));
        assert_eq!(convert_pasteboard_types(&s(&["public.jpeg"])), s(&["public.jpeg", "public.png"]));
        assert_eq!(convert_pasteboard_types(&s(&["public.png", "public.tiff"])), s(&["public.png", "public.tiff"]));
        assert_eq!(convert_pasteboard_types(&s(&["public.html"])), s(&["public.html"]));
    }

    #[test]
    fn macos_lists_every_type_in_order_with_avalonias_names() {
        let pb = FakePasteboard { text_types: s(&["public.html", "public.utf16-external-plain-text"]), ..Default::default() }.item(&[
            ("public.html", Some("<b>hi</b>"), None),
            ("public.utf8-plain-text", Some("hi"), None),
            ("public.utf16-external-plain-text", None, Some(utf16_bytes("hi"))),
            ("com.apple.webarchive", None, Some(vec![1, 2, 3])),
            ("net.avaloniaui.app.uti.my-format", None, Some(vec![9])),
        ]);
        let snap = capture_macos(&pb);
        assert_eq!(snap.formats(), ["public.html", "UnicodeText", "public.utf16-external-plain-text", "com.apple.webarchive", "my-format"]);
        assert_eq!(snap.get("public.html"), Some(&ClipboardValue::Text("<b>hi</b>".into())));
        assert_eq!(snap.get("UnicodeText"), Some(&ClipboardValue::Text("hi".into())));
        // A text type without a string: its bytes as UTF-16.
        assert_eq!(snap.get("public.utf16-external-plain-text"), Some(&ClipboardValue::Text("hi".into())));
        assert_eq!(snap.get("com.apple.webarchive"), Some(&ClipboardValue::Bytes(vec![1, 2, 3])));
        assert_eq!(snap.get("my-format"), Some(&ClipboardValue::Bytes(vec![9])));
    }

    #[test]
    fn macos_reads_a_tiff_as_the_bitmap_and_skips_what_has_no_value() {
        // A screenshot from another app: TIFF only; the native side makes
        // `public.png`'s PNG from it.
        let pb = FakePasteboard { png_from_tiff: Some(png(3, 2)), ..Default::default() }.item(&[("public.tiff", None, Some(vec![0x49, 0x49, 0x2A, 0]))]);
        let snap = capture_macos(&pb);
        assert_eq!(snap.formats(), ["public.tiff", "Bitmap"]);
        let Some(ClipboardValue::Bytes(b)) = snap.get("Bitmap") else { panic!() };
        assert!(matches!(snap.preview("Bitmap"), Ok(cv::Preview::Image(ref i)) if i.width() == 3 && i.height() == 2), "{} bytes", b.len());
        // An undecodable PNG isn't listed (Avalonia's `Bitmap` throws).
        let bad = FakePasteboard::default().item(&[("public.png", None, Some(vec![1, 2, 3]))]);
        assert!(capture_macos(&bad).formats().is_empty());
        // Empty data is no data; a type of no item throws and is skipped.
        let empty = FakePasteboard::default().item(&[("com.example.empty", None, Some(Vec::new()))]);
        assert!(capture_macos(&empty).formats().is_empty());
    }

    #[test]
    fn macos_files_come_from_every_item() {
        let pb = FakePasteboard { existing: s(&["/Users/me/a b.png", "/Users/me/dir/"]), ..Default::default() }
            .item(&[("public.file-url", Some("file:///Users/me/a%20b.png"), None)])
            .item(&[("public.file-url", Some("file:///Users/me/dir/"), None)])
            .item(&[("public.file-url", Some("file:///Users/me/gone.png"), None)]);
        let snap = capture_macos(&pb);
        assert_eq!(snap.formats(), ["FileDrop"]);
        assert_eq!(snap.get("FileDrop"), Some(&ClipboardValue::Files(s(&["/Users/me/a b.png", "/Users/me/dir/"]))));
    }

    #[test]
    fn macos_sharex_image_copy_reads_back_as_bitmap_and_the_marker() {
        let [png_type, marker] = macos_image_item_types(4242);
        assert_eq!(marker, "net.avaloniaui.inproc.uti.n4242");
        let pb = FakePasteboard::default().item(&[(&png_type, None, Some(png(4, 4))), (&marker, Some(""), None)]);
        let snap = capture_macos(&pb);
        // The marker: no data, its empty string as UTF-16 bytes.
        assert_eq!(snap.formats(), ["Bitmap", "net.avaloniaui.inproc.uti.n4242"]);
        assert_eq!(snap.get("net.avaloniaui.inproc.uti.n4242"), Some(&ClipboardValue::Bytes(Vec::new())));
        assert!(capture_macos(&FakePasteboard::default()).formats().is_empty());
    }

    #[derive(Default)]
    struct FakeSelection {
        targets: Vec<String>,
        values: HashMap<String, (String, Vec<u8>)>,
        existing: Vec<String>,
    }

    impl FakeSelection {
        fn with(mut self, target: &str, ty: &str, bytes: &[u8]) -> Self {
            self.targets.push(target.into());
            self.values.insert(target.into(), (ty.into(), bytes.to_vec()));
            self
        }
    }

    impl Selection for FakeSelection {
        fn targets(&self) -> Vec<String> {
            self.targets.clone()
        }
        fn convert(&self, target: &str) -> Option<(String, Vec<u8>)> {
            self.values.get(target).cloned()
        }
        fn exists(&self, path: &str) -> bool {
            self.existing.iter().any(|p| p == path)
        }
    }

    #[test]
    fn x11_text_targets_are_one_text_where_the_first_is() {
        let (f, text) = x11_formats(&s(&["TIMESTAMP", "TARGETS", "MULTIPLE", "SAVE_TARGETS", "STRING", "text/html", "UTF8_STRING", "image/png", "text/plain"]));
        assert_eq!(f.iter().map(Format::name).collect::<Vec<_>>(), ["TIMESTAMP", "UnicodeText", "text/html", "image/png", "Bitmap"]);
        assert_eq!(text, s(&["STRING", "UTF8_STRING", "text/plain"]));
        let (f, _) = x11_formats(&s(&["application/avn-fmt.my-format", "text/uri-list", "image/jpeg"]));
        assert_eq!(f.iter().map(Format::name).collect::<Vec<_>>(), ["my-format", "FileDrop", "image/jpeg", "Bitmap"]);
    }

    #[test]
    fn x11_reads_the_preferred_text_target() {
        let sel = FakeSelection::default()
            .with("TARGETS", "ATOM", &[])
            .with("STRING", "STRING", b"caf\xE9")
            .with("UTF8_STRING", "UTF8_STRING", "café".as_bytes())
            .with("text/html", "text/html", b"<p>caf&eacute;</p>")
            .with("TIMESTAMP", "INTEGER", &[1, 0, 0, 0]);
        let snap = capture_x11(&sel);
        assert_eq!(snap.formats(), ["UnicodeText", "text/html", "TIMESTAMP"]);
        // UTF8_STRING comes before STRING by preference.
        assert_eq!(snap.get("UnicodeText"), Some(&ClipboardValue::Text("café".into())));
        assert_eq!(snap.get("TIMESTAMP"), Some(&ClipboardValue::Bytes(vec![1, 0, 0, 0])));
        // STRING alone: ASCII.
        let sel = FakeSelection::default().with("STRING", "STRING", b"caf\xE9");
        assert_eq!(capture_x11(&sel).get("UnicodeText"), Some(&ClipboardValue::Text("caf?".into())));
        // A reply of another type: no text.
        let sel = FakeSelection::default().with("UTF8_STRING", "INCR", b"x");
        assert!(capture_x11(&sel).formats().is_empty());
    }

    #[test]
    fn x11_images_and_files() {
        let sel = FakeSelection { existing: s(&["/home/me/a b.png"]), ..Default::default() }
            .with("TARGETS", "ATOM", &[])
            .with("image/png", "image/png", &png(2, 2))
            .with("text/uri-list", "text/uri-list", b"# comment\r\nfile:///home/me/a%20b.png\r\nfile:///gone\r\nhttp://x/y\r\n");
        let snap = capture_x11(&sel);
        assert_eq!(snap.formats(), ["image/png", "FileDrop", "Bitmap"]);
        assert_eq!(snap.get("FileDrop"), Some(&ClipboardValue::Files(s(&["/home/me/a b.png"]))));
        assert!(matches!(snap.preview("Bitmap"), Ok(cv::Preview::Image(_))));
        // No existing file: not listed. An undecodable image: no Bitmap.
        let sel = FakeSelection::default().with("text/uri-list", "text/uri-list", b"file:///gone\n").with("image/png", "image/png", b"nope");
        assert_eq!(capture_x11(&sel).formats(), ["image/png"]);
        // ShareX's own image copy: image/png twice, one Bitmap.
        let mut sel = FakeSelection { targets: s(&X11_IMAGE_TARGETS), ..Default::default() };
        sel.values.insert("image/png".into(), ("image/png".into(), png(1, 1)));
        assert_eq!(capture_x11(&sel).formats(), ["image/png", "Bitmap"]);
        assert!(capture_x11(&FakeSelection::default()).formats().is_empty());
    }

    #[test]
    fn file_uris_become_local_paths() {
        assert_eq!(file_uri_local_path("file:///a/b%20c").as_deref(), Some("/a/b c"));
        assert_eq!(file_uri_local_path("FILE://localhost/a").as_deref(), Some("/a"));
        assert_eq!(file_uri_local_path("file://host/share/x").as_deref(), Some("//host/share/x"));
        assert_eq!(file_uri_local_path("http://x/y"), None);
        assert_eq!(file_uri_local_path("# comment"), None);
    }

    #[test]
    fn application_prefixes_come_off_only_for_valid_identifiers() {
        assert_eq!(Format::from_system_name("net.avaloniaui.app.uti.a-b.c", MAC_APP_PREFIX, false).name(), "a-b.c");
        assert_eq!(Format::from_system_name("net.avaloniaui.app.uti.a b", MAC_APP_PREFIX, false).name(), "net.avaloniaui.app.uti.a b");
        assert_eq!(Format::from_system_name("CF_DIB", X11_APP_PREFIX, false).name(), "DeviceIndependentBitmap");
    }
}
