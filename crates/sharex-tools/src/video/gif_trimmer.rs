//! Animated GIF trimmer: `GifLosslessTrimmer` (frames copied without
//! re-encoding: header, palette, extensions and the selected frames' control
//! and image blocks), `AnimatedGifTrimmerDocument` (frame timing, `Export`
//! with its re-encode, the preview frames) and the frames as
//! `SkiaAnimatedImage.GetFrame` decodes them through Skia's GIF codec
//! ([`AnimatedGif`]).

use crate::imaging::{self, Bitmap, GifQuality};
use sharex_skia::raster::{Canvas, Rect, Sampling};

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

// ------------------------------------------------------------ frames

/// `SkCodecAnimation::DisposalMethod` as Skia's Wuffs GIF codec maps a
/// graphic control's disposal (0 and 4-7 keep).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Disposal {
    Keep,
    RestoreBackground,
    RestorePrevious,
}

#[derive(Debug, Clone)]
struct GifFrame {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    disposal: Disposal,
    /// `reportedAlpha() != kOpaque_Alpha`: Wuffs' `opaque_within_bounds` is
    /// false when the frame has a transparent index.
    reports_alpha: bool,
    /// Straight RGBA for all 256 indices: the frame's palette, entries past
    /// it opaque black (Wuffs), the transparent index fully transparent.
    palette: Vec<[u8; 4]>,
    indices: Vec<u8>,
    required: Option<usize>,
    has_alpha: bool,
}

impl GifFrame {
    /// `frame_rect_on_screen`.
    fn rect(&self, sw: u32, sh: u32) -> (u32, u32, u32, u32) {
        let r = (self.left + self.width).min(sw);
        let b = (self.top + self.height).min(sh);
        (self.left.min(sw), self.top.min(sh), r, b)
    }
}

/// `SkiaAnimatedImage` over a GIF: the frames as Skia's codec composites them
/// (`SkFrameHolder::setAlphaAndRequiredFrame`, `SkCodec::handleFrameIndex`,
/// Wuffs' source-over blend of the frame's pixels).
#[derive(Debug, Clone)]
pub struct AnimatedGif {
    pub width: u32,
    pub height: u32,
    frames: Vec<GifFrame>,
}

