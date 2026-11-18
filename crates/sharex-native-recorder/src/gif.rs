//! `Encoding/GifWriter.cs`, "ShareX adaptive GIF encoder": lossless BGRA
//! frames are cached (deflated) to a hidden file beside the output while
//! recording; after stopping, one pass builds a palette for the whole
//! recording (`AdaptiveGifQuantizer`), and a second writes each frame's
//! changed rectangle (duplicates skipped, unchanged pixels transparent when
//! a palette index is free) with centisecond delays rounded on cumulative
//! time. ShareX.HelpersLib's quantizer and GIF writer are `sharex_tools::gif`.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use sharex_tools::gif::{AdaptiveGifQuantizer, AnimatedGifCreator, IndexedImage};
use sharex_tools::imaging::Bitmap;

use crate::options::{RecordingOptions, TICKS_PER_MILLISECOND};

/// What the cache thread hands back: the file, frames written.
type CacheResult = Result<(File, i64), String>;

/// `GifWriter`.
pub struct GifWriter {
    width: u32,
    height: u32,
    output_path: PathBuf,
    frame_bytes: usize,
    frames: Option<SyncSender<(Vec<u8>, i64)>>,
    caching: Option<JoinHandle<CacheResult>>,
    failure: Arc<Mutex<Option<String>>>,
    cache_path: PathBuf,
}

/// `Rectangle` of the changed pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bounds {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl GifWriter {
    pub fn new(options: &RecordingOptions, width: u32, height: u32) -> Result<Self, String> {
        let frame_bytes = width as usize * height as usize * 4;
        let capacity = (64 * 1024 * 1024 / frame_bytes.max(1)).clamp(1, 8);
        let output_path = options.output_path.clone();
        let dir = output_path.parent().unwrap_or(Path::new("."));
        let name = output_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let cache_path = dir.join(format!(".{name}.{}.frames", guid_n()));
        let cache = create_cache(&cache_path).map_err(|e| e.to_string())?;
        let (tx, rx) = sync_channel(capacity);
        let failure = Arc::new(Mutex::new(None));
        let f = failure.clone();
        let caching = std::thread::Builder::new()
            .name("ShareX GIF frame cache".into())
            .spawn(move || {
                let r = cache_frames(cache, rx, frame_bytes);
                if let Err(e) = &r {
                    *f.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.clone());
                }
                r
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            width,
            height,
            output_path,
            frame_bytes,
            frames: Some(tx),
            caching: Some(caching),
            failure,
            cache_path,
        })
    }

    pub fn encoder() -> crate::options::VideoEncoderInfo {
        crate::options::VideoEncoderInfo::gif()
    }

    /// `TryWriteVideo`: queues a copy of a tightly packed BGRA frame;
    /// `false` (dropped) while the cache is behind.
    pub fn try_write_video(&mut self, bgra: &[u8], timestamp: i64) -> Result<bool, String> {
        if let Some(e) = self.failure.lock().unwrap_or_else(|p| p.into_inner()).clone() {
            return Err(e);
        }
        let Some(tx) = &self.frames else { return Ok(false) };
        match tx.try_send((bgra[..self.frame_bytes].to_vec(), timestamp)) {
            Ok(()) => Ok(true),
            Err(TrySendError::Full(_)) => Ok(false),
            Err(TrySendError::Disconnected(_)) => {
                Err(self.failure.lock().unwrap_or_else(|p| p.into_inner()).clone().unwrap_or_else(|| "The GIF frame cache stopped.".into()))
            }
        }
    }

    /// `Finish`: encodes the cached frames into the output; `duration` ends
    /// the last frame. Progress 0 to 100 (only changes are reported).
    pub fn finish(&mut self, duration: i64, progress: &mut dyn FnMut(i32), aborted: &dyn Fn() -> bool) -> Result<(), String> {
        self.frames = None;
        if aborted() {
            return Err(crate::ABORTED.into());
        }
        progress(0);
        let (cache, frame_count) = self.caching.take().ok_or("The GIF frame cache is gone.")?.join().map_err(|_| "The GIF frame cache failed.")??;
        let mut reader = BufReader::with_capacity(1024 * 1024, cache);
        reader.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut pixels = vec![0u8; self.frame_bytes];
        let mut quantizer = AdaptiveGifQuantizer::new();
        let mut previous_progress = 0;
        let mut report = |percent: i32| {
            if percent != previous_progress {
                progress(percent);
                previous_progress = percent;
            }
        };
        // Analyse the lossless cache once to build a stable palette for the whole recording.
        for index in 0..frame_count {
            if aborted() {
                return Err(crate::ABORTED.into());
            }
            read_frame(&mut reader, &mut pixels)?;
            quantizer.add_frame(&self.bitmap(&pixels));
            report(((index + 1) * 20 / frame_count) as i32);
        }
        reader.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut out = BufWriter::new(File::create(&self.output_path).map_err(|e| e.to_string())?);
        let mut creator = AnimatedGifCreator::new(0, 0, true);
        creator.use_global_palette = true;
        let mut previous: Option<IndexedImage> = None;
        let mut pending: Option<IndexedImage> = None;
        let mut offset = (0u32, 0u32);
        let mut transparent_index = -1;
        let mut written_centiseconds = 0i64;
        for index in 0..frame_count {
            if aborted() {
                return Err(crate::ABORTED.into());
            }
            let timestamp = read_frame(&mut reader, &mut pixels)?;
            let image = quantizer.quantize(&self.bitmap(&pixels));
            // Shared palette indices identify all changed pixels, including restored background.
            let bounds = match &previous {
                None => Some(Bounds { x: 0, y: 0, width: image.width, height: image.height }),
                Some(p) => changed_bounds(p, &image),
            };
            if let Some(bounds) = bounds {
                if let Some(p) = pending.take() {
                    write_frame(&mut creator, &p, offset, transparent_index, timestamp, &mut written_centiseconds);
                    out.write_all(&creator.take_output()).map_err(|e| e.to_string())?;
                }
                let (cropped, t) = crop_frame(&image, previous.as_ref(), bounds);
                pending = Some(cropped);
                transparent_index = t;
                offset = (bounds.x, bounds.y);
            }
            previous = Some(image);
            if index + 1 < frame_count {
                report(20 + ((index + 1) * 80 / frame_count) as i32);
            }
        }
        if aborted() {
            return Err(crate::ABORTED.into());
        }
        if let Some(p) = pending {
            write_frame(&mut creator, &p, offset, transparent_index, duration, &mut written_centiseconds);
        }
        out.write_all(&creator.finish()).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        report(100);
        if aborted() {
            return Err(crate::ABORTED.into());
        }
        Ok(())
    }

    /// `ReadFrame`'s bitmap: opaque (captured windows can leave alpha unused).
    fn bitmap(&self, bgra: &[u8]) -> Bitmap {
        let mut rgba = Vec::with_capacity(bgra.len());
        for p in bgra.chunks_exact(4) {
            rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
        }
        Bitmap::from_premultiplied(self.width, self.height, rgba)
    }
}

