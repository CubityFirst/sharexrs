//! Comparing ShareXrs's outputs with ShareX's.

use std::path::Path;

use sharex_tools::imaging::Bitmap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Byte for byte (or the same values).
    Exact,
    /// The same pixels in a different encoding (PNG's zlib stream).
    PixelExact,
    /// Within a small tolerance (lossy encoders, text rasterisation).
    Close,
    Differ,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub kind: Kind,
    pub note: String,
}

impl Outcome {
    pub fn exact() -> Self {
        Outcome { kind: Kind::Exact, note: String::new() }
    }
    pub fn fail(note: impl Into<String>) -> Self {
        Outcome { kind: Kind::Differ, note: note.into() }
    }
    pub fn skip(note: impl Into<String>) -> Self {
        Outcome { kind: Kind::Skipped, note: note.into() }
    }

    /// The worse of two outcomes (notes joined).
    pub fn and(self, other: Outcome) -> Outcome {
        let rank = |k: Kind| match k {
            Kind::Exact => 0,
            Kind::PixelExact => 1,
            Kind::Close => 2,
            Kind::Skipped => 3,
            Kind::Differ => 4,
        };
        let kind = if rank(other.kind) > rank(self.kind) { other.kind } else { self.kind };
        let note = match (self.note.is_empty(), other.note.is_empty()) {
            (true, _) => other.note,
            (_, true) => self.note,
            _ => format!("{}; {}", self.note, other.note),
        };
        Outcome { kind, note }
    }
}

/// An `.sxrb` (premultiplied BGRA) as a premultiplied RGBA [`Bitmap`].
pub fn read_sxrb(path: &Path) -> Option<Bitmap> {
    let d = std::fs::read(path).ok()?;
    if &d[..4] != b"SXRB" {
        return None;
    }
    let w = u32::from_le_bytes(d[4..8].try_into().unwrap());
    let h = u32::from_le_bytes(d[8..12].try_into().unwrap());
    let mut px = d[12..].to_vec();
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    Some(Bitmap::from_premultiplied(w, h, px))
}

/// Pixel statistics of two same-size RGBA buffers.
fn stats(a: &[u8], b: &[u8]) -> (u8, usize) {
    let mut max = 0u8;
    let mut differing = 0;
    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = pa.iter().zip(pb).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
        if d > 0 {
            differing += 1;
            max = max.max(d);
        }
    }
    (max, differing)
}

/// Compares bitmaps: exact, or close when every channel is within `tolerance`.
pub fn bitmaps(label: &str, ours: &Bitmap, theirs: &Bitmap, tolerance: u8) -> Outcome {
    if (ours.width(), ours.height()) != (theirs.width(), theirs.height()) {
        return Outcome::fail(format!(
            "{label}: size {}x{} vs ShareX {}x{}",
            ours.width(),
            ours.height(),
            theirs.width(),
            theirs.height()
        ));
    }
    let (max, n) = stats(ours.data(), theirs.data());
    let total = (ours.width() * ours.height()) as usize;
    if n == 0 {
        Outcome::exact()
    } else if max <= tolerance {
        Outcome { kind: Kind::Close, note: format!("{label}: max ±{max} on {:.2}% of pixels", n as f64 * 100.0 / total as f64) }
    } else {
        Outcome::fail(format!("{label}: max ±{max} on {:.2}% of pixels ({n})", n as f64 * 100.0 / total as f64))
    }
}

fn decode(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some((img.width(), img.height(), img.into_raw()))
}

/// Compares two encoded image files: identical bytes, the same decoded pixels,
/// or close within `tolerance`.
pub fn image_files(label: &str, ours: &Path, theirs: &Path, tolerance: u8) -> Outcome {
    let (Ok(a), Ok(b)) = (std::fs::read(ours), std::fs::read(theirs)) else {
        return Outcome::fail(format!("{label}: missing {}", if ours.exists() { theirs.display() } else { ours.display() }));
    };
    if a == b {
        return Outcome::exact();
    }
    let (Some(da), Some(db)) = (decode(&a), decode(&b)) else {
        return Outcome::fail(format!("{label}: undecodable"));
    };
    if (da.0, da.1) != (db.0, db.1) {
        return Outcome::fail(format!("{label}: size {}x{} vs ShareX {}x{}", da.0, da.1, db.0, db.1));
    }
    let (max, n) = stats(&da.2, &db.2);
    let total = (da.0 * da.1) as usize;
    if n == 0 {
        Outcome { kind: Kind::PixelExact, note: String::new() }
    } else if max <= tolerance {
        Outcome { kind: Kind::Close, note: format!("{label}: decoded max ±{max} on {:.2}%", n as f64 * 100.0 / total as f64) }
    } else {
        Outcome::fail(format!("{label}: decoded max ±{max} on {:.2}% ({n})", n as f64 * 100.0 / total as f64))
    }
}

/// Byte-for-byte comparison of two files.
pub fn bytes(label: &str, ours: &[u8], theirs: &Path) -> Outcome {
    match std::fs::read(theirs) {
        Ok(t) if t == ours => Outcome::exact(),
        Ok(t) => {
            let first = ours.iter().zip(&t).position(|(a, b)| a != b).unwrap_or(ours.len().min(t.len()));
            Outcome::fail(format!("{label}: bytes differ at {first} (len {} vs {})", ours.len(), t.len()))
        }
        Err(_) => Outcome::fail(format!("{label}: ShareX output missing")),
    }
}

/// Text comparison.
#[allow(dead_code)]
pub fn text(label: &str, ours: &str, theirs: &Path) -> Outcome {
    match std::fs::read_to_string(theirs) {
        Ok(t) if t == ours => Outcome::exact(),
        Ok(t) => {
            let line = ours.lines().zip(t.lines()).position(|(a, b)| a != b);
            let detail = match line {
                Some(i) => format!("line {}: {:?} vs ShareX {:?}", i + 1, ours.lines().nth(i).unwrap(), t.lines().nth(i).unwrap()),
                None => format!("length {} vs {}", ours.len(), t.len()),
            };
            Outcome::fail(format!("{label}: {detail}"))
        }
        Err(_) => Outcome::fail(format!("{label}: ShareX output missing")),
    }
}
