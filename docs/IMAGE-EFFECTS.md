# Image effects (ShareX.ImageEffectsLib)

`crates/sharex-image-effects` ports `ShareX.ImageEffectsLib` (and the
`ShareX.HelpersLib` code it stands on) so that a preset applied in ShareXrs gives
the same pixels as in ShareX, and presets move between the two apps unchanged.
`tools/effects-harness` runs ShareX's own DLLs headlessly to check this.

## Where things are in ShareX

| What | ShareX source |
|---|---|
| Effect classes (51) | `ShareX.ImageEffectsLib/{Adjustments,Filters,Drawings,Manipulations}/*.cs` |
| Base class (`Enabled`, `Name`, `Apply`, `ToString`) | `ImageEffect.cs` |
| Preset (`Name`, `Effects`, `ApplyEffects`, `GetDefaultPreset`) | `ImageEffectPreset.cs` |
| Catalog (categories, order in the Add menu) | `Presentation/ImageEffectCatalog.cs` |
| `.sxie` packages | `ImageEffectPackager.cs`, `Presentation/ImageEffectPackagerWindow.axaml(.cs)` |
| Short type names for import/export | `ImageEffectsSerializationBinder.cs` → `HelpersLib/Settings/KnownTypesSerializationBinder.cs` |
| Safe paths for the image-loading effects | `ImageEffectPathHelpers.cs` |
| The window (presets, effects, property grid, preview) | `Presentation/ImageEffectsWindow.axaml(.cs)`, `ImageEffectsViewModel.cs`, `ImageEffectOptionsPanel.axaml(.cs)`, `GradientOptionsPanel.axaml(.cs)`, `ImageEffectsConfirmationWindow` |
| Drawing helpers | `HelpersLib/Helpers/SkiaImageHelpers*.cs`, `SkiaDrawing.cs`, `SkiaColorMatrixManager.cs`, `SkiaConvolutionMatrixManager.cs`, `ImageColorMatrix.cs`, `SkiaPixelBuffer.cs`, `Colors/GradientInfo.cs` |
| After capture task | `ShareX/WorkerTask.cs` (`DoAfterCaptureJobs`), `TaskHelpers.ApplyImageEffects`, `CaptureHelpers/CaptureBase.cs` (`ImageEffectOnlyRegionCapture`) |
| Settings | `TaskSettingsImage`: `ImageEffectPresets`, `SelectedImageEffectPreset`, `ShowImageEffectsWindowAfterCapture`, `ImageEffectOnlyRegionCapture`, `UseRandomImageEffect` |
| Profile folder | `AppPaths.ImageEffectsFolder` = `<personal folder>/ImageEffects`, also `%ShareXImageEffects%` in paths |
| Import | `TaskHelpers.ImportImageEffect` (CLI `-ImageEffect <file>`, `.sxie` file association, drag and drop on the window) |

## Rendering: what ShareX draws with

Everything is **SkiaSharp 3.119** (Skia m119): `UseSystemDrawing` is only for
`System.Drawing.Color/Point/Size` in the settings. Images are `SKBitmap`,
`Bgra8888`, premultiplied (`SkiaImageHelpers.CreateBitmap`). Three kinds of code:

1. **ShareX's own pixel loops** over `SkiaPixelBuffer`, which converts to straight
   alpha with Skia's `ReadPixels` (float pipeline: `unpremul`, round half to even)
   and back with `(c*a+127)/255`. Gamma, Replace color, Selective color, Blur (box,
   4 passes), Gaussian blur and every convolution (3x3, double maths, truncation),
   Color depth, Pixelate, RGB split, Outline, Reflection's fade, Auto crop.
2. **Skia colour matrix filters** (`SKColorFilter.CreateColorMatrix` drawn through
   `drawImage`): Alpha, Black & white, Brightness, Color matrix, Colorize,
   Contrast, Grayscale, Hue, Inverse, Polaroid, Saturation, Sepia, and the masks of
   Shadow and Glow. Skia: `unpremul`, `matrix_4x5`, `clamp_01`, `premul`, then the
   pipeline's `srcover_rgba_8888`.
