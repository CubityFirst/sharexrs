//! `ShareX.HelpersLib/Presentation/AnimatedImagePreview.cs`: an animated
//! image (GIF, WebP) decoded one frame at a time for the main window's
//! preview pane, its thumbnails and the image viewer.
//!
//! GIF frames are Skia's (`SkCodec` over Wuffs: [`AnimatedGif`], the same
//! composition the GIF trimmer uses), decoded on demand as ShareX's
//! `DecodeFrame` does: an independent frame starts from a clear canvas, a
//! frame building on the one already there draws on top of it, anything
//! else rebuilds the canvas from its chain of required frames. WebP frames
//! come from libwebp's own animation decoder (`WebPAnimDecoder`, premultiplied
//! RGBA) where ShareX's SkCodec runs libwebp's demuxer with Skia's blending:
//! the same frames, blended in libwebp's premultiplied row blender instead
//! of Skia's (a level at most on translucent edges).
//!
//! Frames larger than `maxSize` are drawn into a smaller bitmap with
//! `SkiaImageHelpers.HighQualitySampling` (Mitchell), as `NextFrame` does.

use std::path::Path;

use sharex_skia::raster::{Bitmap, Canvas, Rect, Sampling};

use crate::video::gif_trimmer::AnimatedGif;

/// `MinFrameDuration`: browsers treat shorter delays as unset.
pub const MIN_FRAME_DURATION: u32 = 20;
/// `DefaultFrameDuration`.
pub const DEFAULT_FRAME_DURATION: u32 = 100;

/// `CanBeAnimated`: checked by extension, so no file is read.
pub fn can_be_animated(path: &str) -> bool {
    sharex_helpers::file_helpers::check_extension(path, &["gif", "webp"])
}

/// The decoder behind a preview.
enum Source {
    Gif(Box<AnimatedGif>),
    Webp(WebpAnimation),
}

pub struct AnimatedImagePreview {
    source: Source,
    width: u32,
    height: u32,
    frame_count: usize,
    composed: Bitmap,
    scaled: Option<(u32, u32)>,
    decoded_frame: Option<usize>,
}

impl std::fmt::Debug for AnimatedImagePreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AnimatedImagePreview({}x{}, {} frames)", self.width, self.height, self.frame_count)
    }
}

impl AnimatedImagePreview {
    fn new(source: Source, width: u32, height: u32, frame_count: usize, max_size: u32) -> Self {
        // `Math.Min(1, (double)maxSize / Math.Max(width, height))`, the
        // scaled size `Math.Round`ed (to even).
        let scale = (max_size as f64 / width.max(height) as f64).min(1.0);
        let scaled = (scale < 1.0).then(|| {
            let w = ((width as f64 * scale).round_ties_even() as u32).max(1);
            let h = ((height as f64 * scale).round_ties_even() as u32).max(1);
            (w, h)
        });
        Self { source, width, height, frame_count, composed: Bitmap::new(width, height), scaled, decoded_frame: None }
    }

