# OCR harness

A headless .NET console app that runs **ShareX's own** OCR (`ShareX.OCR.dll`, with
the ONNX Runtime, SkiaSharp and models of a ShareX build of `REFERENCE_COMMIT`) over
synthetic images, and writes the result and every intermediate, so
`crates/sharex-ocr` can be compared with ShareX bit for bit. No window opens,
nothing touches the desktop or the clipboard, and nothing leaves the machine: the
models are ShareX's, recognition runs on the CPU. docs/OCR.md has the results.

## Build and run

Needs the .NET SDK that builds ShareX (net11.0) and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (override with `-p:ShareXBuild=<dir>`), with
its `Models` folder and `onnxruntime.dll`.

```
dotnet build tools/ocr-harness -c Release -o G:/scripts/sharexrs-ref/ocr-harness/bin
G:/scripts/sharexrs-ref/ocr-harness/bin/OcrHarness.exe generate G:/scripts/sharexrs-ref/ocr-harness/cases
G:/scripts/sharexrs-ref/ocr-harness/bin/OcrHarness.exe run G:/scripts/sharexrs-ref/ocr-harness/cases G:/scripts/sharexrs-ref/ShareX-build/Models G:/scripts/sharexrs-ref/ocr-harness/work [case prefix]
ORT_DYLIB_PATH=G:/scripts/sharexrs-ref/ShareX-build/onnxruntime.dll cargo run --release -p sharex-ocr --example ocr_validate -- G:/scripts/sharexrs-ref/ocr-harness/cases G:/scripts/sharexrs-ref/ShareX-build/Models G:/scripts/sharexrs-ref/ocr-harness/work [case prefix]
```

The last step runs every case through ShareXrs with **the same ONNX Runtime
library** ShareX used and writes `<work>/report.md`: per case, whether the text,
the detection input pixels, the probability map, the boxes, the line crops, the raw
texts and scores, and the regions and lines are identical.

The crate's CI tests (`crates/sharex-ocr/tests/sharex.rs`) use a small copy of the
inputs and outputs, refreshed after a harness run with

```
python tools/ocr-harness/make_fixtures.py G:/scripts/sharexrs-ref/ocr-harness/cases G:/scripts/sharexrs-ref/ocr-harness/work crates/sharex-ocr/tests/fixtures
```

## What it writes

- `generate` (`Generate.cs`): `<cases>/<case>.png` and `cases.json` (name, size,
  description, the fonts SkiaSharp actually used). Text drawn with SkiaSharp in
  Windows fonts: several fonts, sizes, colours and backgrounds; lines, paragraphs and
  columns; rotated (5°, 15°, 30°, 90°), skewed and vertical CJK text; French, German,
  Spanish, Simplified and Traditional Chinese, Japanese, Greek, and Korean and
  Russian (not in the model's dictionary); a made-up settings window; sizes that make
  ShareX grow or shrink the detection input (mipmaps, odd sizes), very large and
  very long lines; a transparent background; shapes; a blank image.
- `run` (`Program.cs`), per case under `<work>/sharex/<case>/`:
  - `result.json`: `PaddleOCREngine.Recognize`'s regions (corners, text, score),
    lines and `GetText()` three ways; the detection input and map sizes, the boxes,
    the raw texts and scores, the crops' batch layout. Floats are also written as
    their bit patterns (`bits`, `scoreBits`, ...) for exact comparison.
  - `resized.bgra`: the detection model's input (`ImageTensor.Resize`).
  - `prob.f32`: the detection model's probability map (little-endian floats).
  - `crop_<i>.bgra`: each box's line image (`ImageTensor.Crop`).

  The final result is the public API; the intermediates call ShareX's internal
  `ImageTensor`, `TextDetector` and `TextRecognizer` through reflection, the way
  `PaddleOCREngine.Recognize` calls them (`ImageTensor.Fill` through a delegate, since
  it takes a `Span`).
