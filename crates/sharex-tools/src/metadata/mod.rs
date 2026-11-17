//! Metadata: ShareX's built-in metadata reader and stripper (`MetadataService`,
//! `MetadataReader`, `MetadataCollector`, the image/TIFF/ICC/XMP readers, the
//! ISO BMFF, Matroska, RIFF and ASF readers, `MetadataStripper`) and the text
//! the window copies (`MetadataViewModel.BuildMetadataText`).

mod asf;
mod image;
mod iso;
mod matroska;
mod riff;
mod strip;
mod tiff;

use std::collections::HashSet;
use std::path::Path;

use chrono::{DateTime, Local, NaiveDateTime};

pub use strip::{is_iso_video_path, strip};

/// `MetadataValue`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataValue {
    pub group: String,
    pub tag: String,
    pub value: String,
}

/// `MetadataFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataFormat {
    Unknown,
    Jpeg,
    Png,
    Gif,
    Bmp,
    Tiff,
    WebP,
    Icon,
    IsoBaseMedia,
    Matroska,
    Avi,
    Asf,
}

impl MetadataFormat {
    /// `CanStripMetadata`.
    pub fn can_strip(self) -> bool {
        matches!(
            self,
            MetadataFormat::Jpeg
                | MetadataFormat::Png
                | MetadataFormat::Gif
                | MetadataFormat::WebP
                | MetadataFormat::IsoBaseMedia
                | MetadataFormat::Matroska
                | MetadataFormat::Avi
                | MetadataFormat::Asf
        )
    }
}

pub(crate) const ASF_HEADER_GUID: [u8; 16] =
    [0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C];

/// `MetadataFormatDetector.Detect`: from the first 32 bytes (and `.mov`).
pub fn detect(path: &str) -> std::io::Result<MetadataFormat> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = [0u8; 32];
    let mut n = 0;
    while n < 32 {
        let r = f.read(&mut buf[n..])?;
        if r == 0 {
            break;
        }
        n += r;
    }
    Ok(detect_bytes(&buf[..n], path))
}

fn detect_bytes(d: &[u8], path: &str) -> MetadataFormat {
    use MetadataFormat::*;
    if d.starts_with(&[0xFF, 0xD8]) {
        return Jpeg;
    }
    if d.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Png;
    }
    if d.starts_with(b"GIF87a") || d.starts_with(b"GIF89a") {
        return Gif;
    }
    if d.starts_with(b"BM") {
        return Bmp;
    }
    if d.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || d.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) {
        return Tiff;
    }
    if d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"WEBP" {
        return WebP;
    }
    if d.len() >= 12 && &d[..4] == b"RIFF" && &d[8..12] == b"AVI " {
        return Avi;
    }
    if d.starts_with(&[0, 0, 1, 0]) || d.starts_with(&[0, 0, 2, 0]) {
        return Icon;
    }
    if d.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Matroska;
    }
    if d.starts_with(&ASF_HEADER_GUID) {
        return Asf;
    }
    if d.len() >= 12 && &d[4..8] == b"ftyp" {
        return IsoBaseMedia;
    }
    if crate::dotnet::extension(path).eq_ignore_ascii_case(".mov") && d.len() >= 8 {
        let t = &d[4..8];
        if t == b"moov" || t == b"mdat" || t == b"wide" || t == b"free" {
            return IsoBaseMedia;
        }
    }
    Unknown
}

/// A value as `MetadataCollector.Add` formats it.
#[derive(Debug, Clone)]
pub(crate) enum V {
    S(String),
    I(i64),
    U(u64),
    F(f64),
    /// A UTC `DateTime` (`K` is "Z").
    Utc(NaiveDateTime),
    /// A local `DateTime` (`K` is the offset).
    Local(DateTime<Local>),
}

impl From<&str> for V {
    fn from(s: &str) -> Self {
        V::S(s.to_string())
    }
}
impl From<String> for V {
    fn from(s: String) -> Self {
        V::S(s)
    }
}
macro_rules! v_int {
    ($($t:ty => $v:ident),*) => { $(impl From<$t> for V { fn from(x: $t) -> Self { V::$v(x as _) } })* };
}
v_int!(u8 => U, u16 => U, u32 => U, u64 => U, i16 => I, i32 => I, i64 => I, usize => U);
impl From<f64> for V {
    fn from(x: f64) -> Self {
        V::F(x)
    }
}

