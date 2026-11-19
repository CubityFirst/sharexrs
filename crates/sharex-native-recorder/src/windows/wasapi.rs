//! `Audio/WasapiCapture.cs` and `AudioCaptureDevices.cs`: WASAPI shared-mode
//! loopback of the system audio device and microphone capture, on a
//! dedicated event-driven MTA thread with MMCSS "Audio" priority, and the
//! MMDevice endpoint lists.
//!
//! Each endpoint asks Windows for 48 kHz stereo 32-bit float (Windows does
//! the channel mapping and resampling: `AUTOCONVERTPCM` with
//! `SRC_DEFAULT_QUALITY`). Loopback runs in its own hidden audio session,
//! so muting ShareX in the volume mixer doesn't silence the recording; the
//! microphone first tries `IAudioClient3`'s low-latency shared period, then
//! a fresh client at the default period. Packets go through
//! [`drain_packet`] (ShareX's `Drain`) into the mixer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{CloseHandle, HANDLE, RPC_E_CHANGED_MODE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
use windows::Win32::System::Com::{CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateGuid, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, STGM_READ};
use windows::Win32::System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::core::{GUID, HSTRING, w};

use super::err;
use crate::audio::{AudioCapture, AudioDevices, AudioEndpoint, AudioSourceKind, CapturePacket, DrainState, drain_packet};
use crate::clock::TimeSource;
use crate::devices::AudioCaptureDevice;
use crate::mixer::AudioMixer;
use crate::options::RecordingOptions;

const EVENT_CALLBACK: u32 = 0x0004_0000;
const LOOPBACK: u32 = 0x0002_0000;
const AUTO_CONVERT: u32 = 0x8000_0000;
const SRC_QUALITY: u32 = 0x0800_0000;
const SESSION_HIDDEN: u32 = 0x2000_0000;

/// The format ShareX asks every endpoint for: 48 kHz stereo IEEE float.
pub fn capture_format() -> WAVEFORMATEX {
    WAVEFORMATEX { wFormatTag: 3, nChannels: 2, nSamplesPerSec: 48_000, nAvgBytesPerSec: 384_000, nBlockAlign: 8, wBitsPerSample: 32, cbSize: 0 }
}

/// `Initialize`'s stream flags for a loopback or capture endpoint.
pub fn stream_flags(loopback: bool) -> u32 {
    EVENT_CALLBACK | AUTO_CONVERT | SRC_QUALITY | if loopback { LOOPBACK | SESSION_HIDDEN } else { 0 }
}

/// The message when Windows has no default device of a kind.
pub fn no_default_device_message(loopback: bool) -> &'static str {
    if loopback {
        "Windows has no available default system audio output device."
    } else {
        "Windows has no available default microphone. Connect or enable a microphone, or turn off microphone recording."
    }
}

/// An owned Win32 event.
struct Event(HANDLE);

impl Event {
    /// An auto-reset event (`AutoResetEvent`) or a manual-reset one (`ManualResetEvent`).
    fn new(manual: bool) -> Result<Self, String> {
        // SAFETY: creates an unnamed event, closed in `drop`.
        unsafe { CreateEventW(None, manual, false, None) }.map(Self).map_err(err)
    }
    fn set(&self) {
        // SAFETY: a valid event handle.
        let _ = unsafe { SetEvent(self.0) };
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: closes the handle `new` created.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

// SAFETY: event handles may be used from any thread.
unsafe impl Send for Event {}
unsafe impl Sync for Event {}

/// `WaitHandle.WaitAny`: the index of the signalled event, or `None` on a timeout.
fn wait_any(events: &[HANDLE], timeout_ms: u32) -> Option<usize> {
    // SAFETY: valid handles.
    let r = unsafe { WaitForMultipleObjects(events, false, timeout_ms) };
    let i = r.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
    (i < events.len()).then_some(i)
}

/// `CoInitializeEx` (MTA) for the current thread; tolerates an STA thread
/// (`RPC_E_CHANGED_MODE`, a UI thread) as the device lists do.
struct Com(bool);

impl Com {
    fn init(tolerate_sta: bool) -> Result<Self, String> {
        // SAFETY: paired with `CoUninitialize` in `drop` when it succeeded.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Self(true))
        } else if tolerate_sta && hr == RPC_E_CHANGED_MODE {
            Ok(Self(false))
        } else {
            Err(err(hr.into()))
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: pairs with the successful `CoInitializeEx`.
            unsafe { CoUninitialize() };
        }
    }
}

fn enumerator() -> Result<IMMDeviceEnumerator, String> {
    // SAFETY: creates the MMDevice enumerator on a COM thread.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(err)
}

/// `GetDefaultAudioEndpoint` with ShareX's roles: multimedia for output,
/// communications (else multimedia) for input.
fn default_endpoint(enumerator: &IMMDeviceEnumerator, loopback: bool) -> windows::core::Result<IMMDevice> {
    // SAFETY: COM calls on a live enumerator.
    unsafe {
        if loopback {
            enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)
        } else {
            enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications).or_else(|_| enumerator.GetDefaultAudioEndpoint(eCapture, eMultimedia))
        }
    }
}

