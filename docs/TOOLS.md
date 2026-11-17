# Tools (ShareX.Tools): map, validation, window plan

ShareX's Tools menu (`MainMenuBuilder.BuildToolCategories`, ShareXrs's
`main_menu::tool_categories`) opens ~30 tool windows from `ShareX.Tools`, whose
work is done partly in their view models and services and partly in
`ShareX.HelpersLib` and `ShareX/TaskHelpers.cs`. `crates/sharex-tools` ports
that work without the windows, one module per tool. This file maps each tool,
says what matches ShareX (measured with `tools/tools-harness`), what isn't
ported, and what each window will need.

## Crate map

| Module | ShareX source | Entry points |
|---|---|---|
| `imaging` | `SkiaImageHelpers` (`LoadImage`/`Decode`, `CreateBitmap`, `FillBackground`, `Resize`, `ResizeImage`, `CreateThumbnail`, `CropBitmap`, the `DrawImage` shims, `Save`, `SavePNG`, `SaveBMP`, `SaveJPEG`, `SaveGIF`, `Quantize`) | `load_image`, `decode`, `encode`, `save_image`, `save_png`, `save_bmp`, `save_jpeg`, `save_gif`, `fill_background`, `resize`, `create_thumbnail` |
| `jpeg`, `jpeg_decode` | libjpeg-turbo as SkiaSharp's Skia drives it (`SkJpegEncoder`, `SkJpegCodec`) | `encode_rgb`, `decode` |
| `gif` | `IndexedImage`, `AnimatedGifCreator`, `AdaptiveGifQuantizer`, `GifPaletteMapper`, `SkiaOctreeQuantizer`, `SkiaGrayscaleQuantizer` | `IndexedImage`, `AnimatedGifCreator`, `AdaptiveGifQuantizer` |
| `combiner` | `SkiaImageHelpers.CombineImages`, `ImageCombinerOptions`, `TaskHelpers.OpenImageCombiner` | `combine_images`, `combine_files`, `create_preview` |
| `splitter` | `ImageSplitterViewModel.SplitImage`, `SkiaImageHelpers.SplitImage` | `split_image`, `split_image_file` |
| `thumbnailer` | `ImageThumbnailerViewModel.GenerateThumbnails` | `create`, `generate`, `output_path` |
| `resizer` | `ImageResizerService`, `ImageResizerViewModel.ResizeImages` | `resize`, `encode`, `create_preview`, `resize_images` |
| `converter` | `ImageConverterService`, `ImageConverterViewModel.ConvertImages` | `load_image`, `encode`, `create_preview`, `convert_images` |
| `watermark` | `ImageWatermarkService`, `ImageWatermarkViewModel.WatermarkImages` | `apply`, `create_preview`, `watermark_images` |
| `icon` | `IconConverterService` | `convert`, `resize_to_square` |
| `comparer` | `ImageComparisonService`, `ImageComparerViewModel` texts | `compare`, `texts` |
| `hash` | `HashChecker`, `Crc32`, `TaskHelpers.CalculateFileHashAsync`, `HashCheckerViewModel` | `hash_file`, `hashes_match` |
| `qr` | `TaskHelpers.GenerateQRCode`/`SaveQRCodeAsync`/`BarcodeScan`/`CheckQRCodeContent`, ZXing.Net's `SvgRenderer` and `BarcodeReaderGeneric.DecodeMultiple`, `QRCodeViewModel` | `generate`, `generate_png`, `generate_svg_file`, `scan`, `join_results` |
| `indexer` | `Indexer`, `IndexerText`/`Html`/`Xml`/`Json`, `FolderInfo`, `IndexerSettings`, `HtmlHelper`, `URLHelpers.HtmlEncode` | `index`, `index_at`, `scan`, `IndexerSettings` |
| `metadata` | `MetadataService`, `MetadataReader`, `MetadataCollector`, `ImageMetadataReader`, `TiffMetadataReader`, `IccMetadataReader`, `XmpMetadataReader`, `IsoBaseMediaMetadataReader`, `MatroskaMetadataReader`, `RiffMetadataReader`, `AsfMetadataReader`, `MetadataStripper`, `MetadataViewModel` | `read`, `strip`, `can_strip`, `build_text`, `group`, `filter` |
| `video::converter` | `VideoConverterOptions`, `VideoConverterViewModel.BuildArguments`, `VideoConversionOutputInfo` | `VideoConverterOptions::arguments`, `ConverterWindow::build_arguments`, `output_info` |
| `video::trimmer` | `VideoTrimmerService` | `build_trim_arguments`, `validate`, `trim` |
| `video::thumbnailer` | `VideoThumbnailer`, `VideoThumbnailOptions`, `VideoInfo.ToString` | `take_thumbnails`, `combine`, `frame_arguments`, `timestamps` |
| `video::gif_maker` | `AnimatedGifMakerService` | `create`, `create_bytes`, `create_preview_frames` |
| `video::gif_trimmer` | `GifLosslessTrimmer`, `AnimatedGifTrimmerDocument` timing | `GifLosslessTrimmer`, `frame_at`, `nearest_boundary` |
| `monitor_test` | `MonitorTestPatternControl`, `MonitorTestViewModel` | `MonitorTest::render`, `advance_motion` |
| `dotnet`, `text`, `options` | `Array.Sort` introsort, `PriorityQueue`, `System.IO.Path`; `string.Format`/number and date formats; Newtonsoft's lenient settings reading | |

