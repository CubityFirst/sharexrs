//! `TiffMetadataReader` (TIFF and EXIF IFDs, GPS), `IccMetadataReader` and
//! `XmpMetadataReader`.

use std::collections::{HashMap, HashSet};

use super::{Collector, V, ascii, decode_text, fmt, hex_upper, latin1, n0, utf16};

const MAXIMUM_TIFF_READ_SIZE: u64 = 256 * 1024 * 1024;

/// `TiffMetadataReader.ReadFile`.
pub(super) fn read_file(path: &str, c: &mut Collector) -> std::io::Result<()> {
    if std::fs::metadata(path)?.len() > MAXIMUM_TIFF_READ_SIZE {
        c.add("TIFF", "Metadata Notice", "The TIFF is too large for the built-in metadata reader.");
        return Ok(());
    }
    read(&std::fs::read(path)?, c, "TIFF");
    Ok(())
}

/// A TIFF value as `ReadValue` returns it.
#[derive(Clone)]
enum Raw {
    Bytes(Vec<u8>),
    Text(String),
    Unsigned(Vec<u64>),
    Signed(Vec<i64>),
    Doubles(Vec<f64>),
}

/// `TiffMetadataReader.Read(data, metadata, rootGroup)`.
pub(super) fn read(data: &[u8], c: &mut Collector, root: &str) {
    if data.len() < 8 {
        return;
    }
    let le = match &data[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return,
    };
    let mut p = Parser { data, le, root: root.into(), visited: HashSet::new(), gps: HashMap::new() };
    p.read(c);
}

struct Parser<'a> {
    data: &'a [u8],
    le: bool,
    root: String,
    visited: HashSet<u32>,
    gps: HashMap<u16, Raw>,
}

