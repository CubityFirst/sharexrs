//! `ImageMetadataReader`: JPEG, PNG, GIF, BMP and ICO (TIFF and WebP go to
//! their readers), Photoshop resources and IPTC.

use std::collections::BTreeMap;

use super::tiff::{read_icc, read_xmp};
use super::{Collector, MetadataFormat, V, decode_text, fmt, format_duration, latin1};

const MAXIMUM_CHUNK: usize = 32 * 1024 * 1024;

pub(super) fn read(path: &str, format: MetadataFormat, c: &mut Collector) -> std::io::Result<()> {
    match format {
        MetadataFormat::Tiff => return super::tiff::read_file(path, c),
        MetadataFormat::WebP => return super::riff::read_webp(path, c),
        _ => {}
    }
    let data = std::fs::read(path)?;
    match format {
        MetadataFormat::Jpeg => read_jpeg(&data, c),
        MetadataFormat::Png => read_png(&data, c),
        MetadataFormat::Gif => read_gif(&data, c),
        MetadataFormat::Bmp => read_bmp(&data, c),
        MetadataFormat::Icon => read_icon(&data, c),
        _ => {}
    }
    Ok(())
}

fn be16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}
fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap())
}
fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// `TryReadJpegMarker`: skips to the next 0xFF, then fill bytes.
pub(super) fn next_marker(d: &[u8], pos: &mut usize) -> Option<u8> {
    while *pos < d.len() && d[*pos] != 0xFF {
        *pos += 1;
    }
    if *pos >= d.len() {
        return None;
    }
    while *pos < d.len() && d[*pos] == 0xFF {
        *pos += 1;
    }
    let m = *d.get(*pos)?;
    *pos += 1;
    (m != 0).then_some(m)
}

fn is_sof(m: u8) -> bool {
    matches!(m, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF)
}

fn jpeg_encoding(m: u8) -> String {
    match m {
        0xC0 => "Baseline DCT, Huffman coding",
        0xC1 => "Extended sequential DCT, Huffman coding",
        0xC2 => "Progressive DCT, Huffman coding",
        0xC3 => "Lossless, Huffman coding",
        0xC5 => "Differential sequential DCT, Huffman coding",
        0xC6 => "Differential progressive DCT, Huffman coding",
        0xC7 => "Differential lossless, Huffman coding",
        0xC9 => "Extended sequential DCT, arithmetic coding",
        0xCA => "Progressive DCT, arithmetic coding",
        0xCB => "Lossless, arithmetic coding",
        _ => return format!("SOF marker 0x{m:02X}"),
    }
    .into()
}

fn read_jpeg(d: &[u8], c: &mut Collector) {
    if !d.starts_with(&[0xFF, 0xD8]) {
        return;
    }
    let mut pos = 2;
    let mut icc: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    let mut icc_count = 0u8;
    while let Some(m) = next_marker(d, &mut pos) {
        if m == 0xD9 || m == 0xDA {
            break;
        }
        if m == 0x01 || (0xD0..=0xD7).contains(&m) {
            continue;
        }
        if pos + 2 > d.len() {
            break;
        }
        let len = be16(d, pos) as i64 - 2;
        pos += 2;
        if len < 0 || len as usize > MAXIMUM_CHUNK || len as usize > d.len() - pos {
            break;
        }
        let s = &d[pos..pos + len as usize];
        pos += len as usize;
        if is_sof(m) && s.len() >= 6 {
            c.add("JPEG", "Encoding Process", jpeg_encoding(m));
            c.add("JPEG", "Bits Per Sample", s[0]);
            c.add_size("JPEG", be16(s, 3) as i64, be16(s, 1) as i64);
            c.add("JPEG", "Color Components", s[5]);
        } else if m == 0xE0 && s.starts_with(b"JFIF\0") && s.len() >= 14 {
            c.add("JFIF", "JFIF Version", format!("{}.{:02}", s[5], s[6]));
            c.add(
                "JFIF",
                "Resolution Unit",
                match s[7] {
                    1 => "inches",
                    2 => "centimeters",
                    _ => "none",
                },
            );
            c.add("JFIF", "X Resolution", be16(s, 8));
            c.add("JFIF", "Y Resolution", be16(s, 10));
            c.add("JFIF", "Thumbnail Size", format!("{} x {}", s[12], s[13]));
        } else if m == 0xE1 && s.starts_with(b"Exif\0\0") {
            super::tiff::read(&s[6..], c, "EXIF");
        } else if m == 0xE1 && s.starts_with(b"http://ns.adobe.com/xap/1.0/\0") {
            read_xmp(&s[29..], c);
        } else if m == 0xE2 && s.starts_with(b"ICC_PROFILE\0") && s.len() >= 14 {
            let seq = s[12];
            icc_count = icc_count.max(s[13]);
            if seq > 0 {
                icc.entry(seq).or_insert_with(|| s[14..].to_vec());
            }
        } else if m == 0xED && s.starts_with(b"Photoshop 3.0\0") {
            read_photoshop(&s[14..], c);
        } else if m == 0xEE && s.starts_with(b"Adobe") && s.len() >= 12 {
            c.add("Adobe", "DCT Encode Version", be16(s, 5));
            c.add(
                "Adobe",
                "Color Transform",
                match s[11] {
                    0 => V::from("Unknown (RGB or CMYK)"),
                    1 => V::from("YCbCr"),
                    2 => V::from("YCCK"),
                    v => V::from(v),
                },
            );
        } else if m == 0xFE {
            c.add("JPEG", "Comment", decode_text(s));
        }
    }
    if icc_count > 0 && icc.len() == icc_count as usize {
        let profile: Vec<u8> = icc.values().flatten().copied().collect();
        read_icc(&profile, c);
    }
}