/// `IMMDevice.Id` and `FriendlyName`.
fn describe(device: &IMMDevice) -> windows::core::Result<AudioCaptureDevice> {
    // SAFETY: COM calls on a live device; the ID string is freed with CoTaskMemFree.
    unsafe {
        let raw = device.GetId()?;
        let id = raw.to_string();
        CoTaskMemFree(Some(raw.0 as *const _));
        let store = device.OpenPropertyStore(STGM_READ)?;
        let mut value = store.GetValue(&PKEY_Device_FriendlyName)?;
        let name = value.to_string();
        let _ = PropVariantClear(&mut value as *mut _ as *mut _);
        Ok(AudioCaptureDevice { id: id.map_err(|_| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?, name })
    }
}

/// `GetDevices`: the active endpoints of a direction; endpoints that vanish
/// while being read are skipped.
pub fn devices(render: bool) -> Result<Vec<AudioCaptureDevice>, String> {
    let _com = Com::init(true)?;
    let enumerator = enumerator()?;
    // SAFETY: COM calls on a live enumerator and collection.
    unsafe {
        let collection = enumerator.EnumAudioEndpoints(if render { eRender } else { eCapture }, DEVICE_STATE_ACTIVE).map_err(err)?;
        let count = collection.GetCount().map_err(err)?;
        let mut out = Vec::new();
        for i in 0..count {
            if let Ok(d) = collection.Item(i).and_then(|device| describe(&device)) {
                out.push(d);
            }
        }
        Ok(out)
    }
}

/// `GetDefaultDevice`: the default endpoint, `None` when there isn't one.
pub fn default_device(render: bool) -> Option<AudioCaptureDevice> {
    let _com = Com::init(true).ok()?;
    let enumerator = enumerator().ok()?;
    default_endpoint(&enumerator, render).and_then(|d| describe(&d)).ok()
}

/// A WASAPI capture endpoint (`WasapiCapture.Endpoint`).
pub struct WasapiEndpoint {
    client: IAudioClient3,
    capture: IAudioCaptureClient,
    ready: Event,
    kind: AudioSourceKind,
    gain: f32,
    started: bool,
}

// SAFETY: free-threaded (MTA) COM objects, used by one thread at a time.
unsafe impl Send for WasapiEndpoint {}

fn activate(device: &IMMDevice) -> Result<IAudioClient3, String> {
    // SAFETY: activates IAudioClient3 on a live device (CLSCTX_INPROC_SERVER).
    unsafe { device.Activate::<IAudioClient3>(CLSCTX_INPROC_SERVER, None) }.map_err(err)
}

impl WasapiEndpoint {
    /// `Open`: the endpoint (`id`, else the default), initialised and event driven.
    pub fn open(enumerator: &IMMDeviceEnumerator, kind: AudioSourceKind, id: Option<&str>, gain: f32) -> Result<Self, String> {
        let loopback = kind == AudioSourceKind::SystemAudio;
        let device = match id.filter(|s| !s.trim().is_empty()) {
            None => default_endpoint(enumerator, loopback).map_err(|_| no_default_device_message(loopback).to_string())?,
            // SAFETY: COM call on a live enumerator.
            Some(id) => unsafe { enumerator.GetDevice(&HSTRING::from(id)) }.map_err(err)?,
        };
        let mut client = activate(&device)?;
        let ready = Event::new(false)?;
        let format = capture_format();
        let flags = stream_flags(loopback);
        // SAFETY: COM calls with a valid format and out pointers; the session GUID outlives the call.
        unsafe {
            let session: GUID = if loopback { CoCreateGuid().map_err(err)? } else { GUID::zeroed() };
            let mut initialized = false;
            if !loopback {
                let (mut default, mut fundamental, mut minimum, mut maximum) = (0, 0, 0, 0);
                if client.GetSharedModeEnginePeriod(&format, &mut default, &mut fundamental, &mut minimum, &mut maximum).is_ok() {
                    // IAudioClient3 accepts EVENTCALLBACK only, not AUTOCONVERTPCM or SRC_DEFAULT_QUALITY.
                    initialized = client.InitializeSharedAudioStream(EVENT_CALLBACK, minimum, &format, None).is_ok();
                    if !initialized {
                        // A failed initialisation can leave the client half initialised: retry on a fresh one.
                        client = activate(&device)?;
                    }
                }
            }
            // Loopback and devices with a locked engine period use the default period (typically 10 ms).
            if !initialized {
                client
                    .Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 0, 0, &format, if loopback { Some(&session) } else { None })
                    .map_err(err)?;
            }
            client.SetEventHandle(ready.0).map_err(err)?;
            let capture: IAudioCaptureClient = client.GetService().map_err(err)?;
            Ok(Self { client, capture, ready, kind, gain, started: false })
        }
    }
}

