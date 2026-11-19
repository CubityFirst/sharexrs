# Native recorder harness

A headless .NET console app that drives **ShareX's own** native recorder
code (`ShareX.ScreenRecordingLib.dll` from the reference build) with
synthetic input, and writes what it produces, so
`crates/sharex-native-recorder` can be compared with it output for output.
Nothing is captured: no screen, no window, no camera, no audio device.

It follows `ScreenRecorder.Run` without `GraphicsCapture`, `CameraCapture`
and `WasapiCapture`:

- frames go straight into `GpuVideoProcessor.Canvas` (`UpdateSubresource`),
  where Windows Graphics Capture would copy them;
- camera frames are Media Foundation memory samples given to
  `CameraOverlay.Update`, as `CameraCapture`'s source reader delivers them
  (with `ReadSelectedFormat`'s defaults: BT.709 above 576 lines, else
  BT.601; limited range; a negative default stride for bottom-up RGB32);
- sound goes straight into `AudioMixer.Write`, where `WasapiCapture.Drain`
  writes it, on a `RecordingClock` started at 0.

Then ShareX's own classes do the rest: `GraphicsDevice` (default adapter),
`MediaFoundationWriter.Create` (H.264, or HEVC past H.264's limits; the
encoder Windows picks with `RequireHardwareEncoder`), `GpuVideoProcessor`
(`Convert` to NV12, `GetBgraFrame`), `VideoTexturePool`, `CameraOverlay`
with `CameraCircleShader`, `GifWriter` (`TryWriteVideo`, `Finish`) and
`AudioMixer` (`Write`, `Read`), in `Run`'s order: rent a texture, update the
camera, convert or cache the frame, write it, then the audio 50 ms behind.
They're internal, so they're reached by reflection (span methods through
delegates). `WriteAudioUntil` is re-typed (ShareX's returns without a
writer, which the mixer cases have none of).

Besides the recording, the hardware classes' parts that run headlessly
(`Hardware.cs`, and `examples/harness_hardware` on ShareXrs's side):

- **`devices`**: `AudioCaptureDevices.GetSystemAudioDevices`,
  `GetMicrophones`, `GetDefaultSystemAudioDevice`, `GetDefaultMicrophone`
  and `CameraCaptureDevices.GetCameras`: read-only enumeration, whatever
  the machine has (no stream is opened, no device is activated);
- **`drain` cases**: `WasapiCapture.Drain` itself (by reflection, on an
  uninitialised `WasapiCapture` and `Endpoint`) over a fake
  `IAudioCaptureClient` (a hand-made vtable) that hands out the cases'
  packets: frames, flags (discontinuity, silent, timestamp error) and QPC
  times, in batches; one or two endpoints, gains, gaps, jitter, packet sizes
  from 1 to 4096 frames. The PCM is read on the recorder's schedule (50 ms
  behind each batch), plus the discontinuity count. No timestamp error on an
  endpoint's first packet: that one uses the real clock (`RecordingClock.Now`);
- **`hdr` cases**: `HdrToneMapShader.Draw` on a synthetic FP16 scRGB
  texture (values exact in FP16, negative and NaN ones included) into a
  black BGRA canvas at an offset, with `MonitorColorInfo` (SDR white, peak)
  from the case.

## Outputs (per case, in the output folder)