/// `TryDecompress` (zlib, at most 32 MiB).
fn inflate(b: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut z = flate2::read::ZlibDecoder::new(b);
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = z.read(&mut buf).ok()?;
        if n == 0 {
            return Some(out);
        }
        if out.len() + n > MAXIMUM_CHUNK {
            return None;
        }
        out.extend_from_slice(&buf[..n]);
    }
}

fn png_point(s: &[u8]) -> String {
    format!("{}, {}", fmt(be32(s, 0) as f64 / 100000.0, "0.#####"), fmt(be32(s, 4) as f64 / 100000.0, "0.#####"))
}

fn add_text(keyword: &str, value: &str, c: &mut Collector) {
    if keyword.eq_ignore_ascii_case("XML:com.adobe.xmp") {
        read_xmp(value.as_bytes(), c);
    } else {
        c.add("PNG Text", keyword, value);
    }
}

fn read_png(d: &[u8], c: &mut Collector) {
    let mut pos = 8usize;
    while pos + 12 <= d.len() {
        let len = be32(d, pos) as u64;
        let ty = super::ascii(&d[pos + 4..pos + 8]);
        pos += 8;
        if len > i32::MAX as u64 || len > (d.len() - pos) as u64 - 4 {
            break;
        }
        let len = len as usize;
        let read = len <= MAXIMUM_CHUNK
            && matches!(
                ty.as_str(),
                "IHDR" | "pHYs" | "gAMA" | "sRGB" | "cHRM" | "cICP" | "mDCV" | "cLLI" | "tIME" | "tEXt" | "zTXt" | "iTXt" | "eXIf" | "iCCP" | "acTL"
            );
        let s: &[u8] = if read { &d[pos..pos + len] } else { &[] };
        pos += len + 4;
        match ty.as_str() {
            "IHDR" if s.len() == 13 => {
                c.add_size("PNG", be32(s, 0) as i32 as i64, be32(s, 4) as i32 as i64);
                c.add("PNG", "Bit Depth", s[8]);
                c.add(
                    "PNG",
                    "Color Type",
                    match s[9] {
                        0 => "Grayscale".to_string(),
                        2 => "Truecolor".to_string(),
                        3 => "Indexed color".to_string(),
                        4 => "Grayscale with alpha".to_string(),
                        6 => "Truecolor with alpha".to_string(),
                        v => format!("Unknown ({v})"),
                    },
                );
                c.add("PNG", "Interlace", if s[12] == 1 { "Adam7" } else { "None" });
            }
            "pHYs" if s.len() == 9 => {
                let (x, y) = (be32(s, 0), be32(s, 4));
                c.add("PNG", "Pixels Per Unit X", x);
                c.add("PNG", "Pixels Per Unit Y", y);
                c.add("PNG", "Pixel Units", if s[8] == 1 { "meters" } else { "unknown" });
                if s[8] == 1 {
                    c.add("PNG", "Resolution", format!("{} x {} DPI", fmt(x as f64 * 0.0254, "0.##"), fmt(y as f64 * 0.0254, "0.##")));
                }
            }
            "gAMA" if s.len() == 4 => c.add("PNG", "Gamma", be32(s, 0) as f64 / 100000.0),
            "sRGB" if s.len() == 1 => c.add(
                "PNG",
                "Rendering Intent",
                match s[0] {
                    0 => "Perceptual".to_string(),
                    1 => "Relative colorimetric".to_string(),
                    2 => "Saturation".to_string(),
                    3 => "Absolute colorimetric".to_string(),
                    v => v.to_string(),
                },
            ),
            "cHRM" if s.len() == 32 => {
                c.add("PNG", "White Point", png_point(&s[0..]));
                c.add("PNG", "Red Primary", png_point(&s[8..]));
                c.add("PNG", "Green Primary", png_point(&s[16..]));
                c.add("PNG", "Blue Primary", png_point(&s[24..]));
            }
            "cICP" if s.len() == 4 => {
                c.add("PNG", "Color Primaries", s[0]);
                c.add("PNG", "Transfer Characteristics", s[1]);
                c.add("PNG", "Matrix Coefficients", s[2]);
                c.add("PNG", "Video Full Range Flag", s[3]);
            }
            "tIME" if s.len() == 7 => {
                let year = be16(s, 0);
                if (1..=9999).contains(&year) {
                    if let Some(t) = chrono::NaiveDate::from_ymd_opt(year as i32, s[2] as u32, s[3] as u32)
                        .and_then(|d| d.and_hms_opt(s[4] as u32, s[5] as u32, s[6] as u32))
                    {
                        c.add("PNG", "Modification Time", V::Utc(t));
                    }
                }
            }
            "tEXt" | "zTXt" => {
                let Some(sep) = s.iter().position(|&b| b == 0).filter(|&p| p > 0) else { continue };
                let keyword = latin1(&s[..sep]);
                let mut value: Vec<u8> = s[sep + 1..].to_vec();
                if ty == "zTXt" {
                    if value.len() < 2 || value[0] != 0 {
                        continue;
                    }
                    let Some(v) = inflate(&value[1..]) else { continue };
                    value = v;
                }
                add_text(&keyword, &decode_text(&value), c);
            }
            "iTXt" => read_itxt(s, c),
            "eXIf" => super::tiff::read(s, c, "EXIF"),
            "iCCP" => {
                let Some(sep) = s.iter().position(|&b| b == 0).filter(|&p| p > 0) else { continue };
                if sep + 2 > s.len() || s[sep + 1] != 0 {
                    continue;
                }
                c.add("ICC Profile", "Profile Name", latin1(&s[..sep]));
                if let Some(p) = inflate(&s[sep + 2..]) {
                    read_icc(&p, c);
                }
            }
            "acTL" if s.len() == 8 => {
                c.add("PNG", "Animation Frames", be32(s, 0));
                let plays = be32(s, 4);
                c.add("PNG", "Animation Plays", if plays == 0 { V::from("Infinite") } else { V::from(plays) });
            }
            _ => {}
        }
        if ty == "IEND" {
            break;
        }
    }
}

