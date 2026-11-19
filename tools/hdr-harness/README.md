# HDR harness

A headless .NET console app that drives **ShareX's own** HDR screenshot
correction (`HDRScreenCapture` in `ShareX.ScreenCaptureLib.dll` from the
reference build, internal, so reached by reflection) over synthetic HDR
frames, so `crates/sharex-capture/src/hdr.rs` can be compared with it byte
for byte. Nothing is captured: no desktop duplication, no screen.

ShareX corrects a GDI capture with a DXGI duplication of each HDR output
(`ApplyColorCorrection`). The harness starts where the duplication's frame
is mapped (`CaptureOutput` after `Map`), with frames made here as DXGI
delivers them (`R16G16B16A16_Float` scRGB, `R10G10B10A2_UNorm` HDR10 PQ,
with row padding), and a made-up GDI capture: inside the output, the SDR
reference offset by -7..7 per channel (both sides of ShareX's 4-level
tolerance), a pattern elsewhere.

For each case it writes (BGRA, rows packed) the frame and GDI capture, then
`ConvertToSrgb`, `ConvertToSdrReference`, `ConvertWithWindowsToneMapper`
(WIC's `IWICBitmapToneMapper`, or the exception), both after `RotateOutput`,
and the GDI capture after `CaptureOutput`'s steps and
`CopyOutputIntersection`, once through Windows' tone mapper and once through
the fallback (as if the tone mapper threw). `Program.cs` lists the cases:
ramps, NaN/infinities/subnormals, wide-gamut colours, SDR white 1-125, peaks
clamped both ways, row pitch, all four rotations (and unspecified),
translucent pixels, captures overlapping an output at an offset or inside it,
a size mismatch (`CaptureOutput` returns null) and all 1024 PQ codes.

## Build and run

```
dotnet build tools/hdr-harness -c Release -o target/hdr-harness/bin
target/hdr-harness/bin/HdrHarness.exe target/hdr-harness/work
SHAREXRS_HDR_DIR=$PWD/target/hdr-harness/work cargo test -p sharex-capture --test hdr -- --nocapture
```

`crates/sharex-capture/tests/fixtures/hdr` is a copy of a run, so `cargo
test` compares against it without the harness (Windows only: WIC and the
CRT's `powf`).

## Result

168/168 outputs identical (19 cases: tone curve, reference, Windows'
tone mapper, both turned, both corrected captures). On this machine
(Windows 11 26300) WIC's tone mapper accepts FP16 but not HDR10
(`WINCODEC_ERR_UNSUPPORTEDPIXELFORMAT`), so HDR10 frames take ShareX's
fallback in both apps. A turned output is read back unpremultiplied by
`SkiaPixelBuffer`, so its translucent pixels lose precision and the SDR
reference's unmarked pixels turn black; ShareXrs does the same.
