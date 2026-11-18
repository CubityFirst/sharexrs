//! `Audio/AudioMixer.cs`: two continuous stereo streams (system audio and
//! microphone) aligned to the video's QPC timeline. Missing packets become
//! silence; slow endpoint clock drift is followed by resampling with a
//! windowed-sinc filter; the sum is clipped to 16-bit PCM.
//!
//! The arithmetic follows ShareX's types exactly (`float` samples and
//! weights, `double` phase and step, .NET's rounding), so the PCM is the
//! same bytes.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicI64, Ordering};

use crate::clock::RecordingClock;
use crate::options::TICKS_PER_SECOND;

pub const SAMPLE_RATE: i64 = 48_000;
const CAPACITY: usize = (SAMPLE_RATE * 2) as usize;
const FILTER_TAPS: usize = 32;
const FILTER_DELAY: usize = FILTER_TAPS / 2;
const FILTER_PHASES: usize = 128;
/// Ignore sub-42 microsecond packet timestamp variations.
const TIMESTAMP_JITTER: f64 = 2.0;

struct StreamState {
    segment: i64,
    next_frame: i64,
    phase: f64,
    step: f64,
    input: Vec<f32>,
}

impl StreamState {
    fn new() -> Self {
        Self { segment: i64::MIN, next_frame: 0, phase: 0.0, step: 1.0, input: vec![0.0; 1024 * 2] }
    }
}

struct Inner {
    samples: [Vec<f32>; 2],
    positions: [Vec<i64>; 2],
    streams: [StreamState; 2],
    consumed: i64,
}

/// `AudioMixer`.
pub struct AudioMixer {
    clock: Arc<RecordingClock>,
    interpolation: Vec<[f32; FILTER_TAPS]>,
    inner: Mutex<Inner>,
    discontinuities: AtomicI64,
}

impl AudioMixer {
    pub fn new(clock: Arc<RecordingClock>) -> Self {
        Self {
            clock,
            interpolation: create_interpolation(),
            inner: Mutex::new(Inner {
                samples: [vec![0.0; CAPACITY * 2], vec![0.0; CAPACITY * 2]],
                positions: [vec![i64::MIN; CAPACITY], vec![i64::MIN; CAPACITY]],
                streams: [StreamState::new(), StreamState::new()],
                consumed: 0,
            }),
            discontinuities: AtomicI64::new(0),
        }
    }

    pub fn discontinuities(&self) -> i64 {
        self.discontinuities.load(Ordering::SeqCst)
    }

    pub fn mark_discontinuity(&self) {
        self.discontinuities.fetch_add(1, Ordering::SeqCst);
    }

