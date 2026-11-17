//! `IsoBaseMediaMetadataReader`: MP4/MOV/3GP/HEIF boxes.

use super::{Collector, V, fmt, format_duration, latin1, n0, utf16, utf8_or_latin1};

const MAXIMUM_BOX: u64 = 128 * 1024 * 1024;

fn be16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}
fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap())
}
fn be64(b: &[u8], o: usize) -> u64 {
    u64::from_be_bytes(b[o..o + 8].try_into().unwrap())
}
fn fixed16(b: &[u8], o: usize) -> f64 {
    be32(b, o) as i32 as f64 / 65536.0
}

/// `FourCc`: Latin-1, trailing NULs and spaces trimmed.
pub(super) fn four_cc(b: &[u8]) -> String {
    latin1(&b[..4]).trim_end_matches(['\0', ' ']).to_string()
}

#[derive(Clone, Copy)]
struct Bx {
    offset: usize,
    size: usize,
    header: usize,
    code: u32,
}

impl Bx {
    fn payload<'a>(&self, d: &'a [u8]) -> &'a [u8] {
        &d[self.offset + self.header..self.offset + self.size]
    }
    fn ty(&self, d: &[u8]) -> String {
        four_cc(&d[self.offset + 4..])
    }
}

fn boxes(d: &[u8]) -> Vec<Bx> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= d.len() && out.len() < 100_000 {
        let size32 = be32(d, pos);
        let code = be32(d, pos + 4);
        let mut header = 8;
        let mut size = size32 as u64;
        if size32 == 1 {
            if pos + 16 > d.len() {
                break;
            }
            let ext = be64(d, pos + 8);
            if ext > i32::MAX as u64 {
                break;
            }
            size = ext;
            header = 16;
        } else if size32 == 0 {
            size = (d.len() - pos) as u64;
        }
        if size < header as u64 || pos as u64 + size > d.len() as u64 {
            break;
        }
        out.push(Bx { offset: pos, size: size as usize, header, code });
        pos += size as usize;
    }
    out
}

pub(super) fn read(path: &str, c: &mut Collector) -> std::io::Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut track = 0;
    let mut pos = 0u64;
    while pos + 8 <= len {
        let Some((ty, payload_off, payload_size, end)) = file_box(&mut f, pos, len)? else { break };
        if payload_size <= MAXIMUM_BOX && payload_size <= i32::MAX as u64 && matches!(ty.as_str(), "ftyp" | "moov" | "meta") {
            f.seek(SeekFrom::Start(payload_off))?;
            let mut payload = vec![0u8; payload_size as usize];
            f.read_exact(&mut payload)?;
            match ty.as_str() {
                "ftyp" => file_type(&payload, c),
                "moov" => movie(&payload, c, &mut track),
                _ => {
                    meta(&payload, c);
                    spatial_extents(&payload, c, 0);
                }
            }
        }
        pos = end;
    }
    Ok(())
}

/// `TryReadBoxHeader` on the file: (type, payload offset, payload size, end).
fn file_box(f: &mut std::fs::File, offset: u64, parent_end: u64) -> std::io::Result<Option<(String, u64, u64, u64)>> {
    use std::io::{Seek, SeekFrom};
    f.seek(SeekFrom::Start(offset))?;
    let mut h = [0u8; 16];
    if read_up_to(f, &mut h[..8])? != 8 {
        return Ok(None);
    }
    let size32 = be32(&h, 0);
    let ty = four_cc(&h[4..8]);
    let mut header = 8u64;
    let mut size = size32 as u64;
    if size32 == 1 {
        if read_up_to(f, &mut h[8..16])? != 8 {
            return Ok(None);
        }
        let ext = be64(&h, 8);
        if ext > i64::MAX as u64 {
            return Ok(None);
        }
        size = ext;
        header = 16;
    } else if size32 == 0 {
        size = parent_end - offset;
    }
    if size < header || offset + size > parent_end {
        return Ok(None);
    }
    Ok(Some((ty, offset + header, size - header, offset + size)))
}