impl Parser<'_> {
    fn u16(&self, o: usize) -> u16 {
        let b = [self.data[o], self.data[o + 1]];
        if self.le { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) }
    }
    fn i16(&self, o: usize) -> i16 {
        self.u16(o) as i16
    }
    fn u32(&self, o: usize) -> u32 {
        let b: [u8; 4] = self.data[o..o + 4].try_into().unwrap();
        if self.le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
    }
    fn i32(&self, o: usize) -> i32 {
        self.u32(o) as i32
    }
    fn f64(&self, o: usize) -> f64 {
        let b: [u8; 8] = self.data[o..o + 8].try_into().unwrap();
        f64::from_bits(if self.le { u64::from_le_bytes(b) } else { u64::from_be_bytes(b) })
    }

    fn read(&mut self, c: &mut Collector) {
        if self.u16(2) != 42 {
            return;
        }
        let first = self.u32(4);
        let root = self.root.clone();
        c.add(&root, "Byte Order", if self.le { "Little-endian (Intel)" } else { "Big-endian (Motorola)" });
        let next = self.read_ifd(first, &root, 0, c);
        if next > 0 {
            self.read_ifd(next, "Thumbnail", 0, c);
        }
        self.add_gps_position(c);
    }

    fn read_ifd(&mut self, offset: u32, group: &str, depth: i32, c: &mut Collector) -> u32 {
        if depth > 8 || offset as i64 > self.data.len() as i64 - 2 || !self.visited.insert(offset) {
            return 0;
        }
        let mut pos = offset as usize;
        let count = self.u16(pos).min(4096) as usize;
        pos += 2;
        if pos + count * 12 > self.data.len() {
            return 0;
        }
        let mut children: Vec<(String, u32)> = Vec::new();
        for _ in 0..count {
            let tag = self.u16(pos);
            let ty = self.u16(pos + 2);
            let n = self.u32(pos + 4);
            let raw = self.read_value(pos, ty, n);
            pos += 12;
            let Some(raw) = raw else { continue };
            let offset_of = |r: &Raw| match r {
                Raw::Unsigned(v) if !v.is_empty() && v[0] <= u32::MAX as u64 => Some(v[0] as u32),
                _ => None,
            };
            if tag == 0x8769 {
                if let Some(o) = offset_of(&raw) {
                    children.push(("EXIF".into(), o));
                    continue;
                }
            }
            if tag == 0x8825 {
                if let Some(o) = offset_of(&raw) {
                    children.push(("GPS".into(), o));
                    continue;
                }
            }
            if tag == 0xA005 {
                if let Some(o) = offset_of(&raw) {
                    children.push(("Interoperability".into(), o));
                    continue;
                }
            }
            if tag == 0x014A {
                if let Raw::Unsigned(subs) = &raw {
                    let mut child = 1;
                    for &s in subs.iter().take(16) {
                        if s <= u32::MAX as u64 {
                            children.push((format!("SubIFD {child}"), s as u32));
                            child += 1;
                        }
                    }
                    continue;
                }
            }
            if group == "GPS" {
                self.gps.insert(tag, raw.clone());
            }
            if tag == 0x02BC {
                if let Raw::Bytes(xmp) = &raw {
                    read_xmp(xmp, c);
                    continue;
                }
            }
            if tag == 0x8773 {
                if let Raw::Bytes(icc) = &raw {
                    read_icc(icc, c);
                    continue;
                }
            }
            let Some(name) = tag_name(group, tag) else { continue };
            if let Some(v) = format_value(group, tag, &raw) {
                c.add(group, name, v);
            }
        }
        for (g, o) in children {
            self.read_ifd(o, &g, depth + 1, c);
        }
        let next = offset as usize + 2 + count * 12;
        if next + 4 <= self.data.len() { self.u32(next) } else { 0 }
    }

    fn read_value(&self, entry: usize, ty: u16, count: u32) -> Option<Raw> {
        let size: u64 = match ty {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 | 13 => 4,
            5 | 10 | 12 => 8,
            _ => 0,
        };
        if size == 0 || count == 0 || count > 1_000_000 {
            return None;
        }
        let bytes = size * count as u64;
        if bytes > 32 * 1024 * 1024 {
            return None;
        }
        let off = if bytes <= 4 { entry as u64 + 8 } else { self.u32(entry + 8) as u64 };
        if off + bytes > self.data.len() as u64 {
            return None;
        }
        let o = off as usize;
        let n = count as usize;
        Some(match ty {
            1 | 6 | 7 => Raw::Bytes(self.data[o..o + bytes as usize].to_vec()),
            2 => Raw::Text(decode_text(&self.data[o..o + bytes as usize])),
            3 | 13 => Raw::Unsigned((0..n).map(|i| self.u16(o + i * 2) as u64).collect()),
            4 => Raw::Unsigned((0..n).map(|i| self.u32(o + i * 4) as u64).collect()),
            5 => Raw::Doubles(
                (0..n)
                    .map(|i| {
                        let (a, b) = (self.u32(o + i * 8), self.u32(o + i * 8 + 4));
                        if b == 0 { f64::NAN } else { a as f64 / b as f64 }
                    })
                    .collect(),
            ),
            8 => Raw::Signed((0..n).map(|i| self.i16(o + i * 2) as i64).collect()),
            9 => Raw::Signed((0..n).map(|i| self.i32(o + i * 4) as i64).collect()),
            10 => Raw::Doubles(
                (0..n)
                    .map(|i| {
                        let (a, b) = (self.i32(o + i * 8), self.i32(o + i * 8 + 4));
                        if b == 0 { f64::NAN } else { a as f64 / b as f64 }
                    })
                    .collect(),
            ),
            11 => Raw::Doubles((0..n).map(|i| f32::from_bits(self.u32(o + i * 4)) as f64).collect()),
            12 => Raw::Doubles((0..n).map(|i| self.f64(o + i * 8)).collect()),
            _ => return None,
        })
    }

    fn add_gps_position(&self, c: &mut Collector) {
        let s = |t: u16| -> Option<String> {
            let r = match self.gps.get(&t)? {
                Raw::Text(s) => s.clone(),
                Raw::Bytes(b) => decode_text(b),
                _ => String::new(),
            };
            (!r.is_empty()).then_some(r)
        };
        let n = |t: u16| numbers(self.gps.get(&t)?).filter(|v| !v.is_empty());
        let (Some(lat_ref), Some(lat), Some(lon_ref), Some(lon)) = (s(1), n(2), s(3), n(4)) else { return };
        if lat.len() < 3 || lon.len() < 3 {
            return;
        }
        let mut la = lat[0] + lat[1] / 60.0 + lat[2] / 3600.0;
        let mut lo = lon[0] + lon[1] / 60.0 + lon[2] / 3600.0;
        if lat_ref.eq_ignore_ascii_case("S") {
            la = -la;
        }
        if lon_ref.eq_ignore_ascii_case("W") {
            lo = -lo;
        }
        let (a, b) = (fmt(la, "0.######"), fmt(lo, "0.######"));
        c.add("GPS", "GPS Position", format!("{a}, {b}"));
        c.add("GPS", "Map", format!("https://www.openstreetmap.org/?mlat={a}&mlon={b}#map=16/{a}/{b}"));
    }
}

