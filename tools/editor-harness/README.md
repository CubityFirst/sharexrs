# Editor harness

Renders image editor annotations with **ShareX's own code**, headless, so
ShareXrs's renderer (`crates/sharex-image-editor`) can be checked against it
pixel for pixel.

It references `ShareX.ImageEditor.dll`, `ShareX.Avalonia.dll` and
`ShareX.Icons.dll` from a build of the reference commit
(`G:\scripts\sharexrs-ref\ShareX-build`; override with
`-p:ShareXBuild=<dir>`), starts Avalonia on its **headless** platform with
**Skia** drawing (`UseHeadless(new() { UseHeadlessDrawing = false })`, no OS
window, no input, no clipboard), and for each case:

1. opens ShareX's `EditorWindow` (headless) and loads the base image,
2. adds the case's annotations to the editor's `EditorCore` and calls
   `EditorView.OnAnnotationsRestored` (what undo/redo does: the annotation
   layer is rebuilt through `AnnotationVisualFactory`, effect bitmaps and the
   spotlight overlay included),
3. saves `EditorView.GetSnapshot()`, the editor's own export
   (`RenderTargetBitmap` of the canvas), as `<case>.png`.

Annotations are ShareX's JSON (System.Text.Json with `Annotation`'s
`[JsonDerivedType]` discriminators): `{"$type": "Rectangle", "StrokeWidth":
5, "StartPoint": {"X": 40, "Y": 50}, ...}`; enums by name or number.

A case can also have:

- `"base"`: its own base image (relative to cases.json);
- image annotations' `ImagePath` (relative to cases.json), loaded with
  `ImageAnnotation.LoadImage`;
- `"configureFill": true`: each smart eraser clamped to the image and
  filled from it (`ClampVisualBounds`, `ConfigureFill`), as the tool does
  when it's drawn;
- `"ops"`: `EditorCore` operations run in turn after the annotations are
  added (`crop`, `performCrop`, `cutOut`, `performCutOut`, `rotate90cw`,
  `rotate90ccw`, `rotate180`, `rotate`, `flipH`, `flipV`, `resize`,
  `resizeCanvas`, `autoCrop`, `flatten`, `autoExpand`, `undo`, `redo`, and
  `clone` = `EditorView.CloneRegion`);
- `"snapshotRect"`: export with `GetSnapshot(Rect)` (region capture's,
  a `VisualBrush`) instead of `GetSnapshot()`.

Other modes: `--cursors <dir>` dumps every `CursorType`'s bitmap
(`WindowsCursorBitmapRenderer.CreateAnnotationBitmap`), `--emoji <sequence>
<size> <file>` an emoji sticker (`RenderStickerBitmap`), `--emoji-raw
<sequence> <canvas> <font size> <file>` Direct2D's raw emoji
(`RenderWithDirect2D`), `--probe-picture <dir>` the same draws directly
and through an `SKPicture` shader; `--text` and `--fonts` as before;
`--tooltip-theme x` prints what FluentTheme gives ShareX's `ToolTip`
(resources, control theme setters, default delays and offsets, a laid-out
tooltip's size) for the region capture's toolbar tooltips.
`cargo run -p sharex-image-editor --example image_editor_emoji_dump` dumps
ShareXrs's emoji the same way.

`--effects <dir> [id prefix]` (`Effects.cs`) runs the image effect catalogue
(`Core/ImageEffects`) without a window: it writes `catalog.json` (every
effect's id, name, browser label, icon, description, category and
parameters, as `EffectRegistry` and the parameter definitions report them),
generates `cases.json` (each effect on `inputs/a.png`, 48×36 opaque, and
`inputs/b.png`, 32×24 translucent, with its defaults, then each parameter at
its minimum, maximum and a third value, and every enum option; numeric
ranges capped at ±100; `{inputs}` in a file parameter is the inputs folder),
and for each case sets `Random.Shared`'s state from the case's seed
(splitmix64), applies the configured effect as the effect dialog does and
saves the result's premultiplied bytes as `sharex/<case>.png` (a throw is
recorded in `results.json`). `probe/` holds what the port checks its own
plumbing against (`SKBitmap.Pixels` tables, `Random.Shared`'s draws). The
outputs live in `crates/sharex-image-editor/tests/data/effects`:

```
dotnet build tools/editor-harness -c Release -o G:/scripts/sharexrs-ref/editor-harness-fx
dotnet G:/scripts/sharexrs-ref/editor-harness-fx/EditorHarness.dll --effects <dir> [id prefix]
cargo run -p sharex-image-editor --example image_editor_effects_report -- [id prefix] [out dir]
```

## Run

```
dotnet build tools/editor-harness -c Release -o G:/scripts/sharexrs-ref/editor-harness
dotnet G:/scripts/sharexrs-ref/editor-harness/EditorHarness.dll crates/sharex-image-editor/tests/data/cases.json <out dir> [case filter]
```

Then compare with ShareXrs:

```
cargo run -p sharex-image-editor --example render_cases -- <out dir> target/editor-out [case filter]
```

which prints, per case, the largest channel difference and how many pixels
differ. To update the references the tests use, copy the PNGs into
`crates/sharex-image-editor/tests/data/sharex/`
(`cargo test -p sharex-image-editor` then requires every case without a
`"pending"` note to match exactly).

Text renders with the fonts installed on the machine (Segoe UI on
Windows), through Skia's DirectWrite font manager.
