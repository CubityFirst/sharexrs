# Image editor

ShareX 22.4's image editor (`ShareX.ImageEditor`, Avalonia) and its port:
`crates/sharex-image-editor` (annotation model, tool options, renderer) on
`crates/sharex-skia` (Skia m119's CPU rasteriser, shared with the image
effects). The editor's workspace, its region capture host and the
standalone editor window are ported ("The editor UI" below).

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
  `EditorHistory`'s canvas mementos), `editor::image_ops` (the
  `ImageHelpers` they call) and `effects` (the image effect catalogue,
  below).

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

Not ported yet: drop shadows under a rotation (above); emoji pixels
exactly as Direct2D draws them.

## The image effect catalogue (`sharex_image_editor::effects`)

`Core/ImageEffects` is ported whole: all 230 effects ShareX's effect browser
lists (Manipulations 33, Adjustments 32, Filters 149, Drawings 16), each in
its own file under `src/effects/<category>/<id>.rs`, ported line for line
from its `*.cs` with the same parameters, defaults, clamps and order of
floating-point operations. ShareX.ImageEditor doesn't use
`ShareX.ImageEffectsLib` (that's `sharex-image-effects`, a separate
catalogue); the two share only Skia, through `sharex-skia`.

What the effects needed from Skia m119, now in `sharex-skia`: the legacy
sprite blits (`Sprite_D32_S32` with a global alpha), stroked rects
(`SkScan::AntiFrameRect`/`FrameRect`), `SkPath::addArc`, radial gradients,
the Multiply, Overlay, Screen, Modulate, DstIn and Clear blend modes,
`SKColorFilter.CreateBlendMode(c, Modulate)`, image and rect draws through an
anti-aliased clip, rect clips through the scan converters' device clip,
`SkPathPriv::IsNestedFillRects`, and `SKMaskFilter.CreateBlur` (`maskblur`:
`SkMaskFilterBase::filterPath` with `SkMaskBlurFilter`, Gaussian below
sigma 2 and three box passes above; `filterRectsToNine` with
`SkBlurMask::BlurRect`'s analytic profile; `filterRRectToNine`; the
nine-patch). Effects that run as Skia image filters are ported as the
filter graph runs on the CPU: Dilate (the exact windowed maximum) drawn
through its lazy colour filter, colour matrices in highp. `Random.Shared`
is .NET's xoshiro256** (`effects::random`), `new Random(seed)` its Net5
compat generator, and `Array.Sort` .NET's introsort, so seeded effects
draw the same values.

### Using it (for the editor UI)

- Listing: `effects::catalog()` (ShareX's browser order: category, then
  name ignoring case), `effects::by_category(EffectCategory::Filters)`,
  `effects::find("gaussian_blur")`. Each `EffectDefinition` has `id`,
  `name`, `browser_label` (`Name...` for a dialog effect), `icon` (ShareX's
  icon key), `description`, `category`, `apply_immediately` (no dialog)
  and `parameters`.
- Parameters: `ParamDef { key, label, description, kind }`, `kind` a
  `ParamKind`: `Slider { min, max, default, tick, snap, format }`,
  `Numeric { min, max, default, increment, format }`, `Checkbox { default }`,
  `Enum { options, default }` (labels as ShareX shows them),
  `Color { default }`, `Text { default }`, `FilePath { default, filter }`. `def.default_values()` gives an
  `EffectValues`; `values.set(key, ParamValue::Number(..))` (or `Bool`,
  `Choice(index)`, `Color(SkColor)`, `Text`) changes one, and remembers the
  order of changes (`ResizeImage`'s aspect ratio follows the last edited
  side, as ShareX's dialog does).
- Preview: `EffectPreview::start(&core)`, then `preview.preview(def,
  &values)` for each change (the bitmap to show; the editor's image is
  untouched), `preview.apply(&mut core, def, &values)` to commit with undo
  or `preview.cancel()`. Without a dialog: `effects::apply_to_core(&mut
  core, def, &values)`.
- Apply and undo: crop, resize image, resize canvas, rotate (90, 180,
  custom), flips and auto crop go through `EditorCore` (annotations moved
  with the image), every other effect through `apply_image_effect`; each
  is one history step, undone with `core.undo()`. `def.apply(&bitmap,
  &values)` runs an effect on any `Bitmap`. An effect that throws in ShareX
  returns `Err(EffectError)` with .NET's message (Gamma always does;
  Reflection does when its height rounds to 0).