fn numbers(r: &Raw) -> Option<Vec<f64>> {
    match r {
        Raw::Doubles(d) => Some(d.clone()),
        Raw::Unsigned(u) => Some(u.iter().map(|&x| x as f64).collect()),
        Raw::Signed(s) => Some(s.iter().map(|&x| x as f64).collect()),
        _ => Some(Vec::new()),
    }
}

fn format_numbers(v: &[f64]) -> String {
    v.iter().map(|&x| if x.is_nan() { "undefined".to_string() } else { fmt(x, "0.#####") }).collect::<Vec<_>>().join(", ")
}

fn format_value(group: &str, tag: u16, raw: &Raw) -> Option<String> {
    match raw {
        Raw::Text(t) => return Some(t.clone()),
        Raw::Bytes(b) => {
            return Some(if (0x9C9B..=0x9C9F).contains(&tag) {
                utf16(b, false).trim_end_matches('\0').to_string()
            } else if tag == 0x9286 {
                decode_user_comment(b)
            } else if tag == 0x927C {
                format!("{} bytes", n0(b.len() as u64))
            } else if tag == 0x9000 || tag == 0xA000 {
                let v = ascii(b);
                let v = v.trim_end_matches('\0');
                let chars: Vec<char> = v.chars().collect();
                if chars.len() == 4 { format!("{}.{}", chars[0], chars[1..].iter().collect::<String>()) } else { v.to_string() }
            } else if matches!(tag, 0x9101 | 0xA300 | 0xA301) {
                b.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")
            } else if b.len() <= 16 {
                hex_upper(b)
            } else {
                format!("{} bytes", n0(b.len() as u64))
            });
        }
        _ => {}
    }
    let numbers = numbers(raw)?;
    if numbers.is_empty() {
        return None;
    }
    let first = numbers[0];
    if group == "GPS" {
        return Some(match tag {
            0x0000 => format_numbers(&numbers),
            0x0002 | 0x0004 => {
                if numbers.len() >= 3 {
                    format!("{}° {}' {}\"", fmt(numbers[0], "0"), fmt(numbers[1], "0"), fmt(numbers[2], "0.###"))
                } else {
                    format_numbers(&numbers)
                }
            }
            0x0006 => format!("{} m", fmt(first, "0.###")),
            0x0007 => {
                if numbers.len() >= 3 {
                    format!("{}:{}:{} UTC", fmt(numbers[0], "00"), fmt(numbers[1], "00"), fmt(numbers[2], "00.###"))
                } else {
                    format_numbers(&numbers)
                }
            }
            0x000D => format!("{} km/h", fmt(first, "0.###")),
            0x000F | 0x0011 | 0x0018 => format!("{}°", fmt(first, "0.###")),
            _ => format_numbers(&numbers),
        });
    }
    let i = first as i32;
    let or0 = |s: Option<&str>| s.map(str::to_string).unwrap_or_else(|| fmt(first, "0"));
    Some(match tag {
        0x0103 => compression(i),
        0x0106 => photometric(i),
        0x0112 => orientation(i),
        0x011A | 0x011B => fmt(first, "0.###"),
        0x0128 => or0(match first {
            1.0 => Some("None"),
            2.0 => Some("inches"),
            3.0 => Some("centimeters"),
            _ => None,
        }),
        0x829A => {
            if first > 0.0 && first < 1.0 {
                format!("1/{} s", fmt((1.0 / first).round_ties_even(), "0"))
            } else {
                format!("{} s", fmt(first, "0.###"))
            }
        }
        0x829D => format!("f/{}", fmt(first, "0.0#")),
        0x8822 => exposure_program(i),
        0x8827 | 0x8833 => format_numbers(&numbers),
        0x9201 => format!("{} EV", fmt(first, "0.###")),
        0x9202 | 0x9205 => format!("f/{}", fmt(2f64.powf(first / 2.0), "0.0#")),
        0x9204 => {
            let v = if first > 0.0 {
                format!("+{}", fmt(first, "0.###"))
            } else if first < 0.0 {
                format!("-{}", fmt(-first, "0.###"))
            } else {
                "0".into()
            };
            format!("{v} EV")
        }
        0x9206 => format!("{} m", fmt(first, "0.###")),
        0x9207 => metering(i),
        0x9208 => light_source(i),
        0x9209 => flash(i),
        0x920A => format!("{} mm", fmt(first, "0.###")),
        0xA001 => or0(match first {
            1.0 => Some("sRGB"),
            2.0 => Some("Adobe RGB"),
            65535.0 => Some("Uncalibrated"),
            _ => None,
        }),
        0xA402 => or0(match first {
            0.0 => Some("Auto"),
            1.0 => Some("Manual"),
            2.0 => Some("Auto bracket"),
            _ => None,
        }),
        0xA403 => or0(match first {
            0.0 => Some("Auto"),
            1.0 => Some("Manual"),
            _ => None,
        }),
        0xA405 => format!("{} mm", fmt(first, "0")),
        0xA406 => or0(match first {
            0.0 => Some("Standard"),
            1.0 => Some("Landscape"),
            2.0 => Some("Portrait"),
            3.0 => Some("Night"),
            _ => None,
        }),
        0xA432 if numbers.len() >= 4 => format!(
            "{}-{} mm f/{}-{}",
            fmt(numbers[0], "0.###"),
            fmt(numbers[1], "0.###"),
            fmt(numbers[2], "0.0#"),
            fmt(numbers[3], "0.0#")
        ),
        _ => format_numbers(&numbers),
    })
}

