//! `MatroskaMetadataReader`: EBML header, Segment Info, Tracks and Tags.

use super::{Collector, V, fmt, format_duration, hex_upper, latin1, n0};

const MAXIMUM_ELEMENT: u64 = 128 * 1024 * 1024;

/// `StreamElement` / `MemoryElement`.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Element {
    pub id: u64,
    pub offset: u64,
    pub payload: u64,
    pub size: u64,
    pub unknown: bool,
}

impl Element {
    pub fn end(&self) -> u64 {
        self.payload + self.size
    }
    pub fn total(&self) -> u64 {
        self.end() - self.offset
    }
}

fn vint_width(first: u8) -> usize {
    let mut w = 1;
    let mut mask = 0x80u8;
    while w <= 8 && first & mask == 0 {
        w += 1;
        mask >>= 1;
    }
    if w <= 8 { w } else { 0 }
}

/// `TryReadVInt` over a buffer (the stream form reads the same bytes).
fn vint(d: &[u8], pos: &mut usize, keep_marker: bool) -> Option<(u64, bool)> {
    let first = *d.get(*pos)?;
    if first == 0 {
        return None;
    }
    *pos += 1;
    let w = vint_width(first);
    if w == 0 || w > if keep_marker { 4 } else { 8 } || *pos + w - 1 > d.len() {
        return None;
    }
    let mask = (0xFFu32 >> w) as u8;
    let mut value = if keep_marker { first as u64 } else { (first & mask) as u64 };
    let mut all_ones = !keep_marker && (first & mask) == mask;
    for _ in 1..w {
        let next = d[*pos];
        *pos += 1;
        value = (value << 8) | next as u64;
        all_ones &= next == 0xFF;
    }
    Some((value, all_ones))
}

/// `TryReadElementHeader` at `offset` in the file.
pub(super) fn header_at(f: &mut std::fs::File, offset: u64, parent_end: u64) -> Option<Element> {
    let d = super::read_at(f, offset, 12).ok()?;
    let mut pos = 0usize;
    let (id, _) = vint(&d, &mut pos, true)?;
    let (size, unknown) = vint(&d, &mut pos, false)?;
    let payload = offset + pos as u64;
    if !unknown && (size > i64::MAX as u64 || payload + size > parent_end) {
        return None;
    }
    Some(Element { id, offset, payload, size: if unknown { 0 } else { size }, unknown })
}

/// `GetElements`: the children of a buffer.
fn elements(d: &[u8]) -> Vec<Element> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < d.len() && out.len() < 100_000 {
        let start = pos;
        let mut p = pos;
        let Some((id, _)) = vint(d, &mut p, true) else { break };
        let Some((mut size, unknown)) = vint(d, &mut p, false) else { break };
        if unknown || size > i32::MAX as u64 || p as u64 + size > d.len() as u64 {
            if !unknown {
                break;
            }
            size = (d.len() - p) as u64;
        }
        let e = Element { id, offset: start as u64, payload: p as u64, size, unknown };
        out.push(e);
        if unknown {
            break;
        }
        pos = e.end() as usize;
    }
    out
}

fn pay<'a>(d: &'a [u8], e: &Element) -> &'a [u8] {
    &d[e.payload as usize..e.end() as usize]
}

fn unsigned(v: &[u8]) -> u64 {
    v.iter().take(8).fold(0u64, |r, &b| (r << 8) | b as u64)
}

fn signed(v: &[u8]) -> i64 {
    if v.is_empty() || v.len() > 8 {
        return 0;
    }
    let mut u = unsigned(v);
    let bits = v.len() * 8;
    if v[0] & 0x80 != 0 && bits < 64 {
        u |= u64::MAX << bits;
    }
    u as i64
}

fn float(v: &[u8]) -> f64 {
    match v.len() {
        4 => f32::from_bits(u32::from_be_bytes(v.try_into().unwrap())) as f64,
        8 => f64::from_bits(u64::from_be_bytes(v.try_into().unwrap())),
        _ => 0.0,
    }
}

fn string(v: &[u8]) -> String {
    match std::str::from_utf8(v) {
        Ok(s) => s.trim_end_matches('\0').to_string(),
        Err(_) => latin1(v).trim_end_matches('\0').to_string(),
    }
}

