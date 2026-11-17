//! Animated GIF trimmer: `GifLosslessTrimmer` (frames copied without
//! re-encoding: header, palette, extensions and the selected frames' control
//! and image blocks) and `AnimatedGifTrimmerDocument`'s frame timing
//! (`FrameAt`, `NearestBoundary`).
//!
//! Not ported: the re-encoding export (crop/resize or a selection whose first
//! frame depends on earlier ones), which decodes frames with Skia's animated
//! GIF codec and writes them with `AnimatedGifCreator` (`crate::gif`), and the
//! window's preview rendering.

const INVALID: &str = "AnimatedGifTrimmer_InvalidGif";

fn invalid() -> String {
    sharex_strings::tools::lookup(INVALID).unwrap_or("Invalid GIF").to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Extension,
    GraphicControl,
    Image,
}

#[derive(Debug, Clone, Copy)]
struct Block {
    offset: usize,
    length: usize,
    kind: Kind,
    frame: i32,
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    delay: i32,
    covers_screen: bool,
    may_contain_transparency: bool,
    disposal: i32,
}

/// A parsed GIF ready for lossless cuts.
#[derive(Debug, Clone)]
pub struct GifLosslessTrimmer {
    blocks: Vec<Block>,
    frames: Vec<Frame>,
    header_length: usize,
    pub looping: bool,
    pub repeat_count: i32,
}

fn word(b: &[u8], o: usize) -> i32 {
    b[o] as i32 | (b[o + 1] as i32) << 8
}

struct Reader<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, String> {
        let b = *self.d.get(self.pos).ok_or("end of stream")?;
        self.pos += 1;
        Ok(b)
    }
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        if self.pos + n > self.d.len() {
            return Err("end of stream".into());
        }
        let s = &self.d[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        if n > self.d.len() - self.pos {
            return Err("end of stream".into());
        }
        self.pos += n;
        Ok(())
    }
    fn skip_sub_blocks(&mut self) -> Result<(), String> {
        loop {
            let n = self.byte()?;
            if n == 0 {
                return Ok(());
            }
            self.skip(n as usize)?;
        }
    }
}

impl GifLosslessTrimmer {
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// `GetDelay`: centiseconds, at least 1.
    pub fn delay(&self, index: usize) -> i32 {
        self.frames[index].delay.max(1)
    }

    /// `CanCopySelection`: the first frame mustn't restore to a previous one
    /// (disposal 3) unless it's the only frame kept.
    pub fn can_copy_selection(&self, first: usize, end_exclusive: usize) -> bool {
        first == 0 || end_exclusive == first + 1 || self.frames[first].disposal != 3
    }

    /// `NeedsVisualValidation`.
    pub fn needs_visual_validation(&self, first: usize) -> bool {
        first > 0 && (!self.frames[first].covers_screen || self.frames[first].may_contain_transparency)
    }

    /// `Parse`.
    pub fn parse(d: &[u8]) -> Result<Self, String> {
        let mut r = Reader { d, pos: 0 };
        let header = r.take(13)?.to_vec();
        if &header[..6] != b"GIF87a" && &header[..6] != b"GIF89a" {
            return Err(invalid());
        }
        let (sw, sh) = (word(&header, 6), word(&header, 8));
        if sw == 0 || sh == 0 {
            return Err(invalid());
        }
        if header[10] & 0x80 != 0 {
            r.skip(3usize << ((header[10] & 7) + 1))?;
        }
        let header_length = r.pos;
        let mut blocks: Vec<Block> = Vec::new();
        let mut frames: Vec<Frame> = Vec::new();
        let mut pending: i32 = -1;
        let mut pending_delay = 10;
        let mut pending_packed = 0u8;
        let (mut looping, mut repeat) = (false, 0);
        loop {
            let offset = r.pos;
            match r.byte()? {
                0x21 => {
                    let label = r.byte()?;
                    if label == 0xF9 {
                        if pending >= 0 || r.byte()? != 4 {
                            return Err(invalid());
                        }
                        let control = r.take(4)?.to_vec();
                        if r.byte()? != 0 {
                            return Err(invalid());
                        }
                        pending = blocks.len() as i32;
                        pending_packed = control[0];
                        pending_delay = word(&control, 1);
                        blocks.push(Block { offset, length: r.pos - offset, kind: Kind::GraphicControl, frame: -1 });
                    } else {
                        if label == 0x01 {
                            return Err(sharex_strings::tools::lookup("AnimatedGifTrimmer_UnsupportedGif").unwrap_or("Unsupported GIF").into());
                        }
                        if label == 0xFF {
                            if let Some(n) = loop_extension(&d[r.pos..]) {
                                looping = true;
                                repeat = n;
                            }
                        }
                        r.skip_sub_blocks()?;
                        blocks.push(Block { offset, length: r.pos - offset, kind: Kind::Extension, frame: -1 });
                    }
                }
                0x2C => {
                    let desc = r.take(9)?.to_vec();
                    let (left, top, w, h) = (word(&desc, 0), word(&desc, 2), word(&desc, 4), word(&desc, 6));
                    if w == 0 || h == 0 || left + w > sw || top + h > sh {
                        return Err(invalid());
                    }
                    if desc[8] & 0x80 != 0 {
                        r.skip(3usize << ((desc[8] & 7) + 1))?;
                    }
                    r.byte()?;
                    r.skip_sub_blocks()?;
                    let index = frames.len() as i32;
                    frames.push(Frame {
                        delay: pending_delay,
                        covers_screen: left == 0 && top == 0 && w == sw && h == sh,
                        may_contain_transparency: pending_packed & 1 != 0,
                        disposal: ((pending_packed >> 2) & 7) as i32,
                    });
                    if pending >= 0 {
                        blocks[pending as usize].frame = index;
                    }
                    blocks.push(Block { offset, length: r.pos - offset, kind: Kind::Image, frame: index });
                    pending = -1;
                    pending_delay = 10;
                    pending_packed = 0;
                }
                0x3B => {
                    if pending >= 0 || frames.len() < 2 {
                        return Err(invalid());
                    }
                    return Ok(GifLosslessTrimmer { blocks, frames, header_length, looping, repeat_count: repeat });
                }
                _ => return Err(invalid()),
            }
        }
    }

