# Image editor

ShareX 22.4's image editor (`ShareX.ImageEditor`, Avalonia) and its port:
`crates/sharex-image-editor` (annotation model, tool options, renderer) on
`crates/sharex-skia` (Skia m119's CPU rasteriser, shared with the image
effects). The editor's workspace and its region capture host are ported
("The editor UI" below); the standalone editor window isn't yet.

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
  otherwise. A stroke no wider than a device pixel is a hairline
  (`SkDrawTreatAsHairline` → `SkScan::AntiHairPath`); dashes are
  `SkDashPathEffect` over `SkContourMeasure` (conics stay conics), with the
  device clip as the dash's cull rect.
- **Skia**: `ShouldUseAAA` picks analytic AA (`SkScan_AAAPath`) or the
  supersampler; strokes through `SkStroke` (round caps/joins as conics);
  solid colours through the legacy N32 blitters; edges clipped by
  `SkEdgeClipper`.
- **Effects**: each effect annotation computes a bitmap from the source
  image (Gaussian blur with clamped edges, pixelate as `scalePixels` down
  linear / up nearest, magnify as a Mitchell `drawImageRect`, highlight as a
  per-channel cap) and fills its `Rectangle` with it through an `ImageBrush`
  aligned 1:1; spotlights share one overlay (darkened, optionally blurred
  image with each spotlight cleared) drawn below the annotations. The
  magnifier's brush isn't aligned: `Stretch.Fill` over the geometry's
  (deflated) bounds, so `ConfigureTileBrush` resamples the bitmap into an
  intermediate image (legacy `SkBitmapProcState` bilinear) drawn through a
  decal shader; an elliptical magnifier is clipped by
  `Control.Clip = EllipseGeometry` (`clipPath` with anti-aliasing: an
  `SkAAClip`) and its border's corners are rounded into an ellipse.
- **Drop shadows** (`ShadowEffectHelper`, `DropShadowEffect`): a layer bounded
  by the visual grown by the blur padding, restored through
  `SkImageFilters::DropShadow` (sigma `0.288675 × r + 0.5`).
- **Text** (`TextAnnotation`): `FormattedText.BuildGeometry` (HarfBuzz
  advances in doubles, `TextLineImpl` metrics, `SKFont.GetGlyphPath`
  outlines added at float pen positions, NonZero) filled and stroked like a
  shape. **Step and speech balloon text** use `DrawingContext.DrawText`,
  which Skia rasterises through DirectWrite glyph masks.
