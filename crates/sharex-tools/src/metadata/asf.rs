//! `AsfMetadataReader`: WMV/WMA headers.

use super::{Collector, V, ascii, fmt, format_duration, n0, utf16};

/// A .NET `Guid`'s 16 bytes from its string form.
pub(super) const fn guid(a: u32, b: u16, c: u16, d: [u8; 8]) -> [u8; 16] {
    let a = a.to_le_bytes();
    let b = b.to_le_bytes();
    let c = c.to_le_bytes();
    [a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]
}

pub(super) const HEADER: [u8; 16] = guid(0x75B22630, 0x668E, 0x11CF, [0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C]);
pub(super) const FILE_PROPERTIES: [u8; 16] = guid(0x8CABDCA1, 0xA947, 0x11CF, [0x8E, 0xE4, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65]);
pub(super) const STREAM_PROPERTIES: [u8; 16] = guid(0xB7DC0791, 0xA9B7, 0x11CF, [0x8E, 0xE6, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65]);
pub(super) const CONTENT_DESCRIPTION: [u8; 16] = guid(0x75B22633, 0x668E, 0x11CF, [0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C]);
pub(super) const EXTENDED_CONTENT_DESCRIPTION: [u8; 16] =
    guid(0xD2D0A440, 0xE307, 0x11D2, [0x97, 0xF0, 0x00, 0xA0, 0xC9, 0x5E, 0xA8, 0x50]);
pub(super) const HEADER_EXTENSION: [u8; 16] = guid(0x5FBF03B5, 0xA92E, 0x11CF, [0x8E, 0xE3, 0x00, 0xC0, 0x0C, 0x20, 0x53, 0x65]);
pub(super) const PADDING: [u8; 16] = guid(0x1806D474, 0xCADF, 0x4509, [0xA4, 0xBA, 0x9A, 0xAB, 0xCB, 0x96, 0xAA, 0xE8]);
pub(super) const METADATA: [u8; 16] = guid(0xC5F8CBEA, 0x5BAF, 0x4877, [0x84, 0x67, 0xAA, 0x8C, 0x44, 0xFA, 0x4C, 0xCA]);
pub(super) const METADATA_LIBRARY: [u8; 16] = guid(0x44231C94, 0x9498, 0x49D1, [0xA1, 0x41, 0x1D, 0x13, 0x4E, 0x45, 0x70, 0x54]);
const AUDIO_MEDIA: [u8; 16] = guid(0xF8699E40, 0x5B4D, 0x11CF, [0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44, 0x2B]);
const VIDEO_MEDIA: [u8; 16] = guid(0xBC19EFC0, 0x5B4D, 0x11CF, [0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44, 0x2B]);

const MAXIMUM_OBJECT: u64 = 64 * 1024 * 1024;

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub(super) fn le64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// `Guid.ToString()`.
fn guid_string(b: &[u8]) -> String {
    let a = u32::from_le_bytes(b[0..4].try_into().unwrap());
    let c = u16::from_le_bytes([b[4], b[5]]);
    let d = u16::from_le_bytes([b[6], b[7]]);
    format!(
        "{a:08x}-{c:04x}-{d:04x}-{:02x}{:02x}-{}",
        b[8],
        b[9],
        b[10..16].iter().map(|x| format!("{x:02x}")).collect::<String>()
    )
}

fn unicode(v: &[u8]) -> String {
    utf16(v, false).trim_end_matches('\0').to_string()
}