impl Drop for GifWriter {
    fn drop(&mut self) {
        // Disposal also handles startup failure and aborts: discard the queue, end the cache.
        self.frames = None;
        if let Some(h) = self.caching.take() {
            let _ = h.join();
        }
        let _ = std::fs::remove_file(&self.cache_path);
    }
}

/// The hidden cache file, deleted when closed (`FileOptions.DeleteOnClose`).
fn create_cache(path: &Path) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.read(true).write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_ATTRIBUTE_HIDDEN | FILE_FLAG_DELETE_ON_CLOSE | FILE_FLAG_SEQUENTIAL_SCAN.
        o.attributes(0x2).custom_flags(0x0400_0000 | 0x0800_0000);
    }
    let f = o.open(path)?;
    #[cfg(not(windows))]
    std::fs::remove_file(path)?;
    Ok(f)
}

/// `CacheFrames`: timestamp, compressed length, raw deflate (fastest).
fn cache_frames(cache: File, frames: Receiver<(Vec<u8>, i64)>, frame_bytes: usize) -> CacheResult {
    let mut w = BufWriter::with_capacity(1024 * 1024, cache);
    let mut count = 0i64;
    for (pixels, timestamp) in frames {
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(&pixels[..frame_bytes]).map_err(|e| e.to_string())?;
        let data = enc.finish().map_err(|e| e.to_string())?;
        w.write_all(&timestamp.to_le_bytes()).map_err(|e| e.to_string())?;
        w.write_all(&(data.len() as i64).to_le_bytes()).map_err(|e| e.to_string())?;
        w.write_all(&data).map_err(|e| e.to_string())?;
        count += 1;
    }
    w.flush().map_err(|e| e.to_string())?;
    let f = w.into_inner().map_err(|e| e.to_string())?;
    Ok((f, count))
}