fn decode_user_comment(b: &[u8]) -> String {
    if b.len() <= 8 {
        return decode_text(b);
    }
    let enc = ascii(&b[..8]);
    let enc = enc.trim_end_matches(['\0', ' ']);
    match enc {
        "UNICODE" => utf16(&b[8..], true).trim_end_matches('\0').to_string(),
        "ASCII" => decode_text(&b[8..]),
        _ => decode_text(b),
    }
}

fn tag_name(group: &str, tag: u16) -> Option<&'static str> {
    if group == "GPS" {
        return Some(match tag {
            0x0000 => "GPS Version",
            0x0001 => "Latitude Reference",
            0x0002 => "Latitude",
            0x0003 => "Longitude Reference",
            0x0004 => "Longitude",
            0x0005 => "Altitude Reference",
            0x0006 => "Altitude",
            0x0007 => "Time Stamp",
            0x0008 => "Satellites",
            0x0009 => "Status",
            0x000A => "Measure Mode",
            0x000B => "DOP",
            0x000C => "Speed Reference",
            0x000D => "Speed",
            0x000E => "Track Reference",
            0x000F => "Track",
            0x0010 => "Image Direction Reference",
            0x0011 => "Image Direction",
            0x0012 => "Map Datum",
            0x001B => "Processing Method",
            0x001D => "Date Stamp",
            0x001F => "Horizontal Positioning Error",
            _ => return None,
        });
    }
    Some(match tag {
        0x00FE => "Subfile Type",
        0x0100 => "Image Width",
        0x0101 => "Image Height",
        0x0102 => "Bits Per Sample",
        0x0103 => "Compression",
        0x0106 => "Photometric Interpretation",
        0x010E => "Image Description",
        0x010F => "Make",
        0x0110 => "Camera Model Name",
        0x0112 => "Orientation",
        0x0115 => "Samples Per Pixel",
        0x0116 => "Rows Per Strip",
        0x011A => "X Resolution",
        0x011B => "Y Resolution",
        0x011C => "Planar Configuration",
        0x0128 => "Resolution Unit",
        0x0131 => "Software",
        0x0132 => "Modify Date",
        0x013B => "Artist",
        0x013C => "Host Computer",
        0x0201 => "Thumbnail Offset",
        0x0202 => "Thumbnail Length",
        0x8298 => "Copyright",
        0x829A => "Exposure Time",
        0x829D => "F Number",
        0x8822 => "Exposure Program",
        0x8827 => "ISO",
        0x8830 => "Sensitivity Type",
        0x8832 => "Recommended Exposure Index",
        0x8833 => "ISO Speed",
        0x9000 => "EXIF Version",
        0x9003 => "Date/Time Original",
        0x9004 => "Create Date",
        0x9010 => "Offset Time",
        0x9011 => "Offset Time Original",
        0x9012 => "Offset Time Digitized",
        0x9101 => "Components Configuration",
        0x9201 => "Shutter Speed",
        0x9202 => "Aperture",
        0x9204 => "Exposure Compensation",
        0x9205 => "Max Aperture Value",
        0x9206 => "Subject Distance",
        0x9207 => "Metering Mode",
        0x9208 => "Light Source",
        0x9209 => "Flash",
        0x920A => "Focal Length",
        0x927C => "Maker Note",
        0x9286 => "User Comment",
        0x9290 => "Sub Sec Time",
        0x9291 => "Sub Sec Time Original",
        0x9292 => "Sub Sec Time Digitized",
        0x9C9B => "XP Title",
        0x9C9C => "XP Comment",
        0x9C9D => "XP Author",
        0x9C9E => "XP Keywords",
        0x9C9F => "XP Subject",
        0xA000 => "Flashpix Version",
        0xA001 => "Color Space",
        0xA002 => "EXIF Image Width",
        0xA003 => "EXIF Image Height",
        0xA20E => "Focal Plane X Resolution",
        0xA20F => "Focal Plane Y Resolution",
        0xA210 => "Focal Plane Resolution Unit",
        0xA217 => "Sensing Method",
        0xA300 => "File Source",
        0xA301 => "Scene Type",
        0xA401 => "Custom Rendered",
        0xA402 => "Exposure Mode",
        0xA403 => "White Balance",
        0xA404 => "Digital Zoom Ratio",
        0xA405 => "Focal Length In 35mm Format",
        0xA406 => "Scene Capture Type",
        0xA407 => "Gain Control",
        0xA408 => "Contrast",
        0xA409 => "Saturation",
        0xA40A => "Sharpness",
        0xA40C => "Subject Distance Range",
        0xA420 => "Image Unique ID",
        0xA430 => "Camera Owner Name",
        0xA431 => "Body Serial Number",
        0xA432 => "Lens Specification",
        0xA433 => "Lens Make",
        0xA434 => "Lens Model",
        0xA435 => "Lens Serial Number",
        _ => return None,
    })
}

