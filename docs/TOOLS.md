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
| `video::trimmer` | `VideoTrimmerService`, `VideoTrimmerViewModel.FormatTime`/`TryParseTime` | `build_trim_arguments`, `validate`, `trim`, `format_time`, `try_parse_time` |
| `video::thumbnailer` | `VideoThumbnailer`, `VideoThumbnailOptions`, `VideoInfo.ToString` | `take_thumbnails`, `combine`, `frame_arguments`, `timestamps` |
| `video::gif_maker` | `AnimatedGifMakerService` | `create`, `create_bytes`, `create_preview_frames` |
| `video::gif_trimmer` | `GifLosslessTrimmer`, `AnimatedGifTrimmerDocument` (timing, `Export` with its re-encode, `RenderFrame`), `SkiaAnimatedImage.GetFrame` over Skia's GIF codec, `AnimatedGifTrimmerViewModel`'s selection | `GifLosslessTrimmer`, `AnimatedGif::frame`, `AnimatedGifTrimmerDocument::{open, export, export_bytes, render_frame}`, `TrimSelection`, `crop_and_resize` |
| `monitor_test` | `MonitorTestPatternControl`, `MonitorTestViewModel` | `MonitorTest::render`, `advance_motion` |
| `system::clipboard_viewer` | `ClipboardViewerData`, `ClipboardData`, `ClipboardHelpersEx.ImageFromClipboardDib`, `ClipboardViewerViewModel`'s texts | `ClipboardSnapshot::{set, formats, preview}`, `image_from_clipboard_dib`, `format_count_text`, `preview_details` |
| `system::inspect_window` | `InspectWindowViewModel`, `InspectWindowService`'s list rules, `WindowStyles.ToString()` | `details`, `clipboard_text`, `window_styles_to_string`, `visible_windows`, `percent_to_opacity` |
| `system::network_monitor` | `NetworkMonitorViewModel`, `NetworkMonitorProbe`, `NetworkMonitorEvent`, `NetworkMonitorEventLog`, `NetworkInterface.GetIsNetworkAvailable` | `NetworkMonitor::{record, needs_confirmation, update_connection, set_time_range, copy_all_text}`, `probe_result`, `probe` (with a `Pinger`), `append_log`, `is_network_available` |
| `system::borderless_window` | `BorderlessWindowSettings`, `BorderlessWindowManager`, `BorderlessWindowViewModel.Toggle` | `BorderlessWindowSettings::{from_json, to_json}`, `BorderlessWindowManager::toggle` (with `WindowOps`), `toggle` |
| `background_remover` | `BackgroundRemovalService`, `BackgroundRemovalModel`, `BackgroundRemoverOptions`, `BackgroundRemoverViewModel`'s model list | `BackgroundRemovalService::remove_background`, `scan_models`, `select_model`, `load_image`, `BackgroundRemoverOptions::{from_json, to_json}` |
| `analyze_image` | `AIOptions`, `AIProviderFactory`, `OpenAIProvider` (Responses API), `OpenAILegacyProvider` (chat completions), `GeminiProvider`, `OpenRouterProvider`, `AnalyzeImageService`, `AnalyzeImageViewModel`'s prompts and result text, `AnalyzeImageOptionsViewModel`'s lists | `AIOptions::from_json/to_json`, `build_analyze_request`, `parse_analyze_response`, `analyze`, `test_connection`, `load_models`, `result_text` |
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
1333 cases and `cargo run --release -p sharex-tools --example tools_validate`
compares. "Exact" means byte for byte (or equal values/strings); "pixel-exact"
means the bitmap before encoding is identical and the encoded file decodes to
the same pixels (only the PNG zlib stream differs); "close" is within a stated
tolerance.

