//! `Audio/WasapiCapture.cs` without the devices: what ShareX does with each
//! captured packet (`Drain`: discontinuities, timestamp errors, silent
//! packets, the endpoint's gain, into the [`AudioMixer`]) and a capture
//! thread over [`AudioEndpoint`]s. The WASAPI endpoints themselves (shared
//! mode, loopback in a hidden session, 48 kHz stereo float with Windows'
//! own conversion, the event-driven MMCSS thread) are
//! `windows::wasapi`; [`SyntheticEndpoint`] generates sound in their place
//! for tests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::clock::TimeSource;
use crate::mixer::{AudioMixer, SAMPLE_RATE};
use crate::options::{RecordingOptions, TICKS_PER_SECOND};

/// `AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY`.
pub const FLAG_DISCONTINUITY: u32 = 1;
/// `AUDCLNT_BUFFERFLAGS_SILENT`: the buffer must not be read.
pub const FLAG_SILENT: u32 = 2;
/// `AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR`.
pub const FLAG_TIMESTAMP_ERROR: u32 = 4;

/// Which mixer stream an endpoint feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceKind {
    /// Loopback of an output device (stream 0).
    SystemAudio,
    /// A capture device (stream 1).
    Microphone,
}

impl AudioSourceKind {
    pub fn index(self) -> usize {
        match self {
            Self::SystemAudio => 0,
            Self::Microphone => 1,
        }
    }
}

/// One packet from `IAudioCaptureClient::GetBuffer`: 48 kHz interleaved
/// stereo `float` (the format ShareX asks Windows to convert to).
#[derive(Debug, Clone, Copy)]
pub struct CapturePacket<'a> {
    pub frames: u32,
    pub flags: u32,
    /// `QPCPosition`, in 100 ns.
    pub qpc_position: u64,
    pub data: &'a [f32],
}