fn unknown(v: i32) -> String {
    format!("Unknown ({v})")
}

fn compression(v: i32) -> String {
    match v {
        1 => "Uncompressed",
        2 => "CCITT 1D",
        3 => "Group 3 Fax",
        4 => "Group 4 Fax",
        5 => "LZW",
        6 => "Old JPEG",
        7 => "JPEG",
        8 => "Deflate",
        32773 => "PackBits",
        34712 => "JPEG 2000",
        _ => return unknown(v),
    }
    .into()
}

fn photometric(v: i32) -> String {
    match v {
        0 => "WhiteIsZero",
        1 => "BlackIsZero",
        2 => "RGB",
        3 => "Palette",
        4 => "Transparency mask",
        5 => "CMYK",
        6 => "YCbCr",
        8 => "CIELab",
        _ => return unknown(v),
    }
    .into()
}

fn orientation(v: i32) -> String {
    match v {
        1 => "Horizontal (normal)",
        2 => "Mirror horizontal",
        3 => "Rotate 180",
        4 => "Mirror vertical",
        5 => "Mirror horizontal and rotate 270 CW",
        6 => "Rotate 90 CW",
        7 => "Mirror horizontal and rotate 90 CW",
        8 => "Rotate 270 CW",
        _ => return unknown(v),
    }
    .into()
}