fn read_itxt(s: &[u8], c: &mut Collector) {
    let Some(kend) = s.iter().position(|&b| b == 0).filter(|&p| p > 0) else { return };
    if kend + 5 > s.len() {
        return;
    }
    let keyword = latin1(&s[..kend]);
    let mut p = kend + 1;
    let compressed = s[p] == 1;
    p += 1;
    if s[p] != 0 {
        return;
    }
    p += 1;
    let Some(lend) = s[p..].iter().position(|&b| b == 0) else { return };
    let language = super::ascii(&s[p..p + lend]);
    p += lend + 1;
    let Some(tend) = s[p..].iter().position(|&b| b == 0) else { return };
    p += tend + 1;
    let mut value = s[p..].to_vec();
    if compressed {
        let Some(v) = inflate(&value) else { return };
        value = v;
    }
    let tag = if language.trim().is_empty() { keyword } else { format!("{keyword} ({language})") };
    add_text(&tag, &String::from_utf8_lossy(&value), c);
}

fn sub_blocks(d: &[u8], pos: &mut usize) -> Vec<u8> {
    let mut out = Vec::new();
    while *pos < d.len() {
        let size = d[*pos] as usize;
        *pos += 1;
        if size == 0 {
            break;
        }
        if out.len() + size > MAXIMUM_CHUNK {
            *pos += size;
            skip_sub_blocks(d, pos);
            break;
        }
        let end = (*pos + size).min(d.len());
        out.extend_from_slice(&d[*pos..end]);
        *pos += size;
    }
    out
}