- The browser: `effects::browser::Browser::new(label)` (the sections and
  their items, `search`, `visible()`, `register_recent`, `right_click`,
  `load`/`recent_ids`/`favorite_ids` for `ImageEditorOptions`,
  `favorite_menu`) and `browser::score` (`EffectSearch.Score`); the editor
  window's panel is "Image effects" under "The editor UI".

### Validation

`tools/editor-harness --effects` runs every effect in ShareX's own
ShareX.ImageEditor on two generated images (48×36 opaque, 32×24
translucent), with the defaults on both and with each parameter set to its
minimum, maximum and a third value, and every enum option (1202 cases;
file parameters point at `tests/data/effects/inputs`, `Random.Shared` is
seeded per case). It also dumps the catalogue (`catalog.json`, compared field
by field by `tests/effects_catalog.rs`). The outputs are in
`crates/sharex-image-editor/tests/data/effects/sharex`;
`tests/effects_match.rs` (Windows) requires every case to be identical, pixel
for pixel, unless `known.json` lists it, and
`cargo run -p sharex-image-editor --example image_editor_effects_report --
[id prefix] [out dir]` prints each differing case and the totals.

**Result: 1174 of 1202 cases identical; 223 of 230 effects identical in
every case.**

| Category | Effects identical | Cases identical |
|---|---|---|
| Manipulations | 33/33 | 161/161 |
| Adjustments | 32/32 | 159/159 (Gamma's 5 throw in both) |
| Filters | 144/149 | 734/747 |
| Drawings | 14/16 | 120/135 |

Not identical, and why (all in `known.json`):

| Cases | Why |
|---|---|
| ASCII art (5), Matrix digital rain (2), Text watermark (10), macOS window (2), Windows 11 window (2), Polaroid frame (2) | Glyphs: `SKCanvas.DrawText` rasterises DirectWrite masks (`SkScalerContext_DW`, hinted, GDI-compatible metrics); ShareXrs fills the font's outlines. Their cases without text (the windows' and frame's untitled ones, digital rain's empty cells) are identical, and Draw text, which fills `GetTextPath`'s outlines, is identical in all 13. |
| Fake hair (5) | ShareX seeds it from `Environment.TickCount`: its own output changes from run to run. |

The pixel test runs on Windows only: .NET's `MathF` calls the C runtime, as
Rust's `f32` does there. Elsewhere every case still runs (no panics, the
same throws), and the text effects are skipped (they need Windows fonts).

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
ShareX on the 11 region cases. The workspace's `snapshot` (region capture's
output) renders through `render::render_region_cached`; `export` (the
editor window's `GetSnapshot()`) through `render`.

### The workspace's commands (`workspace/ops.rs`)

Beyond drawing, the workspace has `EditorView`'s and `EditorCore`'s
commands, for both hosts:

- History with canvas mementos (`CreateCanvasMemento`: crop, cut out,
  flatten; five kept, the history contiguous) on `editor::History<Memento>`.
- The annotation clipboard (`_clipboardAnnotation`, shared by every
  editor): Ctrl+Shift+C copies, Ctrl+X cuts (undoable), Ctrl+V is the
  host's (an image on the system clipboard first, then the copied shape:
  `paste_annotation`), Ctrl+D duplicates; copies are 20px further and
  selected, and keep their original's `Id` (`Annotation.Clone`). The
  editor goes by `Common::key` instead: a per-object handle (ShareX holds
  the annotation object itself), not serialised, never part of `==`. Undo
  selects again by `SelectedAnnotationId`, the first annotation with the
  selected one's `Id`, as `RestoreState` does.
- Z-order (Home, End, Page Up, Page Down), flatten (Ctrl+Shift+F), the
  right button (`right_pressed`: cancels a drawing or a pending crop,
  `RightClickDeletesShape`, or selects the shape for the context menu).
- Crop: drawn clamped to the image with its adorner (`crop_chrome`: the
  shades, the 2px `4,2` dashed rectangle, the thirds, `X: Y: W: H:`),
  performed on release with `QuickCrop` (the default), else pending with
  ShareX's handles (`CropAnnotation.ResizeBounds`), Enter, a double click
  or the Crop button. Cut out: the 30px square below 15px, then a column or
  row strip (`CutOutAnnotation.AdjustAnnotation` moves the annotations).
  Clone: the region becomes a selected image annotation (`CloneRegion`).
