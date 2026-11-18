//! Decodes an MP4 with Media Foundation's source reader (software decoders,
//! no GPU): NV12 frames cropped to the display aperture, 16-bit PCM.

use std::path::Path;

use sharex_native_recorder::windows::ComSession;
use windows::Win32::Media::MediaFoundation::*;
use windows::core::{HSTRING, Interface};

pub struct Frame {
    pub time: i64,
    pub duration: i64,
    pub pixels: Vec<u8>,
}

pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<Frame>,
    pub audio: Vec<i16>,
}

const FIRST_VIDEO: u32 = 0xFFFF_FFFC;
const FIRST_AUDIO: u32 = 0xFFFF_FFFD;
const ALL_STREAMS: u32 = 0xFFFF_FFFE;

fn e(err: windows::core::Error) -> String {
    err.message()
}

pub fn decode(path: &Path) -> Result<Decoded, String> {
    let _session = ComSession::start(true)?;
    // SAFETY: Media Foundation calls on objects created here; buffers are read while locked.
    unsafe {
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None).map_err(e)?;
        reader.SetStreamSelection(ALL_STREAMS, false).map_err(e)?;
        reader.SetStreamSelection(FIRST_VIDEO, true).map_err(e)?;
        let video = MFCreateMediaType().map_err(e)?;
        video.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(e)?;
        video.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12).map_err(e)?;
        reader.SetCurrentMediaType(FIRST_VIDEO, None, &video).map_err(e)?;
        let has_audio = reader.SetStreamSelection(FIRST_AUDIO, true).is_ok();
        if has_audio {
            let audio = MFCreateMediaType().map_err(e)?;
            audio.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(e)?;
            audio.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).map_err(e)?;
            audio.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(e)?;
            reader.SetCurrentMediaType(FIRST_AUDIO, None, &audio).map_err(e)?;
        }
        let current = reader.GetCurrentMediaType(FIRST_VIDEO).map_err(e)?;
        let size = current.GetUINT64(&MF_MT_FRAME_SIZE).map_err(e)?;
        let (fw, fh) = ((size >> 32) as u32, size as u32);
        let (mut w, mut h) = (fw, fh);
        let mut area = [0u8; 16];
        let mut len = 0u32;
        if current.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut area, Some(&mut len)).is_ok() && len == 16 {
            w = u32::from_le_bytes(area[8..12].try_into().unwrap());
            h = u32::from_le_bytes(area[12..16].try_into().unwrap());
        }
        let mut out = Decoded { width: w, height: h, frames: Vec::new(), audio: Vec::new() };
        for (stream, is_video) in [(FIRST_VIDEO, true), (FIRST_AUDIO, false)] {
            if !is_video && !has_audio {
                continue;
            }
            loop {
                let (mut flags, mut time, mut sample) = (0u32, 0i64, None);
                reader.ReadSample(stream, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample)).map_err(e)?;
                if let Some(sample) = sample {
                    let buffer = sample.ConvertToContiguousBuffer().map_err(e)?;
                    if is_video {
                        let duration = sample.GetSampleDuration().unwrap_or(0);
                        let mut pixels = Vec::with_capacity((w * h * 3 / 2) as usize);
                        if let Ok(two) = buffer.cast::<IMF2DBuffer>() {
                            let (mut scan0, mut pitch) = (std::ptr::null_mut(), 0i32);
                            two.Lock2D(&mut scan0, &mut pitch).map_err(e)?;
                            let p = pitch as usize;
                            for y in 0..h as usize {
                                pixels.extend_from_slice(std::slice::from_raw_parts(scan0.add(y * p), w as usize));
                            }
                            for y in 0..h as usize / 2 {
                                pixels.extend_from_slice(std::slice::from_raw_parts(scan0.add(p * fh as usize + y * p), w as usize));
                            }
                            two.Unlock2D().map_err(e)?;
                        } else {
                            let (mut data, mut max, mut cur) = (std::ptr::null_mut(), 0u32, 0u32);
                            buffer.Lock(&mut data, Some(&mut max), Some(&mut cur)).map_err(e)?;
                            let p = fw as usize;
                            for y in 0..h as usize {
                                pixels.extend_from_slice(std::slice::from_raw_parts(data.add(y * p), w as usize));
                            }
                            for y in 0..h as usize / 2 {
                                pixels.extend_from_slice(std::slice::from_raw_parts(data.add(p * fh as usize + y * p), w as usize));
                            }
                            buffer.Unlock().map_err(e)?;
                        }
                        out.frames.push(Frame { time, duration, pixels });
                    } else {
                        let (mut data, mut cur) = (std::ptr::null_mut(), 0u32);
                        buffer.Lock(&mut data, None, Some(&mut cur)).map_err(e)?;
                        let bytes = std::slice::from_raw_parts(data, cur as usize);
                        out.audio.extend(bytes.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])));
                        buffer.Unlock().map_err(e)?;
                    }
                }
                if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                    break;
                }
            }
        }
        Ok(out)
    }
}