fn contains(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> bool {
    // SkIRect::contains: an empty rect is never contained.
    b.0 < b.2 && b.1 < b.3 && a.0 <= b.0 && a.1 <= b.1 && a.2 >= b.2 && a.3 >= b.3
}

impl AnimatedGif {
    /// `new SkiaAnimatedImage(path)` for GIF bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut opts = ::gif::DecodeOptions::new();
        opts.set_color_output(::gif::ColorOutput::Indexed);
        opts.check_lzw_end_code(false);
        opts.allow_unknown_blocks(true);
        let mut dec = opts.read_info(std::io::Cursor::new(bytes)).map_err(|_| invalid())?;
        let (sw, sh) = (dec.width() as u32, dec.height() as u32);
        let global = dec.global_palette().map(|p| p.to_vec());
        let mut frames: Vec<GifFrame> = Vec::new();
        loop {
            let frame = match dec.read_next_frame() {
                Ok(Some(f)) => f,
                Ok(None) => break,
                // A truncated frame: Skia reports what it has (kIncompleteInput).
                Err(_) if !frames.is_empty() => break,
                Err(_) => return Err(invalid()),
            };
            let rgb = frame.palette.as_deref().or(global.as_deref()).unwrap_or(&[]);
            let mut palette = vec![[0u8, 0, 0, 255]; 256];
            for (i, c) in rgb.chunks_exact(3).take(256).enumerate() {
                palette[i] = [c[0], c[1], c[2], 255];
            }
            if let Some(t) = frame.transparent {
                palette[t as usize] = [0, 0, 0, 0];
            }
            let disposal = match frame.dispose {
                ::gif::DisposalMethod::Background => Disposal::RestoreBackground,
                ::gif::DisposalMethod::Previous => Disposal::RestorePrevious,
                _ => Disposal::Keep,
            };
            frames.push(GifFrame {
                left: frame.left as u32,
                top: frame.top as u32,
                width: frame.width as u32,
                height: frame.height as u32,
                disposal,
                reports_alpha: frame.transparent.is_some(),
                palette,
                indices: frame.buffer.to_vec(),
                required: None,
                has_alpha: false,
            });
        }
        if frames.is_empty() {
            return Err(invalid());
        }
        let mut gif = AnimatedGif { width: sw, height: sh, frames };
        for i in 0..gif.frames.len() {
            gif.set_alpha_and_required_frame(i);
        }
        Ok(gif)
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// The frame `GetFrame(index)` starts from (`SKCodecFrameInfo.RequiredFrame`).
    pub fn required_frame(&self, index: usize) -> Option<usize> {
        self.frames[index].required
    }

    /// `SkFrameHolder::setAlphaAndRequiredFrame` (GIF frames always blend
    /// source-over).
    fn set_alpha_and_required_frame(&mut self, i: usize) {
        let (sw, sh) = (self.width, self.height);
        let screen = (0, 0, sw, sh);
        let reports_alpha = self.frames[i].reports_alpha;
        let frame_rect = self.frames[i].rect(sw, sh);
        let set = |s: &mut Self, required: Option<usize>, has_alpha: bool| {
            s.frames[i].required = required;
            s.frames[i].has_alpha = has_alpha;
        };
        if i == 0 {
            set(self, None, reports_alpha || frame_rect != screen);
            return;
        }
        if !reports_alpha && frame_rect == screen {
            set(self, None, reports_alpha);
            return;
        }
        let mut prev = i - 1;
        while self.frames[prev].disposal == Disposal::RestorePrevious {
            if prev == 0 {
                set(self, None, true);
                return;
            }
            prev -= 1;
        }
        let clear_prev = self.frames[prev].disposal == Disposal::RestoreBackground;
        let mut prev_rect = self.frames[prev].rect(sw, sh);
        if clear_prev && (prev_rect == screen || self.frames[prev].required.is_none()) {
            set(self, None, true);
            return;
        }
        if reports_alpha {
            let has_alpha = self.frames[prev].has_alpha || clear_prev;
            set(self, Some(prev), has_alpha);
            return;
        }
        while contains(frame_rect, prev_rect) {
            match self.frames[prev].required {
                None => {
                    set(self, None, true);
                    return;
                }
                Some(r) => {
                    prev = r;
                    prev_rect = self.frames[prev].rect(sw, sh);
                }
            }
        }
        if self.frames[prev].disposal == Disposal::RestoreBackground {
            set(self, Some(prev), true);
            return;
        }
        let has_alpha = self.frames[prev].has_alpha;
        set(self, Some(prev), has_alpha);
    }

    /// Wuffs decoding one frame into `dst`: its pixels blended source-over
    /// (the transparent index leaves `dst` as it is), clipped to the screen.
    fn draw(&self, index: usize, dst: &mut Bitmap) {
        let f = &self.frames[index];
        let (sw, sh) = (self.width, self.height);
        for y in 0..f.height {
            let dy = f.top + y;
            if dy >= sh {
                break;
            }
            for x in 0..f.width {
                let dx = f.left + x;
                if dx >= sw {
                    break;
                }
                let Some(&i) = f.indices.get((y * f.width + x) as usize) else {
                    return;
                };
                let c = f.palette[i as usize];
                if c[3] != 0 {
                    dst.set_pixel(dx, dy, c);
                }
            }
        }
    }

    /// `SkCodec::getPixels` with `SKCodecOptions(frame, priorFrame)` over the
    /// prior frame's pixels: a prior frame that restores to the background
    /// clears its rectangle first.
    fn decode_frame(&self, index: usize, prior: Option<usize>, dst: &mut Bitmap) {
        if self.frames[index].required.is_some() {
            if let Some(p) = prior {
                let pf = &self.frames[p];
                if pf.disposal == Disposal::RestoreBackground {
                    let (l, t, r, b) = pf.rect(self.width, self.height);
                    for y in t..b {
                        for x in l..r {
                            dst.set_pixel(x, y, [0, 0, 0, 0]);
                        }
                    }
                }
            }
        }
        self.draw(index, dst);
    }

    /// `SkiaAnimatedImage.GetFrame(index)`: the chain of required frames
    /// decoded in turn into one transparent bitmap.
    pub fn frame(&self, index: usize) -> Bitmap {
        let mut chain = vec![index];
        while let Some(r) = self.frames[*chain.last().unwrap()].required {
            chain.push(r);
        }
        let mut bmp = imaging::create_bitmap(self.width, self.height);
        let mut previous = None;
        for &f in chain.iter().rev() {
            self.decode_frame(f, previous, &mut bmp);
            previous = Some(f);
        }
        bmp
    }
}

// ------------------------------------------------------------ document

/// An integer rectangle (`SKRectI`) in GIF pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// `AnimatedGifTrimmerDocument`: the parsed GIF, its frames and timing.
#[derive(Debug, Clone)]
pub struct AnimatedGifTrimmerDocument {
    pub bytes: Vec<u8>,
    pub gif: GifLosslessTrimmer,
    pub image: AnimatedGif,
    starts: Vec<i64>,
}

/// What `Export` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportResult {
    /// The new GIF's bytes.
    Written(Vec<u8>),
    /// `false`: the lossless cut needs frames before its start (the window
    /// then offers a re-encode).
    NeedsReencode,
}