| File | What |
|---|---|
| `<case>.gif`, `<case>.mp4` | the recording |
| `<case>.nv12` | the NV12 the encoder was given for the first three frames (read back from the pool's textures) |
| `<case>.frames` | GIF cases with a camera: every composited BGRA frame (`GetBgraFrame`) |
| `<case>.pcm` | mixer cases: the 16-bit stereo PCM `Read` gives on the recorder's schedule |
| `<case>.json` | the encoder (name, hardware, D3D11), the output size, every sample's time/duration/discontinuity, GIF progress; drain cases: the discontinuities |
| `<case>.bgra` | HDR cases: the canvas after tone mapping |
| `devices.json` | the device lists and defaults |

## Synthetic input

`cases.json` says what to generate; `Synthetic.cs` here and
`crates/sharex-native-recorder/examples/synthetic/mod.rs` generate it the
same way with integer maths (one `float` division per audio sample):
patterns `box` (a moving box and a counter bar over a still background, so
GIF frames change in small rectangles), `scroll` (everything changes) and
`noise` (thousands of colours, for the quantizer and dithering); `still`
ranges repeat a frame (GIF duplicates), `drop` skips slots (dropped frames,
MP4 discontinuities); cameras in NV12, YUY2 and RGB32 (bottom-up too),
rectangle and circle, every corner; sound from one or both sources with
gains, variable packet sizes, timestamp jitter inside and outside the
mixer's dead band, clock drift (±500/800 ppm), gaps, discontinuities,
silent and NaN packets, packets larger than the mixer's buffer.

## Build and run

Needs the .NET SDK that builds ShareX (net11.0), the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (override with
`-p:ShareXBuild=<dir>`), and a GPU with a D3D11 video processor and a Media
Foundation encoder. `CASE=<name>` runs one case.

```
dotnet build tools/native-recorder-harness -c Release -o target/native-recorder-harness/bin
target/native-recorder-harness/bin/NativeRecorderHarness.exe tools/native-recorder-harness/cases.json target/native-recorder-harness/sharex
cargo run -p sharex-native-recorder --example sharex_native_recorder_harness -- tools/native-recorder-harness/cases.json target/native-recorder-harness/sharexrs
cargo run -p sharex-native-recorder --example sharex_native_recorder_harness -- --compare target/native-recorder-harness/sharex target/native-recorder-harness/sharexrs target/native-recorder-harness/report.md
```

`--compare` checks GIF, PCM, NV12 and composited frames byte for byte, the
JSON value for value, and each MP4: bytes; bytes with the `mvhd`/`tkhd`/
`mdhd` creation and modification times zeroed; the box tree; every sample
of every track (through `stsz`/`stsc`/`stco`); the codec configuration and
timing tables; then both files decoded with Media Foundation's software
decoders (frame count, timestamps, durations, pixels, PCM). It exits
non-zero when something differs.

## Results (2026-11-19, RTX 4080, NVIDIA's H.264/HEVC Encoder MFTs)

**65/65 outputs match: 58 byte-identical, 7 equivalent.** The 50 below,
and the hardware cases: `devices.json` identical (5 outputs, 5 inputs,
both defaults, 2 cameras on that machine), 5/5 drain PCM streams and their
discontinuity counts identical, 4/4 HDR canvases byte-identical.

### 2026-11-18

**50/50 outputs match: 43 byte-identical, 7 equivalent.**

- GIF: 9/9 files byte-identical (box, noise, duplicates and drops, odd size,
  50 fps, a frame longer than 655.35 s, three camera overlays), and their
  composited camera frames byte-identical (the circle shader included).
- Mixer: 8/8 PCM streams byte-identical.
- MP4 encoder input: 7/7 NV12 dumps byte-identical (the video processor's
  BT.709 conversion, camera composition by the second stream, scaling).
- MP4: 6/7 byte-identical but the container's creation and modification
  times (H.264 with and without AAC, 60 fps with drops and discontinuities,
  odd size, both camera paths, HEVC above 4096 wide). The seventh (two
  sources, AAC 128 kb/s) has every sample of both tracks identical, the
  same `stsd`/`avcC`/`esds`/`stts`/`ctts`/`stss`/`stsz`, and identical
  decoded frames and PCM; only the chunk interleaving (`stsc`, `stco`, the
  order inside `mdat`) differs, because Media Foundation's sink writer
  interleaves as the asynchronous hardware encoder delivers: ShareX's own
  runs of that case differ from each other the same way.
- JSON: 16/16 identical (encoder names, hardware/D3D11 flags, output sizes,
  every sample time, GIF progress).

Not covered here: anything that opens a device or the desktop (WGC
sessions and frames, WASAPI streams, the camera's source reader and its
mode negotiation on a real camera); a software encoder (`ReadwriteEnableHardwareTransforms`
makes Windows pick the GPU's whenever there is one), other GPUs' drivers
(the NV12 conversion is the driver's), and everything the capture classes
do (WGC, WASAPI devices, the camera's source reader).