    /// `Write`: a packet of interleaved stereo `float`s (empty = silence)
    /// from `source` (0 system audio, 1 microphone) captured at QPC `timestamp`.
    pub fn write(&self, source: usize, timestamp: i64, frames: i32, packet: &[f32], gain: f32, discontinuity: bool) {
        if frames <= 0 {
            return;
        }
        let (relative, segment, ok) = self.clock.try_map(timestamp);
        if !ok {
            return;
        }
        let frames_us = frames as usize;
        let first = relative as f64 * SAMPLE_RATE as f64 / TICKS_PER_SECOND as f64;
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let inner = &mut *guard;
        let consumed = inner.consumed;
        let stream = &mut inner.streams[source];
        let error = first - (stream.next_frame as f64 - stream.phase / stream.step);
        if stream.segment != segment || discontinuity || error.abs() > SAMPLE_RATE as f64 * 0.05 {
            // Only start, resume, or a genuine capture gap may reposition the stream.
            stream.segment = segment;
            stream.next_frame = first.round_ties_even() as i64;
            stream.phase = 0.0;
            stream.step = 1.0;
            stream.input[..FILTER_TAPS * 2].fill(0.0);
        } else {
            // Follow slow endpoint clock drift by changing the interpolation rate smoothly.
            let correction = (error.abs() - TIMESTAMP_JITTER).max(0.0).copysign(error);
            let target_step = (1.0 - correction / (SAMPLE_RATE * 2) as f64).clamp(0.995, 1.005);
            stream.step += (target_step - stream.step) * (1.0f64).min(frames as f64 / (SAMPLE_RATE as f64 * 0.5));
        }
        if stream.input.len() < (frames_us + FILTER_TAPS) * 2 {
            stream.input.resize((frames_us + FILTER_TAPS) * 2, 0.0);
        }
        {
            let input = &mut stream.input[FILTER_TAPS * 2..FILTER_TAPS * 2 + frames_us * 2];
            if packet.is_empty() {
                input.fill(0.0);
            } else {
                input.copy_from_slice(&packet[..frames_us * 2]);
            }
        }
        let mut phase = stream.phase;
        let limit = (frames - FILTER_DELAY as i32) as f64;
        while phase < limit {
            let index = phase.floor() as i32;
            let fraction = (phase - index as f64) as f32;
            let position = stream.next_frame;
            stream.next_frame += 1;
            if position >= consumed && position < consumed + CAPACITY as i64 {
                let (left, right);
                let mut offset = ((index + FILTER_TAPS as i32) * 2) as usize;
                if fraction == 0.0 {
                    // Preserve the exact original samples when no rate correction is needed.
                    left = stream.input[offset];
                    right = stream.input[offset + 1];
                } else {
                    // A windowed-sinc filter avoids the treble loss of linear interpolation.
                    let filter_phase = fraction * FILTER_PHASES as f32;
                    let filter_index = (filter_phase as i32).min(FILTER_PHASES as i32 - 1) as usize;
                    let blend = filter_phase - filter_index as f32;
                    let first_filter = &self.interpolation[filter_index];
                    let next_filter = &self.interpolation[filter_index + 1];
                    offset -= (FILTER_DELAY - 1) * 2;
                    let (mut l, mut r) = (0.0f32, 0.0f32);
                    for tap in 0..FILTER_TAPS {
                        let weight = first_filter[tap] + (next_filter[tap] - first_filter[tap]) * blend;
                        l += stream.input[offset] * weight;
                        r += stream.input[offset + 1] * weight;
                        offset += 2;
                    }
                    left = l;
                    right = r;
                }
                let slot = (position % CAPACITY as i64) as usize;
                inner.positions[source][slot] = position;
                inner.samples[source][slot * 2] = left * gain;
                inner.samples[source][slot * 2 + 1] = right * gain;
            }
            phase += stream.step;
        }
        // Retain filter history and lookahead across packet boundaries.
        stream.phase = phase - frames as f64;
        stream.input.copy_within(frames_us * 2..frames_us * 2 + FILTER_TAPS * 2, 0);
    }

    /// `Read`: 16-bit stereo PCM from `first_frame` on, both sources summed.
    pub fn read(&self, first_frame: i64, destination: &mut [i16]) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let frames = destination.len() / 2;
        for i in 0..frames {
            let position = first_frame + i as i64;
            let slot = (position % CAPACITY as i64) as usize;
            for channel in 0..2 {
                let mut value = 0.0f32;
                for source in 0..2 {
                    if inner.positions[source][slot] == position {
                        value += inner.samples[source][slot * 2 + channel];
                    }
                }
                // Clipping is deterministic, and NaN from a broken endpoint cannot propagate into PCM.
                destination[i * 2 + channel] =
                    if value.is_finite() { ((value * 32767.0) as i32).clamp(-32768, 32767) as i16 } else { 0 };
            }
        }
        inner.consumed = first_frame + frames as i64;
    }
}

/// `CreateInterpolation`: 129 phases of a 32-tap Blackman-windowed sinc,
/// each normalised to unit gain.
fn create_interpolation() -> Vec<[f32; FILTER_TAPS]> {
    (0..=FILTER_PHASES)
        .map(|phase| {
            let mut filter = [0f32; FILTER_TAPS];
            let mut sum = 0f64;
            for (tap, f) in filter.iter_mut().enumerate() {
                let distance = tap as f64 - (FILTER_DELAY as f64 - 1.0) - phase as f64 / FILTER_PHASES as f64;
                let pi = std::f64::consts::PI;
                let sinc = if distance == 0.0 { 1.0 } else { (pi * distance).sin() / (pi * distance) };
                let window = 0.42 + 0.5 * (pi * distance / FILTER_DELAY as f64).cos()
                    + 0.08 * (2.0 * pi * distance / FILTER_DELAY as f64).cos();
                *f = (sinc * window) as f32;
                sum += *f as f64;
            }
            for f in filter.iter_mut() {
                *f /= sum as f32;
            }
            filter
        })
        .collect()
}

