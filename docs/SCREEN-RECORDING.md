# Screen recording

How ShareX 22.4 (the reference commit) records the screen, and where
ShareXrs is with it. Sources:

| ShareX | What |
|---|---|
| `ShareX/ScreenRecordManager.cs` | starting, stopping, pausing, aborting; the recording worker for both recorders |
| `ShareX.ScreenRecordingLib/` (~4k lines) | the **native** recorder: Windows Graphics Capture, Media Foundation, WASAPI, camera overlay, adaptive GIF |
| `ShareX.ScreenCaptureLib/ScreenRecording/` | the **FFmpeg** recorder: `FFmpegOptions`, `ScreenRecordingOptions` (the command line), `ScreenRecorder`, `FFmpegCaptureDevice` |
| `ShareX.HelpersLib/Media/FFmpegCLIManager.cs`, `CLI/ExternalCLIManager.cs` | running FFmpeg |
| `ShareX.ScreenCaptureLib/Presentation/ScreenRecording/` | the recording frame (`ScreenRecordWindow`, `RecordingRegionBorder`) |
| `ShareX/Presentation/ScreenRecording/` | the recorder bar shown before recording (`ScreenRecorderBarWindow`, `ScreenRecorderBarSettings`, `CameraLayoutPreview`) |
| `ShareX.ScreenCaptureLib/Presentation/FFmpegOptions/` | the FFmpeg options window |
| `ShareX/TaskSettings.cs` (`TaskSettingsCapture`) | `FFmpegOptions` and the `ScreenRecord*` settings |

## 1. Two recorders

`TaskSettingsCapture.ScreenRecordUseNative` (default **true**) chooses:

- **Native** (`ShareX.ScreenRecordingLib`, new in the Avalonia rewrite): H.264/AAC
  MP4 or an adaptive GIF, no FFmpeg. The default.
- **FFmpeg** (`ShareX.ScreenCaptureLib/ScreenRecording`): whatever codec
  `FFmpegOptions` says, FFmpeg capturing the screen itself (gdigrab, ddagrab
  or a DirectShow device). Needs `ffmpeg.exe`.

Entry points: the hotkeys `ScreenRecorder`, `ScreenRecorderActiveWindow`,
`ScreenRecorderCustomRegion`, `StartScreenRecorder` (last region) and their
`…GIF` twins (`TaskHelpers.ExecuteJob` → `StartScreenRecording(output,
method)`), Capture › Screen recording / Screen recording (GIF) in the main
menu and tray, and `StopScreenRecording`, `PauseScreenRecording`,
`AbortScreenRecording`. `StartStopRecording` toggles: while recording, the
same hotkey stops (or presses Record on the bar).

### `ScreenRecordManager.StartRecording`

1. FFmpeg with a GIF output: `FFmpegOptions.VideoCodec = gif` (in the task
   settings). FFmpeg with an animated image codec (GIF, WebP, APNG):
   `ScreenRecordTwoPassEncoding = true`.
2. Frame rate: `GIFFPS` (15) for GIFs, `ScreenRecordFPS` (30) otherwise.
3. FFmpeg only: `CheckFFmpeg` (§5) and "video and audio source both can't be
   None".
4. The area: Region → the region overlay (`GetRectangleRegionAsync`, which
   also names the file after the window under it); ActiveWindow → the
   foreground window's rectangle (client area with `CaptureClientArea`; the
   window handle is kept so the native recorder follows the window);
   CustomRegion → `CaptureCustomRegion`; LastRegion →
   `ApplicationConfig.ScreenRecordRegion`. Clipped to the virtual screen, made
   even-sized (native MP4, or FFmpeg codecs other than animated images).
   Empty → nothing happens.
5. `ScreenRecordShowBar` (default true): the recorder bar (§8) to review the
   settings; it can cancel. Its edits are saved (`ScreenRecorderBarSettings
   .Commit`) either way.