3. **Skia canvas drawing**: canvases and offsets (sprite blits), resizing and
   rotating (`SKCubicResampler.Mitchell`, ShareX's `HighQualitySampling`), flips
   (nearest), texture brushes (Rounded corners, Wave edge, Torn edge, tiling),
   gradients (`SKShader.CreateLinearGradient`, clamp), strokes (`DrawRectangleProper`,
   round joins, dash effects), anti-aliased paths, text (`SKFont`, DirectWrite
   typefaces; `DrawText` draws glyphs, `TextPath` fills outlines).

What the port learned about that Skia (all confirmed against the harness):

- libSkiaSharp runs Skia's **SSE** raster pipeline: `mad` is `a*b+c`, never fused
  (FMA gave ±1 in a quarter of the colour matrix cases), stores round half to even.
- Src-over onto N32 goes through the **legacy blitters** whenever it can
  (`SkBlitter::UseLegacyBlitter`): solid colours (`SkARGB32_Blitter`: premultiplied
  as `(c*(a+1))>>8`, `Color32` = `(d*invA + (c<<8) + 128)>>8`) and nearest image
  shaders (`S32A_Opaque_BlitRow32` = `s + (d*(256-sa)>>8)`, `SkBlendARGB32` for
  partial coverage). Integer-offset bitmaps use the sprite blitter (same formula).
  Everything else is the raster pipeline: lowp (16-bit) for solid colours with
  `Src`, clamp-tiled nearest/bilinear images and gradients; highp (float) for
  colour filters, Mitchell sampling and repeat tiling.
- Mitchell sampling changes pixels even at 1:1 (a whole-pixel `DrawImage` with a
  non-identity canvas matrix goes through it), and Skia never treats it as a sprite.

The port lives in the shared `crates/sharex-skia` (the image editor draws
through it too): `sharex_skia::raster` (re-exported here as `skia`) reproduces
those paths with Skia's arithmetic (matrices with `SkMatrix`/`SkM44` float order,
inverse in double, `bicubic_clamp_8888`, `gradient`/`evenly_spaced_2_stop_gradient`
with their stop setup, `ix_and_ptr`'s clamping, the legacy blit formulas).
Coverage comes from the same crate's scan converters, as Skia picks them:
`SkScan::AntiFillPath` (analytic AA, `SkScan_AAAPath.cpp`, or the supersampler,
by `ShouldUseAAA`), `SkScan::AntiFillRect`, edge clipping (`SkEdgeClipper`,
cubics included), and strokes through `SkStroke` (round joins as conics). Each
covered pixel keeps the blitter call that covered it (`blitAntiH`, `blitV`,
`blitAntiH2`, `blitMask`), since the legacy blitters blend each differently.
Paths are `SkPath`s (`AddRoundRect` keeps its conics, so the point count that
picks the scan converter is Skia's). Strokes no wider than a device pixel are
hairlines (`SkScan::AntiHairPath`), dashes are `SkDashPathEffect` over
`SkContourMeasure`. Only aliased path fills still go through tiny-skia (whose
aliased scan converter is Skia's).

## Every effect

Properties in ShareX's order (the JSON order), `= default`, `[setter]`.
Summaries follow `GetSummary`.

### Drawings
| Effect (`$type`) | Properties | How it draws |
|---|---|---|
| Background (`DrawBackground`) | `Color` = Black, `UseGradient` = false, `Gradient` = vertical 4 stops (68,120,194 @0, 13,58,122 @50, 6,36,78 @50, 23,89,174 @100) | `FillBackground`: clear or gradient rect, image on top |
| Background image (`DrawBackgroundImage`) | `ImageFilePath` = "", `Center` = true, `Tile` = false | cover-scaled (Mitchell) or repeat texture, image on top |
| Border (`DrawBorder`) | `Type` = Outside, `Size` = 1 [≥1], `DashStyle` = Solid, `Color` = Black, `UseGradient` = false, `Gradient` = 4 stops | `DrawRectangleProper` stroke (round join, dash 3w/w etc.); outside grows by 2×size |
| Checkerboard (`DrawCheckerboard`) | `Size` = 10 [≥1], `Color` = LightGray, `Color2` = White | repeat texture of a 2×2 pattern, image on top |
| Image (`DrawImage`) | `ImageLocation` = "", `Placement` = TopLeft, `Offset` = 0,0, `SizeMode` = DontResize, `Size` = 0,0, `RotateFlip` = None, `Tile` = false, `AutoHide` = false, `InterpolationMode` = HighQualityBicubic, `CompositingMode` = SourceOver, `Opacity` = 100 [0..100] | `DrawImage(image, rect, sampling, paint)` (alpha = round(opacity×2.55)) or a translated repeat texture |
| Particles (`DrawParticles`) | `ImageFolder` = "", `ImageCount` = 1 [1..1000], `Background` = false, `RandomSize` = false, `RandomSizeMin` = 64, `RandomSizeMax` = 128, `RandomAngle` = false, `RandomAngleMin` = 0, `RandomAngleMax` = 360, `RandomOpacity` = false, `RandomOpacityMin` = 0, `RandomOpacityMax` = 100, `NoOverlap` = false, `NoOverlapOffset` = 0, `EdgeOverlap` = false | `RandomFast`; *.png/*.jpg of the folder; rotation through the canvas; opacity as an alpha colour matrix |
| Text (`DrawTextEx`) | `Text` = "Text", `Placement` = TopLeft, `Offset` = 0,0, `Angle` = 0, `AutoHide` = false, `Font` = "Arial, 36pt", `Color` = 235,235,235, `UseGradient` = false, `Gradient` = random of 3 horizontal 2-stop, `Outline` = false, `OutlineSize` = 5, `OutlineColor` = 235,0,0, `OutlineUseGradient` = false, `OutlineGradient`, `Shadow` = false, `ShadowOffset` = 0,5, `ShadowColor` = 125,0,0,0, `ShadowUseGradient` = false, `ShadowGradient` | `TextPath` (glyph outlines), rotated, shadow, outline stroke, fill |
| Text watermark (`DrawText`) | `Text` = "Text watermark", `Placement` = BottomRight, `Offset` = 5,5, `AutoHide` = false, `TextFont` = "Arial, 11.25pt", `TextRenderingMode` = SystemDefault, `TextColor` = 235,235,235, `DrawTextShadow` = true, `TextShadowColor` = Black, `TextShadowOffset` = -1,-1, `CornerRadius` = 4 [≥0], `Padding` = 5,5,5,5, `DrawBorder` = true, `BorderColor` = Black, `BorderSize` = 1, `DrawBackground` = true, `BackgroundColor` = 42,47,56, `UseGradient` = false, `Gradient` = 4 stops | round rect (inset ½ px) filled and stroked, `canvas.DrawText` per line |

`Text`/`TextEx` parse `%width%`, `%height%` and the other `NameParser` text
tokens. Fonts: `ImageFont` strings "Name, size unit[, style=Bold, Italic]"
(pt/px/in/doc/mm/world/display), pixel size at 96 DPI.

### Manipulations
| Effect | Properties | How it draws |
|---|---|---|
| Auto crop (`AutoCrop`) | `Sides` = Top, Bottom, Left, Right, `Padding` = 0 | `FindAutoCropRectangle` (same colour as the top-left pixel, alpha only when transparent), crop, canvas of the corner colour |
| Canvas (`Canvas`) | `Margin` = 0,0,0,0, `MarginMode` = AbsoluteSize, `Color` = Transparent | image at the offset, margins filled with `Src` |
| Crop (`Crop`) | `Margin` = 0,0,0,0 [negative values rejected] | `CropBitmap`; null (ends the preset) when out of bounds |
| Flip (`Flip`) | `Horizontally` = false, `Vertically` = false | scale -1 through the canvas |
| Force proportions (`ForceProportions`) | `ProportionalWidth` = 1 [≥1], `ProportionalHeight` = 1 [≥1], `Method` = Grow, `GrowFillColor` = Transparent | crop centred, or a centred canvas (`ResizeImage(..., allowEnlarge: false)`) |
| Resize (`Resize`) | `Width` = 250, `Height` = 0, `Mode` = ResizeAll | aspect ratio for a 0 side, Mitchell |
| Rotate (`Rotate`) | `Angle` = 0, `Upsize` = true, `Clip` = false | canvas rotation about the centre, Mitchell, non-AA edges |
| Rounded corners (`RoundedCorners`) | `CornerRadius` = 20 [≥0] | AA round rect filled with the image as a texture |
| Scale (`Scale`) | `WidthPercentage` = 100, `HeightPercentage` = 0 | as Resize |
| Skew (`Skew`) | `Horizontally` = 0, `Vertically` = 0 | three-point affine `DrawBitmap` (nearest) |

### Adjustments (colour matrices unless noted)
Alpha (`Value` = 1, `Addition` = 0), Black & white (`BlackWhite`), Brightness
(`Value` = 0), Color matrix (`MatrixColor`: `Rr` `Rg` `Rb` `Ra` `Ro` `Gr` ... `Ao`,
identity), Colorize (`Color` = Red, `Value` = 0), Contrast (`Value` = 1), Gamma
(`Value` = 1, pixel loop with a table, clamped 0.1–5), Grayscale (`Value` = 1),
Hue (`Angle` = 0), Inverse, Polaroid, Replace color (`SourceColor` = White,
`AutoSourceColor` = false, `TargetColor` = Transparent, `Threshold` = 0; pixel
loop), Saturation (`Value` = 1), Selective color (`LightColor` = White,
`DarkColor` = Black, `PaletteSize` = 10 [2..100 when applied]; pixel loop), Sepia
(`Value` = 1).

### Filters
| Effect | Properties | How |
|---|---|---|
| Blur | `Radius` = 15 [≥3, odd] | box blur ×4 (H, V, H, V) |
| Color depth (`ColorDepth`) | `BitsPerChannel` = 4 [1..8] | pixel loop, `Math.Round` |
| Convolution matrix (`MatrixConvolution`) | `X0Y0`…`X2Y2` = identity, `Factor` = 1.0 (double), `Offset` = 0 (byte) | 3×3 kernel |
| Edge detect, Emboss, Mean removal, Sharpen, Smooth | — | fixed 3×3 kernels |
| Gaussian blur (`GaussianBlur`) | `Radius` = 15 [≥1] | separable kernel, σ = r/3, alpha included |
| Glow | `Size` = 20 [≥0], `Strength` = 1 [≥0.1], `Color` = White, `UseGradient` = false, `Gradient` = random of 3 diagonal 2-stop, `Offset` = 0,0 | canvas, box blur, mask (colour matrix or gradient alpha), image on top |
| Outline | `Size` = 1 [≥1], `Padding` = 0 [≥0], `Color` = Black, `OutlineOnly` = false | distance to opaque pixels |
| Pixelate | `Size` = 10 [≥2], `BorderSize` = 0 [≥0], `BorderColor` = Black | alpha-weighted block average, optional stroked grid texture |
| Reflection | `Percentage` = 20 [1..100], `MaxAlpha` = 255 [0..255], `MinAlpha` = 0 [0..255], `Offset` = 0, `Skew` = false, `SkewSize` = 25 | flipped copy, alpha fade, optional skew |
| RGB split (`RGBSplit`) | `OffsetRed` = -5,0, `OffsetGreen` = 0,0, `OffsetBlue` = 5,0 | pixel loop |
| Shadow | `Opacity` = 0.6 [0.1..1], `Size` = 10 [≥0], `Darkness` = 0, `Color` = Black, `Offset` = 0,0, `AutoResize` = true | mask matrix, box blur, alpha matrix for darkness, image on top |
| Slice | `MinSliceHeight` = 10 [≥1], `MaxSliceHeight` = 100 [≥1], `MinSliceShift` = 0, `MaxSliceShift` = 10 | random horizontal strips (`RandomFast`) |
| Torn edge (`TornEdge`) | `Depth` = 15, `Range` = 20, `Sides` = all, `CurvedEdges` = true | random polygon or closed curve filled with the image |
| Wave edge (`WaveEdge`) | `Depth` = 15, `Range` = 20, `Sides` = all | cosine polygon filled with the image |

## Presets: storage and serialisation

- **Where**: `TaskSettings.ImageSettings.ImageEffectPresets` (a list, in
  `ApplicationConfig.json`'s `DefaultTaskSettings` and each workflow's task
  settings), `SelectedImageEffectPreset`, `UseRandomImageEffect`,
  `ShowImageEffectsWindowAfterCapture`, `ImageEffectOnlyRegionCapture`. A fresh
  profile has one preset, `GetDefaultPreset()`: a 30 px bottom canvas and a
  gradient text watermark. Assets live in `<profile>/ImageEffects`.
- **JSON** (Newtonsoft through `JsonHelpers`, indented, `StringEnumConverter`):
  `{"Name": "", "Effects": [ ... ]}`. Each effect starts with `"$type"`, then its
  own properties in declaration order, then `"Enabled"`, then `"Name"` unless it's
  `""` (`DefaultValueHandling.Ignore`). Effects without a constructor (Black &
  white, Inverse, Polaroid, Edge detect, Emboss, Mean removal, Sharpen, Smooth,
  RGB split, Force proportions) never get their `[DefaultValue("")]`, so they
  write `"Name": null`. Every other property is always written.
- **`$type`**: settings files use `"ShareX.ImageEffectsLib.Canvas, ShareX.ImageEffectsLib"`;
  exports and `.sxie` (through `ImageEffectsSerializationBinder`) use `"Canvas"`.
  ShareX only reads the form matching the serializer it uses; ShareXrs reads both.
- **Values**: colours through `ColorConverter` ("Red", "235, 235, 235",
  "125, 0, 0, 0"; numbers equal to a web colour read back as its name, "0, 0, 128"
  → "Navy"), `Point`/`Size` "x, y", margins "l, t, r, b" (or one value), fonts as
  above, flags as "Top, Bottom, Left, Right", floats in .NET's shortest form,
  gradients `{"Type": "Vertical", "Colors": [{"Color": ..., "Location": 0.0}]}`.
  Reading applies the setters' clamps and skips values that don't convert.
- **`.sxie`**: a zip with `Config.json` (the preset, export form) and the image
  files of an assets folder under the assets folder's name. Import extracts the
  images (≤ 100 MB uncompressed, no paths outside) into `ImageEffects`, then
  `SanitizeImportedPreset` clears image paths that point outside that folder.
- **Paths** for Image, Background image and Particles must be local: no UNC,
  device or URL paths, no reparse points; relative paths are relative to the
  application folder; `%ShareXImageEffects%` and environment variables expand.

## In ShareXrs

- `sharex_image_effects::EffectKind`: every effect, `create(type, rng)` (ShareX's
  constructor defaults), `get_property`/`set_property` by ShareX's property name
  (with the setters' clamps), `property_names()`, `display_name()`,
  `summary()`, `category()`; `CATALOG` lists them by category in ShareX's order.
- `ImageEffect` (`enabled`, `name`, `label()` = `ToString()`),
  `ImageEffectPreset` (`from_json`, `to_json_string(TypeNames::Full|Short)`,
  `apply`, `default_preset`), `preset::sanitize_imported`.
- `apply_image_effects(bitmap, &ImageEffectsSettings, &mut Context)`:
  `TaskHelpers.ApplyImageEffects` without the window. The after capture task
  calls it (`crates/sharexrs/src/capture_job.rs`, before the clipboard and save
  tasks, as `DoAfterCaptureJobs`), with `ImageEffectOnlyRegionCapture` honoured in
  `App::after_capture`. `ShowImageEffectsWindowAfterCapture` only logs until the
  window exists.
- `package::{package, extract}`: `.sxie`.
- `sharex-settings` keeps `ImageEffectPresets` as raw JSON (`Vec<Value>`), so
  settings files round-trip untouched; the crate parses them where they're used.

## Validation

`tools/effects-harness` (see its README) runs ShareX's own
`ShareX.ImageEffectsLib.dll`/`ShareX.HelpersLib.dll`/libSkiaSharp from the
reference build on 121 cases (`tools/effects-harness/cases.json`: every effect,
several parameter sets, on a 157×103 opaque photo-like image, a 121×87 image with
soft and hard transparency, a 9×7 random image and auto-crop images; random
effects with a fixed seed), and writes the raw premultiplied pixels. Then:

```
cargo run --release -p sharex-image-effects --example validate -- tools/effects-harness/cases.json <work>
```

compares per channel (premultiplied B, G, R, A, as ShareX holds them).
`crates/sharex-image-effects/tests/sharex.rs` keeps ShareX's serialised
defaults, labels and six small pixel cases as fixtures, so `cargo test` checks
them without .NET.

**Result: 117 of 121 cases identical, byte for byte**; 1 more within ±1 on a few
edge pixels. (107 before the shared analytic AA, cubic clipping and `SkStroke`;
116 before hairlines.)
Per effect:

| Effect | Cases | Result |
|---|---|---|
| Alpha, Black & white, Brightness, Color matrix, Colorize, Contrast, Gamma, Grayscale, Hue, Inverse, Polaroid, Replace color, Saturation, Selective color, Sepia | 33 | identical |
| Blur, Color depth, Convolution matrix, Edge detect, Emboss, Gaussian blur, Glow, Mean removal, Outline, Reflection, RGB split, Shadow, Sharpen, Slice, Smooth, Wave edge | 33 | identical |
| Pixelate | 3 | identical |
| Torn edge | 2 | identical |
| Auto crop, Canvas, Crop, Flip, Force proportions, Resize, Rotate, Scale, Skew | 25 | identical |
| Rounded corners | 2 | identical |
| Background, Background image, Checkerboard, Image, Particles | 13 | identical |
| Border | 4 | identical |
| Text (`DrawTextEx`) | 3 | default and outline + shadow identical; with a gradient fill: edge pixels ±1 (0.9%) |
| Text watermark (`DrawText`) | 3 (incl. the default preset) | box, border (a hairline), position and colours identical; glyph pixels differ (max ~190, 3–5%). Since `sharex-skia`'s `AntiFillRect` rounds its edges to 8.8 fixed point as Skia m119 does (2026-11-17), the box and border's alpha matches too |
| A chain (Resize, Saturation, Shadow, Border, a disabled Blur) | 1 | identical |

**Presets: every case's preset written back by ShareXrs equals ShareX's
(export form), every effect's default equals ShareX's in both forms, and ShareX's
JSON read and rewritten is unchanged** (including `"Name": null`, named colours,
the default preset and a named, disabled effect). Effect labels
(`ToString()`) and category/effect names match ShareX's for all 51 types.

### Not exact, and why

- **Text watermark glyphs**: ShareX's `canvas.DrawText` rasterises glyphs through
  DirectWrite (hinted, grayscale) at whole-pixel origins; ShareXrs fills the same
  font's outlines (same file, same advances and metrics: hhea ascent/descent/gap,
  unhinted advances, no shaping or kerning as `SKFont.MeasureText`) at the same
  origins. Shapes and positions agree; glyph edge pixels don't. `TextPath`-based
  Text is exact.
- **±1 on partially covered pixels under a gradient** (one Text case): the
  lowp coverage blend for shaded (non-solid) paths isn't fully pinned down.
- **Math library**: Gaussian kernels (`Math.Exp`), Hue (`Math.Cos/Sin`), Rotate
  sizes use the platform's libm; no case differs, but a 1-ulp difference could
  flip a truncation.
- **Platform**: these results are against ShareX on Windows x64. ShareX on another
  CPU path (NEON, or a Skia build with HSW/FMA) differs from itself the same way.
- Not covered by the harness: `DrawImage` with Bilinear sampling (the legacy
  `SkBitmapProcState` bilinear filter is ported for clamped scale-translate
  draws, as the editor's magnifier uses it; repeat/mirror bilinear still goes
  through the pipeline),
  `TextRenderingMode` ClearType (Skia's LCD text), and EXIF-rotated image files
  (decoded by the `image` crate's orientation, not SkCodec).

### Not ported

- The Image effects window and the packager window (plan below), the
  confirmation window, `ShowImageEffectsWindowAfterCapture`.
- Wiring `.sxie` import (CLI `-ImageEffect`, file association, drag and drop) and
  the Tools › Image effects entry: they need the window.
- Text watermark's DirectWrite glyph rasteriser (see above).

## Plan for the Image effects window

Port `Presentation/ImageEffectsWindow.axaml` + `ImageEffectsViewModel.cs` as one
egui window (`crates/sharexrs/src/image_effects_window.rs`), three modes as
ShareX (`Editor` after capture, `Tool` with an image, `Presets` from settings):

1. Capture ShareX's window in each state into `shots/sharex/image-effects-*.png`
   (empty, a preset selected, an effect selected with each property editor kind,
   the Add menu open, light theme).
2. Layout from the `.axaml` (preset list, effect list with check boxes and
   drag-reorder, Add/Remove/Duplicate/Move, the property panel, preview with
   checkerboard, zoom and the source/size line, OK/Cancel/Load/Save/Copy/Paste).
   Strings from `sharex_strings::image_effects_lib`, icons from
   `sharex_ui::keys::ImageEffectIcons`.
3. The property panel (`ImageEffectOptionsPanel`) is generic over
   `EffectKind::property_names()` / `get_property` / `set_property`, choosing the
   editor from the JSON value's shape as ShareX's panel does from the CLR type:
   numbers, check boxes, enums (`dotnet` enums' `ALL` and the
   `ImageEffectEnum_*` strings), colours (colour picker), point/size/margins,
   fonts, file/folder paths, gradients (`GradientOptionsPanel`).
4. Preview: apply the selected preset on a worker thread (`ImageEffectPreset::apply`
   on the source or ShareX's generated sample image, `CreateSampleImage`), as
   `ApplySelectedPresetAsync`; show the result through the existing texture code.
5. Import/export: clipboard JSON in the export form, `.sxie` through
   `package::extract` + `preset::sanitize_imported`, the packager window with
   `package::package`; then `-ImageEffect` and drag and drop.
6. Validate with `--render image-effects[-state]` against the captures and
   `scripts/compare.py`, as every other window.