- The Image tool asks the host (`Request::InsertImage` when chosen,
  `PickImage { at }` on a click; a double click on an image `ReplaceImage`),
  the Emoji tool `EmojiPicker` (`ReplaceEmoji`); images carry their pixels
  (`ImageProps::set_image`). The Cursor tool sizes the annotation from
  `render::cursor`; the smart eraser samples the canvas
  (`SampleCanvasColor`) and fills through `editor_core::configure_smart_eraser_fill`.
- Text: an existing text is edited in its own box over its bounds
  (`ShowTextEditor`, the annotation hidden meanwhile, its rectangle kept);
  selecting a speech balloon opens its editor over the body
  (`ShowSpeechBalloonTextEditor`: a small balloon grows to 200x100, the
  default tail 30px below its middle, auto-size fits as you type), with the
  balloon's tail handle (`BalloonTail`, placed once dragged) and
  `ResizeTail`/`MoveBy`.
- Keys that need the host come back as `KeyResult::command`
  (`Command::CopyImage`, `Paste`, `Save`, `SaveAs`, `NewImage`, `OpenImage`,
  `PinToScreen`, `Upload`, `Print`, `ToggleToolbars`, `Continue`, `Exit`).

Unit tests: `workspace/tests.rs` (clipboard, z-order, quick and pending
crop with undo, cut out, clone, flatten, the right button, the balloon
editor and tail, editing a text, the Image/Emoji requests, the smart
eraser).