/// The Segment element after the EBML header, if any.
pub(super) fn find_segment(f: &mut std::fs::File, len: u64) -> Option<(Element, Element)> {
    let ebml = header_at(f, 0, len)?;
    let mut pos = ebml.end();
    while pos < len {
        let candidate = header_at(f, pos, len)?;
        if candidate.id == 0x1853_8067 {
            return Some((ebml, candidate));
        }
        if candidate.unknown {
            return None;
        }
        pos = candidate.end();
    }
    None
}

pub(super) fn read(path: &str, c: &mut Collector) -> std::io::Result<()> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let Some(ebml) = header_at(&mut f, 0, len) else { return Ok(()) };
    if ebml.id != 0x1A45_DFA3 {
        return Ok(());
    }
    if ebml.size <= MAXIMUM_ELEMENT {
        ebml_header(&super::read_at(&mut f, ebml.payload, ebml.size as usize)?, c);
    }
    let Some((_, segment)) = find_segment(&mut f, len) else { return Ok(()) };
    let seg_end = if segment.unknown { len } else { segment.end() };
    let mut pos = segment.payload;
    while pos < seg_end {
        let start = pos;
        let Some(e) = header_at(&mut f, pos, seg_end) else { break };
        if !e.unknown && e.size <= MAXIMUM_ELEMENT && matches!(e.id, 0x1549_A966 | 0x1654_AE6B | 0x1254_C367) {
            let p = super::read_at(&mut f, e.payload, e.size as usize)?;
            match e.id {
                0x1549_A966 => info(&p, c),
                0x1654_AE6B => tracks(&p, c),
                _ => tags(&p, c),
            }
        }
        if e.unknown {
            break;
        }
        pos = e.end();
        if pos <= start {
            break;
        }
    }
    Ok(())
}

fn ebml_header(d: &[u8], c: &mut Collector) {
    for e in elements(d) {
        let v = pay(d, &e);
        let g = "EBML";
        match e.id {
            0x4282 => c.add(g, "Document Type", string(v)),
            0x4287 => c.add(g, "Document Type Version", unsigned(v)),
            0x4285 => c.add(g, "Document Type Read Version", unsigned(v)),
            0x42F7 => c.add(g, "EBML Read Version", unsigned(v)),
            0x42F2 => c.add(g, "Maximum ID Length", unsigned(v)),
            0x42F3 => c.add(g, "Maximum Size Length", unsigned(v)),
            _ => {}
        }
    }
}

fn info(d: &[u8], c: &mut Collector) {
    let g = "Matroska Info";
    let mut scale = 1_000_000u64;
    let mut duration: Option<f64> = None;
    for e in elements(d) {
        let v = pay(d, &e);
        match e.id {
            0x2AD7B1 => {
                scale = unsigned(v);
                c.add(g, "Timestamp Scale", format!("{} ns", n0(scale)));
            }
            0x4489 => duration = Some(float(v)),
            0x4461 if v.len() == 8 => {
                let epoch = chrono::NaiveDate::from_ymd_opt(2001, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
                let ticks = signed(v) / 100;
                if let Some(t) = epoch.checked_add_signed(chrono::TimeDelta::microseconds(ticks / 10) + chrono::TimeDelta::nanoseconds((ticks % 10) * 100)) {
                    let y = chrono::Datelike::year(&t);
                    if (1..=9999).contains(&y) {
                        c.add(g, "Date UTC", V::Utc(t));
                    }
                }
            }
            0x7BA9 => c.add(g, "Title", string(v)),
            0x4D80 => c.add(g, "Muxing Application", string(v)),
            0x5741 => c.add(g, "Writing Application", string(v)),
            0x7384 => c.add(g, "Segment Filename", string(v)),
            0x73A4 => c.add(g, "Segment UID", hex_upper(v)),
            _ => {}
        }
    }
    if let Some(dur) = duration.filter(|d| *d > 0.0) {
        c.add(g, "Duration", format_duration(dur * scale as f64 / 1_000_000_000.0));
    }
}

fn tracks(d: &[u8], c: &mut Collector) {
    let mut index = 0;
    for e in elements(d).into_iter().filter(|e| e.id == 0xAE) {
        index += 1;
        track(pay(d, &e), c, index);
    }
}

fn yes_no(v: u64) -> &'static str {
    if v != 0 { "Yes" } else { "No" }
}

fn ms(v: u64) -> String {
    format!("{} ms", fmt(v as f64 / 1_000_000.0, "0.###"))
}