/// `double.ToString(CultureInfo.InvariantCulture)`: the shortest round-trip
/// digits, scientific below 1e-4 and from 1e15 (`1E-05`, `1E+15`).
pub fn dotnet_double(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let s = format!("{v:e}"); // shortest digits: d.ddde±x
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let sign = if neg { "-" } else { "" };
    if !(-4..15).contains(&exp) {
        let rest = &digits[1..];
        let m = if rest.is_empty() { digits[..1].to_string() } else { format!("{}.{}", &digits[..1], rest) };
        return format!("{sign}{m}E{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    }
    if exp < 0 {
        format!("{sign}0.{}{}", "0".repeat((-exp - 1) as usize), digits)
    } else {
        let int_len = exp as usize + 1;
        if digits.len() <= int_len {
            format!("{sign}{}{}", digits, "0".repeat(int_len - digits.len()))
        } else {
            format!("{sign}{}.{}", &digits[..int_len], &digits[int_len..])
        }
    }
}

impl V {
    fn text(&self) -> String {
        match self {
            V::S(s) => s.clone(),
            V::I(i) => i.to_string(),
            V::U(u) => u.to_string(),
            V::F(f) => dotnet_double(*f),
            V::Utc(d) => format!("{} Z", d.format("%Y-%m-%d %H:%M:%S%.3f")),
            V::Local(d) => {
                let off = d.offset().local_minus_utc();
                let sign = if off < 0 { '-' } else { '+' };
                let off = off.abs();
                format!("{} {sign}{:02}:{:02}", d.format("%Y-%m-%d %H:%M:%S%.3f"), off / 3600, (off % 3600) / 60)
            }
        }
    }
}

/// `MetadataCollector`: at most 2048 distinct (group, tag, value) entries,
/// values with runs of white space and control characters collapsed.
#[derive(Default)]
pub(crate) struct Collector {
    items: Vec<MetadataValue>,
    keys: HashSet<(String, String, String)>,
}

impl Collector {
    pub fn add(&mut self, group: &str, tag: &str, value: impl Into<V>) {
        if self.items.len() >= 2048 {
            return;
        }
        let text = sanitize(&value.into().text());
        if text.trim().is_empty() {
            return;
        }
        if self.keys.insert((group.to_string(), tag.to_string(), text.clone())) {
            self.items.push(MetadataValue { group: group.into(), tag: tag.into(), value: text });
        }
    }

    /// `Add` with a null value (skipped).
    pub fn add_opt(&mut self, group: &str, tag: &str, value: Option<impl Into<V>>) {
        if let Some(v) = value {
            self.add(group, tag, v);
        }
    }

    pub fn add_size(&mut self, group: &str, width: i64, height: i64) {
        if width > 0 && height > 0 {
            self.add(group, "Image Width", format!("{width} px"));
            self.add(group, "Image Height", format!("{height} px"));
            self.add(group, "Image Size", format!("{width} x {height}"));
        }
    }
}

/// `MetadataCollector.Sanitize`.
fn sanitize(value: &str) -> String {
    let mut out = String::new();
    let mut units = 0;
    let mut previous_ws = false;
    for c in value.chars() {
        if c.is_whitespace() || c.is_control() {
            if !previous_ws {
                out.push(' ');
                units += 1;
                previous_ws = true;
            }
        } else {
            out.push(c);
            units += c.len_utf16();
            previous_ws = false;
        }
        if units >= 4096 {
            break;
        }
    }
    crate::dotnet::trim(&out).to_string()
}

/// `TimeSpan.FromSeconds(s).ToString(@"m\:ss\.fff")` (or `h\:mm\:ss\.fff`
/// from an hour): components of the tick count, hours without days.
pub(crate) fn format_duration(seconds: f64) -> String {
    let ticks = (seconds * 10_000_000.0) as i64;
    let total_ms = ticks / 10_000;
    let ms = (total_ms % 1000).abs();
    let s = ((ticks / 10_000_000) % 60).abs();
    let m = ((ticks / 600_000_000) % 60).abs();
    let h = ((ticks / 36_000_000_000) % 24).abs();
    if seconds >= 3600.0 { format!("{h}:{m:02}:{s:02}.{ms:03}") } else { format!("{m}:{s:02}.{ms:03}") }
}

/// `new UTF8Encoding(false, true).GetString`, falling back to Latin-1.
pub(crate) fn utf8_or_latin1(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&c| c as char).collect(),
    }
}