fn exposure_program(v: i32) -> String {
    match v {
        0 => "Not defined",
        1 => "Manual",
        2 => "Program AE",
        3 => "Aperture-priority AE",
        4 => "Shutter-priority AE",
        5 => "Creative",
        6 => "Action",
        7 => "Portrait",
        8 => "Landscape",
        _ => return v.to_string(),
    }
    .into()
}

fn metering(v: i32) -> String {
    match v {
        0 => "Unknown",
        1 => "Average",
        2 => "Center-weighted average",
        3 => "Spot",
        4 => "Multi-spot",
        5 => "Multi-segment",
        6 => "Partial",
        255 => "Other",
        _ => return v.to_string(),
    }
    .into()
}

fn light_source(v: i32) -> String {
    match v {
        0 => "Unknown",
        1 => "Daylight",
        2 => "Fluorescent",
        3 => "Tungsten",
        4 => "Flash",
        9 => "Fine weather",
        10 => "Cloudy",
        11 => "Shade",
        12 => "Daylight fluorescent",
        13 => "Day white fluorescent",
        14 => "Cool white fluorescent",
        15 => "White fluorescent",
        17 => "Standard light A",
        18 => "Standard light B",
        19 => "Standard light C",
        20 => "D55",
        21 => "D65",
        22 => "D75",
        23 => "D50",
        24 => "ISO studio tungsten",
        255 => "Other",
        _ => return v.to_string(),
    }
    .into()
}

fn flash(v: i32) -> String {
    let fired = if v & 1 != 0 { "Fired" } else { "Did not fire" };
    let ret = match (v >> 1) & 3 {
        2 => ", return not detected",
        3 => ", return detected",
        _ => "",
    };
    format!("{fired}{ret}")
}

// ---------------------------------------------------------------- ICC

fn four_cc(b: &[u8]) -> String {
    ascii(&b[..4]).trim_end_matches(['\0', ' ']).to_string()
}

fn be16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}

fn be32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap())
}

/// `IccMetadataReader.Read`.
pub(super) fn read_icc(p: &[u8], c: &mut Collector) {
    if p.len() < 128 {
        return;
    }
    let g = "ICC Profile";
    c.add(g, "Profile Size", format!("{} bytes", n0(be32(p, 0) as u64)));
    c.add(g, "CMM Type", four_cc(&p[4..8]));
    c.add(g, "Profile Version", format!("{}.{}.{}", p[8] >> 4, p[8] & 15, p[9] >> 4));
    let class = four_cc(&p[12..16]);
    c.add(
        g,
        "Profile Class",
        match class.as_str() {
            "scnr" => "Input device",
            "mntr" => "Display device",
            "prtr" => "Output device",
            "link" => "Device link",
            "spac" => "Color space conversion",
            "abst" => "Abstract",
            "nmcl" => "Named color",
            _ => class.as_str(),
        },
    );
    c.add(g, "Color Space", four_cc(&p[16..20]));
    c.add(g, "Connection Space", four_cc(&p[20..24]));
    if let Some(d) = chrono::NaiveDate::from_ymd_opt(be16(p, 24) as i32, be16(p, 26) as u32, be16(p, 28) as u32)
        .and_then(|d| d.and_hms_opt(be16(p, 30) as u32, be16(p, 32) as u32, be16(p, 34) as u32))
    {
        if (1..=9999).contains(&be16(p, 24)) {
            c.add(g, "Profile Date/Time", V::Utc(d));
        }
    }
    c.add(g, "Primary Platform", four_cc(&p[40..44]));
    c.add(g, "Device Manufacturer", four_cc(&p[48..52]));
    c.add(g, "Device Model", four_cc(&p[52..56]));
    c.add(
        g,
        "Rendering Intent",
        match be32(p, 64) {
            0 => "Perceptual".to_string(),
            1 => "Media-relative colorimetric".to_string(),
            2 => "Saturation".to_string(),
            3 => "ICC-absolute colorimetric".to_string(),
            v => v.to_string(),
        },
    );
    c.add(g, "Profile Creator", four_cc(&p[80..84]));
    if p.len() < 132 {
        return;
    }
    let count = be32(p, 128).min(1024) as usize;
    for i in 0..count {
        let pos = 132 + i * 12;
        if pos + 12 > p.len() {
            break;
        }
        let sig = four_cc(&p[pos..pos + 4]);
        let (off, size) = (be32(p, pos + 4) as u64, be32(p, pos + 8) as u64);
        if size > i32::MAX as u64 || off + size > p.len() as u64 || size < 8 {
            continue;
        }
        let tag = &p[off as usize..(off + size) as usize];
        if let Some(v) = icc_text(tag) {
            let name = match sig.as_str() {
                "desc" => "Profile Description",
                "cprt" => "Copyright",
                "dmnd" => "Device Manufacturer Description",
                "dmdd" => "Device Model Description",
                "vued" => "Viewing Conditions Description",
                _ => sig.as_str(),
            };
            c.add(g, name, v);
        }
    }
}