fn track(d: &[u8], c: &mut Collector, index: i32) {
    let g = format!("Track {index}");
    let g = g.as_str();
    let (mut video, mut audio) = (None, None);
    for e in elements(d) {
        let v = pay(d, &e);
        match e.id {
            0xD7 => c.add(g, "Track Number", unsigned(v)),
            0x73C5 => c.add(g, "Track UID", unsigned(v)),
            0x83 => c.add(
                g,
                "Track Type",
                match unsigned(v) {
                    1 => "Video".to_string(),
                    2 => "Audio".to_string(),
                    3 => "Complex".to_string(),
                    0x10 => "Logo".to_string(),
                    0x11 => "Subtitle".to_string(),
                    0x12 => "Buttons".to_string(),
                    0x20 => "Control".to_string(),
                    0x21 => "Metadata".to_string(),
                    x => x.to_string(),
                },
            ),
            0xB9 => c.add(g, "Enabled", yes_no(unsigned(v))),
            0x88 => c.add(g, "Default", yes_no(unsigned(v))),
            0x55AA => c.add(g, "Forced", yes_no(unsigned(v))),
            0x9C => c.add(g, "Lacing", yes_no(unsigned(v))),
            0x23E383 => c.add(g, "Default Frame Duration", ms(unsigned(v))),
            0x536E => c.add(g, "Name", string(v)),
            0x22B59C => c.add(g, "Language", string(v)),
            0x22B59D => c.add(g, "Language (BCP 47)", string(v)),
            0x86 => c.add(g, "Codec ID", string(v)),
            0x258688 => c.add(g, "Codec Name", string(v)),
            0x56AA => c.add(g, "Codec Delay", ms(unsigned(v))),
            0x56BB => c.add(g, "Seek Pre-roll", ms(unsigned(v))),
            0xE0 => video = Some(v),
            0xE1 => audio = Some(v),
            _ => {}
        }
    }
    if let Some(v) = video {
        let (mut w, mut h) = (0i64, 0i64);
        for e in elements(v) {
            let x = pay(v, &e);
            match e.id {
                0xB0 => w = unsigned(x).min(i32::MAX as u64) as i64,
                0xBA => h = unsigned(x).min(i32::MAX as u64) as i64,
                0x54B0 => c.add(g, "Display Width", unsigned(x)),
                0x54BA => c.add(g, "Display Height", unsigned(x)),
                0x9A => c.add(
                    g,
                    "Interlaced",
                    match unsigned(x) {
                        1 => "Yes",
                        2 => "No",
                        _ => "Unknown",
                    },
                ),
                0x53B8 => c.add(g, "Stereo Mode", unsigned(x)),
                0x53C0 => c.add(g, "Alpha Mode", unsigned(x)),
                _ => {}
            }
        }
        c.add_size(g, w, h);
    }
    if let Some(a) = audio {
        for e in elements(a) {
            let x = pay(a, &e);
            match e.id {
                0xB5 => c.add(g, "Sampling Frequency", format!("{} Hz", fmt(float(x), "0.###"))),
                0x78B5 => c.add(g, "Output Sampling Frequency", format!("{} Hz", fmt(float(x), "0.###"))),
                0x9F => c.add(g, "Channels", unsigned(x)),
                0x6264 => c.add(g, "Bit Depth", unsigned(x)),
                _ => {}
            }
        }
    }
}

fn tags(d: &[u8], c: &mut Collector) {
    let mut index = 0;
    for tag in elements(d).into_iter().filter(|e| e.id == 0x7373) {
        let td = pay(d, &tag);
        for child in elements(td).into_iter().filter(|e| e.id == 0x67C8) {
            index += 1;
            simple_tag(pay(td, &child), c, &format!("Tag {index}"));
        }
    }
}

fn simple_tag(d: &[u8], c: &mut Collector, fallback: &str) {
    let mut name = fallback.to_string();
    let mut value: Option<String> = None;
    let mut nested = Vec::new();
    for e in elements(d) {
        let p = pay(d, &e);
        match e.id {
            0x45A3 => name = string(p),
            0x4487 => value = Some(string(p)),
            0x4485 => value = Some(format!("{} bytes", n0(p.len() as u64))),
            0x67C8 => nested.push(p),
            _ => {}
        }
    }
    if let Some(v) = value {
        c.add("Matroska Tags", &name, v);
    }
    for n in nested {
        simple_tag(n, c, &name);
    }
}
