//! `RiffMetadataReader`: WebP and AVI.

use super::tiff::{read_icc, read_xmp};
use super::{Collector, V, ascii, decode_text, fmt, format_duration};

const MAXIMUM_CHUNK: u64 = 32 * 1024 * 1024;

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u24(b: &[u8], o: usize) -> i64 {
    b[o] as i64 | (b[o + 1] as i64) << 8 | (b[o + 2] as i64) << 16
}

pub(super) fn read_webp(path: &str, c: &mut Collector) -> std::io::Result<()> {
    let d = std::fs::read(path)?;
    let len = d.len() as u64;
    let mut pos = 12u64;
    let mut frames: i32 = 0;
    while pos + 8 <= len {
        let p = pos as usize;
        let ty = ascii(&d[p..p + 4]);
        let size = le32(&d, p + 4) as u64;
        pos += 8;
        let next = pos + size + (size & 1);
        if next > len {
            break;
        }
        let s: &[u8] = if size <= MAXIMUM_CHUNK { &d[pos as usize..(pos + size) as usize] } else { &[] };
        match ty.as_str() {
            "VP8X" if s.len() >= 10 => {
                c.add_size("WebP", 1 + u24(s, 4), 1 + u24(s, 7));
                let mut f = Vec::new();
                for (bit, name) in [(0x20, "ICC profile"), (0x10, "Alpha"), (0x08, "EXIF"), (0x04, "XMP"), (0x02, "Animation")] {
                    if s[0] & bit != 0 {
                        f.push(name);
                    }
                }
                c.add("WebP", "Features", f.join(", "));
            }
            "VP8 " if s.len() >= 10 && s[3..6] == [0x9D, 0x01, 0x2A] => {
                c.add_size("WebP", (le16(s, 6) & 0x3FFF) as i64, (le16(s, 8) & 0x3FFF) as i64);
                c.add("WebP", "Compression", "Lossy VP8");
            }
            "VP8L" if s.len() >= 5 && s[0] == 0x2F => {
                let w = 1 + s[1] as i64 + (((s[2] & 0x3F) as i64) << 8);
                let h = 1 + ((s[2] >> 6) as i64 | (s[3] as i64) << 2 | ((s[4] & 0x0F) as i64) << 10);
                c.add_size("WebP", w, h);
                c.add("WebP", "Compression", "Lossless VP8L");
            }
            "ANIM" if s.len() >= 6 => {
                let loops = le16(s, 4);
                c.add("WebP", "Animation Loop Count", if loops == 0 { V::from("Infinite") } else { V::from(loops) });
            }
            "ANMF" => frames += 1,
            "EXIF" => {
                let exif = s.strip_prefix(b"Exif\0\0").unwrap_or(s);
                super::tiff::read(exif, c, "EXIF");
            }
            "XMP " => read_xmp(s, c),
            "ICCP" => read_icc(s, c),
            _ => {}
        }
        pos = next;
    }
    if frames > 0 {
        c.add("WebP", "Animation Frames", frames);
    }
    Ok(())
}

struct StreamContext {
    index: i32,
    ty: String,
    group: String,
}

pub(super) fn read_avi(path: &str, c: &mut Collector) -> std::io::Result<()> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut index = 0;
    avi_chunks(&mut f, len, 12, len, c, &mut index, None)
}

fn avi_chunks(
    f: &mut std::fs::File,
    file_len: u64,
    start: u64,
    end: u64,
    c: &mut Collector,
    index: &mut i32,
    mut ctx: Option<&mut StreamContext>,
) -> std::io::Result<()> {
    let mut pos = start;
    while pos + 8 <= end {
        let h = super::read_at(f, pos, 12)?;
        if h.len() < 8 {
            break;
        }
        let ty = ascii(&h[0..4]);
        let size = le32(&h, 4) as u64;
        let content = pos + 8;
        let chunk_end = content + size;
        let next = chunk_end + (size & 1);
        if chunk_end > end || chunk_end > file_len {
            break;
        }
        if ty == "LIST" && size >= 4 {
            let list = ascii(&h[8..12]);
            match list.as_str() {
                "strl" => {
                    *index += 1;
                    let mut sc = StreamContext { index: *index, ty: String::new(), group: format!("Track {}", *index) };
                    avi_chunks(f, file_len, content + 4, chunk_end, c, index, Some(&mut sc))?;
                }
                "INFO" => {
                    let info = super::read_at(f, content + 4, (chunk_end - content - 4) as usize)?;
                    avi_info(&info, c);
                }
                "hdrl" | "odml" => avi_chunks(f, file_len, content + 4, chunk_end, c, index, ctx.as_deref_mut())?,
                _ => {}
            }
        } else if size <= MAXIMUM_CHUNK {
            let s = super::read_at(f, content, size as usize)?;
            let s = s.as_slice();
            match ty.as_str() {
                "avih" => avi_header(s, c),
                "strh" => {
                    if let Some(sc) = ctx.as_deref_mut() {
                        stream_header(s, sc, c);
                    }
                }
                "strf" => {
                    if let Some(sc) = ctx.as_deref_mut() {
                        stream_format(s, sc, c);
                    }
                }
                _ => {}
            }
        }
        pos = next;
    }
    Ok(())
}