/// Where `write_audio_until` hands each block (`writer.WriteAudio`).
pub type AudioSink<'a> = dyn FnMut(&[i16], i64) -> Result<(), String> + 'a;

/// `ScreenRecorder.WriteAudioUntil`: hands the mixer's PCM to `write` in
/// 10 ms blocks up to media time `until` (and the rest, when `partial`).
pub fn write_audio_until(
    mixer: &AudioMixer,
    block: &mut [i16],
    first_frame: &mut i64,
    until: i64,
    partial: bool,
    write: &mut AudioSink<'_>,
) -> Result<(), String> {
    let end_frame = (until as i128 * SAMPLE_RATE as i128 / TICKS_PER_SECOND as i128) as i64;
    let block_frames = (block.len() / 2) as i64;
    while *first_frame + block_frames <= end_frame || (partial && *first_frame < end_frame) {
        let count = block_frames.min(end_frame - *first_frame) as usize;
        let output = &mut block[..count * 2];
        mixer.read(*first_frame, output);
        write(output, *first_frame)?;
        *first_frame += count as i64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::VirtualTime;

    fn mixer() -> (Arc<VirtualTime>, AudioMixer) {
        let t = VirtualTime::new(0);
        let clock = Arc::new(RecordingClock::new(t.clone()));
        clock.start(0);
        (t, AudioMixer::new(clock))
    }

    fn ticks(frames: i64) -> i64 {
        frames * TICKS_PER_SECOND / SAMPLE_RATE
    }

    #[test]
    fn exact_packets_pass_through() {
        let (_, m) = mixer();
        let packet: Vec<f32> = (0..960).map(|i| (i as f32 / 960.0) - 0.5).collect();
        m.write(0, 0, 480, &packet, 1.0, false);
        m.write(0, ticks(480), 480, &packet, 1.0, false);
        let mut out = vec![0i16; 960];
        m.read(0, &mut out);
        for (i, v) in out.iter().enumerate() {
            assert_eq!(*v, ((packet[i] * 32767.0) as i32) as i16);
        }
    }

    #[test]
    fn sources_sum_and_clip() {
        let (_, m) = mixer();
        let packet = vec![0.75f32; 960];
        m.write(0, 0, 480, &packet, 1.0, false);
        m.write(1, 0, 480, &packet, 1.0, false);
        let mut out = vec![0i16; 2];
        m.read(0, &mut out);
        assert_eq!(out, [32767, 32767]);
        let (_, m) = mixer();
        m.write(0, 0, 480, &vec![f32::NAN; 960], 1.0, false);
        m.read(0, &mut out);
        assert_eq!(out, [0, 0]);
    }

    #[test]
    fn missing_packets_are_silence() {
        let (_, m) = mixer();
        let mut out = vec![1i16; 960];
        m.read(0, &mut out);
        assert!(out.iter().all(|v| *v == 0));
    }

    #[test]
    fn filters_have_unit_gain() {
        for f in create_interpolation() {
            let sum: f32 = f.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn drift_resamples() {
        let (_, m) = mixer();
        // A device clock 0.1% fast: timestamps fall behind the frames delivered.
        let packet: Vec<f32> = (0..960).map(|i| ((i / 2) as f32 * 0.05).sin() * 0.5).collect();
        let mut written = 0i64;
        for _ in 0..200 {
            let ts = ticks(written) * 999 / 1000;
            m.write(0, ts, 480, &packet, 1.0, false);
            written += 480;
        }
        let mut out = vec![0i16; 960];
        m.read(48_000, &mut out);
        assert!(out.iter().any(|v| *v != 0));
    }
}