The workspace holds an `EditorCore` (the image, the annotations, the
selection and the history) and runs its crop, cut out, flatten, resize
canvas and auto-expand; the workspace only adds what the view does around
them (caches, hover, the host's notifications through `Request`).

### The overlay and the options' popups (both hosts)

`sharex_capture::editor_chrome` draws the editor's `OverlayCanvas` for
both hosts through a `Mapper` with the zoom: outlines, handles, the text
boxes (creation, edit and balloon: the theme's control background, the
balloon's square and borderless, under the handles as ShareX's `ZIndex`
9999), the crop chrome with its handles and Crop button, the cut out and
clone regions.

`sharex_capture::editor_popups` is every option dropdown of
`AnnotationToolbar.axaml`, under its button (`Placement="Bottom"`, 4px),
kept on the work area (its height cut to it, as Avalonia's positioner
does), with light dismiss and Escape; the chevrons turn over while open:

- `ColorPickerDropdown`/`ColorPickerPanel`: Avalonia's `ColorView` on its
  palette tab, ported from the Fluent template embedded in
  `Avalonia.Controls.ColorPicker.dll` 12.1.6 (the tab strip and its Fluent
  UI icons, `FluentColorPalette` 6x8 with ShareX's three replacements, the
  `ColorPreviewer` with its +-0.1/0.2 value accents and shadow), then the
  screen colour picker button and five recent colours
  (`EditorRecentColors`, the colour a popup closes with). The spectrum and
  components tabs fall back to egui's HSV picker, as the app's other colour
  popups do.
- Width, corner radius (grids), border style, arrow style (previews drawn
  by the renderer), alignment, step type, start number, cursor type (the
  cursors' previews from `render::cursor`), font size (custom size box),
  font family (the system's families as DirectWrite names them, each drawn
  in its own face, with search), and the shadow options popup (a right
  click on the shadow button: colour, blur, opacity, offsets).

Region capture opens them from its options row; ShareX's region capture
screen colour picker isn't ported (logged).

Validation (`scripts/annotate-popups.ps1` takes ShareX's popups over the
test pattern, `scripts/compare-popups.py` renders `--render
region-annotate --steps ... --work-area 0,0,1024,720` and compares;
references in `shots/sharex/editor-popups/`):

| Popup | Dark | Light |
|---|---|---|
| Border colour | 0.06% | 0.09% |
| Fill colour | 0.09% | 0.15% |
| Text colour | 0.08% | 0.10% |
| Thickness | 0.01% | 0.01% |
| Border style | 0.26% | 0.24% |
| Corner radius | 0.02% | 0.02% |
| Shadow options | 0.07% | 0.08% |
| Arrow style | 0.17% | 0.17% |
| Font family | 0.44% | 0.40% |
| Alignment | 0.08% | 0.09% |
| Font size | 0.12% | 0.11% |
| Step type | 0.05% | 0.05% |
| Start number | 0.04% | 0.04% |
| Cursor type | 1.18% | 1.65% |

The cursor type list is the one over 0.5%: ShareX's shot has the pointer
hovering "Arrow" with its tooltip (the list opened under the pointer),
and the I-beam and H split previews differ slightly. The region capture
annotation shots (`compare-annotate.py`) stay at 0.00-0.09% with the
balloon editor now open on selection (0.06%: its caret, which ShareX's shot
caught off).

### The editor window (`crates/sharexrs/src/image_editor/`)

`EditorWindow` as `ImageEditorIntegration.ShowEditorDialog` opens it:
maximized (`RememberWindowState`/`IsWindowMaximized`), titled "ShareXrs -
Image Editor - WxH - name", the workspace in the canvas host (`Margin="8"`,
centred, scrolled when larger, middle button pans, Ctrl+wheel and the zoom
dropdown step through `ZoomLevels`, Zoom to fit, `ZoomToFitOnOpen`), and
`EditorBuiltInToolbars`:

- the top `AnnotationToolbar` (`ShowEditingActions="False"`): the File menu
  (New, Open, New from screenshot, Save, Save as, Exit), every tool, Crop,
  Cut out, Canvas size, Background, Image effects (their hotkeys as
  `ToolbarCustomizationItemViewModel`'s), scrolled with its collapsed bar
  when wider than the window, the options row and its popups;
- the bottom toolbar: zoom, the task buttons (Continue, Cancel), Copy, Save,
  Save as, Pin to screen, Print, Upload, New from screenshot (region, full
  screen).

The context menu (`EditorContextMenu`, a plain `ContextMenu`), the exit
confirmation (`ConfirmationDialogView`, when `ShowExitConfirmation` and
dirty), the editor's keys (`EditorView.OnKeyDown`/`OnKeyUp`: Escape
cancels the clone or crop, then the selection, then exits or cancels the
task; Enter confirms a crop or continues).

Outputs as ShareX wires them (`TaskHelpers.AnnotateImageModern`): Copy
(`CopyImage`), Save (the file, or a new name in the task's screenshots
folder), Save as (a file dialog), Pin to screen (`PinToScreen(image,
task)`), Upload (`ProcessImage`), Continue (task mode: the image back to the
waiting task; otherwise `RunImageTask`), Cancel. The tool options go back
into the default task settings.

Opened from: Tools > Image editor and its hotkey (no image), the after
capture Annotate image task (`UiRequest::Annotate`: the task waits; a
cancel stops it), the task list's and history windows' Edit image
(`AnnotateImageFromFile`), and `-ImageEditor <file>`.

Validation (`--render image-editor --image <png> --width 1028 --height
720` against ShareX's window on the same 640x400 image; references in
`shots/sharex/editor-window/`): dark 0.10%, light 0.09% (the zoom button
is a few px wider in ShareX). Context menu (`--render
image-editor-context`, cropped) 0.68% light. Side by side: the same two
ellipses drawn in both editors and copied give a pixel-identical PNG and a
byte-identical CF_DIB.

### The dialogs and the rest (`image_editor/`)

`EditorView`'s modal host (`modal.rs`): the `modal-scrim` over the whole
view (Escape on key up or a press outside closes the dialog; the toolbars
under it take no input), the dialogs' shared frame (popup border, panel,
the title row, the button bar):

- `StartScreenDialogView` (`start_screen.rs`), shown when the editor opens
  without an image outside task mode: new image, open, the clipboard's
  image (or its first image file), a URL (prefilled from the clipboard,
  downloaded on a thread with a 30 s timeout, `EnsureSuccessStatusCode`'s
  message, the invalid-URL and not-an-image statuses), the two
  screenshots, exit, and the recent files (leading ellipsis; a missing file
  leaves the list with its status).
- `NewImageDialogView` (`new_image.rs`): size, transparent or a solid
  colour through its own `ColorPickerDropdown`
  (`sharex_capture::editor_popups::ColorDropdown`).
- `InsertImageDialogView` (`insert_image.rs`) after the Image tool's file
  picker (with no image the picked one becomes the image;
  `ShowInsertImageDialog` off inserts it centred): centred on the monitor
  under the pointer (clamped to the canvas), or the canvas grown below or
  to the right; the dialog sits on the part of the view on that monitor
  (`PositionModalOnCursorScreen`). A double click's replacement keeps the
  image's centre.
