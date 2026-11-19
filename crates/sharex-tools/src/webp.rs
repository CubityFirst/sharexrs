//! WebP as ShareX writes it: `SKPixmap.Encode(SKWebpEncoderOptions)` and
//! `SKBitmap.Encode(SKEncodedImageFormat.Webp, quality)`, i.e. Skia's
//! `SkWebpEncoder::Encode` (`src/encode/SkWebpEncoderImpl.cpp`) over the
//! libwebp SkiaSharp 3.119.4 compiles in.
//!
//! libwebp comes from `libwebp-sys` 0.14.4, whose bundled sources are the
//! ones in `libSkiaSharp.dll` file for file
//! (`tools/tools-harness/libwebp-sources.md5`), so the bytes match ShareX's.
//! Skia's encoder, step by step:
//!
//! 1. `WebPConfigPreset(WEBP_PRESET_DEFAULT, quality)`;
//! 2. lossy: `lossless = 0`, `method = 3` (Chrome's choice), `use_argb = 0`
//!    (libwebp converts to YUV 4:2:0 on import); lossless: `lossless = 1`,
//!    `method = 0`, `use_argb = 1`, the quality being the compression effort;
//! 3. premultiplied pixels are read into unpremultiplied BGRA
//!    (`SkPixmap::readPixels`, [`raster::unpremul`]) and imported with
//!    `WebPPictureImportBGRA`;
//! 4. `WebPEncode`; with a colour space, the result goes through `WebPMux`
//!    with Skia's ICC profile for it as an `ICCP` chunk (`VP8X` + `ICCP` +
//!    the image chunks).

use std::ffi::{c_int, c_void};

use libwebp_sys as w;
use sharex_skia::raster::{self, Bitmap};

/// `SkWebpEncoder::Compression`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    Lossy,
    Lossless,
}

/// `SkWebpEncoder::Encode(stream, pixmap, {compression, quality})`. `icc`: the
/// bitmap has a colour space (sRGB, as SkCodec decodes), so Skia's sRGB
/// profile is embedded. `None` when libwebp fails (an empty bitmap, or one
/// over 16383 px), where Skia returns false and ShareX throws.
pub fn encode(bmp: &Bitmap, compression: Compression, quality: f32, icc: bool) -> Option<Vec<u8>> {
    let (width, height) = (bmp.width(), bmp.height());
    if width == 0 || height == 0 {
        return None;
    }
    // Unpremultiplied BGRA, as `readPixels` into kBGRA_8888/kUnpremul.
    let mut bgra = Vec::with_capacity(width as usize * height as usize * 4);
    for i in 0..(width * height) as usize {
        let p = raster::unpremul(bmp.px(i));
        bgra.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
    }
    unsafe {
        let mut config = std::mem::MaybeUninit::<w::WebPConfig>::uninit();
        if w::WebPConfigInitInternal(
            config.as_mut_ptr(),
            w::WebPPreset::WEBP_PRESET_DEFAULT,
            quality,
            w::WEBP_ENCODER_ABI_VERSION as c_int,
        ) == 0
        {
            return None;
        }
        let mut config = config.assume_init();
        let mut pic = std::mem::MaybeUninit::<w::WebPPicture>::uninit();
        if w::WebPPictureInitInternal(pic.as_mut_ptr(), w::WEBP_ENCODER_ABI_VERSION as c_int) == 0 {
            return None;
        }
        let mut pic = Picture(pic.assume_init());
        pic.0.width = width as c_int;
        pic.0.height = height as c_int;
        match compression {
            Compression::Lossy => {
                config.lossless = 0;
                config.method = 3;
                pic.0.use_argb = 0;
            }
            Compression::Lossless => {
                config.lossless = 1;
                config.method = 0;
                pic.0.use_argb = 1;
            }
        }
        if w::WebPPictureImportBGRA(&mut pic.0, bgra.as_ptr(), (width * 4) as c_int) == 0 {
            return None;
        }
        let mut out: Vec<u8> = Vec::new();
        pic.0.writer = Some(write_vec);
        pic.0.custom_ptr = &mut out as *mut Vec<u8> as *mut c_void;
        if w::WebPEncode(&config, &mut pic.0) == 0 {
            return None;
        }
        drop(pic);
        if !icc {
            return Some(out);
        }
        add_icc(&out, crate::jpeg::SKIA_SRGB_ICC)
    }
}

/// `WebPMuxSetImage` + `WebPMuxSetChunk("ICCP")` + `WebPMuxAssemble`.
unsafe fn add_icc(image: &[u8], icc: &[u8]) -> Option<Vec<u8>> {
    unsafe {
        let mux = w::WebPNewInternal(w::WEBP_MUX_ABI_VERSION as c_int);
        if mux.is_null() {
            return None;
        }
        let encoded = w::WebPData { bytes: image.as_ptr(), size: image.len() };
        let chunk = w::WebPData { bytes: icc.as_ptr(), size: icc.len() };
        let mut assembled = w::WebPData { bytes: std::ptr::null(), size: 0 };
        let ok = w::WebPMuxSetImage(mux, &encoded, 0) == w::WebPMuxError::WEBP_MUX_OK
            && w::WebPMuxSetChunk(mux, c"ICCP".as_ptr(), &chunk, 0) == w::WebPMuxError::WEBP_MUX_OK
            && w::WebPMuxAssemble(mux, &mut assembled) == w::WebPMuxError::WEBP_MUX_OK;
        let out = ok.then(|| std::slice::from_raw_parts(assembled.bytes, assembled.size).to_vec());
        if !assembled.bytes.is_null() {
            w::WebPFree(assembled.bytes as *mut c_void);
        }
        w::WebPMuxDelete(mux);
        out
    }
}

/// A `WebPPicture` freed on drop (`SkAutoTCallVProc<WebPPicture, WebPPictureFree>`).
struct Picture(w::WebPPicture);

impl Drop for Picture {
    fn drop(&mut self) {
        unsafe { w::WebPPictureFree(&mut self.0) }
    }
}

/// The picture's writer: appends to the `Vec<u8>` in `custom_ptr`.
unsafe extern "C" fn write_vec(data: *const u8, size: usize, pic: *const w::WebPPicture) -> c_int {
    unsafe {
        let out = &mut *((*pic).custom_ptr as *mut Vec<u8>);
        if size > 0 {
            out.extend_from_slice(std::slice::from_raw_parts(data, size));
        }
    }
    1
}