Shared with other crates: `sharex-skia` (the canvas, Catmull-Rom/Mitchell
sampling, AA rects, gradients), `sharex-image-effects` (text: fonts, metrics,
glyph outlines; strokes), `sharex-screen-recording` (FFmpeg discovery, the
runner, `GetVideoInfo`), `sharex-helpers::culture` (NLS string comparisons,
moved there from `sharex-history`), `sharex-strings::tools` (ShareX.Tools'
`.resx`).

Two fixes landed in shared code along the way: `sharex-skia`'s
`AntiFillRect` now rounds fractional edges to 8.8 fixed point as Skia m119
does (it truncated, one coverage level low; this also made the image effects'
Border and Text watermark box exact), and its canvas gained Catmull-Rom.

## Validation

`tools/tools-harness` (README there) runs ShareX's own code headlessly over
1321 cases and `cargo run --release -p sharex-tools --example validate`
compares. "Exact" means byte for byte (or equal values/strings); "pixel-exact"
means the bitmap before encoding is identical and the encoded file decodes to
the same pixels (only the PNG zlib stream differs); "close" is within a stated
tolerance.

| Tool | Cases | Exact | Pixel-exact | Close | Differ | Skipped |
|---|---|---|---|---|---|---|
| Image combiner | 96 | 0 | 96 | 0 | 0 | 0 |
| Image comparer | 21 | 21 | 0 | 0 | 0 | 0 |
| Image converter | 190 | 140 | 40 | 0 | 0 | 10 |
| Image converter preview | 10 | 0 | 10 | 0 | 0 | 0 |
| Image splitter | 20 | 0 | 20 | 0 | 0 | 0 |
| Image thumbnailer | 40 | 40 | 0 | 0 | 0 | 0 |
| Image resizer | 96 | 6 | 90 | 0 | 0 | 0 |
| Image watermark | 54 | 1 | 48 | 5 | 0 | 0 |
| Image watermark preview | 1 | 0 | 1 | 0 | 0 | 0 |
| Icon converter | 40 | 30 | 10 | 0 | 0 | 0 |
| Animated GIF maker | 24 | 24 | 0 | 0 | 0 | 0 |
| Animated GIF trimmer (lossless) | 5 | 5 | 0 | 0 | 0 | 0 |
| Hash checker | 6 | 6 | 0 | 0 | 0 | 0 |
| QR code (generate, SVG) | 60 | 60 | 0 | 0 | 0 | 0 |
| QR code (scan) | 21 | 21 | 0 | 0 | 0 | 0 |
| Index folder | 75 | 75 | 0 | 0 | 0 | 0 |
| Metadata (read, copy text, strip) | 17 | 17 | 0 | 0 | 0 | 0 |
| Video converter arguments | 482 | 482 | 0 | 0 | 0 | 0 |
| Video converter output info | 10 | 10 | 0 | 0 | 0 | 0 |
| Video trimmer arguments | 16 | 16 | 0 | 0 | 0 | 0 |
| Video thumbnailer | 4 | 1 | 1 | 2 | 0 | 0 |
| Monitor test | 33 | 0 | 33 | 0 | 0 | 0 |

What the cases cover, per tool:

- **Combiner**: both orientations, three alignments, spacing, wrapping, auto
  background, opaque/transparent/odd sizes; the PNG preview.
- **Splitter / thumbnailer / resizer**: grids 1×2 to 4×5; thumbnail sizes and
  JPEG qualities incl. an EXIF-rotated JPEG source; Fill/Fit/Stretch at sizes
  from 1×1 to 1700×900, PNG and JPEG over a background, the 1600 px previews.
