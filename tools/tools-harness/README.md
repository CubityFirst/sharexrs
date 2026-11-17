# Tools harness

A headless .NET console app that runs **ShareX's own** tool code
(`ShareX.Tools.dll`, and the `ShareX.HelpersLib.dll`, `ShareX.ScreenRecordingLib.dll`
and `ShareX.dll` helpers the tools call, from the reference build) over the
cases in `cases.json`, and writes what each tool produces, so `crates/sharex-tools`
can be compared with ShareX byte for byte. No window opens, nothing touches the
desktop, the clipboard or the network; FFmpeg only runs on synthetic videos in
the work folder. docs/TOOLS.md has the results.

## Build and run

Needs the .NET SDK that builds ShareX (net11.0), the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (override with `-p:ShareXBuild=<dir>`),
and ShareX's portable `ffmpeg.exe` in `G:/scripts/sharexrs-ref/ShareX-22.4.0-portable`
(`DataCases.FFmpegPath`; without it the video cases and the video metadata
inputs are skipped).

```
python tools/tools-harness/make_cases.py tools/tools-harness/cases.json
dotnet build tools/tools-harness -c Release -o G:/scripts/sharexrs-ref/tools-harness/bin
G:/scripts/sharexrs-ref/tools-harness/bin/ToolsHarness.exe tools/tools-harness/cases.json G:/scripts/sharexrs-ref/tools-harness/work [case prefix]
cargo run --release -p sharex-tools --example validate -- tools/tools-harness/cases.json G:/scripts/sharexrs-ref/tools-harness/work [case prefix]
```

The last step runs every case through the Rust port, writes `<work>/sharexrs/*`
and `<work>/report.md`: per tool, how many cases are exact (byte for byte, or
the same values), pixel-exact (same decoded pixels, different encoder bytes),
close (within a stated tolerance), differ, or skipped (with the reason).

The crate's CI tests (`crates/sharex-tools/tests/sharex.rs`) use a small copy
of the inputs and outputs, refreshed after a harness run with

```
python tools/tools-harness/make_fixtures.py G:/scripts/sharexrs-ref/tools-harness/work crates/sharex-tools/tests/fixtures
```

## What it writes

- `inputs/`: the test inputs, generated with integer maths and ShareX's own
  encoders:
  - `Inputs.cs`: opaque, transparent and odd-sized PNGs, a 16-bit PNG, a
    grayscale PNG, JPEGs (one plain, three with EXIF orientations 3, 6 and 8),
    a palette GIF, a 24-bit BMP, a lossless WebP, a watermark logo.
  - `DataCases.cs`: files for the hash checker (`hash/`), folder trees for
    the indexer (`folders/`: hidden entries, culture-sorted and HTML-special
    names), QR and barcode pictures to scan (`qr/`), a custom CSS.
  - `MetaInputs.cs` (`inputs/meta/`): files carrying every kind of metadata the
    Metadata tool reads (EXIF, GPS, XMP, ICC, IPTC, PNG chunks, GIF
    extensions, TIFF, WebP chunks), and MP4/MOV/MKV/WebM/AVI/WMV clips written
    by ShareX's FFmpeg with tags.
  - `SystemCases.cs` (called from `DataCases.cs`): `trim.gif` (five frames with mixed delays, offsets and
    disposals, by ShareX's `AnimatedGifCreator`).
- `sharex/<case>.*`: the tool's output files as ShareX writes them,
  `<case>.sxrb` for a bitmap before encoding (`"SXRB"`, u32 width, u32 height,
  premultiplied BGRA), `<case>.json` for values (counts, strings, names,
  command lines, metadata), and `<case>.error.txt` when ShareX itself fails on
  a case (stale ones are deleted on each run).

## How each tool is driven

- Image tools (`Program.cs`): ShareX's public services are called directly
  where they exist (`SkiaImageHelpers.CombineImages`, `ImageResizerService`,
  `ImageConverterService`, `ImageWatermarkService`, `IconConverterService`,
  `ImageComparisonService`, `AnimatedGifMakerService`). Logic that lives in a
  view model runs through the view model without a window
  (`ImageThumbnailerViewModel.GenerateThumbnails`, by reflection), or through
  its private static method (`ImageSplitterViewModel.SplitImage`).
- Data and video tools (`DataCases.cs`): `HashChecker`,
  `TaskHelpers.GenerateQRCode`/`BarcodeScan` and ZXing's SVG writer as
  `QRCodeViewModel` uses it, `Indexer` with each output's settings,
  `MetadataService` (read, copy text, `CanStripMetadata`, strip),
  `VideoConverterOptions` and `VideoConverterViewModel`'s private
  `BuildArguments`, `VideoConversionOutputInfo` over real FFmpeg probes,
  `VideoTrimmerService`'s argument builder, `VideoThumbnailer` on a synthetic
  clip.
- System tools (`SystemCases.cs`): `GifLosslessTrimmer` and
  `AnimatedGifTrimmerDocument`'s timing; `MonitorTestPatternControl` rendered
  to a `RenderTargetBitmap` through Avalonia.Headless (Skia, no window).

The image, data and video tools need no Avalonia application (their view
models only use CommunityToolkit's observable properties); the Monitor test
starts a headless one. Culture comparisons go through Windows NLS as in
ShareX (`System.Globalization.UseNls`, as ShareX.runtimeconfig.json sets).

## Cases

`make_cases.py` writes `cases.json` (1321 cases): `{ "name", "tool",
...parameters }`, with input file names relative to `inputs/`. Each tool's
parameter sets cover its options; docs/TOOLS.md lists what they cover per
tool.