fn skip_sub_blocks(d: &[u8], pos: &mut usize) {
    while *pos < d.len() {
        let size = d[*pos] as usize;
        *pos += 1;
        if size == 0 {
            return;
        }
        *pos += size;
    }
}

fn read_gif(d: &[u8], c: &mut Collector) {
    if d.len() < 13 {
        return;
    }
    c.add("GIF", "Version", super::ascii(&d[3..6]));
    c.add_size("GIF", le16(d, 6) as i64, le16(d, 8) as i64);
    c.add("GIF", "Color Resolution", ((d[10] >> 4) & 7) + 1);
    c.add("GIF", "Background Color Index", d[11]);
    let mut pos = 13usize;
    if d[10] & 0x80 != 0 {
        let table = 3usize << ((d[10] & 7) + 1);
        pos += table;
        c.add("GIF", "Global Color Table", format!("{} colors", table / 3));
    }
    let mut frames: i32 = 0;
    let mut duration = 0.0f64;
    while pos < d.len() {
        let intro = d[pos];
        pos += 1;
        if intro == 0x3B {
            break;
        }
        if intro == 0x2C {
            if pos + 9 > d.len() {
                break;
            }
            let desc = &d[pos..pos + 9];
            pos += 9;
            frames += 1;
            if desc[8] & 0x80 != 0 {
                pos += 3usize << ((desc[8] & 7) + 1);
            }
            if pos >= d.len() {
                break;
            }
            pos += 1;
            skip_sub_blocks(d, &mut pos);
        } else if intro == 0x21 {
            let Some(&label) = d.get(pos) else { break };
            pos += 1;
            if label == 0xF9 {
                let size = d.get(pos).copied().unwrap_or(0) as usize;
                pos += 1;
                let block = &d[pos.min(d.len())..(pos + size).min(d.len())];
                pos += size + 1;
                if block.len() >= 3 {
                    duration += le16(block, 1) as f64 / 100.0;
                }
            } else if label == 0xFF {
                let size = d.get(pos).copied().unwrap_or(0) as usize;
                pos += 1;
                let app = super::ascii(&d[pos.min(d.len())..(pos + size).min(d.len())]);
                pos += size;
                let content = sub_blocks(d, &mut pos);
                if app.starts_with("NETSCAPE2.0") && content.len() >= 3 && content[0] == 1 {
                    let loops = le16(&content, 1);
                    c.add("GIF", "Animation Loop Count", if loops == 0 { V::from("Infinite") } else { V::from(loops) });
                } else if app.starts_with("XMP DataXMP") {
                    read_xmp(&content, c);
                } else {
                    c.add("GIF", "Application Extension", app);
                }
            } else if label == 0xFE {
                let content = sub_blocks(d, &mut pos);
                c.add("GIF", "Comment", decode_text(&content));
            } else {
                skip_sub_blocks(d, &mut pos);
            }
        } else {
            break;
        }
    }
    c.add("GIF", "Frame Count", frames);
    if duration > 0.0 {
        c.add("GIF", "Duration", format_duration(duration));
    }
}