impl AudioEndpoint for WasapiEndpoint {
    fn kind(&self) -> AudioSourceKind {
        self.kind
    }
    fn gain(&self) -> f32 {
        self.gain
    }
    fn start(&mut self) -> Result<(), String> {
        // SAFETY: an initialised client.
        unsafe { self.client.Start() }.map_err(err)?;
        self.started = true;
        Ok(())
    }
    /// `Drain`: every packet waiting, released after it's handed on.
    fn drain(&mut self, sink: &mut dyn FnMut(CapturePacket<'_>)) -> Result<(), String> {
        loop {
            // SAFETY: the capture client's buffer is read between GetBuffer and ReleaseBuffer, and
            // only when not silent; `frames * 2` floats (48 kHz stereo float) are valid then.
            unsafe {
                let available = self.capture.GetNextPacketSize().map_err(err)?;
                if available == 0 {
                    return Ok(());
                }
                let (mut data, mut frames, mut flags, mut position, mut qpc) = (std::ptr::null_mut(), 0u32, 0u32, 0u64, 0u64);
                self.capture.GetBuffer(&mut data, &mut frames, &mut flags, Some(&mut position), Some(&mut qpc)).map_err(err)?;
                if frames == 0 {
                    return Ok(());
                }
                let samples: &[f32] = if flags & crate::audio::FLAG_SILENT == 0 && !data.is_null() {
                    std::slice::from_raw_parts(data as *const f32, frames as usize * 2)
                } else {
                    &[]
                };
                sink(CapturePacket { frames, flags, qpc_position: qpc, data: samples });
                self.capture.ReleaseBuffer(frames).map_err(err)?;
            }
        }
    }
}

impl Drop for WasapiEndpoint {
    fn drop(&mut self) {
        if self.started {
            // SAFETY: a started client.
            let _ = unsafe { self.client.Stop() };
        }
    }
}

/// [`AudioDevices`] on WASAPI.
pub struct WasapiDevices;

impl AudioDevices for WasapiDevices {
    /// One endpoint on the calling thread (for an [`crate::audio::EndpointCapture`]).
    fn open(&self, kind: AudioSourceKind, device_id: Option<&str>, gain: f32) -> Result<Box<dyn AudioEndpoint>, String> {
        let _com = Com::init(true)?;
        Ok(Box::new(WasapiEndpoint::open(&enumerator()?, kind, device_id, gain)?))
    }
    fn capture(&self, options: &RecordingOptions, mixer: Arc<AudioMixer>, time: Arc<dyn TimeSource>) -> Option<Result<Box<dyn AudioCapture>, String>> {
        Some(WasapiCapture::new(options, mixer, time).map(|c| Box::new(c) as Box<dyn AudioCapture>))
    }
}

/// `WasapiCapture`: the endpoints opened, started and drained on their own thread.
pub struct WasapiCapture {
    start: Arc<Event>,
    stop: Arc<Event>,
    failure: Arc<Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
    disposed: AtomicBool,
}

impl WasapiCapture {
    /// The constructor: starts the thread, waits until the endpoints are open (or failed).
    pub fn new(options: &RecordingOptions, mixer: Arc<AudioMixer>, time: Arc<dyn TimeSource>) -> Result<Self, String> {
        let (start, stop) = (Arc::new(Event::new(true)?), Arc::new(Event::new(true)?));
        let failure = Arc::new(Mutex::new(None));
        let (prepared_tx, prepared_rx) = mpsc::channel();
        let wanted = [
            (options.capture_system_audio, AudioSourceKind::SystemAudio, options.system_audio_device_id.clone(), options.system_audio_gain),
            (options.capture_microphone, AudioSourceKind::Microphone, options.microphone_device_id.clone(), options.microphone_gain),
        ];
        let thread = {
            let (start, stop, failure) = (start.clone(), stop.clone(), failure.clone());
            std::thread::Builder::new()
                .name("ShareX WASAPI capture".into())
                .spawn(move || {
                    let r = run(&wanted, &mixer, time.as_ref(), &start, &stop, &prepared_tx);
                    if let Err(e) = r {
                        *failure.lock().unwrap_or_else(|p| p.into_inner()) = Some(e.clone());
                        let _ = prepared_tx.send(Err(e));
                    }
                })
                .map_err(|e| e.to_string())?
        };
        let mut capture = Self { start, stop, failure, thread: Some(thread), disposed: AtomicBool::new(false) };
        match prepared_rx.recv() {
            Ok(Ok(())) => Ok(capture),
            Ok(Err(e)) => {
                capture.dispose();
                Err(e)
            }
            Err(_) => {
                capture.dispose();
                Err("The WASAPI capture thread ended.".into())
            }
        }
    }

