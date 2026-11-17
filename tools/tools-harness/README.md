# Tools harness

A headless .NET console app that runs **ShareX's own** tool code
(`ShareX.Tools.dll`, and the `ShareX.HelpersLib.dll` / `ShareX.dll` helpers the
tools call, from the reference build) over the cases in `cases.json`, and writes
what each tool produces, so `crates/sharex-tools` can be compared with ShareX
byte for byte. No window opens, nothing touches the desktop, the clipboard or
the network; FFmpeg only runs on synthetic videos in the work folder.

## Build and run

Needs the .NET SDK that builds ShareX (net11.0) and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (override with `-p:ShareXBuild=<dir>`).

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

## What it writes

- `inputs/`: the test inputs, generated with integer maths (`Inputs.cs`):
  opaque, transparent and odd-sized PNGs, a 16-bit PNG, a grayscale PNG,
  JPEGs (one plain, three with EXIF orientations 3, 6 and 8), a palette GIF,
  a 24-bit BMP, a lossless WebP, a watermark logo.
- `sharex/<case>.*`: the tool's output files as ShareX writes them,
  `<case>.sxrb` for a bitmap before encoding (`"SXRB"`, u32 width, u32 height,
  premultiplied BGRA), `<case>.json` for values (counts, strings, names), and
  `<case>.error.txt` when ShareX itself fails on a case.

## How each tool is driven

ShareX's public services are called directly where they exist
(`SkiaImageHelpers.CombineImages`, `ImageResizerService`, `ImageConverterService`,
`ImageWatermarkService`, `IconConverterService`, `ImageComparisonService`,
`AnimatedGifMakerService`). Logic that lives in a view model runs through the
view model without a window (`ImageThumbnailerViewModel.GenerateThumbnails`, by
reflection), or through its private static method (`ImageSplitterViewModel.SplitImage`).
Nothing needs an Avalonia application: the view models only use
CommunityToolkit's observable properties.

## Cases

`make_cases.py` writes `cases.json`: `{ "name", "tool", ...parameters }`, with
input file names relative to `inputs/`. Each tool's parameter sets cover its
options (orientations, alignments, spacing, wrapping, resize modes, formats and
bit depths, JPEG subsampling and quality, GIF quantizers, watermark positions,
rotations and opacities, icon sizes and depths, comparison preview sizes, GIF
delays and loops).