fn icc_text(t: &[u8]) -> Option<String> {
    let ty = four_cc(&t[..4]);
    if ty == "desc" && t.len() >= 12 {
        let len = be32(t, 8) as usize;
        if len > 0 && len <= t.len() - 12 {
            return Some(latin1(&t[12..12 + len]).trim_end_matches('\0').to_string());
        }
    }
    if ty == "text" && t.len() > 8 {
        return Some(ascii(&t[8..]).trim_end_matches('\0').to_string());
    }
    if ty == "mluc" && t.len() >= 28 {
        if be32(t, 8) == 0 {
            return None;
        }
        let (len, off) = (be32(t, 20) as u64, be32(t, 24) as u64);
        if len <= i32::MAX as u64 && off + len <= t.len() as u64 {
            return Some(utf16(&t[off as usize..(off + len) as usize], true));
        }
    }
    None
}

// ---------------------------------------------------------------- XMP

fn friendly(ns: Option<&str>, local: &str) -> String {
    let prefix = match ns.unwrap_or("") {
        "http://purl.org/dc/elements/1.1/" => "Dublin Core",
        "http://ns.adobe.com/xap/1.0/" => "XMP",
        "http://ns.adobe.com/photoshop/1.0/" => "Photoshop",
        "http://ns.adobe.com/exif/1.0/" => "EXIF",
        "http://ns.adobe.com/tiff/1.0/" => "TIFF",
        _ => "",
    };
    if prefix.is_empty() { local.to_string() } else { format!("{prefix} {local}") }
}

fn structural(local: &str) -> bool {
    matches!(local, "about" | "parseType" | "Description" | "RDF" | "Alt" | "Bag" | "Seq" | "li")
}

/// `XmpMetadataReader.Read`: from the first `<` to the last `>`; attributes
/// first, then the leaf elements, at most 256 values.
pub(super) fn read_xmp(data: &[u8], c: &mut Collector) {
    let Some(start) = data.iter().position(|&b| b == b'<') else { return };
    let Some(end) = data.iter().rposition(|&b| b == b'>') else { return };
    if end <= start {
        return;
    }
    let text = String::from_utf8_lossy(&data[start..=end]).into_owned();
    let Ok(doc) = roxmltree::Document::parse(&text) else { return };
    let mut count = 0;
    let elements: Vec<roxmltree::Node> = doc.descendants().filter(|n| n.is_element()).collect();
    for e in &elements {
        for a in e.attributes() {
            if structural(a.name()) {
                continue;
            }
            c.add("XMP", &friendly(a.namespace(), a.name()), a.value());
            count += 1;
            if count >= 256 {
                return;
            }
        }
    }
    for e in &elements {
        if e.children().any(|n| n.is_element()) || structural(e.tag_name().name()) {
            continue;
        }
        let value: String = e.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let name_node = match e.parent_element() {
            Some(p) if p.tag_name().name() == "Alt" => p.parent_element().unwrap_or(*e),
            _ => *e,
        };
        let mut name = friendly(name_node.tag_name().namespace(), name_node.tag_name().name());
        if let Some(lang) = e.attribute(("http://www.w3.org/XML/1998/namespace", "lang")) {
            name.push_str(&format!(" ({lang})"));
        }
        c.add("XMP", &name, value);
        count += 1;
        if count >= 256 {
            return;
        }
    }
}