    fn dispose(&mut self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        self.stop.set();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

type Wanted = [(bool, AudioSourceKind, Option<String>, f32); 2];

/// `Run`: MTA, MMCSS, open, wait for start or stop, then drain on every
/// buffer event (or every 100 ms) until stopped, and once more.
fn run(wanted: &Wanted, mixer: &AudioMixer, time: &dyn TimeSource, start: &Event, stop: &Event, prepared: &mpsc::Sender<Result<(), String>>) -> Result<(), String> {
    let _com = Com::init(false)?;
    let mut task_index = 0u32;
    // SAFETY: registers this thread with MMCSS; reverted below.
    let mmcss = unsafe { AvSetMmThreadCharacteristicsW(w!("Audio"), &mut task_index) }.ok();
    let result = (|| {
        let enumerator = enumerator()?;
        let mut endpoints: Vec<(WasapiEndpoint, DrainState)> = Vec::new();
        for (on, kind, id, gain) in wanted {
            if *on {
                endpoints.push((WasapiEndpoint::open(&enumerator, *kind, id.as_deref(), *gain)?, DrainState::default()));
            }
        }
        let _ = prepared.send(Ok(()));
        if wait_any(&[stop.0, start.0], INFINITE) == Some(0) {
            return Ok(());
        }
        for (e, _) in endpoints.iter_mut() {
            e.start()?;
        }
        let drain_all = |endpoints: &mut Vec<(WasapiEndpoint, DrainState)>| -> Result<(), String> {
            for (e, state) in endpoints.iter_mut() {
                let (source, gain) = (e.kind.index(), e.gain);
                e.drain(&mut |p| drain_packet(state, mixer, source, gain, p, time.now()))?;
            }
            Ok(())
        };
        let events: Vec<HANDLE> = std::iter::once(stop.0).chain(endpoints.iter().map(|(e, _)| e.ready.0)).collect();
        while wait_any(&events, 100) != Some(0) {
            drain_all(&mut endpoints)?;
        }
        drain_all(&mut endpoints)
    })();
    if let Some(h) = mmcss {
        // SAFETY: the handle AvSetMmThreadCharacteristicsW returned.
        let _ = unsafe { AvRevertMmThreadCharacteristics(h) };
    }
    result
}

impl AudioCapture for WasapiCapture {
    fn start(&mut self) {
        self.start.set();
    }
    fn failure(&self) -> Option<String> {
        self.failure.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    fn stop(&mut self) {
        self.dispose();
    }
}

impl Drop for WasapiCapture {
    fn drop(&mut self) {
        self.dispose();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_and_flags_as_sharex() {
        let f = capture_format();
        assert_eq!((f.wFormatTag, f.nChannels, f.nSamplesPerSec, f.nAvgBytesPerSec, f.nBlockAlign, f.wBitsPerSample), (3, 2, 48_000, 384_000, 8, 32));
        assert_eq!(stream_flags(true), 0x0004_0000 | 0x8000_0000 | 0x0800_0000 | 0x0002_0000 | 0x2000_0000);
        assert_eq!(stream_flags(false), 0x0004_0000 | 0x8000_0000 | 0x0800_0000);
        assert_eq!(AUDCLNT_STREAMFLAGS_LOOPBACK, LOOPBACK);
        assert_eq!(AUDCLNT_STREAMFLAGS_EVENTCALLBACK, EVENT_CALLBACK);
        assert_eq!(AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUTO_CONVERT);
        assert_eq!(AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, SRC_QUALITY);
        assert_eq!(AUDCLNT_SESSIONFLAGS_DISPLAY_HIDE, SESSION_HIDDEN);
    }

    #[test]
    fn wait_any_reports_the_signalled_event() {
        let (a, b) = (Event::new(true).unwrap(), Event::new(false).unwrap());
        assert_eq!(wait_any(&[a.0, b.0], 0), None);
        b.set();
        assert_eq!(wait_any(&[a.0, b.0], 0), Some(1));
        // Auto-reset.
        assert_eq!(wait_any(&[a.0, b.0], 0), None);
        a.set();
        assert_eq!(wait_any(&[a.0, b.0], 0), Some(0));
    }
}
