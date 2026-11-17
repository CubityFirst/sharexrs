# Image effects harness

A headless .NET console app that runs **ShareX's own** image effects
(`ShareX.ImageEffectsLib.dll`, `ShareX.HelpersLib.dll`, SkiaSharp/libSkiaSharp
from the reference build) on fixed test images and writes the raw pixels, so
`crates/sharex-image-effects` can be compared with ShareX byte for byte. No
window, no UI context, no desktop.

## Build and run

Needs the .NET SDK that builds ShareX (net11.0) and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (override with `-p:ShareXBuild=<dir>`).

```
dotnet build tools/effects-harness -c Release -o G:/scripts/sharexrs-ref/effects-harness/bin
G:/scripts/sharexrs-ref/effects-harness/bin/EffectsHarness.exe tools/effects-harness/cases.json G:/scripts/sharexrs-ref/effects-harness/work [case prefix]
cargo run --release -p sharex-image-effects --example effects_validate -- tools/effects-harness/cases.json G:/scripts/sharexrs-ref/effects-harness/work [case prefix]
```

The last step writes `<work>/report.md` (per case: size check, largest
difference per premultiplied channel, share of differing pixels; then the preset
round trips and defaults), `<work>/sharexrs/*` and `<work>/diff/*.png`.

## What it writes

- `inputs/`: the test images as `.sxrb` (`"SXRB"`, u32 width, u32 height, then
  premultiplied BGRA, ShareX's `SKBitmap` layout) and `.png` previews, plus
  `watermark.png`, `background.png` and `particles/` for the image effects.
  Generated with integer maths only.
- `sharex/<case>.sxrb|png`: ShareX's result for each case.
- `roundtrip/<case>.json`: the case's preset as ShareX serialises it back
  (export form, `ImageEffectsSerializationBinder`).
- `defaults/<Type>.json` / `<Type>.sxie.json`: each effect's default instance in
  the settings and export forms; `names.txt` (type, category, name, `ToString()`);
  `default-preset.json`; `named.json`.
- `probe.txt`: Skia's font metrics and text widths for a few fonts.

## Cases

`cases.json`: `{ "name", "input", "seed"?, "effects": [ effect JSON in the export
form ] }`. `{inputs}` in strings becomes the inputs folder. Random effects are
made deterministic by swapping `RandomFast`'s generator for `new Random(seed)`'s
(the .NET Framework compatible one), which `RandomFast::seeded` reproduces.