- **Converter**: ten sources (PNG with and without alpha, 16-bit, grayscale,
  palette GIF, 24-bit BMP, lossless WebP, a JPEG, an EXIF orientation-6 JPEG)
  to PNG (Automatic/32/24-bit), BMP (Automatic/32/24), GIF (all five
  quantizers), JPEG (4:2:0/4:2:2/4:4:4 at q90 and q40) and WebP.
- **Watermark**: image watermarks at all nine positions, rotations (0, 30,
  −45, 90, 400°), opacities, scales, JPEG output; five text watermarks.
- **Icon**: four sources, both depths, size sets incl. 256 (PNG entry) and
  duplicates.
- **Comparer**: identical, dotted, different sizes, transparent vs opaque,
  JPEG vs PNG, at preview limits 2048/50/7.
- **GIF maker**: three image sets (incl. transparency and a 420×300), two
  delays, loop on/off, repeat counts.
- **Hash**: files of 0–1 048 579 bytes around the 8 KiB buffer, all six
  algorithms, a missing file.
- **QR**: ten contents (URL, numeric, alphanumeric, UTF-8 with an emoji,
  multi-line, 2952 bytes, 2953 bytes, empty) × six sizes incl. one smaller than
  the code; scans of plain, rotated, inverted, translucent, two-code, dense and
  small codes, Code 128, EAN-13, Data Matrix and a picture without a code, all
  formats and QR only.
- **Index folder**: 25 settings sets over three trees (hidden files and
  folders, names that sort differently by culture, `&`/`'`/non-ASCII names,
  sizes around unit boundaries, an empty folder), all four outputs.
- **Metadata**: JPEG (EXIF/GPS/XMP/two-part ICC/Photoshop IPTC/Adobe/COM),
  progressive JPEG, PNG (every chunk kind), GIF (comment, loop, XMP and other
  application extensions, plain text), big- and little-endian TIFF, BMP, ICO,
  WebP (VP8X/VP8/VP8L/ANIM/ANMF/EXIF/XMP/ICCP), and MP4/MOV/MKV/WebM/AVI/WMV
  written by ShareX's ffmpeg with tags; reading, the copy-all text,
  `CanStripMetadata`, stripping (file bytes) and reading the stripped file.
- **Video**: every codec × CRF/bitrate × four qualities (incl. out of range)
  × video/animation inputs × names with and without extension, through both
  argument builders; trims at fractional and long times, copy and re-encode;
  output info from six real FFmpeg probes and canned text; thumbnailer runs
  (defaults, JPEG 2-column with kept frames, no decorations, separate files)
  on a 12 s synthetic clip.
- **Monitor test**: gray levels incl. out of range, RGB, eight gradients, the
  three patterns at four sizes, moving bars at four offsets both ways,
  rendered by ShareX's own control through Avalonia.Headless.

CI runs `crates/sharex-tools/tests/sharex.rs`: twelve checks against a small
set of ShareX's outputs (`tests/fixtures`, made by the harness's
`make_fixtures.py`) that need no fonts, FFmpeg or Windows.

### Not exact, and why

- **Lossy WebP** (10 converter cases, skipped): there's no Rust port of
  libwebp's VP8 encoder; ShareXrs writes such files lossless. Lossless WebP
  decodes to ShareX's pixels.
- **PNG bytes**: Skia compresses with its zlib and libpng's filter heuristics;
  ShareXrs's PNGs decode to the same pixels (the 256 px entry of 32-bit icons
  too). The icon converter's own 8-bit PNG entry is byte for byte (zlib-rs,
  row by row as .NET's `ZLibStream`).
- **Text glyphs** (text watermark, the video thumbnailer's summary and
  timestamps): Skia rasterises DirectWrite glyph masks, ShareXrs fills the
  same font's outlines at the same positions (as the image effects' Text
  watermark, docs/IMAGE-EFFECTS.md). Characters Segoe UI lacks aren't drawn
  with a fallback font in the watermark (`FallbackFonts`).
- **QR multi-code scanning, QR only**: rxing's `QRCodeMultiReader` gives up on
  an undecodable candidate where ZXing skips it; ShareXrs then retries with the
  generic multiple reader limited to QR codes. Results matched on every case.
- **JPEG decoding** covers sequential Huffman files (what cameras and Skia
  write); progressive, arithmetic-coded and CMYK files go through the `image`
  crate and may differ by a level or two.