fn lookup(key: &str, fallback: &str) -> String {
    sharex_strings::tools::lookup(key).unwrap_or(fallback).to_string()
}

impl AnimatedGifTrimmerDocument {
    /// `Open`, from the file's bytes (the caller checks the `.gif` extension
    /// and that the file exists: `AnimatedGifTrimmer_InvalidGif` otherwise).
    pub fn open(bytes: Vec<u8>) -> Result<Self, String> {
        let gif = GifLosslessTrimmer::parse(&bytes)?;
        let image = AnimatedGif::decode(&bytes)?;
        if image.frame_count() != gif.frame_count() {
            return Err(invalid());
        }
        let starts = gif.starts();
        Ok(AnimatedGifTrimmerDocument { bytes, gif, image, starts })
    }

    /// `Open(filePath)`.
    pub fn open_file(path: &str) -> Result<Self, String> {
        let p = std::path::Path::new(path);
        let is_gif = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif"));
        if !p.is_file() || !is_gif {
            return Err(invalid());
        }
        Self::open(std::fs::read(p).map_err(|_| invalid())?)
    }

    pub fn width(&self) -> u32 {
        self.image.width
    }
    pub fn height(&self) -> u32 {
        self.image.height
    }
    pub fn frame_count(&self) -> usize {
        self.gif.frame_count()
    }
    /// `Duration` in seconds.
    pub fn duration(&self) -> f64 {
        *self.starts.last().unwrap() as f64 / 100.0
    }
    pub fn frame_start(&self, index: usize) -> f64 {
        self.starts[index] as f64 / 100.0
    }
    pub fn frame_end(&self, index: usize) -> f64 {
        self.starts[index + 1] as f64 / 100.0
    }
    pub fn frame_at(&self, seconds: f64) -> usize {
        frame_at(&self.starts, seconds)
    }
    pub fn nearest_boundary(&self, seconds: f64, include_end: bool) -> usize {
        nearest_boundary(&self.starts, seconds, include_end)
    }
    pub fn can_copy_selection(&self, first: usize, end_exclusive: usize) -> bool {
        self.gif.can_copy_selection(first, end_exclusive)
    }

    /// `RenderFrame(index, maxDimension)` without the PNG: the frame
    /// shrunk (`SkiaImageHelpers.Resize`) to fit `max_dimension`.
    pub fn render_frame_bitmap(&self, index: usize, max_dimension: u32) -> Bitmap {
        let frame = self.image.frame(index);
        let (w, h) = (frame.width(), frame.height());
        let scale = (max_dimension as f64 / w.max(h) as f64).min(1.0);
        let nw = ((w as f64 * scale).round_ties_even() as u32).max(1);
        let nh = ((h as f64 * scale).round_ties_even() as u32).max(1);
        imaging::resize(&frame, nw, nh)
    }

    /// `RenderFrame(index, token, maxDimension = 640)`: the preview PNG.
    pub fn render_frame(&self, index: usize, max_dimension: u32) -> Vec<u8> {
        imaging::encode_png(&self.render_frame_bitmap(index, max_dimension))
    }

