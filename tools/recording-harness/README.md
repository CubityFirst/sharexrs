# Recording harness

A headless .NET console app that runs **ShareX's own** FFmpeg recorder code
(`ShareX.ScreenCaptureLib.dll`, `ShareX.HelpersLib.dll` from the reference
build) over the cases in `cases.json` and writes the FFmpeg command lines
ShareX would run, so `crates/sharex-screen-recording` can be compared with
ShareX string for string. FFmpeg never starts, nothing is recorded, no window
opens.

[Harmony](https://github.com/pardeike/Harmony) 3.0 (the first release that
patches on CoreCLR 11) replaces four methods while the cases run:

- `FFmpegCLIManager.Run(string)`: the argument string is captured instead of
  starting FFmpeg (and `ConcatenateVideos`' list file is read while it
  exists); a `dshow` case's canned output is appended to `Output`, as FFmpeg
  would have written it.
- `DesktopScreen.AllScreens`: the case's monitors (ddagrab picks one), so
  multi-monitor layouts can be tested without the real ones.
- `RegistryHelpers.CreateRegistry(path, name, int, hive)`: the
  screen-capture-recorder values are captured; the registry isn't touched.
- `FileHelpers.CreateDirectoryFromFilePath`: no folders are created.

Everything else is ShareX's code as it runs: `ScreenRecorder` (constructor,
`StartRecording`, `FFmpegEncodeVideo`, `FFmpegEncodeAsGIF`),
`ScreenRecordingOptions.GetFFmpegCommands`/`GetFFmpegArgs(isCustom)`,
`FFmpegOptions` read from the case's JSON with Newtonsoft and ShareX's
settings rules (enums by name or number, a value that doesn't convert keeps
its default), `FFmpegCLIManager.GetDirectShowDevices`, `ConcatenateVideos`,
and the private `FFmpeg_DataReceived` (called by reflection with the case's
lines) for the encode start and progress events.

## Build and run

Needs the .NET SDK that builds ShareX (net11.0) and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (override with `-p:ShareXBuild=<dir>`).

```
python tools/recording-harness/make_cases.py tools/recording-harness/cases.json
dotnet build tools/recording-harness -c Release -o G:/scripts/sharexrs-ref/recording-harness/bin
G:/scripts/sharexrs-ref/recording-harness/bin/RecordingHarness.exe tools/recording-harness/cases.json G:/scripts/sharexrs-ref/recording-harness/sharex.json G:/scripts/sharexrs-ref/recording-harness/work
cargo run -p sharex-screen-recording --example harness -- tools/recording-harness/cases.json G:/scripts/sharexrs-ref/recording-harness/sharex.json G:/scripts/sharexrs-ref/recording-harness/work
```

The last step runs the same cases through ShareXrs (FFmpeg replaced by a
capture through `FFmpegCliManager::set_run_override`, the registry through
`ScreenRecorder::set_registry_writer`), compares every field, writes
`<work>/rust.json` and `<work>/report.md`, and exits non-zero if any case
differs. Errors are compared by presence (ShareX throws where ShareXrs
returns `None`).

## Cases

`make_cases.py` writes `cases.json` (checked in). Kinds:

- `record`: a `ScreenRecorder` for the options, then `StartRecording`: the
  command line and registry writes, plus `Extension`, `IsEvenSizeRequired`,
  `IsSourceSelected`. Every video codec with and without sound, every audio
  codec with video and alone, no source, x264 CRF/bitrate/presets, x265,
  VP8/VP9, Xvid, NVENC presets × tunes, AMF usages and qualities, QSV
  presets, audio bitrates and qualities, frame rates, negative and odd
  regions, cursor off, durations (2.25 s is "2.3": .NET's rounding),
  gdigrab/ddagrab/DirectShow/screen-capture-recorder sources (any case),
  ddagrab on one and three monitors (which one, the offset, even sizes,
  NVENC without `hwdownload`), quotes and backslashes in device names, user
  arguments, custom commands (placeholders in any case, trimming, lossless
  ignoring them, no output path), lossless first passes, output paths
  (no extension, dotted folders, trailing dot), unknown and numeric enum
  values.
- `commands`: `GetFFmpegCommands` alone (null output and input paths).
- `custom_args`: `GetFFmpegArgs(true)`, the custom command box's starting
  text.
- `encode_video`, `encode_gif`: the second passes; GIF for every stats mode ×
  dither.
- `concat`: `ConcatenateVideos`' arguments and list file.
- `dshow`: `GetDirectShowDevices` on device lists as old and new FFmpegs print
  them.
- `progress`: lines through `FFmpeg_DataReceived`: "Press [q] to stop" (any
  case), the duration, progress percentages (raw floats and the whole-percent
  changes `ScreenRecorder` reports), `Output`.

Real FFmpeg runs (start, `q`, pause/resume, restart, abort, kill, GIF and
two-pass encodes) are `crates/sharex-screen-recording/tests/ffmpeg.rs`, on
`-f lavfi` test sources: `SHAREXRS_TEST_FFMPEG=<ShareX's ffmpeg.exe> cargo
test -p sharex-screen-recording --test ffmpeg`.