- **Platform**: culture-sorted names and hidden attributes follow Windows NLS
  and `FILE_ATTRIBUTE_HIDDEN`; elsewhere a case-folding sort and dot-files, as
  .NET does there. Results are against ShareX on Windows x64.

## Not ported

- **Background remover**: needs an ONNX model run through ONNX Runtime
  (`BackgroundRemovalService`: `InferenceSession`, DirectML/CPU devices,
  `AppPaths.ModelsFolder`, `BackgroundRemoverOptions` with the selected model
  file and device). The pre/post-processing around the model (tensor layout,
  mask resize and apply) is deterministic and could be ported with an ONNX
  runtime crate later; stopped here as planned.
- **Image beautifier**: not a ShareX.Tools tool; `TaskHelpers.OpenImageBeautifier`
  opens the image editor with its background panel (`AnnotateImageModern(...,
  openBackgroundPanel: true)`), so it belongs to the image editor port
  (docs/IMAGE-EDITOR.md).
- **Analyze image**: four AI providers (`OpenAIProvider` (Responses API),
  `OpenAILegacyProvider` (chat completions), `GeminiProvider`,
  `OpenRouterProvider`) build JSON requests with the image as a data URI, plus
  `TestConnectionAsync`/`LoadModelsAsync` (`v1/models`) and `AIOptions` (in
  `TaskSettingsTools.AIOptions`). Portable on `sharex-uploaders`' HTTP layer
  and checkable by capturing ShareX's requests (Harmony on `HttpClient`)
  without network; not done yet.
- **Animated GIF trimmer re-encode** (crop/resize, or a cut whose first frame
  depends on earlier ones): decodes frames with Skia's animated GIF codec and
  writes them with `AnimatedGifCreator` (ported). Needs a Skia-equivalent frame
  compositor; the `image` crate's could be measured against it.