/// A capture endpoint (`WasapiCapture.Endpoint`).
pub trait AudioEndpoint: Send {
    fn kind(&self) -> AudioSourceKind;
    fn gain(&self) -> f32;
    /// `IAudioClient::Start`.
    fn start(&mut self) -> Result<(), String>;
    /// The packets waiting (`GetNextPacketSize`/`GetBuffer`/`ReleaseBuffer`), each handed to `sink`.
    fn drain(&mut self, sink: &mut dyn FnMut(CapturePacket<'_>)) -> Result<(), String>;
}

/// `WasapiCapture.Open`: the endpoints a recording asks for.
pub trait AudioDevices: Send + Sync {
    fn open(&self, kind: AudioSourceKind, device_id: Option<&str>, gain: f32) -> Result<Box<dyn AudioEndpoint>, String>;
    /// `new WasapiCapture(options, mixer)` when the devices run their own
    /// capture thread (WASAPI's event-driven one); `None` (the default):
    /// an [`EndpointCapture`] over [`Self::open`]'s endpoints.
    fn capture(&self, _options: &RecordingOptions, _mixer: Arc<AudioMixer>, _time: Arc<dyn TimeSource>) -> Option<Result<Box<dyn AudioCapture>, String>> {
        None
    }
}

/// Per-endpoint state of `Drain`.
#[derive(Debug, Default, Clone, Copy)]
pub struct DrainState {
    pub next_packet_time: i64,
}

/// `Drain`'s handling of one packet.
pub fn drain_packet(state: &mut DrainState, mixer: &AudioMixer, source: usize, gain: f32, packet: CapturePacket<'_>, now: i64) {
    let flags = packet.flags;
    let frames = packet.frames as i64;
    if flags & FLAG_DISCONTINUITY != 0 {
        mixer.mark_discontinuity();
    }
    let packet_time = if flags & FLAG_TIMESTAMP_ERROR == 0 {
        packet.qpc_position as i64
    } else if state.next_packet_time != 0 {
        state.next_packet_time
    } else {
        now - frames * TICKS_PER_SECOND / SAMPLE_RATE
    };
    if flags & FLAG_TIMESTAMP_ERROR != 0 {
        mixer.mark_discontinuity();
    }
    state.next_packet_time = packet_time + frames * TICKS_PER_SECOND / SAMPLE_RATE;
    // Silent packets still advance the sample clock; their buffer must not be read.
    let data: &[f32] = if flags & FLAG_SILENT == 0 { &packet.data[..packet.frames as usize * 2] } else { &[] };
    mixer.write(source, packet_time, packet.frames as i32, data, gain, flags & FLAG_DISCONTINUITY != 0);
}

/// The running capture (`WasapiCapture`'s lifetime as the recorder sees it).
pub trait AudioCapture: Send {
    fn start(&mut self);
    /// `Failure`.
    fn failure(&self) -> Option<String>;
    /// Drains on the caller's thread when there is no capture thread (virtual time).
    fn poll(&mut self) {}
    /// `Dispose`: stops capturing after a last drain.
    fn stop(&mut self);
}

struct Endpoint {
    inner: Box<dyn AudioEndpoint>,
    state: DrainState,
}

/// `WasapiCapture` over [`AudioEndpoint`]s: opened up front (so errors
/// reach `PrepareAsync`), started with the recording, drained every 10 ms
/// on its own thread (ShareX waits for WASAPI's buffer events, at most 100 ms).
pub struct EndpointCapture {
    endpoints: Arc<Mutex<Vec<Endpoint>>>,
    mixer: Arc<AudioMixer>,
    time: Arc<dyn TimeSource>,
    threaded: bool,
    started: bool,
    stop: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl EndpointCapture {
    /// The system audio and/or microphone endpoints `options` ask for.
    pub fn open(devices: &dyn AudioDevices, options: &RecordingOptions, mixer: Arc<AudioMixer>, time: Arc<dyn TimeSource>, threaded: bool) -> Result<Self, String> {
        let mut endpoints = Vec::new();
        if options.capture_system_audio {
            endpoints.push(devices.open(AudioSourceKind::SystemAudio, options.system_audio_device_id.as_deref().filter(|s| !s.trim().is_empty()), options.system_audio_gain)?);
        }
        if options.capture_microphone {
            endpoints.push(devices.open(AudioSourceKind::Microphone, options.microphone_device_id.as_deref().filter(|s| !s.trim().is_empty()), options.microphone_gain)?);
        }
        Ok(Self {
            endpoints: Arc::new(Mutex::new(endpoints.into_iter().map(|inner| Endpoint { inner, state: DrainState::default() }).collect())),
            mixer,
            time,
            threaded,
            started: false,
            stop: Arc::new(AtomicBool::new(false)),
            failure: Arc::new(Mutex::new(None)),
            thread: None,
        })
    }

    fn drain_all(endpoints: &Mutex<Vec<Endpoint>>, mixer: &AudioMixer, time: &dyn TimeSource) -> Result<(), String> {
        let mut eps = endpoints.lock().unwrap_or_else(|e| e.into_inner());
        for e in eps.iter_mut() {
            let (source, gain) = (e.inner.kind().index(), e.inner.gain());
            let state = &mut e.state;
            e.inner.drain(&mut |p| drain_packet(state, mixer, source, gain, p, time.now()))?;
        }
        Ok(())
    }
}

impl AudioCapture for EndpointCapture {
    fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        let started = {
            let mut eps = self.endpoints.lock().unwrap_or_else(|e| e.into_inner());
            eps.iter_mut().try_for_each(|e| e.inner.start())
        };
        if let Err(e) = started {
            *self.failure.lock().unwrap_or_else(|p| p.into_inner()) = Some(e);
            return;
        }
        if !self.threaded {
            return;
        }
        let (endpoints, mixer, time, stop, failure) = (self.endpoints.clone(), self.mixer.clone(), self.time.clone(), self.stop.clone(), self.failure.clone());
        self.thread = std::thread::Builder::new()
            .name("ShareX WASAPI capture".into())
            .spawn(move || {
                loop {
                    let stopping = stop.load(Ordering::SeqCst);
                    if let Err(e) = Self::drain_all(&endpoints, &mixer, time.as_ref()) {
                        *failure.lock().unwrap_or_else(|p| p.into_inner()) = Some(e);
                        return;
                    }
                    if stopping {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            })
            .ok();
    }

    fn failure(&self) -> Option<String> {
        self.failure.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn poll(&mut self) {
        if self.started
            && !self.threaded
            && self.failure().is_none()
            && let Err(e) = Self::drain_all(&self.endpoints, &self.mixer, self.time.as_ref())
        {
            *self.failure.lock().unwrap_or_else(|p| p.into_inner()) = Some(e);
        }
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        } else {
            self.poll();
        }
    }
}

impl Drop for EndpointCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// A generated endpoint: a tone (system audio) or a chirp (microphone) in
/// 10 ms packets stamped on the [`TimeSource`], as a device would deliver
/// them; scriptable flags and clock drift for tests.
pub struct SyntheticEndpoint {
    kind: AudioSourceKind,
    gain: f32,
    time: Arc<dyn TimeSource>,
    started_at: Option<i64>,
    delivered: u64,
    /// Parts per million the device's clock runs fast.
    pub drift_ppm: i64,
    /// Flags for packet `n` (by index).
    pub flags: Option<Box<dyn Fn(u64) -> u32 + Send>>,
}

impl SyntheticEndpoint {
    pub const PACKET_FRAMES: u64 = 480;

    pub fn new(kind: AudioSourceKind, gain: f32, time: Arc<dyn TimeSource>) -> Self {
        Self { kind, gain, time, started_at: None, delivered: 0, drift_ppm: 0, flags: None }
    }

    /// Sample `frame` of the signal (left, right).
    pub fn signal(kind: AudioSourceKind, frame: u64) -> (f32, f32) {
        let t = frame as f64 / SAMPLE_RATE as f64;
        match kind {
            AudioSourceKind::SystemAudio => {
                let a = (2.0 * std::f64::consts::PI * 440.0 * t).sin() * 0.4;
                let b = (2.0 * std::f64::consts::PI * 660.0 * t).sin() * 0.3;
                (a as f32, b as f32)
            }
            AudioSourceKind::Microphone => {
                let f = 200.0 + 400.0 * (t % 1.0);
                let v = (2.0 * std::f64::consts::PI * f * t).sin() * 0.25;
                (v as f32, v as f32)
            }
        }
    }
}

impl AudioEndpoint for SyntheticEndpoint {
    fn kind(&self) -> AudioSourceKind {
        self.kind
    }
    fn gain(&self) -> f32 {
        self.gain
    }
    fn start(&mut self) -> Result<(), String> {
        self.started_at = Some(self.time.now());
        Ok(())
    }
    fn drain(&mut self, sink: &mut dyn FnMut(CapturePacket<'_>)) -> Result<(), String> {
        let Some(start) = self.started_at else { return Ok(()) };
        let now = self.time.now();
        loop {
            let first = self.delivered;
            let end = first + Self::PACKET_FRAMES;
            // The device's own clock: `drift_ppm` fast means packets arrive (and are stamped) early.
            let device_ticks = |frames: u64| (frames as i64 * TICKS_PER_SECOND / SAMPLE_RATE) * 1_000_000 / (1_000_000 + self.drift_ppm);
            if start + device_ticks(end) > now {
                return Ok(());
            }
            let mut data = vec![0f32; Self::PACKET_FRAMES as usize * 2];
            for i in 0..Self::PACKET_FRAMES {
                let (l, r) = Self::signal(self.kind, first + i);
                data[i as usize * 2] = l;
                data[i as usize * 2 + 1] = r;
            }
            let flags = self.flags.as_ref().map_or(0, |f| f(first / Self::PACKET_FRAMES));
            sink(CapturePacket { frames: Self::PACKET_FRAMES as u32, flags, qpc_position: (start + device_ticks(first)) as u64, data: &data });
            self.delivered = end;
        }
    }
}

/// [`AudioDevices`] that open [`SyntheticEndpoint`]s.
pub struct SyntheticAudioDevices {
    pub time: Arc<dyn TimeSource>,
    pub drift_ppm: i64,
}

impl AudioDevices for SyntheticAudioDevices {
    fn open(&self, kind: AudioSourceKind, _device_id: Option<&str>, gain: f32) -> Result<Box<dyn AudioEndpoint>, String> {
        let mut e = SyntheticEndpoint::new(kind, gain, self.time.clone());
        e.drift_ppm = self.drift_ppm;
        Ok(Box::new(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{RecordingClock, VirtualTime};

    #[test]
    fn packet_flags() {
        let t = VirtualTime::new(TICKS_PER_SECOND);
        let clock = Arc::new(RecordingClock::new(t.clone()));
        clock.start(TICKS_PER_SECOND);
        let mixer = AudioMixer::new(clock);
        let mut state = DrainState::default();
        let data = vec![0.5f32; 960];
        let p = |flags, qpc| CapturePacket { frames: 480, flags, qpc_position: qpc, data: &data };
        drain_packet(&mut state, &mixer, 0, 1.0, p(0, TICKS_PER_SECOND as u64), 0);
        assert_eq!(state.next_packet_time, TICKS_PER_SECOND + 100_000);
        // A timestamp error continues from the last packet and counts as a discontinuity.
        drain_packet(&mut state, &mixer, 0, 1.0, p(FLAG_TIMESTAMP_ERROR, 0), 0);
        assert_eq!(state.next_packet_time, TICKS_PER_SECOND + 200_000);
        assert_eq!(mixer.discontinuities(), 1);
        drain_packet(&mut state, &mixer, 0, 1.0, p(FLAG_DISCONTINUITY | FLAG_SILENT, (TICKS_PER_SECOND + 200_000) as u64), 0);
        assert_eq!(mixer.discontinuities(), 2);
        let mut out = vec![0i16; 1440 * 2];
        mixer.read(0, &mut out);
        assert_eq!(out[0], 16383);
        assert_eq!(out[480 * 2], 16383);
        // The silent packet is silence.
        assert_eq!(out[1000 * 2], 0);
    }

    #[test]
    fn synthetic_endpoint_paces_packets() {
        let t = VirtualTime::new(0);
        let mut e = SyntheticEndpoint::new(AudioSourceKind::SystemAudio, 1.0, t.clone());
        e.start().unwrap();
        let mut n = 0;
        e.drain(&mut |_| n += 1).unwrap();
        assert_eq!(n, 0);
        t.advance(TICKS_PER_SECOND / 10);
        e.drain(&mut |p| {
            assert_eq!(p.qpc_position as i64 % 100_000, 0);
            n += 1
        })
        .unwrap();
        assert_eq!(n, 10);
    }
}
