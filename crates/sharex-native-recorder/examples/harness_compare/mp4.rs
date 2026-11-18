//! The MP4 comparison: bytes, bytes without the times, box tree, sample
//! tables and codec configuration, then decoded media.

use std::path::Path;

use super::Outcome;
use super::decode;

/// A box: its path (`moov/trak[1]/mdia/mdhd`), where its payload is, and its size.
struct Box {
    path: String,
    start: usize,
    header: usize,
    size: usize,
}

const CONTAINERS: &[&[u8; 4]] = &[b"moov", b"trak", b"mdia", b"minf", b"stbl", b"dinf", b"edts", b"mvex", b"udta"];

fn walk(data: &[u8], start: usize, end: usize, prefix: &str, out: &mut Vec<Box>) {
    let mut at = start;
    let mut counts = std::collections::HashMap::new();
    while at + 8 <= end {
        let mut size = u32::from_be_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = data[at + 4..at + 8].try_into().unwrap();
        let mut header = 8;
        if size == 1 {
            size = u64::from_be_bytes(data[at + 8..at + 16].try_into().unwrap()) as usize;
            header = 16;
        } else if size == 0 {
            size = end - at;
        }
        if size < header || at + size > end {
            break;
        }
        let name = String::from_utf8_lossy(&kind).into_owned();
        let n = counts.entry(name.clone()).or_insert(0);
        *n += 1;
        let path = if *n > 1 { format!("{prefix}{name}[{n}]") } else { format!("{prefix}{name}") };
        out.push(Box { path: path.clone(), start: at, header, size });
        if CONTAINERS.contains(&&kind) {
            walk(data, at + header, at + size, &format!("{path}/"), out);
        } else if &kind == b"stsd" {
            // Version/flags, entry count, then sample entries.
            walk(data, at + header + 8, at + size, &format!("{path}/"), out);
        } else if &kind == b"avc1" || &kind == b"hvc1" || &kind == b"hev1" {
            walk(data, at + header + 78, at + size, &format!("{path}/"), out);
        } else if &kind == b"mp4a" {
            walk(data, at + header + 28, at + size, &format!("{path}/"), out);
        }
        at += size;
    }
}

fn boxes(data: &[u8]) -> Vec<Box> {
    let mut out = Vec::new();
    walk(data, 0, data.len(), "", &mut out);
    out
}

/// The file with mvhd/tkhd/mdhd creation and modification times zeroed.
fn without_times(data: &[u8]) -> Vec<u8> {
    let mut d = data.to_vec();
    for b in boxes(data) {
        let leaf = b.path.rsplit('/').next().unwrap();
        let leaf = leaf.split('[').next().unwrap();
        if matches!(leaf, "mvhd" | "tkhd" | "mdhd") {
            let p = b.start + b.header;
            let version = d[p];
            let len = if version == 1 { 16 } else { 8 };
            d[p + 4..p + 4 + len].fill(0);
        }
    }
    d
}