fn read_frame(reader: &mut BufReader<File>, pixels: &mut [u8]) -> Result<i64, String> {
    let mut b = [0u8; 8];
    reader.read_exact(&mut b).map_err(|e| e.to_string())?;
    let timestamp = i64::from_le_bytes(b);
    reader.read_exact(&mut b).map_err(|e| e.to_string())?;
    let len = i64::from_le_bytes(b) as usize;
    let mut compressed = vec![0u8; len];
    reader.read_exact(&mut compressed).map_err(|e| e.to_string())?;
    DeflateDecoder::new(&compressed[..]).read_exact(pixels).map_err(|e| e.to_string())?;
    Ok(timestamp)
}

/// `GetChangedBounds`: the rectangle of differing palette indices, `None`
/// for a duplicate frame.
fn changed_bounds(previous: &IndexedImage, image: &IndexedImage) -> Option<Bounds> {
    let w = image.width as usize;
    let (mut left, mut top, mut right, mut bottom) = (w as i64, image.height as i64, -1i64, -1i64);
    for y in 0..image.height as usize {
        let current = &image.pixels[y * w..(y + 1) * w];
        let before = &previous.pixels[y * w..(y + 1) * w];
        let first = current.iter().zip(before).take_while(|(a, b)| a == b).count();
        if first == w {
            continue;
        }
        // Only search for a new right edge when pixels beyond the known edge differ.
        let from = (right + 1) as usize;
        if current[from..] != before[from..] {
            let mut last = w - 1;
            while last > first && current[last] == before[last] {
                last -= 1;
            }
            right = last as i64;
        }
        left = left.min(first as i64);
        top = top.min(y as i64);
        bottom = y as i64;
    }
    (bottom >= 0).then(|| Bounds {
        x: left as u32,
        y: top as u32,
        width: (right + 1 - left) as u32,
        height: (bottom + 1 - top) as u32,
    })
}

/// `CropFrame`: the changed rectangle, with unchanged pixels transparent
/// when a palette index is unused by the changed ones (`-1` = none).
fn crop_frame(image: &IndexedImage, previous: Option<&IndexedImage>, b: Bounds) -> (IndexedImage, i32) {
    let w = image.width as usize;
    let mut transparent_index = -1i32;
    let palette_len = image.palette.len();
    if let Some(previous) = previous {
        let mut used = [false; 256];
        let mut used_count = 0usize;
        let mut has_unchanged = false;
        let mut y = b.y as usize;
        while y < (b.y + b.height) as usize && used_count < palette_len {
            let start = y * w + b.x as usize;
            for x in 0..b.width as usize {
                let color = image.pixels[start + x];
                if color == previous.pixels[start + x] {
                    has_unchanged = true;
                } else if !used[color as usize] {
                    used[color as usize] = true;
                    used_count += 1;
                }
            }
            y += 1;
        }
        // Borrow a palette index unused by changed pixels; never remove a colour.
        if has_unchanged && used_count < palette_len {
            transparent_index = (0..palette_len).find(|&c| !used[c]).map_or(-1, |c| c as i32);
        }
    }
    let (bw, bh) = (b.width as usize, b.height as usize);
    if transparent_index >= 0 {
        let previous = previous.expect("transparency needs a previous frame");
        let mut patch = vec![0u8; bw * bh];
        for y in 0..bh {
            let source = (b.y as usize + y) * w + b.x as usize;
            for x in 0..bw {
                patch[y * bw + x] = if image.pixels[source + x] == previous.pixels[source + x] {
                    transparent_index as u8
                } else {
                    image.pixels[source + x]
                };
            }
        }
        return (IndexedImage::new(b.width, b.height, patch, image.palette.clone()), transparent_index);
    }
    if b.width == image.width && b.height == image.height {
        return (image.clone(), transparent_index);
    }
    let mut pixels = vec![0u8; bw * bh];
    for y in 0..bh {
        let s = (b.y as usize + y) * w + b.x as usize;
        pixels[y * bw..(y + 1) * bw].copy_from_slice(&image.pixels[s..s + bw]);
    }
    (IndexedImage::new(b.width, b.height, pixels, image.palette.clone()), transparent_index)
}