    /// `TryCreate(filePath, maxSize)`: `None` when the file isn't an
    /// animated image with more than one frame.
    pub fn try_create(path: &Path, max_size: u32) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        Self::from_bytes(bytes, max_size)
    }

    /// [`Self::try_create`] over the file's bytes (the codec sniffs the format).
    pub fn from_bytes(bytes: Vec<u8>, max_size: u32) -> Option<Self> {
        if bytes.starts_with(b"GIF8") {
            let gif = AnimatedGif::decode(&bytes).ok()?;
            let n = gif.frame_count();
            (n > 1).then(|| {
                let (w, h) = (gif.width, gif.height);
                Self::new(Source::Gif(Box::new(gif)), w, h, n, max_size)
            })
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            let webp = WebpAnimation::new(bytes)?;
            let (w, h, n) = (webp.width, webp.height, webp.frame_count);
            (n > 1).then(|| Self::new(Source::Webp(webp), w, h, n, max_size))
        } else {
            None
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// The size of the frames [`Self::next_frame`] returns.
    pub fn output_size(&self) -> (u32, u32) {
        self.scaled.unwrap_or((self.width, self.height))
    }

    /// `NextFrame`: the next frame (looping), premultiplied, and how long
    /// it should be shown in milliseconds.
    pub fn next_frame(&mut self) -> Result<(Bitmap, u32), String> {
        let index = self.decoded_frame.map_or(0, |d| (d + 1) % self.frame_count);
        let duration = self.decode_frame(index)?;
        let output = match self.scaled {
            Some((w, h)) => {
                let mut out = Bitmap::new(w, h);
                Canvas::new(&mut out).draw_image_rect(
                    &self.composed,
                    None,
                    Rect::xywh(0.0, 0.0, w as f32, h as f32),
                    Sampling::Mitchell,
                    None,
                );
                out
            }
            None => self.composed.clone(),
        };
        Ok((output, if duration < MIN_FRAME_DURATION { DEFAULT_FRAME_DURATION } else { duration }))
    }

    /// `DecodeFrame(index)`; returns the frame's raw duration.
    fn decode_frame(&mut self, index: usize) -> Result<u32, String> {
        let duration = match &mut self.source {
            Source::Gif(gif) => {
                let required = gif.required_frame(index);
                if required.is_none() {
                    // An independent frame: start from a clear canvas.
                    self.composed.data_mut().fill(0);
                    gif.decode_frame(index, None, &mut self.composed);
                } else if required == self.decoded_frame {
                    // The usual case: the frame draws on the canvas's frame.
                    gif.decode_frame(index, self.decoded_frame, &mut self.composed);
                } else {
                    // A "restore previous" disposal: rebuild from the chain.
                    let mut chain = vec![index];
                    while let Some(r) = gif.required_frame(*chain.last().unwrap()) {
                        chain.push(r);
                    }
                    self.composed.data_mut().fill(0);
                    let mut prior = None;
                    for &f in chain.iter().rev() {
                        gif.decode_frame(f, prior, &mut self.composed);
                        prior = Some(f);
                    }
                }
                gif.duration_ms(index)
            }
            Source::Webp(webp) => webp.frame(index, &mut self.composed)?,
        };
        self.decoded_frame = Some(index);
        Ok(duration)
    }
}

/// libwebp's animation decoder over the file's bytes.
struct WebpAnimation {
    data: Vec<u8>,
    decoder: *mut libwebp_sys::WebPAnimDecoder,
    width: u32,
    height: u32,
    frame_count: usize,
    /// The index of the frame `GetNext` gives next, and the previous
    /// frame's timestamp (durations are the differences).
    next: usize,
    last_timestamp: i32,
}

// The decoder is only ever used by its owner.
unsafe impl Send for WebpAnimation {}

impl WebpAnimation {
    fn new(data: Vec<u8>) -> Option<Self> {
        use libwebp_sys as w;
        unsafe {
            let mut options = std::mem::MaybeUninit::<w::WebPAnimDecoderOptions>::zeroed();
            if w::WebPAnimDecoderOptionsInit(options.as_mut_ptr()) == 0 {
                return None;
            }
            let mut options = options.assume_init();
            options.color_mode = w::WEBP_CSP_MODE::MODE_rgbA;
            options.use_threads = 0;
            let mut s = Self { data, decoder: std::ptr::null_mut(), width: 0, height: 0, frame_count: 0, next: 0, last_timestamp: 0 };
            let webp_data = w::WebPData { bytes: s.data.as_ptr(), size: s.data.len() };
            s.decoder = w::WebPAnimDecoderNew(&webp_data, &options);
            if s.decoder.is_null() {
                return None;
            }
            let mut info = std::mem::MaybeUninit::<w::WebPAnimInfo>::zeroed();
            if w::WebPAnimDecoderGetInfo(s.decoder, info.as_mut_ptr()) == 0 {
                return None;
            }
            let info = info.assume_init();
            s.width = info.canvas_width;
            s.height = info.canvas_height;
            s.frame_count = info.frame_count as usize;
            Some(s)
        }
    }

    /// Composes frame `index` into `dst` (the canvas's size); returns its
    /// duration. Frames come in order; going back starts over.
    fn frame(&mut self, index: usize, dst: &mut Bitmap) -> Result<u32, String> {
        use libwebp_sys as w;
        unsafe {
            if index < self.next {
                w::WebPAnimDecoderReset(self.decoder);
                self.next = 0;
                self.last_timestamp = 0;
            }
            loop {
                let mut buf: *mut u8 = std::ptr::null_mut();
                let mut timestamp = 0;
                if w::WebPAnimDecoderGetNext(self.decoder, &mut buf, &mut timestamp) == 0 || buf.is_null() {
                    return Err(format!("Frame {index} decoding failed."));
                }
                let duration = (timestamp - self.last_timestamp).max(0) as u32;
                self.last_timestamp = timestamp;
                let current = self.next;
                self.next += 1;
                if current == index {
                    let len = self.width as usize * self.height as usize * 4;
                    dst.data_mut().copy_from_slice(std::slice::from_raw_parts(buf, len));
                    return Ok(duration);
                }
            }
        }
    }
}

impl Drop for WebpAnimation {
    fn drop(&mut self) {
        if !self.decoder.is_null() {
            unsafe { libwebp_sys::WebPAnimDecoderDelete(self.decoder) };
        }
    }
}

/// `PlayAsync`'s clock: when the next frame is due, in milliseconds since
/// playback started (`nextFrameTime = Math.Max(nextFrameTime + duration,
/// clock.ElapsedMilliseconds)`).
pub fn next_frame_time(previous: u64, duration: u32, elapsed: u64) -> u64 {
    (previous + duration as u64).max(elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GIF of `colours.len()` frames, `w`x`h`, each filled with one colour
    /// and shown for `delay` hundredths of a second.
    pub(crate) fn gif(w: u16, h: u16, colours: &[[u8; 3]], delay: u16) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let palette: Vec<u8> = colours.iter().flatten().copied().collect();
            let mut enc = ::gif::Encoder::new(&mut out, w, h, &palette).unwrap();
            enc.set_repeat(::gif::Repeat::Infinite).unwrap();
            for i in 0..colours.len() {
                let f = ::gif::Frame {
                    width: w,
                    height: h,
                    delay,
                    buffer: std::borrow::Cow::Owned(vec![i as u8; w as usize * h as usize]),
                    ..Default::default()
                };
                enc.write_frame(&f).unwrap();
            }
        }
        out
    }

    fn webp(w: u32, h: u32, colours: &[[u8; 4]], duration: i32) -> Vec<u8> {
        use libwebp_sys as wp;
        unsafe {
            let mut opts = std::mem::MaybeUninit::<wp::WebPAnimEncoderOptions>::zeroed();
            assert!(wp::WebPAnimEncoderOptionsInitInternal(opts.as_mut_ptr(), wp::WEBP_MUX_ABI_VERSION as i32) != 0);
            let enc = wp::WebPAnimEncoderNewInternal(w as i32, h as i32, opts.as_ptr(), wp::WEBP_MUX_ABI_VERSION as i32);
            let mut config = std::mem::MaybeUninit::<wp::WebPConfig>::zeroed();
            assert!(wp::WebPConfigInitInternal(config.as_mut_ptr(), wp::WebPPreset::WEBP_PRESET_DEFAULT, 75.0, wp::WEBP_ENCODER_ABI_VERSION as i32) != 0);
            let mut config = config.assume_init();
            config.lossless = 1;
            let mut t = 0;
            for c in colours {
                let mut pic = std::mem::MaybeUninit::<wp::WebPPicture>::zeroed();
                assert!(wp::WebPPictureInit(pic.as_mut_ptr()));
                let mut pic = pic.assume_init();
                pic.use_argb = 1;
                pic.width = w as i32;
                pic.height = h as i32;
                let rgba: Vec<u8> = (0..w * h).flat_map(|_| *c).collect();
                assert!(wp::WebPPictureImportRGBA(&mut pic, rgba.as_ptr(), (w * 4) as i32) != 0);
                assert!(wp::WebPAnimEncoderAdd(enc, &mut pic, t, &config) != 0);
                wp::WebPPictureFree(&mut pic);
                t += duration;
            }
            assert!(wp::WebPAnimEncoderAdd(enc, std::ptr::null_mut(), t, &config) != 0);
            let mut data = std::mem::MaybeUninit::<wp::WebPData>::zeroed();
            assert!(wp::WebPAnimEncoderAssemble(enc, data.as_mut_ptr()) != 0);
            let data = data.assume_init();
            let out = std::slice::from_raw_parts(data.bytes, data.size).to_vec();
            wp::WebPFree(data.bytes as *mut _);
            wp::WebPAnimEncoderDelete(enc);
            out
        }
    }

    #[test]
    fn only_gif_and_webp_can_be_animated() {
        assert!(can_be_animated("a.gif"));
        assert!(can_be_animated("C:/x/B.WEBP"));
        assert!(!can_be_animated("a.png"));
        assert!(!can_be_animated("gif"));
    }

    #[test]
    fn a_gif_plays_its_frames_in_turn_and_loops() {
        let bytes = gif(4, 3, &[[255, 0, 0], [0, 255, 0], [0, 0, 255]], 5);
        let mut a = AnimatedImagePreview::from_bytes(bytes, 1024).unwrap();
        assert_eq!(a.frame_count(), 3);
        assert_eq!(a.output_size(), (4, 3));
        let expected = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 0, 0, 255]];
        for e in expected {
            let (frame, duration) = a.next_frame().unwrap();
            assert_eq!(frame.pixel(1, 1), e);
            assert_eq!(duration, 50);
        }
    }

    #[test]
    fn short_or_missing_delays_play_at_100_ms() {
        // 1/100 s is under MinFrameDuration, 0 is unset: both become 100 ms.
        for delay in [0, 1] {
            let mut a = AnimatedImagePreview::from_bytes(gif(2, 2, &[[0, 0, 0], [9, 9, 9]], delay), 64).unwrap();
            assert_eq!(a.next_frame().unwrap().1, DEFAULT_FRAME_DURATION);
        }
        // 2/100 s is MinFrameDuration itself: kept.
        let mut a = AnimatedImagePreview::from_bytes(gif(2, 2, &[[0, 0, 0], [9, 9, 9]], 2), 64).unwrap();
        assert_eq!(a.next_frame().unwrap().1, 20);
    }

    #[test]
    fn a_still_image_is_not_animated() {
        assert!(AnimatedImagePreview::from_bytes(gif(2, 2, &[[1, 2, 3]], 10), 64).is_none());
        assert!(AnimatedImagePreview::from_bytes(b"\x89PNG\r\n\x1a\n".to_vec(), 64).is_none());
        assert!(AnimatedImagePreview::from_bytes(webp(2, 2, &[[1, 2, 3, 255]], 100), 64).is_none());
    }

    #[test]
    fn large_frames_are_scaled_to_the_maximum_size() {
        // 300x100 at 128: scale 0.42666, 128 x 42.666 -> 43.
        let mut a = AnimatedImagePreview::from_bytes(gif(300, 100, &[[10, 20, 30], [40, 50, 60]], 10), 128).unwrap();
        assert_eq!(a.output_size(), (128, 43));
        let (frame, _) = a.next_frame().unwrap();
        assert_eq!((frame.width(), frame.height()), (128, 43));
        assert_eq!(frame.pixel(64, 20), [10, 20, 30, 255]);
    }

    #[test]
    fn an_animated_webp_plays_with_its_durations() {
        let bytes = webp(3, 2, &[[255, 0, 0, 255], [0, 0, 255, 255], [0, 255, 0, 255]], 70);
        let mut a = AnimatedImagePreview::from_bytes(bytes, 1024).unwrap();
        assert_eq!(a.frame_count(), 3);
        let expected = [[255, 0, 0, 255], [0, 0, 255, 255], [0, 255, 0, 255], [255, 0, 0, 255], [0, 0, 255, 255]];
        for e in expected {
            let (frame, duration) = a.next_frame().unwrap();
            assert_eq!(frame.pixel(1, 1), e);
            assert_eq!(duration, 70);
        }
    }

    #[test]
    fn the_clock_never_falls_behind_the_elapsed_time() {
        assert_eq!(next_frame_time(0, 100, 0), 100);
        assert_eq!(next_frame_time(100, 100, 150), 200);
        // A slow decode: the next frame is due now, not in the past.
        assert_eq!(next_frame_time(200, 100, 450), 450);
    }
}
