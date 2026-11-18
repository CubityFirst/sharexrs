use super::*;
use crate::audio::SyntheticAudioDevices;
use crate::media::{CpuMediaFactory, Mp4Call};
use crate::source::SyntheticSources;

type Tick = Arc<dyn Fn(&RecorderHandle) + Send + Sync>;

fn backends(t: &Arc<VirtualTime>, media: Arc<CpuMediaFactory>, capture_fps: i64, tick: Option<Tick>) -> Backends {
    Backends {
        capture: Arc::new(SyntheticSources { capture_fps }),
        audio: Some(Arc::new(SyntheticAudioDevices { time: t.clone(), drift_ppm: 0 })),
        camera: None,
        media,
        time: t.clone(),
        virtual_time: Some(t.clone()),
        tick,
    }
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sharexrs-native-{}", crate::gif::guid_n()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn options(path: &std::path::Path) -> RecordingOptions {
    let mut o = RecordingOptions::new(path);
    o.region = Rect::new(10, 20, 64, 48);
    o
}

/// Records on virtual time; `tick` sees the recorder before every wait.
fn record_with(
    o: RecordingOptions,
    media: Arc<CpuMediaFactory>,
    capture_fps: i64,
    tick: impl Fn(&RecorderHandle) + Send + Sync + 'static,
) -> Result<RecordingResult, RecordError> {
    let t = VirtualTime::new(TICKS_PER_SECOND);
    let r = ScreenRecorder::new(o, backends(&t, media, capture_fps, Some(Arc::new(tick)))).unwrap();
    r.start()?;
    r.completion()
}

fn video_calls(media: &CpuMediaFactory) -> Vec<(i64, i64, i64, bool)> {
    media
        .mp4_calls
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            Mp4Call::Video { frame, timestamp, end, discontinuity } => Some((*frame, *timestamp, *end, *discontinuity)),
            _ => None,
        })
        .collect()
}

#[test]
fn frames_on_the_grid_with_audio() {
    let path = temp("a.mp4");
    let media = Arc::new(CpuMediaFactory::default());
    let mut o = options(&path);
    o.duration = TICKS_PER_SECOND;
    let r = record_with(o, media.clone(), 60, |_| {}).unwrap();
    assert_eq!(r.video_frames, 30);
    assert_eq!(r.dropped_video_frames, 0);
    assert_eq!(r.duration, TICKS_PER_SECOND);
    let video = video_calls(&media);
    assert_eq!(video[1], (1, 333_333, 666_666, false));
    // Audio: 1 s in 10 ms blocks, contiguous.
    let calls = media.mp4_calls.lock().unwrap().clone();
    let mut next = 0;
    for c in &calls {
        if let Mp4Call::Audio { frame_offset, frames } = c {
            assert_eq!(*frame_offset, next);
            next += *frames as i64;
        }
    }
    assert_eq!(next, 48_000);
    assert_eq!(calls.last(), Some(&Mp4Call::Finish));
    assert!(path.is_file());
}

#[test]
fn pause_and_move_keep_the_timeline() {
    let path = temp("p.mp4");
    let media = Arc::new(CpuMediaFactory::default());
    let mut o = options(&path);
    o.capture_system_audio = false;
    let state = Mutex::new(0);
    let r = record_with(o, media.clone(), 30, move |r| {
        let mut s = state.lock().unwrap();
        let e = r.elapsed();
        if *s == 0 && e >= TICKS_PER_SECOND / 2 {
            *s = 1;
            r.pause();
        } else if *s == 1 {
            *s = 2;
            r.resume_at(Rect::new(100, 100, 65, 48)).unwrap();
        } else if *s == 2 && e >= TICKS_PER_SECOND {
            r.request_stop();
        }
    })
    .unwrap();
    // Slots 0 to 30: the stop comes after the slot at 1 s.
    assert_eq!(r.video_frames, 31);
    let video = video_calls(&media);
    assert_eq!(video.iter().filter(|v| v.3).count(), 1);
    for w in video.windows(2) {
        assert_eq!(w[1].0, w[0].0 + 1);
    }
}