    /// `Copy`: the header, every extension, and the selected frames' blocks,
    /// then the trailer.
    pub fn copy(&self, input: &[u8], first: usize, end_exclusive: usize) -> Result<Vec<u8>, String> {
        if !self.can_copy_selection(first, end_exclusive) {
            return Err(sharex_strings::tools::lookup("AnimatedGifTrimmer_DependentFrame").unwrap_or("Dependent frame").into());
        }
        let mut out = input[..self.header_length].to_vec();
        for b in &self.blocks {
            if b.kind != Kind::Extension && (b.frame < first as i32 || b.frame >= end_exclusive as i32) {
                continue;
            }
            out.extend_from_slice(&input[b.offset..b.offset + b.length]);
        }
        out.push(0x3B);
        Ok(out)
    }

    /// The frames' start times in centiseconds, and the end (`_starts`).
    pub fn starts(&self) -> Vec<i64> {
        let mut s = vec![0i64];
        for i in 0..self.frames.len() {
            s.push(s[i] + self.delay(i) as i64);
        }
        s
    }
}

/// `TryReadLoopExtension` on the bytes after the 0xFF label.
fn loop_extension(d: &[u8]) -> Option<i32> {
    if d.first() != Some(&11) || d.len() < 17 {
        return None;
    }
    let id = &d[1..12];
    if id != b"NETSCAPE2.0" && id != b"ANIMEXTS1.0" {
        return None;
    }
    if d[12] != 3 {
        return None;
    }
    (d[13] == 1).then(|| word(d, 14))
}

/// `FrameAt(seconds)`.
pub fn frame_at(starts: &[i64], seconds: f64) -> usize {
    let frames = starts.len() - 1;
    let cs = (seconds * 100.0).floor().clamp(0.0, (starts[frames] - 1) as f64) as i64;
    let index = match starts.binary_search(&cs) {
        Ok(i) => i as i64,
        Err(i) => i as i64 - 1,
    };
    index.clamp(0, frames as i64 - 1) as usize
}

/// `NearestBoundary(seconds, includeEnd)`.
pub fn nearest_boundary(starts: &[i64], seconds: f64, include_end: bool) -> usize {
    let frames = starts.len() - 1;
    let cs = (seconds * 100.0).round_ties_even().clamp(0.0, starts[frames] as f64) as i64;
    let mut index = match starts.binary_search(&cs) {
        Ok(i) => i,
        Err(i) => {
            let mut i = i;
            if i > 0 && (i == starts.len() || cs - starts[i - 1] < starts[i] - cs) {
                i -= 1;
            }
            i
        }
    };
    index = index.min(if include_end { frames } else { frames - 1 });
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing() {
        let starts = vec![0, 10, 30, 60];
        assert_eq!(frame_at(&starts, 0.15), 1);
        assert_eq!(frame_at(&starts, 5.0), 2);
        assert_eq!(nearest_boundary(&starts, 0.21, true), 2);
        assert_eq!(nearest_boundary(&starts, 0.6, true), 3);
        assert_eq!(nearest_boundary(&starts, 0.6, false), 2);
    }
}
