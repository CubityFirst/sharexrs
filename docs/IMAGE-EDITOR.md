# Image editor

ShareX 22.4's image editor (`ShareX.ImageEditor`, Avalonia) and its port:
`crates/sharex-image-editor` (annotation model, tool options, renderer) on
`crates/sharex-skia` (Skia m119's CPU rasteriser, shared with the image
effects). The editor UI isn't ported yet; the plan is at the end.

## Map of ShareX's editor

`ShareX.ImageEditor/` (a library; `ShareX.ImageEditor.App/` is a standalone
host around it):

| Folder | What's there |
|---|---|
| `Core/Annotations` | The annotation model: `Base/Annotation.cs` (common properties, `[JsonDerivedType]` discriminators), `Shapes/` (Rectangle, Ellipse, Line, Arrow, Freehand, Image, Emoji, Cursor, SmartEraser, Crop, CutOut, `CurvedSegmentHelper`), `Text/` (Text, Number = the Step tool, SpeechBalloon, `TailGeometryHelper`), `Effects/` (`BaseEffectAnnotation`, Highlight, Blur, Pixelate, Magnify, Spotlight), enums (`ArrowStyle`, `BorderStyle`, `StepType`, `CursorType`, `TextHorizontalAlignment`, `AnnotationCategory`). Each annotation's `*.Visual.cs` builds its Avalonia control (`CreateVisual`). |
| `Core/Editor` | `EditorCore` (source image, annotation list, selection, crop/cut-out/flatten, `GetSnapshot` fallback), `EditorTool` (the 21 tools). |
| `Core/History` | `EditorHistory`/`EditorMemento`: undo/redo as annotation-list snapshots. |
| `Core/ImageEffects` | The editor's own image effects (Adjustments, Drawings, Filters, Manipulations, Parameters: ~240 files), a separate catalogue from `ShareX.ImageEffectsLib`. |
| `Integration` | `ImageEditorIntegration.ShowEditorDialog` (what ShareX calls), `ImageEditorOptions` (tool settings, kept in `TaskSettings.ToolsSettings.ImageEditorOptions`), `ImageEditorToolbarItemOptions`, recent colours, clipboard and wallpaper services. |
| `Presentation/Views` | `EditorWindow` (a window around one `EditorView` with `UseBuiltInToolbars`), `EditorView` (split in partials: core bridge, toolbar handlers, effects host, clipboard, canvas resize, auto-expand, emoji picker, image insert, subscriptions), `EditorBuiltInToolbars`, `BackgroundSettingsPanel`, the dialogs (confirmation, new image, insert image, emoji picker, start screen, toolbar customisation). |
| `Presentation/Controls` | `AnnotationToolbar` and its pickers (colour, border style, arrow style, corner radius, cursor type, font family/size, number), `EffectBrowserPanel`, `SKCanvasControl` (the image), `SpotlightOverlayControl`, `SpeechBalloonControl`, `OutlinedTextControl`, `SelectionResizeNode`. |
| `Presentation/Controllers` | Input (tool gestures), selection (handles, text editing), zoom. |
| `Presentation/Rendering` | `AnnotationEffectVisualUpdater` (effect bitmaps into `ImageBrush`es), `AnnotationCoordinateHelper`, `CropAdorner`, `WindowsCursorBitmapRenderer`, SkiaSharp conversions. |
| `Presentation/ViewModels` | `MainViewModel` (partials: tool options, canvas, image, background, effect preview, notifications, toolbar customisation, lifetime), dialog view models, `EditorToolbarAdapter`. |
| `Presentation/Effects`, `Theming`, `Emoji`, `EasterEggs` | Effect catalogue/search for the browser, theme resources, emoji data, the shader easter egg. |

### How ShareX uses it

- **After capture / Tools › Image editor**: `TaskHelpers` calls
  `ImageEditorIntegration.ShowEditorDialog(image, options, callbacks)`, which
  opens `EditorWindow` modally and returns the edited `SKBitmap` (or null).
  `TaskSettingsWindow` edits `ImageEditorOptions`.