    /// `Export`'s argument checks and the lossless/re-encode choice, on
    /// bytes: `crop` is the area kept, `size` the saved size. `progress`
    /// gets 0-100.
    pub fn export_bytes(
        &self,
        first: usize,
        end_exclusive: usize,
        reencode: bool,
        crop: CropRect,
        size: (i32, i32),
        mut progress: Option<&mut dyn FnMut(f64)>,
    ) -> Result<ExportResult, String> {
        if end_exclusive > self.frame_count() || first >= end_exclusive {
            return Err("Specified argument was out of the range of valid values. (Parameter 'firstFrame')".into());
        }
        let (w, h) = (self.width() as i32, self.height() as i32);
        let full = CropRect { x: 0, y: 0, width: w, height: h };
        let inside = crop.x >= 0 && crop.y >= 0 && crop.x + crop.width <= w && crop.y + crop.height <= h;
        if crop.width <= 0 || crop.height <= 0 || !inside || size.0 < 1 || size.1 < 1 {
            return Err("Specified argument was out of the range of valid values. (Parameter 'crop')".into());
        }
        let transform = crop != full || size != (crop.width, crop.height);
        let reencode = reencode || transform;
        if !reencode && !self.can_copy_selection(first, end_exclusive) {
            return Ok(ExportResult::NeedsReencode);
        }
        if reencode {
            return Ok(ExportResult::Written(self.reencode(first, end_exclusive, crop, size, transform, &mut progress)));
        }
        let out = self.gif.copy(&self.bytes, first, end_exclusive)?;
        if let Some(p) = progress.as_mut() {
            // GifLosslessTrimmer.Copy reports per block; the window shows the end.
            p(100.0);
        }
        if self.gif.needs_visual_validation(first) && !self.first_frame_matches(&out, first) {
            return Ok(ExportResult::NeedsReencode);
        }
        Ok(ExportResult::Written(out))
    }

    /// `Export(output, ...)`: [`Self::export_bytes`] written to `output` through
    /// a temporary file beside it. `Ok(false)` is ShareX's `false` (a
    /// dependent lossless cut).
    #[allow(clippy::too_many_arguments)]
    pub fn export(
        &self,
        source_path: &str,
        output: &str,
        first: usize,
        end_exclusive: usize,
        reencode: bool,
        crop: CropRect,
        size: (i32, i32),
        progress: Option<&mut dyn FnMut(f64)>,
    ) -> Result<bool, String> {
        let full = |p: &str| std::path::absolute(p).map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
        if full(source_path) == full(output) {
            return Err(lookup("AnimatedGifTrimmer_SourceOverwrite", "The output file can't be the source GIF."));
        }
        let out_path = std::path::Path::new(output);
        if !out_path.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif")) {
            return Err(lookup("AnimatedGifTrimmer_OutputExtension", "The output file must be a GIF."));
        }
        let bytes = match self.export_bytes(first, end_exclusive, reencode, crop, size, progress)? {
            ExportResult::Written(b) => b,
            ExportResult::NeedsReencode => return Ok(false),
        };
        let dir = std::path::absolute(out_path)
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temporary = dir.join(format!(".sharex-gif-trim-{nonce:032x}.gif"));
        let result = std::fs::write(&temporary, &bytes).and_then(|_| std::fs::rename(&temporary, out_path));
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|e| e.to_string())?;
        Ok(true)
    }

    /// `Reencode`: each frame decoded, cropped and resized if asked,
    /// quantized adaptively and written by `AnimatedGifCreator` with the
    /// source's delays and loop.
    fn reencode(
        &self,
        first: usize,
        end_exclusive: usize,
        crop: CropRect,
        size: (i32, i32),
        transform: bool,
        progress: &mut Option<&mut dyn FnMut(f64)>,
    ) -> Vec<u8> {
        let mut creator =
            crate::gif::AnimatedGifCreator::new(self.gif.delay(first) * 10, self.gif.repeat_count, self.gif.looping);
        for i in first..end_exclusive {
            let frame = self.image.frame(i);
            let frame = if transform { crop_and_resize(&frame, crop, size) } else { frame };
            let image = imaging::quantize(&frame, GifQuality::Adaptive);
            creator.add_frame(&image, self.gif.delay(i) * 10, 0, 0, true, None);
            if let Some(p) = progress.as_mut() {
                p((i - first + 1) as f64 / (end_exclusive - first) as f64 * 100.0);
            }
        }
        creator.finish()
    }

    /// `FirstFrameMatches`: the cut's first frame equals the source frame.
    fn first_frame_matches(&self, trimmed: &[u8], original: usize) -> bool {
        let Ok(t) = AnimatedGif::decode(trimmed) else {
            return false;
        };
        let a = self.image.frame(original);
        let b = t.frame(0);
        a.width() == b.width() && a.height() == b.height() && a.data() == b.data()
    }
}