- `EmojiPickerDialogView` (`emoji_picker.rs`) over ShareX's bundled
  catalogue (`sharex_image_editor::emoji_catalog`: groups in ShareX's
  order, the search scoring) with `RenderPreviewBitmap`'s 48px previews,
  a few rendered a frame; picking inserts a 160px emoji centred on the
  pointer's monitor or replaces the double-clicked one.
- `ConfirmationDialogView`: the colour warning sign (the emoji font on
  Segoe UI's line), the message's four lines.

The Canvas size tool (`canvas_size.rs`, `EditorView.CanvasResize`): the
frame with eight handles over a contrasting halo, room added round the
image in the scroll viewer (the image stays put), a large image zoomed to
70% of the view while it's open, the area to add in `CanvasFillColor`
(checkerboard when transparent), the area to drop shaded, the panel (size,
fill colour, Apply, Cancel); Enter or a double click applies, Escape or a
right click cancels, X or its button toggles it. The canvas scroll viewer
shows Fluent's collapsed scroll bars; the canvas sits on
`ShareX.Brush.Checkered` (15px squares, scaled with the zoom).

`MainViewModel.Notifications` (`notify.rs`): the editor's own toast at the
bottom of the view (new image, opened, saved, copied, pinned, uploading,
cropped, cut out, inserted), 2.4 s plus the time under the pointer, faded
by `popup-surface`'s transitions, dismissed by a click; none for the task
actions when `AutoCloseEditorOnTask` closes the editor after them. These
are the editor's, not the app's toasts (which aren't ported).

Also: recent files (`ImageEditorOptions.RecentImageFiles`, the File menu's
Open recent), New from screenshot (hidden 250 ms, the task's region capture
or the full screen), the colour popups' screen colour picker (over the
window, as `ScreenColorPickerWindow.PickAsync(owner)`), dropped image
files, `AutoCopyImageToClipboard`, `AutoExpandCanvas` (after a draw, move,
resize or text commit; one undo step; the view scrolled by the room
added), `RememberWindowState` (`ApplySavedWindowState`/`SaveWindowState`),
`AutoCloseEditorOnTask`, disabled export buttons without an image, Cancel
asking before closing (`CancelCommand`), and region capture as the
workspace host (its shown pointer is an unselected image annotation that
doesn't make it dirty, `InsertWorkspaceImageAnnotation`).

The File menu is a `MenuFlyout`: `sharex_ui::menu::MenuStyle::Flyout`
(Fluent's `MenuItem` metrics that ShareX's `ContextMenu MenuItem` style
doesn't reach), its first item selected on open.

Validation (`scripts/compare-editor-dialogs.sh`, ShareX's shots in
`shots/sharex/editor-dialogs/`, inputs from
`scripts/editor-dialogs-inputs.py`, the editor opened with
`scripts/editor-open.ps1`):

| Screen | Dark | Light |
|---|---|---|
| Start screen | 0.14% | 0.10% |
| New image | 0.10% | 0.09% |
| New image created (notification) | 0.15% | 0.16% |
| Insert image | 0.15% | 0.11% |
| Emoji picker | 0.22% | 0.05% |
| Canvas size | 0.14% | 0.13% |
| Exit confirmation | 0.56% | 0.57% (ShareX's shot has another tool chosen) |
| File menu (of the menu / of the window) | 1.42% / 0.21% | 2.36% / 0.35% |
| Context menu | 0.89% / 0.16% | 0.68% / 0.12% |

Side by side: the test image, the 200x120 image inserted below in both
editors and copied: CF_DIB byte-identical, the PNG pixel-identical. Both
apps place the centred insert at the monitor's centre (canvas 210,133
for ShareX's window); the result depends on where each window sits.

### The Background panel (`image_editor/background.rs`, `render::background`)

`BackgroundSettingsPanel` in the left sidebar (`Border.editor-sidebar.left`,
282 wide, the top toolbar floating over its heading), toggled by the
Background item (`ToggleSettingsPanel`; Crop and Cut out close it): margin,
padding, rounded corner and shadow radius sliders (Fluent's auto-height
slider: 15 + the 20px thumb + 15), the smart padding switch
(`LabeledToggleSwitch`, disabled and turned off when the image has no
uniform border: `DisableBackgroundSmartPaddingIfUnavailable`), the 3x3
output ratios (`SetOutputRatio`, not saved), the background combo box
(Gradient, Color, Transparent, Image, and Wallpaper where the wallpaper
service exists), the gradient preview whose halves open their colour's
`ColorPickerPanel`, the 60 presets (`ApplyGradientPreset`), the colour
preview, Browse background image... with its path. Everything is kept in
`ImageEditorOptions`. While it's open the canvas shows, and every export
(copy, save, upload, pin, print, Continue) renders, the image on its
background.

`sharex_image_editor::render::background` is `GetSnapshot()` of the
`PreviewFrame`: the frame size (`CalculateOutputPadding`, Avalonia's layout
rounding of the paddings, the viewport centred in its slot), the brush over
the frame (gradient through the raster pipeline; image or wallpaper through
`ConfigureTileBrush`'s intermediate: UniformToFill, Uniform, Fill, centred,
tiled), the canvas shadow (`BoxShadow` 0,10, alpha scaled from the shadow
colour's: the rounded rect in an `SkImageFilters::Blur` layer the size of
the device grown by ceil(3 sigma), drawn back through an anti-aliased
difference clip, `SkAAClip::op(kDifference)`), then inside the rounded clip
the padding colour (the image's top-left pixel) and the viewport (smart
padding's crop, `EnsureSmartPaddingCache`). `sharex-skia` gained the
difference clip and rect/bitmap draws through an anti-aliased clip.

Wallpaper: Windows' `SPI_GETDESKWALLPAPER` (then `TranscodedImageCache`),
laid out as Fill, as `WindowsDesktopWallpaperService`;
`SHAREXRS_TEST_WALLPAPER=<file>[;Layout]` stands in for it in tests. The
macOS and Linux wallpaper services aren't ported (no Wallpaper option
there).

Validation: the editor harness got a `background` case field (the options,
the panel open; a fake wallpaper service hands it a generated image, never
the desktop's): 26 cases in `cases.json`, 9 pixel-identical, 16 within one
level on 3-5 pixels at the blurred shadow's corners (`tolerance` in the
case), and annotations over the viewport's edge within 5 levels on 0.03%
(ShareX chops their edges at the frame's clip). The panel vs ShareX
(`--render image-editor --state background[-gradient|-color|-image][-scrolled]`,
shots in `shots/sharex/editor-background/`): dark transparent 0.16%,
gradient 0.16%, gradient scrolled 0.06%, colour 0.15%, image 0.16%; light
transparent 0.12%, gradient 0.12%, image 0.13%. Side by side: Gradient
chosen in both editors and copied: 880x640, CF_DIB identical but for the
same 4 shadow-corner pixels (one level).

### Toolbar customisation (`sharex_capture::toolbar_items`, `toolbar_editor.rs`)

`ToolbarCustomizationItemViewModel`'s definitions, `CreateFromOptions`
(saved order, unknown ids dropped, missing items after their predecessor),
`ToOptions`, tooltips and `ToolbarHotkeyHelper`; region capture's items
(`RegionCaptureToolbar.CreateItems`: the hidden ellipse and freehand
regions first, no Crop or Cut out). The editor's toolbar and its hotkeys
follow `ImageEditorOptions.ToolbarItems` (the defaults saved when empty,
`persist`), region capture's follow `RegionCaptureOptions.ToolbarItems`
(the region shapes show when turned on; using them logs, those regions
aren't ported). Task settings › Image editor's and › Region capture's
Customize toolbar... open `ToolbarCustomizationDialogView` over the window
(move up/down, begin group, show/hide, the two hotkey boxes: click, press
a shortcut, Backspace/Delete clears; Reset, OK with `RefreshValidation`,
Cancel, Escape, the backdrop). vs ShareX (`--render
task-settings-tools-image-editor --content toolbar-editor`,
`task-settings-capture-region --content region-toolbar-editor`): image
editor dark 0.52%, light 0.46%, region capture dark 0.49% (the centred
"NumPad" texts sit a sub-pixel apart). The ellipse region shown and OK in
both apps saves the same 21 items (a test compares with the JSON ShareX
wrote). Also ShareX's one-in-ten sunglasses emoji tool icon.

### Print (`print.rs`, `dialogs/print_window.rs`)

The editor's Print and Application settings › Image print settings...
(the active monitor, preview only): the Print options window (unless
`DontShowPrintSettingsDialog`), Preview (`PrintPreviewWindow`), Print...
(on a thread: `PrintDlgEx` when `ShowPrintDialog`, else the default
printer or `DefaultPrinterOverride`; the page from `RenderImagePage`,
drawn with `StretchDIBits` over the page bounds). `RenderImagePage`
against ShareX's own (`tools/tools-harness --print-page`,
`crates/sharexrs/tests/data/print`): 3 of 4 pages pixel-identical, the
enlarged one within one level. Print options vs ShareX dark 0.64%, light
0.78% (`--render print-window`; ShareX's light window opened from its
editor's Print button and closed without printing). End to end: the test image printed with no
dialog to Microsoft Print to PDF into a temp file (`SHAREXRS_PRINT_TO_FILE`):
one A4 page, the image (JPEG: the driver's compression) over the whole
page, the content where ShareX's page has it (mean error 0.33 levels); the
PDF was deleted. ShareX's own print couldn't run here: Windows 11's print
dialog (`PrintDialog.exe`) never shows a window in this remote session.

### Image effects (`image_editor/effects.rs`, `effects::browser`)

The toolbar's Image Effects item (`ToggleEffectsPanel`, needs an image)
opens `EffectBrowserPanel`: the `overlay-scrim` over the whole view (the
toolbars too; a press on it closes the browser) and the 800px
`floating-panel` 125px from the top, as tall as its content up to 125px
from the bottom. Its search box (`Search image effects... (230)`, focused
and cleared on open), then the sections: Recent and Favorites (each with
its light bulb hint), Manipulations, Adjustments, Filters, Drawings, each
"Name (count)" over a wrap of `Button.effect-item`s (the 16px icon, the
localised browser label, the description as tooltip). A click runs the
effect and puts it first in Recent (ten kept); a right click removes an
item from Recent there and toggles its favourite elsewhere; both lists are
kept in `ImageEditorOptions.RecentEffects`/`FavoriteEffects` (resolved
through `NormalizeEffectId` and ShareX's aliases on load). A right click on
the Image Effects item opens the favourites menu (by name). The model is
`sharex_image_editor::effects::browser` (unit-tested): `EffectSearch.Score`
(keywords, filler words, one typo in words of five letters or more),
matches by score then label length then label, categories by their best
match while searching with Recent and Favorites first, and each category's
items in label order as `EffectCategory.Insert` sorts them.

Running an effect follows `EditorView.EffectsHost`: rotate 90/180 and the
flips (editor operations without parameters) and the `ApplyImmediately`
effects apply at once through `EditorCore`; everything else opens
`SchemaDrivenEffectDialog` in the right sidebar (`editor-sidebar.right`,
282 wide; the canvas narrows, the top toolbar floats over the dialog's
title): the effect's icon and name, then per parameter a slider with its
`0.##` value capsule, a `LabeledToggleSwitch`, a combo box, the
`ColorPickerDropdown` (its popup with the screen colour picker), a numeric
box (`0` or `0.0`, its increment), a text box, or a path box with
Browse..., then Cancel and Apply. Crop and Resize image start at the
image's size. Every change previews at once (`PreviewEffect` on the image
as it was: the canvas and the window title show the preview, the
annotations stay over it, the Background panel's frame is off while
previewing). Apply commits the latest preview with undo (editor operations
through `EditorCore`, annotations moved) and shows ShareX's notification
("Applied Gaussian blur.", or the operation's own); Cancel, Escape, the
Image Effects item and the scrim drop it. Region capture has no image
effects (`RegionCaptureToolbar.CreateItems` keeps only the tools).

Validation (`scripts/compare-editor-effects.sh`, ShareX's shots in
`shots/sharex/editor-effects/` taken with `scripts/editor-effects.ps1` in
the maximized editor on the 640x400 test image; `--render image-editor
--state effects[:search][@recent=...]|effects-favorites|effect:<id>[:key=value,...]`):

| Screen | Dark | Light |
|---|---|---|
| Browser (light: with eight in Recent) | 0.41% | 0.24% |
| Search "blur" | 0.13% | 0.11% |
| Resize image (numeric, toggle; Manipulations) | 0.08% | 0.06% |
| Rotate (slider, toggle) | 0.06% | 0.06% |
| Colorize (colour, slider; Adjustments) | 0.06% | 0.05% |
| Channel swap (enum) | 0.05% | 0.05% |
| ASCII art (sliders, text, toggles; Filters) | 1.50% | 1.48% |
| Background image (file path; Drawings) | 0.06% | 0.06% |
| Border (enums, slider, colour; Drawings; the image grows) | 0.08% | 0.06% |
| Gaussian blur (slider) | 0.06% | 0.05% |
| Favourites menu (the menu, aligned) | | 1.59% |

Each dialog shot includes its live preview on the canvas. Only text
anti-aliasing differs, and ASCII art's preview glyphs (DirectWrite, as in
the catalogue's validation); the favourites menu's extra is its popup
window's transparent corners. The live window matches its render (browser
0.41%, Gaussian blur dialog 0.06%).

Side by side (`scripts/effects-side-by-side.ps1`: each app's editor on the
test image, the browser searched, the first result opened, Apply, Copy,
the clipboard dumped, undo): 20 effects (Gaussian blur at its default and
at a slider position clicked in both, Colorize, Channel swap, Border,
Invert, Emboss, Polaroid, Rotate 90° clockwise, Flip horizontal, Black and
white, Heat map, Checkerboard, Pixelate, Sharpen, Vignette, Sepia, Rounded
corners, Shadow, Posterize): **20/20 copied PNGs pixel-identical, 20/20
CF_DIBs byte-identical.** Rounded corners' translucent corners showed how
ShareX's copy unpremultiplies (in `f32`: the channel over 255 times the
inverse of the alpha over 255, times 255, rounded half to even; now the
editor's `rgba_of`): neither Skia's `readPixels` nor integer rounding
matched both of its ties.

### The rest

- `AutoSizeWindow` (`TryAutoSize`): once there's an image, the window fits
  it (the frame, at least the toolbar's width, the toolbars' room above and
  below, 24 more) centred in the work area, or is maximized. `EditorWindow`
  has no minimum size (ShareXrs's 640x480 was dropped). Side by side with
  the same options (a toolbar of five items, since the whole toolbar is
  wider than the 1024x768 monitor and always maximizes): a 300x200 image
  gives a 651x467 window at 186,126, 640x400 696x667 at 164,26, 900x500
  maximized, in both apps.
- A new text grows the canvas while it's typed
  (`OnLiveTextEditorBoundsChanged`: the overflow + 150 right, + 16 down,
  outside the history), replaced by the exact fit on commit.
- Region capture: the Emoji tool opens the editor's emoji picker over the
  overlay under the pointer (inserted centred on that monitor), and the
  colour popups' screen colour picker opens the app's
  (`ScreenColorPickerWindow.PickAsync`), its colour going to the option.
  Exercised live over `--test-pattern`: the picker opens on the pointer's
  monitor and inserts a 160px emoji centred there; the screen colour
  picker reads the pattern's exact colour (#BC58C8 at 700,600) and sets the
  border colour. Two bugs that only showed live were fixed: the overlay
  never gave the pointer to the popups' egui widgets (the palette, the
  eyedropper, lists and sliders did nothing), and the Emoji button's
  release went to the picker, leaving the toolbar pressed on Emoji so that
  the next tool click opened the picker again.
- The 1280x720 window: a dialog created maximized was then clamped and
  centred, which winit turned into a later restore, leaving a 1280x720
  normal size on the 1024x768 monitor (and `winshot.ps1`'s `SW_RESTORE`
  showing it). Dialogs are now sized, placed and shown, then maximized;
  restored, the editor matches ShareX's (1044x759 at -10,-19).
  CenterScreen centres in the work area.

### Not ported yet

- The macOS and Linux wallpaper services; the Background sidebar's
  open/close animation; ellipse and freehand regions (their toolbar items
  only log).
- ShareX's print checked live (its print dialog doesn't show in this
  session); the print preview window wasn't compared.
- Effects: the browser's item fade-in and the panel's open/close
  transitions; the favourites menu as its own OS window (it's drawn in the
  editor window, so it stays inside it: ShareX's popup is clamped to the
  screen, 5px further left at the right edge); tooltips are egui's; the
  dialogs' preview runs on the UI thread as ShareX's does (slow for big
  images in a debug build); an effect that throws on Apply (Gamma) is
  logged and the panel closes (ShareX's unhandled exception wasn't
  compared).

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
