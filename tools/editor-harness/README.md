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