/// `AnimatedGifTrimmerViewModel`'s selection without the window: the frame
/// range, the playhead, the crop and the output scale, and the rules tying
/// them together (a crop or resize forces a re-encode).
#[derive(Debug, Clone, PartialEq)]
pub struct TrimSelection {
    pub width: i32,
    pub height: i32,
    pub frame_count: usize,
    pub start_index: usize,
    /// Exclusive.
    pub end_index: usize,
    pub position_index: usize,
    pub crop: CropRect,
    pub scale: f64,
    pub reencode: bool,
}

impl TrimSelection {
    /// After `LoadInputAsync`: everything selected, the whole GIF, scale 1.
    pub fn new(doc: &AnimatedGifTrimmerDocument) -> Self {
        let (w, h) = (doc.width() as i32, doc.height() as i32);
        TrimSelection {
            width: w,
            height: h,
            frame_count: doc.frame_count(),
            start_index: 0,
            end_index: doc.frame_count(),
            position_index: 0,
            crop: CropRect { x: 0, y: 0, width: w, height: h },
            scale: 1.0,
            reencode: false,
        }
    }

    fn full(&self) -> CropRect {
        CropRect { x: 0, y: 0, width: self.width, height: self.height }
    }

    /// `OutputSize`: the crop times the scale, rounded half to even, 1-65535.
    pub fn output_size(&self) -> (i32, i32) {
        let f = |v: i32| ((v as f64 * self.scale).round_ties_even() as i64).clamp(1, 65535) as i32;
        (f(self.crop.width), f(self.crop.height))
    }

    /// `IsTransformed`.
    pub fn is_transformed(&self) -> bool {
        self.crop != self.full() || self.output_size() != (self.crop.width, self.crop.height)
    }

    /// `CanUseLossless` (with the window able to browse).
    pub fn can_use_lossless(&self) -> bool {
        !self.is_transformed()
    }

    /// `CanTrim` (with the window able to edit).
    pub fn can_trim(&self) -> bool {
        self.start_index > 0 || self.end_index < self.frame_count || self.is_transformed()
    }

    /// `CanPlay`.
    pub fn can_play(&self) -> bool {
        self.end_index - self.start_index > 1
    }

    fn transform_changed(&mut self) {
        if self.is_transformed() {
            self.reencode = true;
        }
    }

    /// `SetCrop`: clipped to the GIF; ignored when empty or unchanged.
    pub fn set_crop(&mut self, rect: CropRect) {
        let l = rect.x.max(0);
        let t = rect.y.max(0);
        let r = (rect.x + rect.width).min(self.width);
        let b = (rect.y + rect.height).min(self.height);
        // SKRectI.Intersect leaves the rect alone when they don't intersect.
        let rect = if l < r && t < b { CropRect { x: l, y: t, width: r - l, height: b - t } } else { rect };
        if rect.width < 1 || rect.height < 1 || rect == self.crop {
            return;
        }
        self.crop = rect;
        self.transform_changed();
    }

    fn set_scale(&mut self, scale: f64) {
        if (scale - self.scale).abs() < 1e-9 {
            return;
        }
        self.scale = scale;
        self.transform_changed();
    }

    /// `OutputWidth`'s setter: scales the crop, keeping its aspect ratio.
    pub fn set_output_width(&mut self, value: f64) {
        if value > 0.0 && self.crop.width > 0 {
            self.set_scale(value / self.crop.width as f64);
        }
    }