pub(super) fn read(path: &str, c: &mut Collector) -> std::io::Result<()> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let head = super::read_at(&mut f, 0, 30)?;
    if head.len() < 30 || head[..16] != HEADER {
        return Ok(());
    }
    let header_size = le64(&head, 16);
    let count = le32(&head, 24);
    // Only the header objects are read.
    let d = super::read_at(&mut f, 0, header_size.min(len).min(i64::MAX as u64) as usize)?;
    c.add("ASF", "Header Size", format!("{} bytes", n0(header_size)));
    c.add("ASF", "Header Object Count", count);
    let header_end = header_size.min(i64::MAX as u64).min(d.len() as u64);
    let mut track = 0;
    let mut pos = 30u64;
    let mut i = 0;
    while i < count && pos + 24 <= header_end {
        let p = pos as usize;
        let id: [u8; 16] = d[p..p + 16].try_into().unwrap();
        let size = le64(&d, p + 16);
        if size < 24 || size > i64::MAX as u64 || pos + size > header_end {
            break;
        }
        let payload_size = (size - 24).min(MAXIMUM_OBJECT) as usize;
        let data = &d[p + 24..p + 24 + payload_size];
        if id == FILE_PROPERTIES {
            file_properties(data, c);
        } else if id == STREAM_PROPERTIES {
            track += 1;
            stream_properties(data, c, track);
        } else if id == CONTENT_DESCRIPTION {
            content_description(data, c);
        } else if id == EXTENDED_CONTENT_DESCRIPTION {
            extended(data, c);
        } else if id == HEADER_EXTENSION {
            header_extension(data, c);
        }
        pos += size;
        i += 1;
    }
    Ok(())
}