- **Region capture**: `RegionCaptureWindow.axaml` hosts the same `EditorView`
  as its workspace (`<views:EditorView x:Name="EditorWorkspace"/>`). The
  window calls `ConfigureForFullscreenWorkspace()` and
  `LoadWorkspaceImage(screenshot, opaque)`, attaches its own
  `AnnotationToolbar` (`AttachHostAnnotationToolbar`), forwards pointer
  input, cancels interactions on Escape, and on confirm renders the selected
  rectangle with annotations through `EditorView.GetSnapshot(Rect)` (no second
  full-desktop bitmap). Annotations drawn during region capture are the same
  annotation objects the editor uses.

### How it renders (the output we match)

The exported image is `EditorView.GetSnapshot()`: the `PreviewFrame` subtree
(the image in `SKCanvasControl`, the spotlight overlay, the annotation
`Canvas` of per-annotation Avalonia controls; selection handles hidden)
arranged at 1:1 and rendered into a 96-DPI `RenderTargetBitmap`. That is
Avalonia 12.1.6's `ImmediateRenderer` on Skia m119 (SkiaSharp 3.119.4), so
the pixels come from:

- **Avalonia layout**: positions rounded half-to-even, sizes rounded up
  (`LayoutHelper` at scale 1); render transforms (rotation about the centre).
- **Shapes** (`Shape.Render` → `DrawGeometry(fill, pen, geometry)`):
  `Rectangle`/`Ellipse` geometry is the bounds deflated by half the stroke;
  `StreamGeometry` keeps a separate fill path for unclosed figures (EvenOdd
  by default); the fill is drawn, then the stroke; pen dashes are
  `dashes × thickness`, miter limit 10, round caps for solid lines, flat
  otherwise.
- **Skia**: `ShouldUseAAA` picks analytic AA (`SkScan_AAAPath`) or the
  supersampler; strokes through `SkStroke` (round caps/joins as conics);
  solid colours through the legacy N32 blitters; edges clipped by
  `SkEdgeClipper`.
- **Effects**: each effect annotation computes a bitmap from the source
  image (Gaussian blur with clamped edges, pixelate as `scalePixels` down
  linear / up nearest, magnify as a Mitchell `drawImageRect`, highlight as a
  per-channel cap) and fills its `Rectangle` with it through an `ImageBrush`
  aligned 1:1; spotlights share one overlay (darkened, optionally blurred
  image with each spotlight cleared) drawn below the annotations.
- **Drop shadows** (`ShadowEffectHelper`, `DropShadowEffect`): a layer bounded
  by the visual grown by the blur padding, restored through
  `SkImageFilters::DropShadow` (sigma `0.288675 × r + 0.5`).
- **Text** (`TextAnnotation`): `FormattedText.BuildGeometry` (HarfBuzz
  advances in doubles, `TextLineImpl` metrics, `SKFont.GetGlyphPath`
  outlines added at float pen positions, NonZero) filled and stroked like a
  shape. **Step and speech balloon text** use `DrawingContext.DrawText`,
  which Skia rasterises through DirectWrite glyph masks.

## Annotation types

Common (`Annotation`): `StrokeColor` = `#ef4444`, `StrokeWidth` = 4,
`FillColor` = `#00000000`, `ShadowEnabled` = false, `ShadowColor` =
`#80000000`, `ShadowBlurRadius` = 4, `ShadowOpacity` = 1, `ShadowOffsetX/Y` =
3, `StartPoint`/`EndPoint`, `RotationAngle` = 0, `ZIndex`, `Id`, `ToolType`.
JSON is ShareX's System.Text.Json shape: `{"$type": "Rectangle", ...}`,
missing properties taking the constructor's defaults.