- **OCR** (Windows.Media.Ocr / ShareX.OCR), **Remote storage browser**
  (uploaders' file listings), **Video player** (Media Foundation), the screen
  tools (color pickers, rulers, mouse highlighter, pin to screen) and the other
  System tools (borderless window, inspect window, clipboard viewer, network
  monitor): OS/UI-bound, ported with their windows.

## Window plan

Every window ports its `.axaml` (layout, sizes, strings) and code-behind
(file dialogs, drag & drop, clipboard) on top of the engine above; the
view model's state and commands become the window's state. Common pieces:
`sharex_ui` controls, the settings form layout used by Application/Task
settings, a file list control (Add/Remove/Move with multi-select, used by
combiner, thumbnailer, resizer, converter, watermark, GIF maker), an image
preview pane, a status line, and running work off the UI thread with a
version counter so stale previews are dropped (ShareX's `_previewVersion`).

| Tool | ShareX window (width×height, min width) | Needs |
|---|---|---|
| Image combiner | `ImageCombinerWindow` 980×650, 820 | file list (`ImageCountText`), Horizontal/Vertical toggle with alignment options swapping Top/Center/Bottom and Left/Center/Right, Space and Wrap after, Auto fill background, live PNG preview (`create_preview`), Combine → `UploadManager.RunImageTask` (the after-capture tasks); settings `TaskSettingsTools.ImageCombinerOptions` (`ImageCombinerOptions::from_json/to_json`) |
| Image splitter | `ImageSplitterWindow` 680 wide, 580 | image and output folder pickers (output defaults to the image's folder), rows/columns, `GridSizeText`, `can_split`; opens the folder with the first file |
| Image thumbnailer | `ImageThumbnailerWindow` 900×590, 760 | file list, width/height/quality (150/150/90), output folder (defaults to the first file's), file name `$filename_th`, messages `Generated_one/many`/`Generation_failed` |
| Image resizer | `ImageResizerWindow` 1180×650, 960 | file list, width/height (1920×1080), mode (Fill/Fit/Stretch, default Fit), PNG/JPEG with quality and a background colour, output folder and `$filename_resized`, preview after a 100 ms debounce (`create_preview`), `PreviewSizeText` "w × h" |
| Image converter | `ImageConverterWindow` 1180×650, 960 | file list, format (PNG/JPEG/WebP/GIF/BMP) with per-format options: JPEG quality and subsampling, WebP lossless and effort/quality, PNG bit depth, filter and compression level (the last two only change bytes), GIF quality, BMP bit depth, background colour; Catmull-Rom 1600 px preview, `$filename_converted` |
| Image watermark | `ImageWatermarkWindow` 1220×700, 1000 | file list, Text/Image, text, size and colour, image path and scale, position (3×3), margin, opacity, rotation, PNG/JPEG and background, preview (`create_preview` scales margin and text size), `$filename_watermarked` |
| Icon converter | `IconConverterWindow` 860×570, 720 | image picker/drop (`icon::load_image`), size checkboxes 16–256 (all on), 32-bit / 8-bit, Convert → save dialog with the `.ico` bytes, `Image_dimensions`/`Icon_saved` |
| Image comparer | `ImageComparerWindow` 1180×760, 900 | two image pickers (`comparer::load_image`), Slider mode (ShareX's `ImageComparisonSlider` control: a draggable split between the two images) and Diff view (`ComparisonResult.diff`), similarity text in green when identical (`texts`, `is_identical`), 2048 px linear previews |
| Animated GIF maker | `AnimatedGifMakerWindow` 1120×650, 900 | file list, delay (10–655350 ms), loop and repeat count, animated preview (`create_preview_frames`), save dialog → `create` |
| Animated GIF trimmer | `AnimatedGifTrimmerWindow` 900×720, 760 | `AnimatedGifTrimmerTimeline` (frames, selection handles snapping with `nearest_boundary`), crop overlay, output size, frame preview (needs a GIF frame compositor), lossless export (`GifLosslessTrimmer::copy`) with the dependent-frame fallback, re-encode (not ported) |
| Hash checker | `HashCheckerWindow` 700 wide, 700 | file and second-file pickers and drop, compare-two toggle, algorithm (SHA-256 default, `display_name`), progress bar (`hash_file`'s callback), Stop (the cancel flag), result in upper case, match/differ state (`hashes_match`), notification sound on completion |
| QR code | `QRCodeWindow` 940×610, 760 | content box (2952-byte limit message), size (0 fits the preview area, at least 64: `effective_size`), preview (`generate_png`), Copy image (clipboard), Save (PNG/JPEG/BMP via `imaging::save_image`, SVG via `generate_svg_file`), Upload, Scan screen/region (capture crate) and image file (`scan`), results joined (`join_results`) |
| Index folder | `DirectoryIndexerWindow` 980×680, 820 | folder picker, output (HTML/Text/XML/JSON), each output's settings, source preview (text outputs) or a notice (HTML), Save as (`Index_for` name and extension) and Upload (`TaskHelpers.OpenDirectoryIndexer`: a text upload named `.html` etc.); `IndexerSettings` from `TaskSettingsTools.IndexerSettings` with `BinaryUnits` from the application settings |
| Metadata | `MetadataWindow` 900×650, 700 | open/drop, grouped list (`group`), search (`filter`), Copy all (`build_text`), Strip (confirmation, `can_strip`, `strip`), URLs as links (`https?://[^\s]+`), title with the file name |
| Video converter | `VideoConverterWindow` 850×550, 700 | input/output pickers (output name `<input>-output`), codec list (`display_name`), CRF/bitrate controls per codec (`can_choose_rate_control`, `quality_range`), progress from `FFmpegCliManager`'s encode progress, Stop, the output info line (`output_info` over `probe_arguments`), auto open folder; settings `TaskSettingsTools.VideoConverterOptions`, FFmpeg path from the capture settings (`CheckFFmpeg`) |
| Video thumbnailer | `VideoThumbnailerWindow` 900×650, 760 | video picker, the `VideoThumbnailOptions` form, progress (`take_thumbnails`' callback), result list; uploads each output when `UploadThumbnails` (TaskHelpers) |
| Video trimmer | `VideoTrimmerWindow` 900×720, 760 | Media Foundation preview and timeline (`VideoTrimmerTimeline`); on other platforms an FFmpeg-decoded preview; start/end and Set start/end, Copy vs Re-encode (`build_trim_arguments`), save dialog with the forced extension (`validate`), progress (`trim`) |
| Monitor test | `MonitorTestWindow`: borderless, topmost, not in the taskbar, sized to the screen under the cursor; a 720-wide controls panel | mode buttons, per-mode controls (gray, RGB with hex text and a 320×320 colour view, gradient colours and direction, pattern and size, motion direction and speed 100–2000 px/s, wheel ±50), the pattern drawn by `MonitorTest::render` each frame with `advance_motion`; left click or Space toggles the controls, right click or Esc closes, Left/Right change mode |