/// `DecodeText`: trailing NULs dropped, UTF-8 or Latin-1.
pub(crate) fn decode_text(b: &[u8]) -> String {
    let end = b.iter().rposition(|&c| c != 0).map(|i| i + 1).unwrap_or(0);
    utf8_or_latin1(&b[..end])
}

pub(crate) fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// `Encoding.ASCII.GetString`: bytes above 127 become '?'.
pub(crate) fn ascii(b: &[u8]) -> String {
    b.iter().map(|&c| if c < 128 { c as char } else { '?' }).collect()
}

/// `Encoding.Unicode` / `BigEndianUnicode.GetString` (lossy).
pub(crate) fn utf16(b: &[u8], big_endian: bool) -> String {
    let mut units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| if big_endian { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
        .collect();
    let mut s = String::from_utf16_lossy(&units);
    if b.len() % 2 == 1 {
        s.push('\u{FFFD}');
    }
    units.clear();
    s
}

/// `Convert.ToHexString`.
pub(crate) fn hex_upper(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

/// `{n:N0}`.
pub(crate) fn n0(v: u64) -> String {
    crate::text::format_number(v as f64, "N0", true)
}

/// `{v:fmt}` for a double in the current culture.
pub(crate) fn fmt(v: f64, spec: &str) -> String {
    crate::text::format_number(v, spec, false)
}

/// Up to `len` bytes of `f` from `offset` (fewer at the end of the file).
pub(crate) fn read_at(f: &mut std::fs::File, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::with_capacity(len.min(1 << 20));
    f.take(len as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// `FormatFileSize`.
fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{} {} ({} bytes)", fmt(size, "0.##"), UNITS[unit], n0(bytes)) }
}

fn format_name(format: MetadataFormat, ext: &str) -> &'static str {
    use MetadataFormat::*;
    match format {
        Jpeg => "JPEG",
        Png => "PNG",
        Gif => "GIF",
        Bmp => "BMP",
        Tiff => "TIFF",
        WebP => "WebP",
        Icon => "Windows icon",
        IsoBaseMedia => match ext.to_lowercase().as_str() {
            ".mov" => "QuickTime",
            ".3gp" | ".3g2" => "3GPP",
            ".heic" | ".heif" => "HEIF",
            ".avif" => "AVIF",
            _ => "ISO Base Media",
        },
        Matroska => "Matroska / WebM",
        Avi => "AVI",
        Asf => "ASF / Windows Media",
        Unknown => "Unknown",
    }
}

fn mime_type(format: MetadataFormat, ext: &str) -> &'static str {
    use MetadataFormat::*;
    match format {
        Jpeg => "image/jpeg",
        Png => "image/png",
        Gif => "image/gif",
        Bmp => "image/bmp",
        Tiff => "image/tiff",
        WebP => "image/webp",
        Icon => "image/x-icon",
        Matroska => "video/x-matroska",
        Avi => "video/x-msvideo",
        Asf => "video/x-ms-asf",
        IsoBaseMedia => match ext.to_lowercase().as_str() {
            ".mov" => "video/quicktime",
            ".3gp" => "video/3gpp",
            ".3g2" => "video/3gpp2",
            ".heic" | ".heif" => "image/heif",
            ".avif" => "image/avif",
            _ => "video/mp4",
        },
        Unknown => "application/octet-stream",
    }
}

/// `MetadataService.ReadMetadataAsync` / `MetadataReader.Read`.
pub fn read(path: &str) -> Result<Vec<MetadataValue>, String> {
    if !Path::new(path).is_file() {
        return Err(sharex_strings::tools::MetadataService_Selected_file_not_found().into());
    }
    let format = detect(path).map_err(|e| e.to_string())?;
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let name = crate::dotnet::file_name(path);
    let ext = crate::dotnet::extension(path);
    let mut c = Collector::default();
    c.add("File", "File Name", name);
    c.add("File", "File Size", format_file_size(meta.len()));
    c.add("File", "File Type", format_name(format, ext));
    c.add("File", "File Type Extension", ext.trim_start_matches('.').to_uppercase());
    c.add("File", "MIME Type", mime_type(format, ext));
    if let Ok(t) = meta.created() {
        c.add("File System", "Created", V::Local(DateTime::<Local>::from(t)));
    }
    if let Ok(t) = meta.modified() {
        c.add("File System", "Modified", V::Local(DateTime::<Local>::from(t)));
    }
    use MetadataFormat::*;
    let r = match format {
        Jpeg | Png | Gif | Bmp | Tiff | WebP | Icon => image::read(path, format, &mut c),
        IsoBaseMedia => iso::read(path, &mut c),
        Matroska => matroska::read(path, &mut c),
        Avi => riff::read_avi(path, &mut c),
        Asf => asf::read(path, &mut c),
        Unknown => Ok(()),
    };
    r.map_err(|e| e.to_string())?;
    Ok(c.items)
}

/// `MetadataService.CanStripMetadata`.
pub fn can_strip(path: &str) -> bool {
    if !Path::new(path).is_file() {
        return false;
    }
    match detect(path) {
        Ok(MetadataFormat::IsoBaseMedia) => is_iso_video_path(path),
        Ok(f) => f.can_strip(),
        Err(_) => false,
    }
}

/// `MetadataViewModel.BuildMetadataText`: the "Copy all" text.
pub fn build_text(entries: &[MetadataValue]) -> String {
    let nl = crate::dotnet::NEW_LINE;
    let mut out = String::new();
    for (key, items) in group(entries) {
        if !out.is_empty() {
            out.push_str(nl);
        }
        out.push_str(&format!("# {key}{nl}"));
        for e in items {
            out.push_str(&format!("    {}: {}{nl}", e.tag, e.value));
        }
    }
    out.trim_end().to_string()
}

/// `GroupBy(x => x.Group, StringComparer.OrdinalIgnoreCase)`: groups in order
/// of first appearance, keyed by the first spelling.
pub fn group(entries: &[MetadataValue]) -> Vec<(String, Vec<&MetadataValue>)> {
    let mut groups: Vec<(String, Vec<&MetadataValue>)> = Vec::new();
    for e in entries {
        let key = sharex_helpers::culture::ordinal_upper(&e.group);
        match groups.iter_mut().find(|(k, _)| sharex_helpers::culture::ordinal_upper(k) == key) {
            Some(g) => g.1.push(e),
            None => groups.push((e.group.clone(), vec![e])),
        }
    }
    groups
}

/// `MetadataViewModel.ApplyFilter`: entries whose group, tag or value contains
/// the search text, ignoring case.
pub fn filter<'a>(entries: &'a [MetadataValue], search: &str) -> Vec<&'a MetadataValue> {
    let f = search.trim();
    if f.is_empty() {
        return entries.iter().collect();
    }
    let ci = |s: &str| sharex_helpers::culture::invariant_ignore_case_contains(s, f);
    entries.iter().filter(|e| ci(&e.group) || ci(&e.tag) || ci(&e.value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_like_dotnet() {
        assert_eq!(dotnet_double(0.45455), "0.45455");
        assert_eq!(dotnet_double(1.0), "1");
        assert_eq!(dotnet_double(0.00001), "1E-05");
        assert_eq!(dotnet_double(0.0001), "0.0001");
        assert_eq!(dotnet_double(1e15), "1E+15");
        assert_eq!(dotnet_double(123456.5), "123456.5");
    }

    #[test]
    fn durations() {
        assert_eq!(format_duration(3.5), "0:03.500");
        assert_eq!(format_duration(3725.25), "1:02:05.250");
    }

    #[test]
    fn sanitizes() {
        assert_eq!(sanitize("  a \t\n b\u{0}c  "), "a b c");
    }
}