6. `ApplicationConfig.ScreenRecordRegion` = the area. The recording frame
   (`ScreenRecordWindow`, §7) opens around it.
7. The path: screenshots folder + `GetFileName` (the name pattern) with
   `gif`/`mp4` (native), `mp4` (two-pass) or `FFmpegOptions.Extension`;
   `HandleExistsFile` may cancel.
8. The worker records (§2 or §3), then the after capture window, then a
   file task (`WorkerTask.CreateFileJobTask`: upload etc. as for any file).

## 2. The native recorder (`ShareX.ScreenRecordingLib`)

`ScreenRecorder(RecordingOptions)`: `PrepareAsync` (during the start delay)
sets up everything, `StartAsync` starts capturing, `Pause`/`Resume(region)`
in process, `RequestStop`, `RequestAbort`, `Completion` gives frames written,
dropped, audio discontinuities and the encoder.

- **Capture**: Windows Graphics Capture (`GraphicsCapture`), one session per
  monitor the region touches or the window (`CreateForWindow`, follows it
  when occluded), cursor (`IsCursorCaptureEnabled`), no yellow border,
  `MinUpdateInterval` = 1/fps. Frames composed on the GPU (D3D11,
  `GpuVideoProcessor`: BGRA → NV12, Rec. 709), HDR monitors tone mapped with
  `ScreenRecordHDRToneMapping` (`HdrToneMapShader`). Paused regions can move
  (to another monitor too) with the same size.
- **Timing**: `RecordingClock` (QPC). Frames are taken on a fixed grid
  (`FrameTimer`); late slots are dropped, never caught up; 10 s without a
  frame fails.
- **MP4**: Media Foundation sink writer, H.264 Main (HEVC Main above 4096
  wide / 4096×2304 pixels, else H.264 downscaled), `ScreenRecordVideoBitrate`
  kbps (10000), hardware encoder required by default
  (`ScreenRecordRequireHardwareEncoder`). AAC-LC 48 kHz stereo, 192 kb/s.
- **Sound**: WASAPI shared mode, loopback of the system audio device and/or
  the microphone (`…DeviceId`, empty = default; gains 0–4), resampled and
  drift-corrected into one 48 kHz stereo stream (`AudioMixer`). Devices
  listed with the MMDevice API (`AudioCaptureDevices`).
- **Camera**: an overlay (`CameraCapture` via MF source reader,
  `CameraOverlay`, circle or rectangle, corner, width %, margin, resolution
  and fps).
- **GIF** (`GifWriter`, "ShareX adaptive GIF encoder"): lossless BGRA frames
  cached to a hidden temp file during recording; after stopping, one pass
  builds a global palette for the whole recording (`AdaptiveGifQuantizer`),
  the second writes only the changed rectangle of each frame, skipping
  duplicates, with centisecond timing; progress 0–100 to the tray icon.
- Output is written to `<name>-recording-<guid>.<ext>` and moved over the
  final path only when it completed (aborts and restarts never touch an
  existing file).

**Not ported.** It's Windows-only by design (WGC, D3D11, MF, WASAPI). The
macOS/Linux equivalents are snapr's (ScreenCaptureKit/xcap +
AVFoundation, PipeWire/wlr-screencopy + FFmpeg; `docs/MACOS-LEARNINGS.md`,
`G:/scripts/snapr/src/record.rs`, `encode/`).

## 3. The FFmpeg recorder

### How it records on Windows

FFmpeg captures the screen itself; ShareX never hands it frames.

- **gdigrab** (`VideoSource = "gdigrab"`, the default): `-f gdigrab
  -framerate N -offset_x X -offset_y Y -video_size WxH -draw_mouse 0|1 -i
  desktop`, the area in virtual-screen pixels.
- **ddagrab** (Windows 10+): the Desktop Duplication filter through lavfi,
  `-f lavfi -i ddagrab=output_idx=I:draw_mouse=true|false:framerate=N:
  offset_x=X:offset_y=Y:video_size=WxH:output_fmt=bgra[,hwdownload,
  format=bgra]`. The monitor is the one (primary first, then Avalonia's
  order) the area covers most; the offset is relative to it, the size made
  even again for video codecs. NVENC codecs take the D3D11 frames directly
  (no `hwdownload`).