| Tool | Cases | Exact | Pixel-exact | Close | Differ | Skipped |
|---|---|---|---|---|---|---|
| Image combiner | 96 | 0 | 96 | 0 | 0 | 0 |
| Image comparer | 21 | 21 | 0 | 0 | 0 | 0 |
| Image converter | 238 | 208 | 30 | 0 | 0 | 0 |
| Image converter preview | 10 | 0 | 10 | 0 | 0 | 0 |
| Image splitter | 20 | 0 | 20 | 0 | 0 | 0 |
| Image thumbnailer | 40 | 40 | 0 | 0 | 0 | 0 |
| Image resizer | 96 | 6 | 90 | 0 | 0 | 0 |
| Image watermark | 54 | 1 | 48 | 5 | 0 | 0 |
| Image watermark preview | 1 | 0 | 1 | 0 | 0 | 0 |
| Icon converter | 40 | 30 | 10 | 0 | 0 | 0 |
| Animated GIF maker | 24 | 24 | 0 | 0 | 0 | 0 |
| Animated GIF trimmer (lossless) | 5 | 5 | 0 | 0 | 0 | 0 |
| Animated GIF trimmer (frames, preview, export) | 5 | 0 | 5 | 0 | 0 | 0 |
| Trimmers' time texts | 1 | 1 | 0 | 0 | 0 | 0 |
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
| Clipboard viewer previews | 1 | 0 | 1 | 0 | 0 | 0 |
| Inspect window texts | 1 | 1 | 0 | 0 | 0 | 0 |
| Background remover | 4 | 4 | 0 | 0 | 0 | 0 |
| Analyze image (requests and answers) | 141 | 107 | 34 | 0 | 0 | 0 |

What the cases cover, per tool:

- **Combiner**: both orientations, three alignments, spacing, wrapping, auto
  background, opaque/transparent/odd sizes; the PNG preview.
- **Splitter / thumbnailer / resizer**: grids 1×2 to 4×5; thumbnail sizes and
  JPEG qualities incl. an EXIF-rotated JPEG source; Fill/Fit/Stretch at sizes
  from 1×1 to 1700×900, PNG and JPEG over a background, the 1600 px previews.
- **Converter**: ten sources (PNG with and without alpha, 16-bit, grayscale,
  palette GIF, 24-bit BMP, lossless WebP, a JPEG, an EXIF orientation-6 JPEG)
  to PNG (Automatic/32/24-bit), BMP (Automatic/32/24), GIF (all five
  quantizers), JPEG (4:2:0/4:2:2/4:4:4 at q90 and q40) and WebP (lossy q90,
  lossless effort 75); and WebP alone at lossy q0/1/50/100 and lossless
  effort 0/1/50/100 over six images (with and without alpha, sizes off the
  16 px macroblock grid, 1×1, 1900×1100): every WebP file byte for byte
  (68/68).
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
- **GIF trimmer**: `SkiaAnimatedImage.GetFrame` for every frame of five
  GIFs (`trim.gif`, a GIF maker output, and `disp0-2.gif`: transparent and
  opaque, partial and full frames, every disposal incl. restore-to-previous,
  a first frame smaller than the screen): 28/28 frames identical; 52
  exports (lossless cuts, re-encodes, crops, resizes, crop and resize, a
  1-pixel crop, cuts that need re-encoding, out-of-range arguments), every
  written GIF byte for byte; the previews (`RenderFrame` at 640 and 20 px)
  pixel-identical ("pixel-exact" in the table: their PNG bytes). The time
  boxes' `FormatTime`/`TryParseTime` on 47 values.