pub fn compare(a: &Path, b: &Path, x: &[u8], y: &[u8]) -> Result<Outcome, String> {
    let mut notes = Vec::new();
    if x == y {
        notes.push("bytes identical".to_string());
    } else if without_times(x) == without_times(y) {
        notes.push("identical but the container's creation/modification times".to_string());
    } else {
        let (bx, by) = (boxes(x), boxes(y));
        let tree = |v: &[Box]| v.iter().map(|b| b.path.clone()).collect::<Vec<_>>();
        if tree(&bx) != tree(&by) {
            return Err(format!("box trees differ: {:?} vs {:?}", tree(&bx), tree(&by)));
        }
        let (tx, ty) = (without_times(x), without_times(y));
        let mut differing = Vec::new();
        let mut same_tables = Vec::new();
        for (p, q) in bx.iter().zip(&by) {
            let leaf = p.path.rsplit('/').next().unwrap().split('[').next().unwrap();
            let is_container = CONTAINERS.iter().any(|c| leaf.as_bytes() == *c) || matches!(leaf, "stsd" | "avc1" | "hvc1" | "mp4a");
            if is_container {
                continue;
            }
            let (pa, pb) = (&tx[p.start..p.start + p.size], &ty[q.start..q.start + q.size]);
            if pa != pb {
                differing.push(format!("{} ({} vs {} bytes)", p.path, p.size, q.size));
            } else if matches!(leaf, "stts" | "ctts" | "stss" | "stsz" | "avcC" | "hvcC" | "esds" | "colr" | "pasp" | "btrt") {
                same_tables.push(p.path.clone());
            }
        }
        // The chunk layout (mdat order, stsc, stco) may differ: Media Foundation's sink
        // writer interleaves tracks as the encoders deliver (ShareX differs from itself
        // run to run). The samples themselves must not.
        let (sa, sb) = (track_samples(&tx, &bx), track_samples(&ty, &by));
        let samples_same = sa == sb;
        let layout = |d: &String| d.starts_with("mdat") || d.contains("/stco") || d.contains("/co64") || d.contains("/stsc");
        let unexplained: Vec<&String> = differing.iter().filter(|d| !(samples_same && layout(d))).collect();
        let counts: Vec<String> = sa.iter().map(|t| t.len().to_string()).collect();
        notes.push(format!(
            "box tree identical; every sample of every track identical ({} samples); only the chunk interleaving differs ({})",
            counts.join(" + "),
            differing.join(", ")
        ));
        notes.push(format!("identical tables: {}", same_tables.join(", ")));
        if !unexplained.is_empty() {
            notes.push(format!("**unexplained differences**: {unexplained:?}"));
        }
    }
    let da = decode::decode(a)?;
    let db = decode::decode(b)?;
    let mut decoded = vec![format!("{} video frames ({}x{}), {} audio frames", da.frames.len(), da.width, da.height, da.audio.len() / 2)];
    if da.frames.len() != db.frames.len() {
        return Err(format!("{} vs {} decoded video frames", da.frames.len(), db.frames.len()));
    }
    let times_a: Vec<_> = da.frames.iter().map(|f| (f.time, f.duration)).collect();
    let times_b: Vec<_> = db.frames.iter().map(|f| (f.time, f.duration)).collect();
    if times_a != times_b {
        return Err(format!("decoded timestamps differ: {times_a:?} vs {times_b:?}"));
    }
    decoded.push("same timestamps and durations".into());
    let max = da.frames.iter().zip(&db.frames).map(|(p, q)| p.pixels.iter().zip(&q.pixels).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap_or(0)).max().unwrap_or(0);
    decoded.push(if max == 0 { "decoded pixels identical".into() } else { format!("decoded pixels differ by at most {max}") });
    if da.audio.len() != db.audio.len() {
        decoded.push(format!("**decoded audio lengths differ: {} vs {}**", da.audio.len(), db.audio.len()));
    } else if !da.audio.is_empty() {
        let amax = da.audio.iter().zip(&db.audio).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap_or(0);
        decoded.push(if amax == 0 { "decoded audio identical".into() } else { format!("decoded audio differs by at most {amax}") });
    }
    let identical = x == y;
    let note = format!("{}; decoded: {}", notes.join("; "), decoded.join(", "));
    if note.contains("**") {
        return Err(note);
    }
    Ok(if identical { Outcome::Identical } else { Outcome::Close(note) })
}

fn u32_at(d: &[u8], i: usize) -> usize {
    u32::from_be_bytes(d[i..i + 4].try_into().unwrap()) as usize
}

/// Each track's samples' bytes, found through stsz, stsc and stco.
fn track_samples(data: &[u8], boxes: &[Box]) -> Vec<Vec<Vec<u8>>> {
    let mut tracks = Vec::new();
    let traks: Vec<&str> = boxes.iter().filter(|b| b.path.rsplit('/').next().unwrap().starts_with("trak")).map(|b| b.path.as_str()).collect();
    for trak in traks {
        let find = |leaf: &str| boxes.iter().find(|b| b.path == format!("{trak}/mdia/minf/stbl/{leaf}")).map(|b| b.start + b.header);
        let (Some(stsz), Some(stsc), Some(stco)) = (find("stsz"), find("stsc"), find("stco")) else { continue };
        let fixed = u32_at(data, stsz + 4);
        let count = u32_at(data, stsz + 8);
        let sizes: Vec<usize> = (0..count).map(|i| if fixed != 0 { fixed } else { u32_at(data, stsz + 12 + i * 4) }).collect();
        let chunks = u32_at(data, stco + 4);
        let offsets: Vec<usize> = (0..chunks).map(|i| u32_at(data, stco + 8 + i * 4)).collect();
        let entries = u32_at(data, stsc + 4);
        let runs: Vec<(usize, usize)> = (0..entries).map(|i| (u32_at(data, stsc + 8 + i * 12), u32_at(data, stsc + 12 + i * 12))).collect();
        let mut samples = Vec::new();
        let mut sample = 0;
        for (c, offset) in offsets.iter().enumerate() {
            let chunk = c + 1;
            let per = runs.iter().rev().find(|(first, _)| *first <= chunk).map_or(0, |r| r.1);
            let mut at = *offset;
            for _ in 0..per {
                if sample >= sizes.len() {
                    break;
                }
                samples.push(data[at..at + sizes[sample]].to_vec());
                at += sizes[sample];
                sample += 1;
            }
        }
        tracks.push(samples);
    }
    tracks
}