- **DirectShow** (any other value): `-f dshow -framerate N -i
  video="<name>"[:audio="<name>"]`. ShareX's own DirectShow filters
  ([ShareX/RecorderDevices](https://github.com/ShareX/RecorderDevices/releases/tag/v0.12.10),
  "Install recorder devices" in the options window, hidden in the Store
  build): `screen-capture-recorder` (screen; ShareX writes its region to
  `HKCU\Software\screen-capture-recorder`: `start_x`, `start_y`,
  `capture_width`, `capture_height`, `default_max_fps` = 60,
  `capture_mouse_default_1`) and `virtual-audio-capturer` (what's playing).
- **Sound** is always DirectShow: `-f dshow … -audio_buffer_size 80 -i
  audio="<name>"`, before the screen input. There's no WASAPI loopback in
  this path: system sound needs `virtual-audio-capturer` or a stereo mix
  device.
- Every device input gets `-thread_queue_size 1024 -rtbufsize 256M`
  (`AppendInputDevice`).
- Device names: `-list_devices true -f dshow -i dummy`, parsed from `"name"
  (video)` / `(audio)` lines (and the old "DirectShow video devices"
  headers); alternative names skipped. The options window lists None,
  gdigrab, ddagrab (Win10+), then `dshow (<name>)` for each.

### The command line (`ScreenRecordingOptions.GetFFmpegArgs`)

In order: inputs (above; or `-i "<input>"` when encoding a file), `UserArgs`,
the video codec, the audio codec, `-t <duration>` (fixed duration, "0.0"
format: .NET rounds half away from zero from 7 significant digits, so 2.25 s
is `-t 2.3`), `-y`, `"<output with the codec's extension>"`.

Video (`-c:v <codec> -r <fps>` first, except APNG):

| Codec (`FFmpegVideoCodec`) | Arguments | Ext. |
|---|---|---|
| `libx264`, `libx265` | `-preset <x264_Preset>` `-tune zerolatency` (recording only) `-crf <x264_CRF>` or `-b:v <x264_Bitrate>k` `-pix_fmt yuv420p -movflags +faststart` | mp4 |
| `libvpx`, `libvpx_vp9` (`-c:v libvpx-vp9`) | `-deadline realtime` (recording only) `-b:v <VPx_Bitrate>k -pix_fmt yuv420p` | webm |
| `libxvid` | `-qscale:v <XviD_QScale>` | avi |
| `h264_nvenc`, `hevc_nvenc` | `-preset <p1..p7> -tune <hq/ll/ull/lossless> -b:v <NVENC_Bitrate>k -movflags +faststart` | mp4 |
| `h264_amf`, `hevc_amf` | `-usage <AMF_Usage> -quality <speed/balanced/quality> -b:v <AMF_Bitrate>k -pix_fmt yuv420p` | mp4 |
| `h264_qsv`, `hevc_qsv` | `-preset <QSV_Preset> -b:v <QSV_Bitrate>k` | mp4 |
| `gif` | (nothing more; real GIFs are two-pass, below) | gif |
| `libwebp` | `-lossless 0 -preset default -loop 0` | webp |
| `apng` | `-f apng -plays 0` (no `-c:v`/`-r`) | apng |

HEVC codecs add `-tag:v hvc1`. There is **no AV1** in ShareX (the enum has
none; `FFmpegCLIManager` only declares unused AV1 CRF limits).

Audio (only with an audio source, and never with an animated image codec):
AAC `-c:a aac -ac 2 -b:a <AAC_Bitrate>k` (m4a alone), Opus `-c:a libopus
-b:a <k>` (opus), Vorbis `-c:a libvorbis -qscale:a <q>` (ogg), MP3 `-c:a
libmp3lame -qscale:a <q>` (mp3).

**Two-pass** (`ScreenRecordTwoPassEncoding`, forced for animated images):
the recording is `-c:v libx264 -r N -preset ultrafast -tune zerolatency -qp
0` into an .mp4 (custom commands ignored), then `ProcessTwoPassEncoding`
encodes it into a new file name with the codec (`FFmpegEncodeVideo`: `-i
"<rec>"` + the codec arguments; custom commands apply) or, for GIF,
`FFmpegEncodeAsGIF`:

```
-i "<rec>" -lavfi "palettegen=stats_mode=<full|diff|single>[palette],[0:v][palette]paletteuse=dither=<GIFDither>[:bayer_scale=<n>][:new=1]" -y "<out>.gif"
```

(`bayer_scale` only with `bayer`, `new=1` only with `single`), then deletes
the recording. ShareX's other GIF path (`ScreenRecordOutput.GIF`:
screenshots into a `HardDiskCache`, `AnimatedGifCreator`) is never
constructed in 22.4.

**Custom commands** (`UseCustomCommands`, not for the lossless pass): the
text with `$fps$`, `$area_x$`, `$area_y$`, `$area_width$`, `$area_height$`,
`$cursor$` (1/0), `$duration$` ("0.0"), `$output$` (path with the codec's
extension) replaced (any case), trimmed. Switching them on fills the box with
`GetFFmpegArgs(isCustom: true)`: the same line with those placeholders.

### Running FFmpeg (`FFmpegCLIManager`)

- One argument string (`ProcessStartInfo.Arguments`), no window, the
  program's folder as working directory, stdin/stdout/stderr redirected,
  UTF-8. Only `.exe` paths run. The output's non-empty lines are kept
  (lines end at `\n`, `\r` or `\r\n`, so progress lines count).
- "Press [q] to stop" (any case) = encoding started → the recording frame
  turns red and the timer starts.
- Progress (second passes): the first `Duration: h:m:s.f, start:` is the
  length, then each `time=h:m:s.f bitrate=` is a percentage; the recorder
  reports whole percents when they change (to the tray icon's text).
- **Stop** (`Close`): `q` + newline to stdin, which finishes the file; the
  third call kills FFmpeg. A non-zero exit shows the output in an output box.
- **Concatenation** (after a pause): a list file `<out>.txt` with `file
  '<path>'` lines, `-f concat -safe 0 -i "<list>" -c copy "<out>"`.

## 4. Recording states

`ScreenRecordWindow.Status` (`ScreenRecordingStatus`): Waiting → Working
(FFmpeg starting) → Recording ("Press [q]") → Stopped / Paused / Aborted.
The worker reports `ScreenRecordState` (Waiting, BeforeStart, AfterStart,
AfterRecordingStart, RecordingEnd, Encoding).

| Action | Waiting | Working | Recording | Paused |
|---|---|---|---|---|
| Start/Stop (button, hotkey) | starts now (skips the delay) | abort | stop (`q`) | stop (keeps what's recorded) |
| Pause/Resume | starts now | – | FFmpeg: stop the segment; native: pause in process | FFmpeg: next segment; native: resume |
| Restart (recording/paused only) | – | – | discard, back to Waiting (delay again) | same |
| Abort | stop, delete | same | same | same |

FFmpeg can't pause, so a pause ends the FFmpeg run; on resume the finished
file is renamed `<name>-concat.<ext>`, the next segment recorded into the
path, and the two joined (`-c copy`) into `<name>-temp` and renamed back.
The start delay (`ScreenRecordStartDelay`, 3 s, when enabled) is a countdown
that Start skips. A fixed duration (`ScreenRecordDuration`) is `-t` for
FFmpeg. "Action completed" sound after each segment. Abort deletes the file;
`ScreenRecordAskConfirmationOnAbort` asks first (in the frame).

## 5. Finding FFmpeg

- `FFmpegOptions.FFmpegPath`: `CLIPath` when `OverrideCLIPath` and not
  empty, else `ffmpeg.exe` beside ShareX.exe; folder variables expanded,
  relative to ShareX's folder.
- `CheckFFmpeg` (before every FFmpeg recording and the video tools): 32-bit
  Windows needs a custom path; the file must exist, else "FFmpeg does not
  exist at the following path". Nothing else is searched.
- ShareX **ships** it: its CI downloads BtbN's
  `ffmpeg-n9.0-latest-win64-gpl-9.0.zip` (winarm64 for ARM) and copies
  `ffmpeg.exe` beside ShareX.exe (`.github/workflows/build.yml`). The
  downloaders in HelpersLib (`FFmpegDownloader`: the dead zeranoe URL;
  `FFmpegGitHubDownloader`: ShareX/FFmpeg releases, `*win64.zip`, extract
  `ffmpeg.exe` ≤ 200 MB) aren't called anywhere in 22.4.
- The reference here: `G:/scripts/sharexrs-ref/ShareX-22.4.0-portable/
  ffmpeg.exe` (n9.0.2, GPL, with libx264/x265/vpx/xvid/webp, NVENC, AMF,
  QSV). The source build (`ShareX-build`) has none.

## 6. The FFmpeg options window (later)

`FFmpegOptionsWindow`: custom FFmpeg path + browse, video/audio sources with
refresh and a status line (found N devices / FFmpeg not found / enumeration
failed), install recorder devices, codec tabs (x264/x265 preset, CRF or
bitrate; VPx bitrate; Xvid q; NVENC preset/tune/bitrate; AMF usage/quality/
bitrate; QSV preset/bitrate; GIF stats mode, dither, Bayer scale; AAC
64–320 in 32s, Opus 32–512, Vorbis 0–10, MP3 9–0), user arguments, custom
commands with the command preview, reset. Opened from Task settings ›
Screen recorder.

## 7. The recording frame (later)

`ScreenRecordWindow`: a transparent topmost window around the area with
`RecordingRegionBorder` (dashed dark backdrop, accent goldenrod while
waiting, red while recording), a toolbar (461×42) with the timer (status dot,
elapsed or countdown), Start/Stop, Pause, Restart, Abort (labels optional:
`ScreenRecordShowButtonLabels`; timer optional), an inline abort
confirmation; draggable while waiting; its own tray icon (gold video-off
while waiting, then progress during encoding) with the same four commands.
Excluded from capture for the native recorder.

## 8. The recorder bar (later)

`ScreenRecorderBarWindow`, before recording when `ScreenRecordShowBar`: a
draggable bar: grip, **Record**, Start delay (toggle + 1–5 s menu); sources
(native only: System audio and Microphone with device, volume; Camera with
device, layout preview, shape, corner, width, margin, resolution, fps),
Cursor, Mouse highlighter (+ its options); Options (FPS or GIF FPS, bitrate
(native), fixed duration, show the bar, timer, button labels); Cancel. For
FFmpeg the audio/camera/bitrate parts are hidden and the title is "Screen
recorder". Record re-checks a captured window (moved, minimized:
"target unavailable").

## 9. ShareXrs: `crates/sharex-screen-recording`

The FFmpeg recorder, ported; the options model is
`sharex_settings::FFmpegOptions` (typed in `TaskSettingsCapture`, ShareX's
JSON byte for byte, `FixSources` and the 14.1.2/15.0.1 upgrade tasks).

| Module | Ports |
|---|---|
| `args` | `ScreenRecordingOptions` (`get_ffmpeg_commands`, `get_ffmpeg_args(is_custom)`, ddagrab's monitor choice, the screen-capture-recorder registry values), `gif_encode_args`, `concat_args`/`concat_list` |
| `dotnet` | the .NET behaviour the strings depend on: `Path.ChangeExtension`, `float.ToString("0.0")`, ordinal ignore-case `Replace`, `TimeSpan.TryParse`, `EscapeCLIText`, `AppendTextToFileName`, .NET's Unix argv splitting |
| `cli` | `FFmpegCLIManager` + `ExternalCLIManager`: run, output, "Press [q]", progress, `close` (`q`, kill on the third), `get_direct_show_devices`, `get_video_info`, `concatenate_videos`; multicast events |
| `recorder` | `ScreenRecorder` (FFmpeg output): segment, `ffmpeg_encode_video`, `ffmpeg_encode_as_gif`, HKCU writes |
| `session` | `RecordingControl` (the frame's status machine: start/stop, pause/resume, restart, abort, `ChangeState`), `prepare_ffmpeg_recording` (StartRecording's checks), `run_ffmpeg_session` (the worker: delay, segments, concat, restart, two-pass, cleanup) with `SessionHooks` for file names, sounds and the error box |
| `discovery` | `FFmpegPath`, `CheckFFmpeg`; `resolve_ffmpeg` adds fallbacks (below) |
| `devices` | DirectShow parsing and ShareX's source lists; AVFoundation and PulseAudio listing (additions) |
| `platform` | macOS/Linux inputs (additions, below) |

### Validation

- **Command lines, against ShareX's DLLs**: `tools/recording-harness`
  (README there) runs ShareX.ScreenCaptureLib's own `ScreenRecorder`,
  `ScreenRecordingOptions`, `FFmpegCLIManager` with Harmony standing in for
  FFmpeg, the monitors, the registry and folder creation; the crate's
  `harness` example runs the same 172 cases and compares every field:
  **172/172 identical** (115 recordings: every video codec with/without
  sound, every audio codec with video and alone, codec options, NVENC
  preset × tune, AMF, QSV, frame rates, regions, cursor, 10 durations,
  gdigrab/ddagrab (1 and 3 monitors)/DirectShow/screen-capture-recorder with
  its registry values, quoting, user arguments, custom commands, lossless
  passes, output paths, unknown and numeric enum values; 27 GIF second
  passes (stats × dither); 15 video second passes; 5 custom-command
  templates; concat with its list file; 3 DirectShow lists; 3 output-parsing
  runs with the raw progress floats and ShareX's whole-percent events).
- **Running FFmpeg** (`tests/ffmpeg.rs`, ShareX's own `ffmpeg.exe` n9.0.2,
  `SHAREXRS_TEST_FFMPEG=…`), on `-f lavfi` `testsrc`/`sine` paced to real
  time, never the screen: 11/11 pass: stop with `q` (h264 + aac, the state
  sequence BeforeStart → AfterStart → AfterRecordingStart → RecordingEnd),
  fixed duration ends by itself (1.0 s), pause/resume joins two segments
  (`-concat`/`-temp` cleaned up), restart keeps only the new take, abort
  deletes, GIF = lossless mp4 + palettegen (mp4 deleted, Encoding state,
  progress to ≥ 90 %), two-pass VP9 + Opus WebM, a bad option reports
  FFmpeg's output, `-nostdin` ignores `q` and the third stop kills it,
  progress from a real encode (duration read, only changes reported),
  DirectShow listing on this machine.

### Outside Windows (ShareXrs additions)

ShareX's FFmpeg recorder only runs on Windows (it only runs `.exe`s and its
sources are Windows devices). `platform::platform_ffmpeg_commands` keeps
ShareX's options and everything after the inputs, and maps the inputs:

| ShareX source | macOS | Linux X11 | Linux Wayland |
|---|---|---|---|
| gdigrab / ddagrab | `-f avfoundation -capture_cursor 0/1 -framerate N -i "<screen>:<audio\|none>"` + `-filter:v crop=W:H:X:Y` (screen pixels) | `-f x11grab -framerate N -video_size WxH -draw_mouse 0/1 -i :0.0+X,Y` | `-f rawvideo -pixel_format bgra -video_size WxH -framerate N -i -` (frames from the portal/screencopy, stopped by closing stdin) |
| DirectShow camera | AVFoundation device | `-f v4l2 -i /dev/videoN` | same |
| DirectShow audio | AVFoundation audio device (same input) | `-f pulse -i <source>` (`.monitor` = what's playing) or `-f alsa` | same |

macOS's FFmpeg screen input uses `AVCaptureScreenInput` (deprecated in
macOS 15; no system sound), so the plan there is raw frames from
ScreenCaptureKit as snapr records (`Input::RawVideo`). Devices:
`-f avfoundation -list_devices true -i ""`; `pactl list short sources`.
FFmpeg is found by ShareX's rule, then (only without a custom path) PATH on
Windows; beside the executable, the bundle's `Resources`, Homebrew,
MacPorts, PATH on macOS (Finder-started apps don't get the shell's PATH);
PATH, `/usr/bin`, `/usr/local/bin` on Linux. Hardware codecs: NVENC works on
Linux with NVIDIA's driver; AMF/QSV are Windows-oriented (VAAPI and
VideoToolbox aren't ShareX codecs, so they aren't offered: a ShareX profile
must stay readable by ShareX).

**Untested**: every macOS and Linux path (inputs, device listing, `pactl`,
fallback locations, .NET-style argv splitting in a real spawn; the argument
strings are unit-tested only), ddagrab and gdigrab against a real screen
(not allowed here: the desktop belongs to another agent), the HKCU writes
against a real screen-capture-recorder, NVENC/AMF/QSV encoders actually
encoding, a 32-bit Windows.

### Not ported yet

- The native recorder (§2), which is ShareX's default: until it is,
  ShareXrs should run FFmpeg for `ScreenRecordUseNative` too (its GIFs then
  come from palettegen, not the adaptive encoder; no WASAPI loopback, no
  camera overlay).
- All UI: the recording frame, the recorder bar, the FFmpeg options window,
  the abort confirmation, tray icon states, the output box for FFmpeg errors,
  the mouse highlighter during recording.
- The start flow's app parts: region selection for recording, active window
  and client area, `ScreenRecordRegion`, the after capture window, the file
  task, `HandleExistsFile`'s dialog.
- `GetVideoInfo` is ported as ShareX has it, which finds no streams in
  FFmpeg 9's output (`Stream #0:0[0x1](und)`): ShareX's patterns predate the
  stream ids; only duration and bitrate come out (same in ShareX).

### The hook into the app

1. `App::run_job` (`crates/sharexrs/src/main.rs`): the eight recorder
   `HotkeyType`s → a `ScreenRecordManager` in the app (`StartStopRecording`):
   while a session runs, the same job calls `control.start_stop_recording()`;
   `StopScreenRecording` → `control.request_stop()` (ShareX's `StopRecording`
   stops FFmpeg without touching the frame's status), `PauseScreenRecording`
   → `pause_resume_recording`, `AbortScreenRecording` → `abort_recording`. Main menu/tray Capture ›
   Screen recording (GIF) (`main_menu.rs` `ScreenRecording`,
   `ScreenRecordingGif`) start Region recordings.
2. The area: Region → the region overlay in a select-only mode
   (`sharex_capture::region`, as `GetRectangleRegionAsync`; capture_job.rs's
   `prepare` minus the save); ActiveWindow → `capture.rs`'s active window
   rectangle; CustomRegion / LastRegion from the settings.
3. `discovery::resolve_ffmpeg` + `session::prepare_ffmpeg_recording`
   (errors → ShareX's message boxes), save `ScreenRecordRegion`.
4. A `RecordingControl` for the frame/tray/hotkeys; `run_ffmpeg_session` on a
   thread with `SessionHooks` from capture_job.rs's file naming
   (`GetScreenshotsFolder`, the `NameParser` pattern, `FileExistAction`),
   the action completed sound, and the output box.
5. The finished path → a file task in `tasks.rs` (the task list shows it;
   video thumbnails are still missing there), then the after upload
   tasks.