    /// `OutputHeight`'s setter.
    pub fn set_output_height(&mut self, value: f64) {
        if value > 0.0 && self.crop.height > 0 {
            self.set_scale(value / self.crop.height as f64);
        }
    }

    /// `ResetCrop` (also after a successful export).
    pub fn reset_crop(&mut self) {
        self.scale = 1.0;
        self.crop = self.full();
        self.transform_changed();
    }

    /// `Seek`.
    pub fn seek(&mut self, doc: &AnimatedGifTrimmerDocument, seconds: f64) {
        self.position_index = doc.frame_at(seconds);
    }

    /// `SetStart`: the nearest frame boundary, before the end.
    pub fn set_start(&mut self, doc: &AnimatedGifTrimmerDocument, seconds: f64) {
        let index = doc.nearest_boundary(seconds, false).min(self.end_index - 1);
        if index != self.start_index {
            self.start_index = index;
            self.position_index = index;
        }
    }

    /// `SetEnd`: the nearest boundary (the end included), after the start.
    pub fn set_end(&mut self, doc: &AnimatedGifTrimmerDocument, seconds: f64) {
        let index = doc.nearest_boundary(seconds, true).max(self.start_index + 1);
        if index != self.end_index {
            self.end_index = index;
            self.position_index = (index - 1).min(self.frame_count - 1);
        }
    }

    /// `SetStartTime(text)`: a `TryParseTime` text, or nothing.
    pub fn set_start_time(&mut self, doc: &AnimatedGifTrimmerDocument, text: &str) {
        if let Some(s) = super::trimmer::try_parse_time(text) {
            self.set_start(doc, s);
        }
    }

    /// `SetEndTime(text)`.
    pub fn set_end_time(&mut self, doc: &AnimatedGifTrimmerDocument, text: &str) {
        if let Some(s) = super::trimmer::try_parse_time(text) {
            self.set_end(doc, s);
        }
    }

    /// `StepPosition(frames)`.
    pub fn step_position(&mut self, frames: i32) {
        self.position_index = (self.position_index as i64 + frames as i64).clamp(0, self.frame_count as i64 - 1) as usize;
    }

    /// `NotifySelectionChanged`'s status: `AnimatedGifTrimmer_UseReencode`
    /// when a lossless cut can't start at the selected frame.
    pub fn status_text(&self, doc: &AnimatedGifTrimmerDocument) -> String {
        if self.reencode || doc.can_copy_selection(self.start_index, self.end_index) {
            String::new()
        } else {
            sharex_strings::tools::AnimatedGifTrimmer_UseReencode().to_string()
        }
    }

    /// `FrameText`: `Frame {0} of {1}`.
    pub fn frame_text(&self) -> String {
        crate::text::format_dotnet(
            sharex_strings::tools::AnimatedGifTrimmer_Frame(),
            &[((self.position_index + 1) as i64).into(), (self.frame_count as i64).into()],
        )
    }

    /// The save dialog's suggested name: `<input name>-trimmed.gif`.
    pub fn output_name(input: &str) -> String {
        let name = crate::dotnet::file_name(input);
        let stem = name.rfind('.').map_or(name, |i| &name[..i]);
        format!("{stem}-trimmed.gif")
    }

    /// `ExportAsync`'s call: [`AnimatedGifTrimmerDocument::export`] with this
    /// selection.
    pub fn export(
        &self,
        doc: &AnimatedGifTrimmerDocument,
        source_path: &str,
        output: &str,
        progress: Option<&mut dyn FnMut(f64)>,
    ) -> Result<bool, String> {
        doc.export(source_path, output, self.start_index, self.end_index, self.reencode, self.crop, self.output_size(), progress)
    }
}

/// `CropAndResize`: `DrawImage(image, crop, dest, Mitchell)`.
pub fn crop_and_resize(source: &Bitmap, crop: CropRect, size: (i32, i32)) -> Bitmap {
    let mut out = imaging::create_bitmap(size.0 as u32, size.1 as u32);
    Canvas::new(&mut out).draw_image_rect(
        source,
        Some(Rect::xywh(crop.x as f32, crop.y as f32, crop.width as f32, crop.height as f32)),
        Rect::xywh(0.0, 0.0, size.0 as f32, size.1 as f32),
        Sampling::Mitchell,
        None,
    );
    out
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