fn read_bmp(d: &[u8], c: &mut Collector) {
    if d.len() < 54 {
        return;
    }
    let dib = le32(d, 14);
    c.add(
        "BMP",
        "DIB Header",
        match dib {
            12 => "BITMAPCOREHEADER".to_string(),
            40 => "BITMAPINFOHEADER".to_string(),
            108 => "BITMAPV4HEADER".to_string(),
            124 => "BITMAPV5HEADER".to_string(),
            v => format!("{v} bytes"),
        },
    );
    if dib >= 40 {
        let w = le32(d, 18) as i32;
        let h = (le32(d, 22) as i32).unsigned_abs() as i64;
        c.add_size("BMP", w as i64, h);
        c.add("BMP", "Bits Per Pixel", le16(d, 28));
        c.add(
            "BMP",
            "Compression",
            match le32(d, 30) {
                0 => "Uncompressed".to_string(),
                1 => "RLE 8-bit".to_string(),
                2 => "RLE 4-bit".to_string(),
                3 => "Bit fields".to_string(),
                4 => "JPEG".to_string(),
                5 => "PNG".to_string(),
                6 => "Alpha bit fields".to_string(),
                v => format!("Unknown ({v})"),
            },
        );
        c.add("BMP", "Image Data Offset", le32(d, 10));
    }
}

fn read_icon(d: &[u8], c: &mut Collector) {
    if d.len() < 6 {
        return;
    }
    let ty = le16(d, 2);
    let count = le16(d, 4);
    c.add("ICO", "Resource Type", if ty == 2 { "Cursor" } else { "Icon" });
    c.add("ICO", "Image Count", count);
    for i in 0..count.min(256) as usize {
        let o = 6 + i * 16;
        if o + 16 > d.len() {
            break;
        }
        let e = &d[o..o + 16];
        let w = if e[0] == 0 { 256 } else { e[0] as i32 };
        let h = if e[1] == 0 { 256 } else { e[1] as i32 };
        c.add("ICO", &format!("Image {}", i + 1), format!("{w} x {h}, {} colors, {} bpp, {} bytes", e[2], le16(e, 6), le32(e, 8)));
    }
}

fn read_photoshop(d: &[u8], c: &mut Collector) {
    let mut pos = 0usize;
    while pos + 12 <= d.len() && &d[pos..pos + 4] == b"8BIM" {
        let id = be16(d, pos + 4);
        let name_len = d[pos + 6] as usize;
        let mut field = 1 + name_len;
        if field & 1 != 0 {
            field += 1;
        }
        let size_pos = pos + 6 + field;
        if size_pos + 4 > d.len() {
            break;
        }
        let size = be32(d, size_pos) as u64;
        let vpos = size_pos + 4;
        if size > i32::MAX as u64 || vpos as u64 + size > d.len() as u64 {
            break;
        }
        let v = &d[vpos..vpos + size as usize];
        if id == 0x0404 {
            read_iptc(v, c);
        } else if id == 0x0424 {
            read_xmp(v, c);
        }
        pos = vpos + size as usize + (size as usize & 1);
    }
}

fn read_iptc(d: &[u8], c: &mut Collector) {
    let name = |ds: u8| -> Option<&'static str> {
        Some(match ds {
            5 => "Object Name",
            7 => "Edit Status",
            10 => "Urgency",
            15 => "Category",
            20 => "Supplemental Category",
            25 => "Keywords",
            40 => "Special Instructions",
            55 => "Date Created",
            60 => "Time Created",
            65 => "Originating Program",
            70 => "Program Version",
            80 => "By-line",
            85 => "By-line Title",
            90 => "City",
            92 => "Sub-location",
            95 => "Province/State",
            100 => "Country Code",
            101 => "Country",
            103 => "Original Transmission Reference",
            105 => "Headline",
            110 => "Credit",
            115 => "Source",
            116 => "Copyright Notice",
            118 => "Contact",
            120 => "Caption/Abstract",
            122 => "Writer/Editor",
            _ => return None,
        })
    };
    let mut pos = 0;
    while pos + 5 <= d.len() {
        if d[pos] != 0x1C {
            pos += 1;
            continue;
        }
        let (record, ds) = (d[pos + 1], d[pos + 2]);
        let len = be16(d, pos + 3) as usize;
        pos += 5;
        if pos + len > d.len() {
            break;
        }
        if record == 2 {
            if let Some(n) = name(ds) {
                c.add("IPTC", n, decode_text(&d[pos..pos + len]));
            }
        }
        pos += len;
    }
}