- **Clipboard viewer**: `ClipboardViewerData.GetPreview`, built by
  reflection over a `ClipboardData` the harness fills (the real clipboard
  is never read): text, files, raw bytes as hex, PNG/`Bitmap`/`image/png`
  (an undecodable one too), `CF_DIB` at 24 and 32 bpp, bottom-up and
  top-down, bit fields with and without alpha, an opaque 32 bpp with zero
  alpha bytes, 8 bpp with a palette (Skia's BMP decoder), truncated,
  `CF_DIBV5`, a missing format: 28/28 previews (18 images pixel-identical).
- **Inspect window**: `WindowStyles.ToString()` on 44 style and extended
  style values (aliases, composite names, unknown bits) and
  `FormatRectangle` on 6 rectangles.
- **Background remover**: `make_bg_models.py` writes four tiny ONNX models
  with the local `onnx` package (no model is downloaded): NCHW 32x32 with a
  negative output (the sigmoid branch), NCHW 24x40 all positive (min-max),
  NHWC in and channels-last out, a dynamic input (ShareX's 1024x1024) with
  a 2-D output. Each runs over three images (alpha, opaque, odd size)
  through ShareX's `BackgroundRemovalService` on the CPU device and
  ShareXrs's with the same `onnxruntime.dll` (`ORT_DYLIB_PATH`): 12/12
  results byte-identical; display names, session caching and device texts
  equal.
- **Monitor test**: gray levels incl. out of range, RGB, eight gradients, the
  three patterns at four sizes, moving bars at four offsets both ways,
  rendered by ShareX's own control through Avalonia.Headless.
- **Analyze image** (`run_analyze.sh`: the harness's `--analyze` mode runs
  ShareX's `AnalyzeImageService`, `tools_analyze_harness` ShareXrs's, both
  against `tools/uploaders-harness/mock_server.py`; the request, every
  header but the user agent and the body byte for byte, and the answer
  read): all four providers with a PNG file, a JPEG file and image bytes
  (opaque and with alpha), empty and undecodable bytes; prompts with
  quotes, HTML characters, backslashes, non-ASCII, an emoji, line and
  paragraph separators, empty and null; OpenAI's reasoning effort rules
  (`gpt-5.` models, `minimal` → `none`, defaults), verbosity, null models,
  custom URLs (with and without `/v1/responses`, trailing slash, blank),
  keys with URL characters and null keys; answers that are well formed,
  empty, null, of the wrong type, malformed, BOM-prefixed, with dates
  Newtonsoft rewrites, and HTTP errors; test connection (each provider,
  long bodies cut at 200 characters, missing keys, an unknown provider, a
  status without a reason phrase) and the model list (sorting, blank ids,
  missing members, wrong types, errors). "Pixel-exact" there means the
  Gemini/OpenRouter PNG of image bytes: Skia's zlib stream isn't ported,
  the pixels are the same. JSON readers' exception texts
  (`System.Text.Json`, Newtonsoft) aren't copied: ShareXrs fails on the same
  answers in its own words. The .NET side rewrites every `HttpClient`'s
  requests to the mock through `HttpHandlerDiagnosticListener` (Gemini and
  OpenRouter make a plain `new HttpClient()` that a factory swap can't
  reach), after checking that both kinds of client reach it. CI replays
  the recorded requests and answers (`tests/analyze_image.rs`, fixtures in
  `tests/fixtures/analyze`).

CI runs `crates/sharex-tools/tests/sharex.rs`: thirteen checks against a small
set of ShareX's outputs (`tests/fixtures`, made by the harness's
`make_fixtures.py`) that need no fonts, FFmpeg or Windows.

### WebP: ShareX's libwebp

ShareX encodes WebP with the libwebp compiled into SkiaSharp 3.119.4's
`libSkiaSharp.dll` (mono/skia `release/3.119.4`, f568ac9). Its PDB lists
every libwebp file with its MD5 (`tools/tools-harness/libwebp-sources.md5`,
from `llvm-pdbutil dump -files`), and the libwebp that the crates.io crate
`libwebp-sys` 0.14.4 bundles (in `vendor/`) matches all 112 of them, so
`sharex-tools` depends on exactly that version (pinned `=0.14.4`, no default
features: no threads, which Skia's single-threaded config never uses).
`crate::webp` then calls it as Skia's `SkWebpEncoderImpl.cpp` does:
`WebPConfigPreset(DEFAULT, quality)`; lossy `method = 3`, `use_argb = 0`;
lossless `method = 0`, `use_argb = 1` with the effort as the quality; the
premultiplied pixels read into unpremultiplied BGRA as `readPixels` does
and imported with `WebPPictureImportBGRA`; `WebPEncode`; and, since the
converter's bitmaps carry SkCodec's sRGB colour space, `WebPMux` adds
Skia's sRGB profile as an `ICCP` chunk (`VP8X`, `ICCP`, `ALPH`, `VP8 `/`VP8L`).
All 68 WebP files come out byte for byte, lossy and lossless. libwebp's SIMD
paths are bit-exact with its C code, and the CI test
(`webp_is_byte_identical`) checks five of the files on macOS (NEON) and
Linux too.

Linting macOS from Windows (`scripts/clippy-macos.ps1`): libwebp-sys's build
script compiles C, which needs a compiler for the target that Windows
lacks; since clippy never links, the script points `cc` at a stub
(`scripts/cc-stub.py`) that writes empty objects and archives.

### Not exact, and why

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

- **Background remover on the GPU**: ShareX's Auto and GPU devices add
  DirectML (`AppendExecutionProvider_DML` on the DXGI adapter with the most
  dedicated memory). ShareXrs loads a CPU ONNX Runtime, so Auto runs on the
  CPU and reports "CPU (GPU unavailable)", as ShareX does when DirectML
  fails, and GPU fails. On the CPU the results are byte-identical.
- **Image beautifier**: not a ShareX.Tools tool; `TaskHelpers.OpenImageBeautifier`
  opens the image editor with its background panel (`AnnotateImageModern(...,
  openBackgroundPanel: true)`), so it belongs to the image editor port
  (docs/IMAGE-EDITOR.md, "The Background panel": its export is 23/26
  pixel-identical to ShareX's).
- **Analyze image** differs from ShareX on purpose in two ways. ShareX's
  OpenAI providers set `Authorization` on `HttpClientFactory`'s shared
  client, so every later request through it (other tools, uploads, Gemini's
  model list) carries the OpenAI key to whatever host it goes to; ShareXrs
  sets it on the request only (the harness's `leak_2_gemini_after_openai`
  shows ShareX sending it to Google). And Gemini's and OpenRouter's
  requests carry ShareXrs's user agent, where ShareX's plain `HttpClient`
  sends none. ureq doesn't give the server's reason phrase, so a test
  connection message shows the standard one for a non-standard phrase.
- **GIF decoding** goes through the `gif` crate (LZW, interlacing) and
  composites as Skia's codec does (`SkFrameHolder`'s required frames, the
  prior frame's restore-to-background, Wuffs' source-over pixels, palette
  entries past the table opaque black). Truncated or malformed GIFs may
  stop at a different frame than Wuffs does.
- **The System tools' native side**: reading the clipboard, enumerating and
  reading windows (`WindowInfo`), sending ICMP echoes and changing other
  apps' window styles belong to the windows (sharex-capture already lists
  windows for Capture › Window). The engines take their results
  (`ClipboardSnapshot`, `WindowSnapshot`/`WindowEntry`, `Pinger`,
  `WindowOps`). `is_network_available` is read-only and Windows only (it
  says yes elsewhere for now). There's no "canvas/sheet splitter" tool in
  this ShareX version; the Image splitter is above.
- **Video player** (Media Foundation): OS/UI-bound, ported with its
  window. OCR is `sharex-ocr` (docs/OCR.md), the Remote storage browser's
  data side `sharex_uploaders::remote_storage` (docs/UPLOADERS.md), the
  screen tools are in "The Screen tools in ShareXrs".

## The Images windows in ShareXrs

`crates/sharexrs/src/image_tools/` ports the eight Images tool windows on
the engines above; the Image effects window is in docs/IMAGE-EFFECTS.md.

| File | ShareX source | What's there |
|---|---|---|
| `mod.rs` | `ToolStyles.axaml`, the windows' shared blocks | `tool-panel`, `section-heading`, `row-label`, `path-text` (ellipsis); the icon+text, text and icon-only buttons; `FileList` (the `ListBox` with `SelectionMode="Multiple"`: click, Ctrl, Shift, `SelectedItem` as Avalonia's anchor; 14px item icons; SemiBold items where the template doesn't set Normal); `list_panel` (Add, Remove, Move up/down, the count, the drop hint); `preview_pane` (toolbar header with path and size, the 4px indeterminate `ProgressBar`, `ImageComparisonSlider` with `DownOnly`, the 54px placeholder); `Job` (work on a thread for the newest request only, `_previewVersion`, with the view models' 100 ms `Task.Delay`); `Picture` (an image as a texture, shrunk with Skia's linear + mipmaps); Fluent's 50px `Slider` row with its `MinWidth="40"` value text; Avalonia's `ColorPicker`; `Grid` heights that keep the spacing of hidden rows |
| `slider.rs` | `Controls/ImageComparisonSlider.cs` | `GetImageBounds` (rounded edges, `StretchDirection`), the 12px checkerboard tiled from the control's origin, the left image clipped at the divider, the 2px line and 34x52 handle; dragging moves the divider when it moves a pixel |
| `combiner.rs` | `ImageCombinerWindow`, `ImageCombinerViewModel`, `TaskHelpers.OpenImageCombiner` | the list with moves, orientation (switching it blanks the alignment combo as Avalonia does and saves LeftOrTop), alignment, spacing, wrap after, auto fill; options saved into the task's `ToolsSettingsReference.ImageCombinerOptions` as they change; the PNG preview (two images at least); Combine images → `RunImageTask` (the after capture tasks) |
| `splitter.rs` | `ImageSplitterWindow`, `ImageSplitterViewModel` | image and folder pickers (the folder defaults to the image's), columns/rows, the grid size text, Split image → `<name><n>.png`, the folder opened with the first; `SizeToContent="Height"` |
| `thumbnailer.rs` | `ImageThumbnailerWindow`, `ImageThumbnailerViewModel` | list, size, JPEG quality, template, folder (the first file's), Generate thumbnails, the message panel |
| `resizer.rs` | `ImageResizerWindow`, `ImageResizerViewModel`, `ImageResizerService` | list (first selected), width/height/mode, PNG/JPEG with quality and background, template, folder, preview after 100 ms (`CreatePreview`), `{w} × {h}`, Resize image; also opened by the task list's Resize image with the selected images |
| `converter.rs` | `ImageConverterWindow`, `ImageConverterViewModel`, `ImageConverterService` | list, format with each format's options (WebP lossless/quality/effort, PNG depth/level/filter, BMP depth, GIF quality, JPEG quality/subsampling), background colour where needed, template, the scrolling options column, preview, Convert images |
| `watermark.rs` | `ImageWatermarkWindow`, `ImageWatermarkViewModel`, `ImageWatermarkService` | list, text (text, size, colour) or image (file, scale) watermark, position, margin, opacity, rotation, PNG/JPEG, template, folder, preview, Apply watermark |
| `comparer.rs` | `ImageComparerWindow`, `ImageComparerViewModel`, `ImageComparisonService` | two pickers (`No image selected`), Slider / Diff view, the similarity (white, green when identical, in both themes as ShareX) |
| `icon_converter.rs` | `IconConverterWindow`, `IconConverterViewModel`, `IconConverterService` | Browse or drop, sizes, bit depth, preview and dimensions, Convert... → save dialog → `.ico`, the status line |
| `open.rs` | `TaskHelpers.Open*`, `ToolsIntegration.Show*`, `UploadInfoManager.ResizeImages/CombineImages` | the Tools menu entries, the hotkeys (`ExecuteJob`), the task list's Resize image and Combine images › Horizontally/Vertically (`TaskHelpers.CombineImages` with the default task settings' options) |
| `render.rs` | | `--render image-combiner|image-splitter|image-thumbnailer|image-resizer|image-converter|image-watermark|image-comparer|icon-converter` (`--files`, `--image`, `--select`, `--state`) |

Dialog windows (`crate::dialogs`) are now held to Windows' maximum tracking
size (screen plus frame) as Avalonia's are: on a 1024x768 screen ShareX's
1180x650 windows open 1028x650, and so do ShareXrs's.

### Validation

ShareX's windows were captured with `scripts/image-tools-shots.ps1` (UI
Automation, the synthetic inputs fed through the file dialogs by
`scripts/file-dialog.ps1`), and `scripts/image-tools-compare.sh` renders and
compares every state (at the captured size). Pixels differing by more than
40, dark / light:

| Window | States (dark) | Light |
|---|---|---|
| Combiner | empty 0.18%, three images 0.24%, row selected 0.25%, horizontal 0.39% | 0.14%, 0.17%, 0.15%, 0.23% |
| Splitter | empty 0.13%, loaded 3×2 0.37% | 0.11%, 0.31% |
| Thumbnailer | empty 0.22%, three images 0.37% | 0.17%, 0.28% |
| Resizer | empty 0.14%, three images (Fit 1920×1080 preview) 0.30%, JPEG 0.37% | 0.10%, 0.18%, 0.25% |
| Converter | empty 0.19%, PNG 0.36%, JPEG 0.37%, WebP 0.31%, GIF 0.29%, BMP 0.31% | 0.12%, 0.21%, 0.24%, 0.20%, 0.17%, 0.18% |
| Watermark | empty 0.25%, text 0.37%, image 0.43% | 0.20%, 0.25%, 0.31% |
| Comparer | empty 0.14%, one image 0.16%, slider 0.13%, diff 0.12%, identical 0.10% | 0.13%, 0.15%, 0.11%, 0.18%, 0.11% |
| Icon converter | empty 0.20%, loaded 0.21% | 0.17%, 0.19% |

What's left is text anti-aliasing (ClearType) and a pixel here and there on
downscaled previews. A live ShareXrs window (the thumbnailer with three
images) is identical to its render (0 pixels over 40).

Behaviour, side by side (`scripts/image-tools-behaviour.ps1`: the same
client-coordinate clicks and dialog answers in both apps, default
settings): thumbnails byte-identical (3 JPEGs); splitter (6 PNGs), resizer
(3 × 1920×1080), converter (3 PNGs) and the combined image (through the
after capture tasks into each app's Screenshots folder) pixel-identical;
watermark identical but for the text glyphs (DirectWrite, see above); the
icon's six entries pixel-identical, the 256px PNG entry's zlib stream
differing as in the harness. Saved settings: after the same edits
(Horizontal, spacing 2, auto fill off) both apps wrote the same
`ImageCombinerOptions`.

Not ported in these windows: the preview's click (`ImageViewerWindowIntegration`,
the image viewer isn't ported), Avalonia's `ColorView` flyout (the shared
HSV popup stands in, opaque), tooltips are egui's, a drop of several files
on the splitter or icon converter takes the last one winit reports (ShareX
the first), focus rings and keyboard navigation of the lists.

## The Screen tools in ShareXrs

`crates/sharexrs/src/screen_tools/` ports the Screen category of the Tools
menu on the engines in `sharex-tools` (`color`, `ruler`, `mouse_highlighter`,
`pin`).

| File | ShareX source | What's there |
|---|---|---|
| `mod.rs` | `TaskHelpers.ShowColorPickerDialog`, `OpenScreenColorPicker`, `OpenRuler`, `OpenClassicRuler`, `OpenMouseHighlighter`, `PinToScreen*`, `ToolsIntegration` | the Tools menu entries, the hotkeys (`ExecuteJob`), the task list's and history windows' Pin to screen, the after capture Pin to screen task, `-PinToScreen <file>`; `ToolsSettingsReference` (`tools`) |
| `overlay.rs` | `ConfigureOverlayBounds` | one borderless topmost window per monitor (transparent over the live screen, or opaque), desktop-pixel pointers, ShareX's cursors |
| `screen_picker.rs` | `ShareX.Avalonia/Windows/ScreenColorPickerWindow`, `ScreenColorPickerOptions` | the transparent overlay, the 15x15 magnifier sampled with `BitBlt` (no `CAPTUREBLT`), the swatch and `InfoText`; left click picks (Ctrl: `FormatCtrl`), right click or Escape cancels; the text copied as `CodeMenuEntryPixelInfo.Parse` makes it, the action sound |
| `color_picker.rs` | `HelpersLib/Presentation/ColorPicker/ColorPickerWindow`, `AdvancedColorPickerControl` | the box and slider in six modes, name/hex/decimal/RGBA/CMYK/HSB, current and previous, the Copy menu, recent (`ApplicationConfig.RecentColors`, named and numbered colours kept apart as `Color.Equals` does) or standard colours, pick from screen (the window minimized) or the clipboard |
| `ruler.rs` | `Tools/Ruler/RulerWindow`, `Controls/RulerOverlayControl` | Measure: the transparent, not topmost overlay over the screen captured at open, colour runs, content bounds, tolerance, nudging, Ctrl+C |
| `classic_ruler.rs` | `Tools/ClassicRuler/ClassicRulerWindow`, `Controls/ClassicRulerOverlayControl` | Ruler: the frozen screen, window hover and click, drawn/moved/resized rectangles, ticks, crosshair, card, magnifier, keys, the 500 ms input delay |
| `highlighter.rs` | `Tools/MouseHighlighter/*` | the manager (manual, recording, auto activate), the `WH_MOUSE_LL` hook thread, the layered overlays drawn with `sharex-skia` and `UpdateLayeredWindow`, the window |
| `pin.rs` | `Tools/PinToScreen/*` | the pin window, the startup window, the options window |
| `render.rs` | | `--render color-picker` (`--state standard|swatch`), `pin-startup` (`--state error`), `pin-options`, `mouse-highlighter` (`--state circle`) |

### Validation

ShareX's windows were captured with `winshot.ps1`, its overlays with
`scripts/screen-tools-session.ps1` over `sharexrs --test-pattern` (the tool
opened by ShareX's Tools menu or by each app's own `-<HotkeyType>` command
line; `raise:` lifts the Measure window, which isn't topmost, above the
pattern). `scripts/screen-tools-compare.sh` renders and compares the windows.
Pixels differing by more than 40:

| Screen | Dark | Light |
|---|---|---|
| Color picker (empty recent, standard, a swatch chosen) | 1.14%, 1.05%, 0.99% | standard 1.01% |
| Pin to screen startup (empty, clipboard error) | 1.05%, 0.97% | 0.92% |
| Pin to screen options | 0.60% | |
| Mouse highlighter window (ripple, circle) | 0.63%, 0.60% | circle 0.42% |
| Screen color picker overlay at 300,300 and near the corner | 0.04%, 0.04% | |
| Measure: tolerance label, a line, vertical, mid-drag, rectangle | 0.02%, 0.01%, 0.01%, 0.04%, 0.04% | |
| Ruler: at rest (hovered window), mid-drag, selection | 0.13%, 0.13%, 0.16% | |
| Pin (at rest, toolbar, 110%, 90% opacity, from a region) | 0.00%, 0.05%, 0.05%, 0.00%, 0.00% | |
| Mouse highlighter, circle mode, primary then secondary held | 0 pixels differ | |

What's left in the windows is text: ClearType against grayscale, and a pixel
of centring where a glyph run measures a pixel narrower.

Behaviour, side by side over the same pattern:

- Screen color picker: the same pixel (300,300) copies `2C2CC8` in both
  apps (also with ShareXrs only in the tray).
- Color picker: the Copy menu's six formats for (76,255,0) equal ShareX's
  clipboard text (a unit test holds them).
- Measure: the same 16 px line, 15 px vertical line and 201 × 151 px
  rectangle; Ctrl+C copies `201 × 151 px` in both.
- Ruler: Ctrl+C after drawing 200,150 to 500,400 copies the same three
  lines (position, size/area/perimeter, distance 390.51 px, angle 39.81°).
- Pin to screen: a pin from the clipboard sits at the same place
  (bottom right, 10 px in) with the image pixel for pixel; a pin from the
  same region covers it pixel for pixel, border included (only the soft
  shadow differs, by at most 13 levels). That needed two fixes: CF_DIBV5's
  trailing masks (sharex-capture) and egui's dithering, now off in the
  overlay and pin windows.
- Mouse highlighter: the held circles are identical pixels (both draw with
  Skia's rasteriser into a layered window).

Not ported or not checked: the colour flyouts (`ColorView`) of the
highlighter's and pin options' colour buttons (logged; the pin options use
the shared HSV popup), the colour picker's selection mode (`PickColor`,
used by no ported caller) and its Copy menu as an OS popup (drawn inside
the window), the highlighter on macOS/Linux (hook and overlays), the
highlighter while recording and the ripple animation checked live, the
settings JSON after the same edits (ShareX keeps these options in memory
until its next save), tooltips are egui's, and other DPIs and monitors than
one 1024x768 at 100%. ShareX keeps no pin out of captures, so neither does
ShareXrs. ShareX's region capture has no colour picker mode in this version.

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
| Animated GIF trimmer | `AnimatedGifTrimmerWindow` 900×720, 760 | `AnimatedGifTrimmerDocument::open_file` on a thread (`AnimatedGifTrimmer_Loading`); `TrimSelection` behind the timeline (`set_start`/`set_end` snap to frame boundaries, `seek`, `step_position`), the time boxes (`set_start_time`/`set_end_time`, `format_time`), the crop overlay (`set_crop`), the output size (`set_output_width`/`height`), `can_use_lossless`, `status_text`, `frame_text`; the preview (`render_frame(index, u32::MAX)`) and 12 thumbnails (`render_frame(frame_at(duration * i / 12), 640)`); play with each frame's delay; Export → save dialog (`TrimSelection::output_name`) → `TrimSelection::export` with progress (`Ok(false)`: `AnimatedGifTrimmer_UseReencode`), then `reset_crop` and `VideoTrimmer_Saved` |
| Hash checker | `HashCheckerWindow` 700 wide, 700 | file and second-file pickers and drop, compare-two toggle, algorithm (SHA-256 default, `display_name`), progress bar (`hash_file`'s callback), Stop (the cancel flag), result in upper case, match/differ state (`hashes_match`), notification sound on completion |
| QR code | `QRCodeWindow` 940×610, 760 | content box (2952-byte limit message), size (0 fits the preview area, at least 64: `effective_size`), preview (`generate_png`), Copy image (clipboard), Save (PNG/JPEG/BMP via `imaging::save_image`, SVG via `generate_svg_file`), Upload, Scan screen/region (capture crate) and image file (`scan`), results joined (`join_results`) |
| Index folder | `DirectoryIndexerWindow` 980×680, 820 | folder picker, output (HTML/Text/XML/JSON), each output's settings, source preview (text outputs) or a notice (HTML), Save as (`Index_for` name and extension) and Upload (`TaskHelpers.OpenDirectoryIndexer`: a text upload named `.html` etc.); `IndexerSettings` from `TaskSettingsTools.IndexerSettings` with `BinaryUnits` from the application settings |
| Metadata | `MetadataWindow` 900×650, 700 | open/drop, grouped list (`group`), search (`filter`), Copy all (`build_text`), Strip (confirmation, `can_strip`, `strip`), URLs as links (`https?://[^\s]+`), title with the file name |
| Video converter | `VideoConverterWindow` 850×550, 700 | input/output pickers (output name `<input>-output`), codec list (`display_name`), CRF/bitrate controls per codec (`can_choose_rate_control`, `quality_range`), progress from `FFmpegCliManager`'s encode progress, Stop, the output info line (`output_info` over `probe_arguments`), auto open folder; settings `TaskSettingsTools.VideoConverterOptions`, FFmpeg path from the capture settings (`CheckFFmpeg`) |
| Video thumbnailer | `VideoThumbnailerWindow` 900×650, 760 | video picker, the `VideoThumbnailOptions` form, progress (`take_thumbnails`' callback), result list; uploads each output when `UploadThumbnails` (TaskHelpers) |
| Video trimmer | `VideoTrimmerWindow` 900×720, 760 | Media Foundation preview and timeline (`VideoTrimmerTimeline`); on other platforms an FFmpeg-decoded preview; start/end and Set start/end, Copy vs Re-encode (`build_trim_arguments`), save dialog with the forced extension (`validate`), progress (`trim`) |
| Analyze image | `AnalyzeImageWindow` 1000×650, 820×560; `AnalyzeImageOptionsWindow` 650×590 | image from a file, the region capture (`AutoStartRegion`, PNG bytes) or a task's file; prompt box with `preset_prompts`, Analyze (`CanAnalyze`: an image, `has_api_key`, not busy; "Configure API key" otherwise) running `analyze` off the UI thread with the elapsed time (`AnalyzeImageViewModel_Time_ms`), the result (`result_text`), Copy (and `AutoCopyResult`), the notification sound; options window: provider, keys (`[JsonEncrypt]`), models with Load models (`load_models`), custom URL, reasoning effort and verbosity lists, Test connection (`test_connection`), the API key page link (`api_key_page`); settings `TaskSettingsTools.AIOptions` (`AIOptions::from_json/to_json`) |
| Background remover | `BackgroundRemoverWindow` | the models folder (`AppPaths.ModelsFolder`): `scan_models`, `select_model`, `Model::display_name`; the device combo (`Device::ALL`); `BackgroundRemoverOptions` saved as they change; an image from a file or a drop (`load_image`); Remove background → `BackgroundRemovalService::remove_background` on a thread; the comparison slider (the image tools' `ImageComparisonSlider`); Save/Save as; the guide link and notifications |
| Clipboard viewer | `ClipboardViewerWindow` | the clipboard read into a `ClipboardSnapshot` (Avalonia's names: `UnicodeText`, `FileDrop`, `Bitmap` as PNG bytes, `DeviceIndependentBitmap`, `Format17`, raw bytes otherwise); the formats list (`format_count_text`, `reselect` on Refresh); the preview (`preview`, `preview_details`, `preview_failed_text`); Clear clipboard |
| Inspect window | `InspectWindowWindow`, `InspectWindowPickerOverlay` | the window list (`visible_windows` over enumerated `WindowEntry`s), the picker (the window under the cursor, top level or control), `details` of a `WindowSnapshot`, Copy (`clipboard_text`), top-most and opacity (`percent_to_opacity`) for top-level windows |
| Network monitor | `NetworkMonitorWindow` | `targets`, `INTERVALS`, `time_ranges`; the probe loop off the UI thread (`probe` with an ICMP `Pinger`; `needs_confirmation` → wait `DISCONNECT_CONFIRMATION_DELAY`, probe again; `record`; `update_connection` → `append_log` to `Logs/NetworkMonitor.log`); the chart over `visible_samples`; the summary texts; events (`timestamp_text`, `status_text`); Copy all, Clear log, Open log |
| Borderless window | `BorderlessWindowWindow` | `BorderlessWindowSettings` (`initial_title`), the window list (as the Inspect window's), Toggle → `toggle` with a native `WindowOps` (`SearchWindow`, styles, `SetWindowPos`) and one `BorderlessWindowManager` for the session (also the Make active window borderless hotkey: `BorderlessWindowManager::toggle` on the foreground window) |
| Monitor test | `MonitorTestWindow`: borderless, topmost, not in the taskbar, sized to the screen under the cursor; a 720-wide controls panel | mode buttons, per-mode controls (gray, RGB with hex text and a 320×320 colour view, gradient colours and direction, pattern and size, motion direction and speed 100–2000 px/s, wheel ±50), the pattern drawn by `MonitorTest::render` each frame with `advance_motion`; left click or Space toggles the controls, right click or Esc closes, Left/Right change mode |