fn avi_header(s: &[u8], c: &mut Collector) {
    if s.len() < 40 {
        return;
    }
    let us = le32(s, 0);
    let max_bytes = le32(s, 4);
    let frames = le32(s, 16);
    c.add_opt("AVI", "Frame Rate", (us > 0).then(|| format!("{} fps", fmt(1_000_000.0 / us as f64, "0.###"))));
    c.add("AVI", "Frame Count", frames);
    c.add_opt("AVI", "Duration", (us > 0).then(|| format_duration(frames.wrapping_mul(us) as f64 / 1_000_000.0)));
    c.add_opt("AVI", "Maximum Data Rate", (max_bytes > 0).then(|| format!("{} Mbps", fmt(max_bytes as f64 * 8.0 / 1_000_000.0, "0.###"))));
    c.add("AVI", "Stream Count", le32(s, 24));
    c.add_size("AVI", le32(s, 32) as i32 as i64, le32(s, 36) as i32 as i64);
}

fn stream_header(s: &[u8], sc: &mut StreamContext, c: &mut Collector) {
    if s.len() < 48 {
        return;
    }
    sc.ty = ascii(&s[0..4]);
    sc.group = match sc.ty.as_str() {
        "vids" => format!("Video Track {}", sc.index),
        "auds" => format!("Audio Track {}", sc.index),
        "txts" => format!("Text Track {}", sc.index),
        _ => format!("Track {}", sc.index),
    };
    let g = sc.group.clone();
    c.add(
        &g,
        "Stream Type",
        match sc.ty.as_str() {
            "vids" => "Video",
            "auds" => "Audio",
            "txts" => "Text",
            other => other,
        },
    );
    c.add(&g, "Codec", ascii(&s[4..8]));
    let (scale, rate, length) = (le32(s, 20), le32(s, 24), le32(s, 32));
    if scale > 0 && rate > 0 {
        c.add(&g, if sc.ty == "vids" { "Frame Rate" } else { "Rate" }, fmt(rate as f64 / scale as f64, "0.###"));
        c.add(&g, "Duration", format_duration(length as f64 * scale as f64 / rate as f64));
    }
    c.add(&g, "Length", length);
    let sample = le32(s, 44);
    if sample > 0 {
        c.add(&g, "Sample Size", format!("{sample} bytes"));
    }
}

fn stream_format(s: &[u8], sc: &StreamContext, c: &mut Collector) {
    let g = &sc.group;
    if sc.ty == "vids" && s.len() >= 20 {
        c.add_size(g, le32(s, 4) as i32 as i64, (le32(s, 8) as i32).unsigned_abs() as i64);
        c.add(g, "Bits Per Pixel", le16(s, 14));
        c.add(g, "Compression", ascii(&s[16..20]));
    } else if sc.ty == "auds" && s.len() >= 16 {
        c.add(g, "Audio Format", wave_format(le16(s, 0)));
        c.add(g, "Channels", le16(s, 2));
        c.add(g, "Sample Rate", format!("{} Hz", le32(s, 4)));
        c.add(g, "Average Bitrate", format!("{} kbps", fmt(le32(s, 8) as f64 * 8.0 / 1000.0, "0.###")));
        c.add(g, "Bits Per Sample", le16(s, 14));
    }
}

pub(super) fn wave_format(v: u16) -> String {
    match v {
        1 => "PCM",
        2 => "Microsoft ADPCM",
        3 => "IEEE float",
        6 => "A-law",
        7 => "mu-law",
        0x50 => "MPEG",
        0x55 => "MP3",
        0x00FF => "AAC",
        0x0161 => "Windows Media Audio",
        0x2000 => "AC-3",
        _ => return format!("0x{v:04X}"),
    }
    .into()
}

/// `ReadAviInfo` over the INFO list's content.
fn avi_info(d: &[u8], c: &mut Collector) {
    let end = d.len() as u64;
    let mut pos = 0u64;
    while pos + 8 <= end {
        let p = pos as usize;
        let ty = ascii(&d[p..p + 4]);
        let size = le32(d, p + 4) as u64;
        pos += 8;
        let next = pos + size + (size & 1);
        if next > end {
            break;
        }
        if size <= 1024 * 1024 {
            let name = match ty.as_str() {
                "INAM" => "Title",
                "IART" => "Artist",
                "ICMT" => "Comment",
                "ICOP" => "Copyright",
                "ICRD" => "Creation Date",
                "ISFT" => "Software",
                "IGNR" => "Genre",
                "IKEY" => "Keywords",
                "ISBJ" => "Subject",
                "ISRC" => "Source",
                "ITCH" => "Technician",
                "IDIT" => "Date/Time Original",
                other => other,
            }
            .to_string();
            c.add("AVI Info", &name, decode_text(&d[pos as usize..(pos + size) as usize]));
        }
        pos = next;
    }
}