/// `WriteFrame`: the frame lasts until `until` (ticks), rounded on
/// cumulative centiseconds; longer than 655.35 s repeats it.
fn write_frame(creator: &mut AnimatedGifCreator, image: &IndexedImage, offset: (u32, u32), transparent_index: i32, until: i64, written: &mut i64) {
    const TICKS_PER_CENTISECOND: i64 = TICKS_PER_MILLISECOND * 10;
    let end = (*written + 1).max((until + TICKS_PER_CENTISECOND / 2) / TICKS_PER_CENTISECOND);
    while *written < end {
        let delay = (end - *written).min(u16::MAX as i64) as i32;
        creator.add_frame(image, delay * 10, offset.0 as u16, offset.1 as u16, false, Some(transparent_index));
        *written += delay as i64;
    }
}

/// A random `Guid.ToString("N")`-shaped name part.
pub(crate) fn guid_n() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut s = String::with_capacity(32);
    for _ in 0..2 {
        let mut h = RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        s.push_str(&format!("{:016x}", h.finish()));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{Rect, TICKS_PER_SECOND};

    fn frame(w: u32, h: u32, n: u32) -> Vec<u8> {
        let mut v = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let inside = (n * 2..n * 2 + 8).contains(&x) && (4..12).contains(&y);
                v[i..i + 4].copy_from_slice(&if inside { [0, 0, 255, 255] } else { [x as u8 * 4, y as u8 * 4, 128, 0] });
            }
        }
        v
    }

    #[test]
    fn writes_changed_rectangles() {
        let dir = std::env::temp_dir().join(format!("sharexrs-gif-{}", guid_n()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("a.gif");
        let mut o = RecordingOptions::new(&out);
        o.region = Rect::new(0, 0, 32, 16);
        let mut g = GifWriter::new(&o, 32, 16).unwrap();
        let fps = 10;
        for n in 0..6u32 {
            let t = n as i64 * TICKS_PER_SECOND / fps;
            // Frame 3 repeats frame 2: skipped, its time goes to frame 2.
            let f = frame(32, 16, if n == 3 { 2 } else { n });
            while !g.try_write_video(&f, t).unwrap() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        let mut progress = Vec::new();
        g.finish(6 * TICKS_PER_SECOND / fps, &mut |p| progress.push(p), &|| false).unwrap();
        drop(g);
        assert_eq!(progress.first(), Some(&0));
        assert_eq!(progress.last(), Some(&100));
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
        assert_eq!(*bytes.last().unwrap(), 0x3B);
        // 5 distinct frames; the delays add up to 60 cs.
        let mut delays = Vec::new();
        let mut i = 0;
        while i + 8 < bytes.len() {
            if bytes[i] == 0x21 && bytes[i + 1] == 0xF9 && bytes[i + 2] == 4 {
                delays.push(u16::from_le_bytes([bytes[i + 4], bytes[i + 5]]));
                i += 8;
            } else {
                i += 1;
            }
        }
        assert_eq!(delays, [10, 10, 20, 10, 10]);
        // No cache file left behind.
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn delays_round_cumulatively() {
        let img = IndexedImage::new(1, 1, vec![0], vec![[0, 0, 0, 255], [255, 255, 255, 255]]);
        let mut c = AnimatedGifCreator::new(0, 0, true);
        let mut written = 0;
        // 15 fps: 6.67 cs per frame becomes 7, 6, 7...
        let mut ends = Vec::new();
        for n in 1..=3 {
            write_frame(&mut c, &img, (0, 0), -1, n * TICKS_PER_SECOND / 15, &mut written);
            ends.push(written);
        }
        assert_eq!(ends, [7, 13, 20]);
        // At least a centisecond each.
        write_frame(&mut c, &img, (0, 0), -1, 0, &mut written);
        assert_eq!(written, 21);
    }
}