fn read_up_to(f: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    use std::io::Read;
    f.read(buf)
}

fn file_type(d: &[u8], c: &mut Collector) {
    if d.len() < 8 {
        return;
    }
    c.add("ISO Base Media", "Major Brand", four_cc(d));
    c.add("ISO Base Media", "Minor Version", be32(d, 4));
    let mut brands: Vec<String> = Vec::new();
    let mut p = 8;
    while p + 4 <= d.len() {
        let b = four_cc(&d[p..]);
        if !b.trim().is_empty() && !brands.contains(&b) {
            brands.push(b);
        }
        p += 4;
    }
    c.add("ISO Base Media", "Compatible Brands", brands.join(", "));
}

fn movie(d: &[u8], c: &mut Collector, track: &mut i32) {
    for b in boxes(d) {
        let p = b.payload(d);
        match b.ty(d).as_str() {
            "mvhd" => movie_header(p, c),
            "trak" => {
                *track += 1;
                track_box(p, c, *track);
            }
            "udta" => user_data(p, c, "QuickTime Metadata"),
            "meta" => meta(p, c),
            _ => {}
        }
    }
}

fn quicktime_date(c: &mut Collector, group: &str, name: &str, seconds: u64) {
    if seconds == 0 || seconds > 10_000_000_000 {
        return;
    }
    let epoch = chrono::NaiveDate::from_ymd_opt(1904, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
    if let Some(t) = epoch.checked_add_signed(chrono::TimeDelta::seconds(seconds as i64)) {
        if chrono::Datelike::year(&t) <= 9999 {
            c.add(group, name, V::Utc(t));
        }
    }
}

fn movie_header(d: &[u8], c: &mut Collector) {
    if d.len() < 24 {
        return;
    }
    let v1 = d[0] == 1;
    let (mo, ts, du) = if v1 { (12, 20, 24) } else { (8, 12, 16) };
    if d.len() < du + if v1 { 8 } else { 4 } {
        return;
    }
    let read = |o: usize| if v1 { be64(d, o) } else { be32(d, o) as u64 };
    let creation = read(4);
    let modification = read(mo);
    let timescale = be32(d, ts);
    let duration = read(du);
    quicktime_date(c, "Movie", "Creation Time", creation);
    quicktime_date(c, "Movie", "Modification Time", modification);
    c.add("Movie", "Time Scale", timescale);
    if timescale > 0 && duration != u64::MAX {
        c.add("Movie", "Duration", format_duration(duration as f64 / timescale as f64));
    }
    let rate = if v1 { 32 } else { 20 };
    let volume = rate + 4;
    if d.len() >= volume + 2 {
        c.add("Movie", "Preferred Rate", fixed16(d, rate));
        c.add("Movie", "Preferred Volume", be16(d, volume) as i16 as f64 / 256.0);
    }
}

fn track_box(d: &[u8], c: &mut Collector, n: i32) {
    let group = format!("Track {n}");
    let mut media = None;
    for b in boxes(d) {
        match b.ty(d).as_str() {
            "tkhd" => track_header(b.payload(d), c, &group),
            "mdia" => media = Some(b),
            "udta" => user_data(b.payload(d), c, &group),
            _ => {}
        }
    }
    if let Some(m) = media {
        media_box(m.payload(d), c, &group);
    }
}

fn track_header(d: &[u8], c: &mut Collector, g: &str) {
    if d.len() < 84 {
        return;
    }
    let v1 = d[0] == 1;
    let (id, du, mx, wo) = if v1 { (20, 28, 52, 88) } else { (12, 20, 40, 76) };
    if d.len() < wo + 8 {
        return;
    }
    c.add(g, "Track ID", be32(d, id));
    let duration = if v1 { be64(d, du) } else { be32(d, du) as u64 };
    if duration != 0 && duration != u64::MAX {
        c.add(g, "Duration Units", duration);
    }
    let w = fixed16(d, wo).round_ties_even() as i32;
    let h = fixed16(d, wo + 4).round_ties_even() as i32;
    if w > 0 && h > 0 {
        c.add_size(g, w as i64, h as i64);
    }
    if d.len() >= mx + 20 {
        let a = fixed16(d, mx);
        let b = fixed16(d, mx + 4);
        let rotation = if (b - 1.0).abs() < 0.01 && a.abs() < 0.01 {
            90
        } else if (a + 1.0).abs() < 0.01 && b.abs() < 0.01 {
            180
        } else if (b + 1.0).abs() < 0.01 && a.abs() < 0.01 {
            270
        } else {
            0
        };
        if rotation != 0 {
            c.add(g, "Rotation", format!("{rotation}°"));
        }
    }
}

fn media_box(d: &[u8], c: &mut Collector, g: &str) {
    let mut handler = String::new();
    for b in boxes(d) {
        let p = b.payload(d);
        match b.ty(d).as_str() {
            "mdhd" => media_header(p, c, g),
            "hdlr" => handler = handler_box(p, c, g),
            "minf" => {
                for s in boxes(p) {
                    if s.ty(p) != "stbl" {
                        continue;
                    }
                    let st = s.payload(p);
                    for child in boxes(st) {
                        if child.ty(st) == "stsd" {
                            sample_descriptions(child.payload(st), c, g, &handler);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn media_header(d: &[u8], c: &mut Collector, g: &str) {
    if d.len() < 24 {
        return;
    }
    let v1 = d[0] == 1;
    let (ts, du, lo) = if v1 { (20, 24, 32) } else { (12, 16, 20) };
    if d.len() < lo + 2 {
        return;
    }
    let timescale = be32(d, ts);
    let duration = if v1 { be64(d, du) } else { be32(d, du) as u64 };
    c.add(g, "Media Time Scale", timescale);
    if timescale > 0 && duration != u64::MAX {
        c.add(g, "Media Duration", format_duration(duration as f64 / timescale as f64));
    }
    let lang = be16(d, lo);
    if lang != 0 && lang != 0x7FFF {
        let ch = |v: u16| char::from_u32(((v & 31) + 0x60) as u32).unwrap();
        c.add(g, "Language", [ch(lang >> 10), ch(lang >> 5), ch(lang)].iter().collect::<String>());
    }
}

fn handler_box(d: &[u8], c: &mut Collector, g: &str) -> String {
    if d.len() < 12 {
        return String::new();
    }
    let ty = four_cc(&d[8..]);
    c.add(
        g,
        "Handler Type",
        match ty.as_str() {
            "vide" => "Video",
            "soun" => "Audio",
            "text" | "sbtl" | "subt" => "Subtitle",
            "meta" => "Metadata",
            "hint" => "Hint",
            other => other,
        },
    );
    if d.len() > 24 {
        let mut name = &d[24..];
        let end = name.iter().rposition(|&b| b != 0).map(|i| i + 1).unwrap_or(0);
        name = &name[..end];
        if !name.is_empty() {
            if name[0] as usize == name.len() - 1 {
                name = &name[1..];
            }
            c.add(g, "Handler Name", utf8_or_latin1(name));
        }
    }
    ty
}

fn codec_name(codec: &str) -> String {
    match codec {
        "avc1" | "avc3" => format!("H.264 / AVC ({codec})"),
        "hvc1" | "hev1" => format!("H.265 / HEVC ({codec})"),
        "av01" => "AV1".into(),
        "vp09" => "VP9".into(),
        "mp4v" => "MPEG-4 Visual".into(),
        "jpeg" => "Motion JPEG".into(),
        "ap4h" => "Apple ProRes 4444".into(),
        "apch" => "Apple ProRes 422 HQ".into(),
        "apcn" => "Apple ProRes 422".into(),
        "apcs" => "Apple ProRes 422 LT".into(),
        "apco" => "Apple ProRes 422 Proxy".into(),
        "mp4a" => "MPEG-4 Audio / AAC".into(),
        "ac-3" => "Dolby Digital (AC-3)".into(),
        "ec-3" => "Dolby Digital Plus (E-AC-3)".into(),
        "alac" => "Apple Lossless".into(),
        "Opus" => "Opus".into(),
        "flac" => "FLAC".into(),
        other => other.into(),
    }
}

fn sample_descriptions(d: &[u8], c: &mut Collector, g: &str, handler: &str) {
    if d.len() < 8 {
        return;
    }
    let count = be32(d, 4).min(64);
    let mut pos = 8usize;
    let mut i = 0;
    while i < count && pos + 8 <= d.len() {
        let size = be32(d, pos) as usize;
        if size < 8 || pos + size > d.len() {
            break;
        }
        let e = &d[pos..pos + size];
        let codec = four_cc(&e[4..]);
        c.add(g, &if i == 0 { "Codec".to_string() } else { format!("Codec {}", i + 1) }, codec_name(&codec));
        if handler == "vide" && e.len() >= 36 {
            c.add_size(g, be16(e, 32) as i64, be16(e, 34) as i64);
            if e.len() >= 84 {
                c.add(g, "Bit Depth", be16(e, 82));
            }
        } else if handler == "soun" && e.len() >= 36 {
            c.add(g, "Channels", be16(e, 24));
            c.add(g, "Bits Per Sample", be16(e, 26));
            c.add(g, "Sample Rate", format!("{} Hz", fmt(fixed16(e, 32), "0.###")));
        }
        pos += size;
        i += 1;
    }
}

fn metadata_name(ty: &str) -> Option<&'static str> {
    Some(match ty {
        "©nam" => "Title",
        "©ART" => "Artist",
        "©alb" => "Album",
        "©day" => "Date",
        "©too" => "Encoder",
        "©cmt" => "Comment",
        "©gen" => "Genre",
        "©wrt" => "Composer",
        "©cpy" => "Copyright",
        "©grp" => "Grouping",
        "aART" => "Album Artist",
        "desc" => "Description",
        "ldes" => "Long Description",
        "©xyz" => "Location",
        "keyw" => "Keywords",
        "purd" => "Purchase Date",
        "tvsh" => "TV Show",
        "tven" => "TV Episode ID",
        "tvnn" => "TV Network",
        "catg" => "Category",
        "purl" => "Podcast URL",
        "egid" => "Episode Global ID",
        "covr" => "Cover Art",
        "trkn" => "Track Number",
        "disk" => "Disc Number",
        "tmpo" => "Tempo",
        _ => return None,
    })
}

fn user_data(d: &[u8], c: &mut Collector, group: &str) {
    for b in boxes(d) {
        let ty = b.ty(d);
        let mut p = b.payload(d);
        if ty == "meta" {
            meta(p, c);
            continue;
        }
        let Some(name) = metadata_name(&ty) else { continue };
        if p.is_empty() {
            continue;
        }
        if p.len() >= 4 {
            let len = be16(p, 0) as usize;
            if len > 0 && len <= p.len() - 4 {
                p = &p[4..4 + len];
            }
        }
        let end = p.iter().rposition(|&x| x != 0).map(|i| i + 1).unwrap_or(0);
        c.add(group, name, utf8_or_latin1(&p[..end]));
    }
}

fn meta(d: &[u8], c: &mut Collector) {
    if d.len() < 4 {
        return;
    }
    let children = &d[4..];
    let mut keys = Vec::new();
    let mut ilst = None;
    for b in boxes(children) {
        match b.ty(children).as_str() {
            "keys" => keys = metadata_keys(b.payload(children)),
            "ilst" => ilst = Some(b),
            _ => {}
        }
    }
    if let Some(b) = ilst {
        item_list(b.payload(children), &keys, c);
    }
}

fn metadata_keys(d: &[u8]) -> Vec<String> {
    let mut keys = Vec::new();
    if d.len() < 8 {
        return keys;
    }
    let count = be32(d, 4).min(4096);
    let mut pos = 8usize;
    let mut i = 0;
    while i < count && pos + 8 <= d.len() {
        let size = be32(d, pos) as usize;
        if size < 8 || pos + size > d.len() {
            break;
        }
        keys.push(utf8_or_latin1(&d[pos + 8..pos + size]));
        pos += size;
        i += 1;
    }
    keys
}

fn item_list(d: &[u8], keys: &[String], c: &mut Collector) {
    for item in boxes(d) {
        let ty = item.ty(d);
        let name = if item.code > 0 && item.code as usize <= keys.len() {
            keys[item.code as usize - 1].clone()
        } else {
            metadata_name(&ty).map(str::to_string).unwrap_or_else(|| ty.clone())
        };
        let data = item.payload(d);
        if ty == "----" {
            let mut custom = String::new();
            for child in boxes(data) {
                let p = child.payload(data);
                match child.ty(data).as_str() {
                    "name" if p.len() > 4 => custom = utf8_or_latin1(&p[4..]),
                    "data" => add_item(if custom.is_empty() { &name } else { &custom }, p, c),
                    _ => {}
                }
            }
        } else {
            for child in boxes(data) {
                if child.ty(data) == "data" {
                    add_item(&name, child.payload(data), c);
                }
            }
        }
    }
}

fn mostly_text(v: &[u8]) -> bool {
    if v.is_empty() {
        return false;
    }
    let text = v.iter().filter(|&&b| (0x20..0x7F).contains(&b) || b >= 0xC2).count();
    text >= v.len() * 3 / 4
}

fn add_item(name: &str, d: &[u8], c: &mut Collector) {
    if d.len() < 8 {
        return;
    }
    let ty = be32(d, 0) & 0x00FF_FFFF;
    let v = &d[8..];
    let signed = || -> i64 {
        match v.len() {
            1 => v[0] as i8 as i64,
            2 => be16(v, 0) as i16 as i64,
            4 => be32(v, 0) as i32 as i64,
            n if n >= 8 => be64(v, 0) as i64,
            _ => 0,
        }
    };
    let unsigned = || -> u64 {
        match v.len() {
            1 => v[0] as u64,
            2 => be16(v, 0) as u64,
            4 => be32(v, 0) as u64,
            n if n >= 8 => be64(v, 0),
            _ => 0,
        }
    };
    let text = match ty {
        1 => utf8_or_latin1(v),
        2 => utf16(v, true),
        13 => format!("JPEG image ({} bytes)", n0(v.len() as u64)),
        14 => format!("PNG image ({} bytes)", n0(v.len() as u64)),
        21 => signed().to_string(),
        22 => unsigned().to_string(),
        _ if mostly_text(v) => {
            let end = v.iter().rposition(|&x| x != 0).map(|i| i + 1).unwrap_or(0);
            utf8_or_latin1(&v[..end])
        }
        _ => format!("{} bytes", n0(v.len() as u64)),
    };
    c.add("Metadata", name, text);
}

fn spatial_extents(d: &[u8], c: &mut Collector, depth: i32) {
    if depth > 8 {
        return;
    }
    for b in boxes(d) {
        let p = b.payload(d);
        match b.ty(d).as_str() {
            "ispe" if p.len() >= 12 => c.add_size("Image Properties", be32(p, 4) as i32 as i64, be32(p, 8) as i32 as i64),
            "iprp" | "ipco" | "iref" | "iinf" | "dinf" => spatial_extents(p, c, depth + 1),
            _ => {}
        }
    }
}