- **Images** (`ImageAnnotation`, the Clone tool's images, emoji, cursors):
  an Avalonia `Image` control (`Stretch.Fill` for images, the default
  `Uniform` for emoji and cursors) laid out by `MeasureCore`/`ArrangeCore`
  (the slot rounded up, the uniform size centred, the position rounded),
  drawn by `Image.Render` → `DrawBitmap` → `drawImageRect` with bilinear
  sampling and anti-aliased edges. Skia draws whole-pixel unscaled bitmaps
  as sprites (`SkTreatAsSprite`), the rest through an image shader whose
  clamped bilinear sampling runs in lowp (`bilerp_clamp_8888`, fixed
  point). Rotations use Avalonia's `Matrix.ToRadians` (`angle *
  0.0174532925`, not pi / 180).
- **Cursors** (`WindowsCursorBitmapRenderer`): thirteen are ShareX's
  bundled 1-bit `.cur` files, decoded as Skia decodes them (the cursor
  directory rewritten as an icon's; `SkIcoCodec`'s BMP, palette opaque,
  AND mask transparent); the rest are Windows' cursors, `LoadCursor` +
  `DrawIconEx` into a 32-bit DIB (monochrome ones recovered against black
  and white), so they exist only on Windows, and follow the user's cursor
  scheme.
- **Emoji** (`WindowsEmojiBitmapRenderer.RenderStickerBitmap`): the
  emoji's text drawn by Direct2D (`DrawTextLayout` with
  `EnableColorFont`, grayscale anti-aliasing) in **Segoe UI Emoji**, whose
  glyphs are COLRv1 paint graphs (glyph clips, transforms, solid fills,
  linear and radial gradients, `SrcIn` composites), on a transparent
  canvas twice the sticker's size at 1.32 times its size; then trimmed to
  its non-transparent bounds and drawn (Catmull-Rom) into a `size` x
  `size` square with 14 px of padding, `size` being the bounds' larger
  side rounded up. ShareX sets the text format's centring after creating
  the layout, so the layout keeps the top-left alignment. ShareXrs shapes
  the text with HarfBuzz and paints the same COLRv1 graphs itself
  (`render::emoji`), lands them on the same pixels with the same bounds,
  and trims and fits as ShareX does; Direct2D's own edge coverage and
  gradient steps differ slightly. Off Windows, ShareX falls back to
  Skia's default typeface (no emoji); ShareXrs uses an installed COLR
  emoji font (Noto Color Emoji) when there's no Segoe UI Emoji.
- **Smart eraser**: a `Rectangle` filled with the sampled colour, or with
  the stored edge (a 1-pixel row or column) stretched through an
  `ImageBrush` with `BitmapInterpolationMode.None`.
- **Rotated effects**: blur, pixelate and highlight draw their processed
  image under the inverse rotation into a bitmap of the truncated bounds
  (`UpdateEffectFromProcessedSource`), the magnifier its zoomed image the
  same way (`UpdateRotatedEffect`, Mitchell), stretched over the rotated
  `Rectangle`.

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
| `Ellipse` | `BorderStyle` = Solid | ✅ |
| `Line` | `BorderStyle` = Solid, `CurvePoint`, `CurvePointActivated` = false | ✅ |
| `Arrow` | `Style` = Classic (Modern, Double, Basic, Line), `RoundedCorners` = true, `CurvePoint`, `CurvePointActivated` | ✅ |
| `Freehand` | `Points`, `BorderStyle`, `ShowArrowHead` = false, `SmoothingTolerance` = 2 | ✅ |
| `Highlight` | `Amount` = 10, `IsFreehand`, `IsEllipse` (base effect) | ✅ |
| `Blur` | same | ✅ |
| `Pixelate` | same | ✅ |
| `Magnify` | `Amount` = 2, `IsEllipse`, `BorderStyle` | ✅ |
| `Spotlight` | `DarkenOpacity` = 180, `BlurAmount` = 0, `IsEllipse` | ✅ |
| `Text` | `Text`, `TextColor` = `#FF000000`, `FontSize` = 48, `FontFamily` = Segoe UI, `IsBold`, `IsItalic`, `HorizontalAlignment` = Center | ✅ |
| `Number` (Step) | `Number` = 1, `StepType` = Numeric, `FontSize` = 24, `TextColor` = white, `IsBold` = true, `TailPoint`, `TailEnabled` = true | ⚠️ circle and tail exact, glyphs differ |
| `SpeechBalloon` | `TailPoint`, `TailEnabled` = true, `AutoSize`, `Text`, `FontSize` = 20, `FontFamily`, `TextColor`, `IsBold`, `IsItalic`, `HorizontalAlignment`, `CornerRadius` = 10 | ⚠️ body and tail exact, glyphs differ |
| `Image` (and the Clone tool's, `ToolType` = Clone) | `ImagePath`; the bitmap (`ImageBitmap`, not serialised) in `ImageProps::bitmap` | ✅ |
| `Emoji` | `UnicodeSequence`, `DisplayName` | ⚠️ same pixels and bounds, Direct2D's edges and gradients differ |
| `Cursor` | `CursorType` = Default | ✅ (system cursors on Windows only) |
| `SmartEraser` | rectangle's + `FillMode` = SolidColor, `EdgePixels` | ✅ |
| `Crop`, `CutOut` | (operations on the image: `EditorCore`) | ✅ |

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
  (`AntiFillPath`, `AntiFillRect`), `clip` + `cubics` (`SkEdgeClipper`,
  `SkLineClipper`, the double-precision cubic solvers), `stroke`
  (`SkStroke`), `hair` (`SkScan_Hairline`, `SkScan_Antihair`), `dash`
  (`SkDashPath`, `SkContourMeasure`), `aaclip` (`SkAAClip`,
  `SkAAClipBlitter`), `blitter` (legacy N32 blitters), `blur`, `resample`
  (`scalePixels`, Mitchell), `raster` (the image effects' pipeline stages,
  sprite blitter, legacy image shaders incl. `SkBitmapProcState` bilinear,
  matrices, shaders, gradients, `Bitmap`, `Canvas` with an anti-aliased
  clip), `pathops` (`SkPathOps`: `Op` and `Simplify`). The image effects
  crate uses it too.
- `sharex-image-editor`: `model` (annotations, JSON), `options`
  (`ImageEditorOptions`), `render` (`render(base, annotations)`: shapes,
  effects, text, images, cursors, emoji, smart erasers, shadows;
  `render_region`, region capture's export), `editor::EditorCore` (the
  image, its annotations and the operations that change the image, with
  `EditorHistory`'s canvas mementos) and `editor::image_ops` (the
  `ImageHelpers` they call).

## EditorCore and its image operations

`sharex_image_editor::editor::EditorCore` is `Core/Editor/EditorCore.cs`
without its input handling (the workspace does that): the source image and
annotations, `AddAnnotation`/`RemoveAnnotation` (step renumbering),
`ClearAll`, undo and redo, and every operation that changes the image, each
recorded as a canvas memento (`EditorHistory` keeps 50 snapshots, at most
five with an image copy, contiguous from the newest):

| Operation | What happens to the annotations |
|---|---|
| `crop(rect)`, `perform_crop()` (the crop annotation's bounds, then the annotation removed) | the rectangle truncated to whole pixels inside the image; annotations entirely outside removed, the rest moved |
| `cut_out(start, end, vertical)`, `perform_cut_out(rect, vertical)` (from the middle of the tool's rectangle, as long as it is wide or high) | `CutOutAnnotation.AdjustAnnotation`: inside the strip removed; across or after it both end points moved back by the cut, the type's own points collapsed |
| `rotate_90_clockwise`, `rotate_90_counter_clockwise`, `rotate_180`, `rotate_custom_angle(angle, auto_resize)` | points rotated with the image, `RotationAngle` added |
| `flip_horizontal`, `flip_vertical` | points mirrored, `RotationAngle` = 360 - angle |
| `resize_image(w, h)` (Catmull-Rom) | points scaled, stroke by the smaller scale, text font by the vertical one |
| `resize_canvas(top, right, bottom, left, colour)`, `auto_expand_canvas_to_fit`, `expand_canvas_live` (outside the history) | moved by the room added left and top |
| `auto_crop(tolerance)` | moved by the crop's corner |
| `flatten_image(composite)` | cleared (they're in the image now) |
| `clone_region(rect)` (`EditorView.CloneRegion`) | a new image annotation of those pixels, `ToolType` = Clone, selected |

As in ShareX, a line's or arrow's curve point isn't moved by these
(`TransformAdditionalPoints` doesn't cover it), and `PerformCutOut` and
`CutOut` record their memento before checking the cut. The smart eraser's
drawing steps are `editor_core::clamp_smart_eraser` (`ClampVisualBounds`)
and `configure_smart_eraser_fill` (`ConfigureFill`), and
`sample_canvas_color` its starting colour.

## Validation

`tools/editor-harness` (see its README) runs ShareX's own editor headless
(Avalonia headless platform with Skia drawing; no window, no input) on
`crates/sharex-image-editor/tests/data/cases.json` (158 cases, most on a
400×300 image, some on generated images in `tests/data/images`) and saves
`EditorView.GetSnapshot()`, after the case's `EditorCore` operations
(`ops`), or `GetSnapshot(Rect)` (`snapshotRect`). Those PNGs are the
references in `tests/data/sharex/`; `cargo test -p sharex-image-editor`
requires every case without a `"pending"` note to be identical (and the
same size), and
`cargo run -p sharex-image-editor --example render_cases -- <sharex dir>`
prints per case the largest channel difference and the share of differing
pixels. Cases are generated by `tests/data/add_cases.py`; images by
`tests/data/images/make_images.py`.

**Result: 146 of 158 cases identical, pixel for pixel.** Per group:

| Group | Identical | Not identical |
|---|---|---|
| Shapes, effects, text, steps, balloons (below) | 68/73 | 5 glyph-mask cases |
| Image annotations (1:1, fractional, scaled up and down, reversed, rotated, tiny, over shapes) | 8/8 | |
| Cursors (bundled: plain, uniform, rotated, fractional; system: default, I-beam, hand) | 7/7 | |
| Smart eraser (solid, rotated, edges from JSON, `ConfigureFill` horizontal, vertical, solid, clamped, fractional, reversed) | 9/9 | |
| Image operations (crop and `PerformCrop` with undo/redo, cut out vertical/horizontal/effects/undo, `PerformCutOut`, rotate 90/270/180/custom/clipped, flips, resize down/up/text, resize canvas grow/crop, auto crop, auto-expand, flatten, clone, history trim and mixed) | 34/34 | |
| Rotated effects (blur, pixelate, highlight, magnify, elliptical magnify) | 5/5 | |
| Drop shadows on steps and balloons | 4/6 | 2 rotated |
| Region capture's export (`GetSnapshot(Rect)`: shapes, translucent fills, hairlines, effects, shadows, images, text, none, clipped) | 11/11 | |
| Emoji | 0/5 | Direct2D |

System cursor cases run only where the system's cursor is the bitmap ShareX
drew (`tests/data/cursors`, from `EditorHarness --cursors`); emoji cases
need Segoe UI Emoji.

The first 73 cases: every rectangle
(plain, odd width, radius, dashes, rotated, off-canvas, shadows), ellipse
(solid, filled, dotted, dash-dot, rotated, off-canvas), line (curved, wide,
1 px, 2 px dashed), arrow (all five styles, straight and curved, zero
length, thick, shadow), freehand (plain, arrow head, dashed, two points),
highlight (4 cases incl. fractional and off-canvas), blur (3), pixelate
(2), magnify (rectangle; ellipse with a border), spotlight (rectangle;
ellipse with blur), text (3, Segoe UI), a stack of overlapping annotations,
small shapes, and 13 steps and speech balloons with tails and no visible text
(tails in every direction, strokes 0 to 6 px, square, rounded and round
bodies, a default tail, a rotated balloon). (51 before hairlines,
`SkDashPath` and `SkAAClip`; 55 of 60 before PathOps.)

Not identical, and why:

| Case | Max Δ, pixels differing | Why |
|---|---|---|
| step, step-letter | 215–223, 0.03–0.07% | Glyphs: `DrawText` rasterises DirectWrite (hinted) masks; ShareXrs fills the same font's outlines at the same positions. The circle is exact. |
| step-tail, balloon | 191–195, 0.11–0.14% | Glyphs only: every differing pixel is inside the text's box, and the same annotations without visible text (step-tail-notext, balloon-tail-notext) are identical. |
| balloon-notail | 165, 0.60% | Glyphs only. |
| emoji-grinning, -small, -zwj, -flag, -rotated | 17–89, 0.01–4% | Direct2D's rasteriser: the raw emoji lands on the same pixels with the same bounds as Direct2D's (`EditorHarness --emoji-raw`), edge coverage and gradient steps differ (mostly by a few levels). |
| shadow-balloon-rotated, shadow-rect-rotated | 174–219 | A drop shadow under a rotation: Skia filters the layer in its unrotated space and draws it back rotated (resampled); ShareXrs blurs in device space. Unrotated shadows are identical. |

Fonts: text cases use the same font files as ShareX (Segoe UI from
`%WINDIR%\Fonts`; Avalonia's default Inter from `SHAREXRS_INTER_DIR`, dumped
by `EditorHarness --fonts`). Shaping is HarfBuzz in both (harfrust here), so
advances and positions agree; only `DrawText`'s DirectWrite masks can't be
reproduced by filling outlines.

Tails: a step's or a balloon's tail is united with its body
(`CombinedGeometry(Union)` → `SKPath.Op`, Skia's PathOps), and the union's
outline (where the oval's conics are chopped, in double precision, which
segment the walk starts from) decides where the stroker's joins fall. So
`sharex_skia::pathops` is Skia m119's `src/pathops` ported file by file:
`SkOpEdgeBuilder`, the curve maths (`SkDLine`/`SkDQuad`/`SkDConic`/`SkDCubic`,
`SkReduceOrder`, `SkLineParameters`), `SkIntersections` with the line/curve
solvers and `SkTSect` for curve/curve, `SkOpSegment`/`SkOpSpan`/`SkOpAngle`/
`SkOpCoincidence`, the winding (`SkPathOpsWinding`), `SkPathWriter`, `Op`
and `Simplify`, with the object graph kept as Skia links it (raw pointers in
an arena) and its sorts (`SkTQSort`) ported for Skia's tie order.
`crates/sharex-skia/tests/pathops.rs` compares `op` with SkiaSharp's
`SKPath.Op` run by `tools/pathops-harness` on ShareX's own libSkiaSharp:
264 cases (steps and balloons as ShareX builds them, ovals, rectangles,
coincident edges, quads, cubics, a cubic loop, stars, every op, and 200
seeded random pairs), every result path identical verb for verb and bit for
bit. Off Windows the cubic cases are skipped: their roots go through
`acos`/`cos`/`cbrt`, exact only with the C runtime libSkiaSharp uses.

Not ported yet: drop shadows under a rotation (above); the editor's own
image effect catalogue (`Core/ImageEffects`, ~240 effects behind the
effect browser; `ResizeImage`, rotation, flips and auto crop are ported for
`EditorCore`); emoji pixels exactly as Direct2D draws them.

## The editor UI

`sharex_image_editor::editor` is the workspace `EditorView` is, with no
window: `Workspace` holds the image and annotations (`EditorCore`), draws
them with the tools from `ImageEditorOptions` (`EditorInputController`),
selects, moves, resizes and rotates them (`EditorSelectionController`:
ShareX's hit tests, the base `HitTest` tolerance of 10 that C# binds, handles,
hover outlines), undoes and redoes as annotation-list snapshots
(`EditorHistory`: adds, deletes, clears and reorders, not moves, as ShareX),
deletes with step renumbering, types text in place, and takes the editor's
keys (`EditorView.OnKeyDown`/`OnKeyUp`). `ToolState` is
`MainViewModel.ToolOptions`: each tool loads its own options and writes them
back as they change. `Workspace::composite` is `render::render_cached` of the
annotations over the image (the effects' whole-image sources cached: about
7 ms for 2560x1440 in a debug build once cached), so the live view is the
export.

Region capture (`crates/sharex-capture/src/region`) hosts it as
`RegionCaptureWindow` hosts `EditorView`: the toolbar's tools and hotkeys
switch to the annotation tools, Tab, NumPad0 and the region button back, the
tool options row (`ToolOptionsBorder`, `options_row.rs`) hangs under the
toolbar, right and middle clicks run the capture actions on the active tool's
annotations, Escape drops the selection first, and the capture is
`GetSnapshot(Rect)` of the composite (image modified).

Validated against ShareX's overlay over the test pattern
(`scripts/annotate-tools.ps1` takes ShareX's shots, `scripts/compare-annotate.py`
renders `--render region-* --steps` and compares): every tool chosen, every
drawable shape mid-draw and finished with its handles, text typed and
committed, a step placed: 0.00-0.04% of pixels differ (the text box 0.09%),
text anti-aliasing and the handles' shadow; live ShareXrs gives the same
(0.01-0.04%). Output, side by side: eleven annotations (rectangle, ellipse,
arrow, line, freehand, highlight, blur, pixelate, magnify, text, step) drawn
with the same input in both apps, then a region captured: same size, 99.37%
of pixels identical; 0.52% differ by at most 3 levels on anti-aliased edges
(see below), more only on the freehand stroke (the moves ShareX received
differ: Avalonia can coalesce pointer moves) and the step's number (glyphs,
above).

ShareX's region capture renders its output through a `VisualBrush` of the
workspace (`GetSnapshot(Rect)`), not as `GetSnapshot()` draws the editor's
export, and its anti-aliased and translucent pixels come out up to 3 levels
different. The brush is recorded as an `SKPicture` and drawn through a
picture shader (`UseScalableRasterization`), and Skia rasterises a picture
shader's tile as `kRGBA_8888` with an sRGB colour space: not N32 on
Windows (BGRA), so `SkBlitter::UseLegacyBlitter` is false and every shape
goes through `SkRasterPipelineBlitter` (lowp: the colour premultiplied in
float, partial coverage scaled or lerped, srcover as `load_dst` +
`srcover` with `div255_accurate` since the tile has a colour space), while
layers for drop shadows still draw with the legacy blitters; edges are
clipped (curves chopped) at the tile. `EditorHarness --probe-picture`
shows it in plain SkiaSharp: the same draws through a picture shader
differ by up to 3 levels, through `DrawPicture` not at all. Ported:
`sharex_skia::blitter::with_pipeline_blitters` and `with_device_clip`, and
`render::render_region` (`EditorCore::snapshot_region`), identical to
ShareX on the 11 region cases. The workspace's `snapshot` still crops the
composite; it should call `render::render_region_cached`.

Not ported yet: the options' dropdowns and colour popups (they draw closed;
toggles and sliders work), the shadow options popup, the speech balloon's text
editor and default tail on selection (ShareX opens both as soon as a balloon
is drawn), editing an existing text in its own box (double click edits it in
the creation box), the Image, Emoji, Clone, Cursor and Smart eraser tools
(selectable; Image and Emoji open dialogs in ShareX, not ported; their
annotations render and `EditorCore` has their steps), the crop and cut out
tools (`EditorCore` has the operations), copy/paste/duplicate of annotations, the context menu,
`RightClickDeletesShape`, the cursor annotation ShareX inserts for a shown
pointer (ShareXrs draws the pointer into the frozen screen), the
standalone editor window.

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