| `$type` (tool) | Own properties = default | Rendered by ShareXrs |
|---|---|---|
| `Rectangle` | `BorderStyle` = Solid, `CornerRadius` = 0 | ✅ |
| `Ellipse` | `BorderStyle` = Solid | ✅ (dotted/dash-dot: see below) |
| `Line` | `BorderStyle` = Solid, `CurvePoint`, `CurvePointActivated` = false | ✅ (1 px: see below) |
| `Arrow` | `Style` = Classic (Modern, Double, Basic, Line), `RoundedCorners` = true, `CurvePoint`, `CurvePointActivated` | ✅ |
| `Freehand` | `Points`, `BorderStyle`, `ShowArrowHead` = false, `SmoothingTolerance` = 2 | ✅ |
| `Highlight` | `Amount` = 10, `IsFreehand`, `IsEllipse` (base effect) | ✅ |
| `Blur` | same | ✅ |
| `Pixelate` | same | ✅ |
| `Magnify` | `Amount` = 2, `IsEllipse`, `BorderStyle` | ✅ rectangle; elliptical ❌ |
| `Spotlight` | `DarkenOpacity` = 180, `BlurAmount` = 0, `IsEllipse` | ✅ |
| `Text` | `Text`, `TextColor` = `#FF000000`, `FontSize` = 48, `FontFamily` = Segoe UI, `IsBold`, `IsItalic`, `HorizontalAlignment` = Center | ✅ |
| `Number` (Step) | `Number` = 1, `StepType` = Numeric, `FontSize` = 24, `TextColor` = white, `IsBold` = true, `TailPoint`, `TailEnabled` = true | ⚠️ circle exact, glyphs and tail differ |
| `SpeechBalloon` | `TailPoint`, `TailEnabled` = true, `AutoSize`, `Text`, `FontSize` = 20, `FontFamily`, `TextColor`, `IsBold`, `IsItalic`, `HorizontalAlignment`, `CornerRadius` = 10 | ⚠️ body exact, glyphs and tail differ |
| `Image` | `ImagePath` | ❌ model only |
| `Emoji` | `UnicodeSequence`, `DisplayName` | ❌ model only |
| `Cursor` | `CursorType` = Default | ❌ model only |
| `SmartEraser` | rectangle's + `FillMode` = SolidColor, `EdgePixels` | ❌ model only |
| `Crop`, `CutOut` | (operations on the image, not drawn) | ❌ |

`ImageEditorOptions` (tool defaults the toolbar starts from) is ported field
for field in `sharex_image_editor::options` (e.g. `BorderColorHex` =
`#FFF23C3C`, `Thickness` = 4, `TextFontSize` = 48, `StepFontSize` = 30,
`BlurStrength` = 30, `PixelateStrength` = 20, `MagnifierStrength` = 2,
`SpotlightStrength` = 30, `HighlightFillColorHex` = `#FFFFFF00`);
`sharex-settings` keeps it as raw JSON in the task settings, so it
round-trips untouched.

## The crates

- `sharex-skia`: Skia m119, ported with its arithmetic. `path` (`SkPath`),
  `aaa` (analytic AA), `saa` (supersampler incl. `MaskSuperBlitter`), `scan`
  (`AntiFillPath`, `AntiFillRect`), `clip` + `cubics` (`SkEdgeClipper`, the
  double-precision cubic solvers), `stroke` (`SkStroke`), `blitter` (legacy
  N32 blitters), `blur`, `resample` (`scalePixels`, Mitchell), `raster` (the
  image effects' pipeline stages, sprite blitter, matrices, shaders,
  gradients, `Bitmap`, `Canvas`). The image effects crate uses it too.
- `sharex-image-editor`: `model` (annotations, JSON), `options`
  (`ImageEditorOptions`), `render` (`render(base, annotations)`: shapes,
  effects, text, shadows).

## Validation

`tools/editor-harness` (see its README) runs ShareX's own editor headless
(Avalonia headless platform with Skia drawing; no window, no input) on
`crates/sharex-image-editor/tests/data/cases.json` (60 cases on a 400×300
image) and saves `EditorView.GetSnapshot()`. Those PNGs are the references
in `tests/data/sharex/`; `cargo test -p sharex-image-editor` requires every
case without a `"pending"` note to be identical, and
`cargo run -p sharex-image-editor --example render_cases -- <sharex dir>`
prints per case the largest channel difference and the share of differing
pixels.