#[test]
fn late_slots_are_dropped_not_caught_up() {
    let path = temp("d.mp4");
    let media = Arc::new(CpuMediaFactory { no_slot: vec![5, 6], ..Default::default() });
    let mut o = options(&path);
    o.capture_system_audio = false;
    o.duration = TICKS_PER_SECOND / 2;
    let r = record_with(o, media.clone(), 30, |_| {}).unwrap();
    assert_eq!(r.video_frames, 13);
    assert_eq!(r.dropped_video_frames, 2);
    assert_eq!(video_calls(&media)[5], (7, 7 * TICKS_PER_SECOND / 30, 8 * TICKS_PER_SECOND / 30, true));
}

#[test]
fn abort_deletes_the_output() {
    let path = temp("x.mp4");
    let mut o = options(&path);
    o.capture_system_audio = false;
    let r = record_with(o, Arc::new(CpuMediaFactory::default()), 30, |r| {
        if r.elapsed() > TICKS_PER_SECOND / 3 {
            r.request_abort();
        }
    });
    assert_eq!(r, Err(RecordError::Canceled("Recording aborted.".into())));
    assert!(!path.exists());
}

#[test]
fn stop_before_start_cancels() {
    let path = temp("s.mp4");
    let t = VirtualTime::new(0);
    let r = ScreenRecorder::new(options(&path), backends(&t, Arc::new(CpuMediaFactory::default()), 30, None)).unwrap();
    r.prepare().unwrap();
    assert!(path.exists());
    r.request_stop();
    assert_eq!(r.completion(), Err(RecordError::Canceled("Recording stopped before capture began.".into())));
    assert!(!path.exists());
}

#[test]
fn existing_files_are_never_overwritten() {
    let path = temp("e.mp4");
    std::fs::write(&path, b"keep").unwrap();
    let t = VirtualTime::new(0);
    let r = ScreenRecorder::new(options(&path), backends(&t, Arc::new(CpuMediaFactory::default()), 30, None)).unwrap();
    assert!(r.prepare().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");
}

#[test]
fn a_closed_window_ends_the_recording() {
    struct Ending(i64);
    impl CaptureSources for Ending {
        fn open(&self, o: &RecordingOptions, time: Arc<dyn TimeSource>) -> Result<Box<dyn crate::source::FrameSource>, String> {
            let mut s = crate::source::SyntheticSource::new(o, time, 30)?;
            s.end_at = Some(self.0);
            Ok(Box::new(s))
        }
    }
    let path = temp("w.mp4");
    let mut o = options(&path);
    o.capture_system_audio = false;
    let t = VirtualTime::new(TICKS_PER_SECOND);
    let mut b = backends(&t, Arc::new(CpuMediaFactory::default()), 30, None);
    b.capture = Arc::new(Ending(10));
    let r = ScreenRecorder::new(o, b).unwrap();
    r.start().unwrap();
    let result = r.completion().unwrap();
    assert!(result.video_frames < 12, "{result:?}");
    assert!(path.is_file());
}

#[test]
fn gif_end_to_end() {
    let path = temp("g.gif");
    let mut o = options(&path);
    o.frames_per_second = 15;
    o.duration = TICKS_PER_SECOND;
    let t = VirtualTime::new(TICKS_PER_SECOND);
    let r = ScreenRecorder::new(o, backends(&t, Arc::new(CpuMediaFactory::default()), 30, None)).unwrap();
    let progress = Arc::new(Mutex::new(Vec::new()));
    let p = progress.clone();
    r.on_encoding_progress(move |v| p.lock().unwrap().push(v));
    r.start().unwrap();
    let result = r.completion().unwrap();
    assert!(result.video_frames + result.dropped_video_frames >= 15, "{result:?}");
    assert_eq!(result.encoder, VideoEncoderInfo::gif());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..6], b"GIF89a");
    assert_eq!(progress.lock().unwrap().last(), Some(&100));
}

#[test]
fn resume_at_checks_the_size() {
    let path = temp("r.mp4");
    let t = VirtualTime::new(0);
    let r = ScreenRecorder::new(options(&path), backends(&t, Arc::new(CpuMediaFactory::default()), 30, None)).unwrap();
    assert!(r.resume_at(Rect::new(0, 0, 65, 49)).is_ok());
    assert!(r.resume_at(Rect::new(0, 0, 66, 48)).is_err());
}