fn file_properties(d: &[u8], c: &mut Collector) {
    if d.len() < 80 {
        return;
    }
    let g = "ASF";
    let file_size = le64(d, 16);
    let creation = le64(d, 24);
    let packets = le64(d, 32);
    let play = le64(d, 40);
    let preroll = le64(d, 56);
    c.add(g, "Declared File Size", format!("{} bytes", n0(file_size)));
    c.add(g, "Data Packet Count", packets);
    if creation > 0 && creation <= i64::MAX as u64 {
        // DateTime.FromFileTimeUtc: 100 ns ticks since 1601.
        let base = chrono::NaiveDate::from_ymd_opt(1601, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
        let t = base
            .checked_add_signed(chrono::TimeDelta::seconds((creation / 10_000_000) as i64))
            .and_then(|t| t.checked_add_signed(chrono::TimeDelta::nanoseconds(((creation % 10_000_000) * 100) as i64)));
        if let Some(t) = t.filter(|t| chrono::Datelike::year(t) <= 9999) {
            c.add(g, "Creation Time", V::Utc(t));
        }
    }
    let duration = play as f64 / 10_000_000.0 - preroll as f64 / 1000.0;
    if duration > 0.0 {
        c.add(g, "Duration", format_duration(duration));
    }
    c.add(g, "Preroll", format!("{preroll} ms"));
    let flags = le32(d, 64);
    c.add(g, "Broadcast", if flags & 1 != 0 { "Yes" } else { "No" });
    c.add(g, "Seekable", if flags & 2 != 0 { "Yes" } else { "No" });
    c.add(g, "Minimum Packet Size", le32(d, 68));
    c.add(g, "Maximum Packet Size", le32(d, 72));
    c.add(g, "Maximum Bitrate", format!("{} kbps", fmt(le32(d, 76) as f64 / 1000.0, "0.###")));
}

fn audio_codec(v: u16) -> String {
    match v {
        1 => "PCM",
        2 => "Microsoft ADPCM",
        3 => "IEEE float",
        0x50 => "MPEG",
        0x55 => "MP3",
        0x00FF => "AAC",
        0x0161 => "Windows Media Audio",
        0x0162 => "Windows Media Audio Professional",
        0x0163 => "Windows Media Audio Lossless",
        0x2000 => "AC-3",
        _ => return format!("0x{v:04X}"),
    }
    .into()
}

fn stream_properties(d: &[u8], c: &mut Collector, index: i32) {
    if d.len() < 54 {
        return;
    }
    let ty = &d[..16];
    let (video, audio) = (ty == VIDEO_MEDIA, ty == AUDIO_MEDIA);
    let g = if video {
        format!("Video Track {index}")
    } else if audio {
        format!("Audio Track {index}")
    } else {
        format!("Track {index}")
    };
    let g = g.as_str();
    c.add(g, "Stream Type", if video { "Video".to_string() } else if audio { "Audio".to_string() } else { guid_string(ty) });
    let len = le32(d, 40) as u64;
    let flags = le16(d, 48);
    c.add(g, "Stream Number", (flags & 0x7F) as i32);
    c.add(g, "Encrypted", if flags & 0x8000 != 0 { "Yes" } else { "No" });
    if len > (d.len() - 54) as u64 {
        return;
    }
    let f = &d[54..54 + len as usize];
    if audio && f.len() >= 16 {
        c.add(g, "Codec", audio_codec(le16(f, 0)));
        c.add(g, "Channels", le16(f, 2));
        c.add(g, "Sample Rate", format!("{} Hz", le32(f, 4)));
        c.add(g, "Average Bitrate", format!("{} kbps", fmt(le32(f, 8) as f64 * 8.0 / 1000.0, "0.###")));
        c.add(g, "Bits Per Sample", le16(f, 14));
    } else if video && f.len() >= 51 {
        let w = le32(f, 0) as i32;
        let h = le32(f, 4) as i32;
        c.add_size(g, w as i64, h.unsigned_abs() as i64);
        c.add(g, "Bits Per Pixel", le16(f, 25));
        c.add(g, "Codec", ascii(&f[27..31]).trim_end_matches(['\0', ' ']).to_string());
    }
}

fn content_description(d: &[u8], c: &mut Collector) {
    if d.len() < 10 {
        return;
    }
    let names = ["Title", "Author", "Copyright", "Description", "Rating"];
    let mut pos = 10;
    for (i, name) in names.iter().enumerate() {
        let len = le16(d, i * 2) as usize;
        if pos + len > d.len() {
            break;
        }
        c.add("ASF Metadata", name, unicode(&d[pos..pos + len]));
        pos += len;
    }
}

fn extended(d: &[u8], c: &mut Collector) {
    if d.len() < 2 {
        return;
    }
    let count = le16(d, 0).min(4096);
    let mut pos = 2usize;
    let mut i = 0;
    while i < count && pos + 2 <= d.len() {
        let name_len = le16(d, pos) as usize;
        pos += 2;
        if pos + name_len + 4 > d.len() {
            break;
        }
        let name = unicode(&d[pos..pos + name_len]);
        pos += name_len;
        let ty = le16(d, pos);
        let vlen = le16(d, pos + 2) as usize;
        pos += 4;
        if pos + vlen > d.len() {
            break;
        }
        let v = &d[pos..pos + vlen];
        let text = match ty {
            0 => unicode(v),
            2 if v.len() >= 4 => (if le32(v, 0) != 0 { "Yes" } else { "No" }).to_string(),
            3 if v.len() >= 4 => le32(v, 0).to_string(),
            4 if v.len() >= 8 => le64(v, 0).to_string(),
            5 if v.len() >= 2 => le16(v, 0).to_string(),
            6 if v.len() >= 16 => guid_string(&v[..16]),
            _ => format!("{} bytes", n0(v.len() as u64)),
        };
        c.add("ASF Metadata", &name, text);
        pos += vlen;
        i += 1;
    }
}

fn header_extension(d: &[u8], c: &mut Collector) {
    if d.len() < 22 {
        return;
    }
    let ext = le32(d, 18) as usize;
    if ext > d.len() - 22 {
        return;
    }
    let mut pos = 22usize;
    let end = pos + ext;
    while pos + 24 <= end {
        let id = &d[pos..pos + 16];
        let size = le64(d, pos + 16);
        if size < 24 || size > i32::MAX as u64 || pos as u64 + size > end as u64 {
            break;
        }
        let payload = (size - 24) as usize;
        if id == METADATA || id == METADATA_LIBRARY {
            c.add(
                "ASF Metadata",
                if id == METADATA { "Metadata Object" } else { "Metadata Library Object" },
                format!("{} bytes", n0(payload as u64)),
            );
        }
        pos += size as usize;
    }
}