**Result: 51 of 60 cases identical, pixel for pixel**: every rectangle
(plain, odd width, radius, dashes, rotated, off-canvas, shadows), ellipse
(solid, filled, rotated, off-canvas), line (curved, wide, 2 px dashed),
arrow (all five styles, straight and curved, zero length, thick, shadow),
freehand (plain, arrow head, dashed, two points), highlight (4 cases incl.
fractional and off-canvas), blur (3), pixelate (2), rectangular magnify,
spotlight (rectangle; ellipse with blur), text (3, Segoe UI), a stack of
overlapping annotations and small shapes.

Not identical, and why:

| Case | Max Δ, pixels differing | Why |
|---|---|---|
| ellipse-dot, ellipse-dashdot | 60, 0.43% / 0.40% | Dashes along conics: Skia's `SkDashPath` measures the conic itself; ShareXrs still dashes through tiny-skia (quads). |
| line-thin (1 px) | 53, 0.88% | A stroke ≤ 1 device pixel is a hairline in Skia (`SkScan_Antihair`), not ported. |
| magnify-ellipse-border | 250, 16.8% | The elliptical magnifier clips with an anti-aliased clip (`SkAAClip`), not ported. |
| step, step-letter | 215–223, 0.03–0.07% | Glyphs: `DrawText` rasterises DirectWrite (hinted) masks; ShareXrs fills the same font's outlines at the same positions. The circle is exact. |
| step-tail, balloon | 222–255, 1.7–2.4% | The tail is a union with the body (`CombinedGeometry` → Skia PathOps), not ported; plus the glyphs. |
| balloon-notail | 165, 0.60% | Glyphs only. |

Fonts: text cases use the same font files as ShareX (Segoe UI from
`%WINDIR%\Fonts`; Avalonia's default Inter from `SHAREXRS_INTER_DIR`, dumped
by `EditorHarness --fonts`). Shaping is HarfBuzz in both (harfrust here), so
advances and positions agree; only `DrawText`'s DirectWrite masks can't be
reproduced by filling outlines.

Not ported yet: Image, Emoji, Cursor and Smart eraser rendering; rotated
effect annotations; `SkDashPath`, hairlines, `SkAAClip`, PathOps; crop,
cut-out and flatten.

## Plan for the editor UI

One egui widget, `EditorWorkspace`, used by both hosts, as ShareX shares
`EditorView`:

1. **Workspace widget** (`crates/sharexrs/src/editor/workspace.rs`): the
   source image as a texture, annotations drawn live with egui shapes while
   editing and through `sharex_image_editor::render` for the committed
   layer (cached per annotation, re-rendered on change, so what you see is
   the export), zoom/pan (`EditorZoomController`), selection handles and
   resize nodes (`EditorSelectionController`), tool gestures
   (`EditorInputController`), undo/redo as annotation-list snapshots
   (`EditorHistory`), text editing in place. Export = `render` over the
   image, cropped for region capture (`GetSnapshot(Rect)`).
2. **Annotation toolbar** (`AnnotationToolbar.axaml`) and its pickers,
   ported from the `.axaml` with `sharex_ui::theme` sizes and colours,
   `sharex_strings::image_editor` strings and `sharex_ui::keys` icons;
   options bound to `ImageEditorOptions` (saved in the task settings).
3. **Region capture overlay**: the existing region capture window hosts the
   workspace full-screen (`ConfigureForFullscreenWorkspace`), with the
   capture's annotation toolbar, Escape cancelling the current interaction
   first, and the confirmed region exported with its annotations.
4. **Standalone editor window** (`EditorWindow`): the workspace plus
   `EditorBuiltInToolbars`, background panel, start screen and dialogs (new
   image, insert image, confirmation, toolbar customisation, emoji picker),
   opened from after-capture tasks and Tools, returning the edited image.
5. Each screen validated against ShareX with `scripts/winshot.ps1`,
   `sharexrs --render` and `scripts/compare.py` (reference screenshots in
   `shots/sharex/editor-*.png`), as `docs/PORTING.md` requires.
