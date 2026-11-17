# OCR (ShareX.OCR)

ShareX 22.4's text recognition, ported as `crates/sharex-ocr`. The same image gives
the same text, the same boxes and the same scores, bit for bit, as ShareX's
(validated below). Source: `ShareX.OCR/` (~1.3k lines, 7 files), plus its callers in
`ShareX/` and `ShareX.Tools/Tools/OCR/`.

## The engine

- **Models**: PaddleOCR's **PP-OCRv6** in ONNX form, a DB text detector and a CTC
  text recognizer. No angle classifier and no language choice: the recognition model
  is multilingual (`PaddleOCREngine`: "there is no language to choose").
- **Runtime**: ONNX Runtime **1.24.4** through the `Microsoft.ML.OnnxRuntime.DirectML`
  package. ShareX **never adds the DirectML execution provider**: its
  `SessionOptions` only set `GraphOptimizationLevel = ORT_ENABLE_ALL`, so every
  session runs on the **CPU provider** with ONNX Runtime's defaults (intra-op threads
  = physical cores, sequential execution, memory arena and patterns on). The
  DirectML package is just where its `onnxruntime.dll` comes from. There's no
  separate DirectML path to compare against.
- **Preprocessing** is Skia (SkiaSharp 3.119 = Skia m119) drawing: Catmull-Rom to
  scale up, **linear filtering between linear mipmaps** to scale down.
- One engine per process (`PaddleOCREngine.Shared`, created under a lock on first
  use; a failed load is retried on the next call). `OCRHelper.Recognize` runs it on
  the thread pool.

### Models: files, origin, lookup

| | |
|---|---|
| Files | `Models/PP-OCRv6_small_det/{inference.onnx (9 880 512 B), inference.yml}` and `Models/PP-OCRv6_small_rec/{inference.onnx (21 159 378 B), inference.yml}` |
| Origin | Committed to ShareX's repository (commit `62a7cc30`, "Add the PP-OCRv6 small detection and recognition models") and copied beside the app at build time (`ShareX.OCR.csproj`: `None Include="Models\**"` → `$(OutDir)/Models`). PaddleOCR is Apache-2.0 (`Licenses/PaddleOCR_license.txt`) |
| Where | `PaddleOCREngine.DefaultModelsFolder` = `AppContext.BaseDirectory/Models`, **beside the executable**, not in the profile |
| Choice | Detection and recognition are picked separately, first found of `PP-OCRv6_medium_*`, `PP-OCRv6_small_*`, `PP-OCRv6_tiny_*`, each also as `<name>_onnx` (Hugging Face's download folder name). Found = both `inference.onnx` and `inference.yml` exist. None found: the built-in `small` folder, so loading reports *it* missing (`FileNotFoundException`) |
| Download | **None.** ShareX has no download URL, no progress UI and no hash check anywhere in the repo. A user who wants the medium models copies them into `Models/` by hand |
| Verification | **None** beyond the two files existing; a broken file fails in `new InferenceSession` |

`inference.yml` is read by `ModelConfig`, a line scanner rather than a YAML parser:
the first `key: value` at any depth (`thresh`, `box_thresh`, `unclip_ratio`,
`max_candidates`) and the `- item` list under the first `key:` line (`mean`, `std`,
`image_shape`, `character_dict`), with single quotes (`''` = `'`) and double quotes
(`Regex.Unescape`). Small models' values: detection `thresh 0.2`, `box_thresh
0.45`, `unclip_ratio 1.4`, `max_candidates 3000`, ImageNet mean/std; recognition
`image_shape [3, 48, 320]` and an 18 708-entry dictionary.

## The pipeline

`PaddleOCREngine.Recognize(bitmap, minScore = 0.5)`:

1. **Opaque copy** (`ImageTensor.ToOpaqueBgra`): drawn on white in a BGRA8888
   bitmap, so transparency reads as background.
2. **Detection** (`TextDetector.Detect`):
   - Input size (`GetInputSize`): scale up so the short side is ≥ 64, then down so the
     long side is ≤ 4000 (in `float`), each side rounded to a multiple of 32 with
     `Math.Round` (the float as a double, **halves to even**), at least 32. The aspect
     ratio is not kept.
   - `ImageTensor.Resize`: `DrawImage` into that size: Catmull-Rom when neither side
     shrinks, else `SKSamplingOptions(Linear, MipmapMode.Linear)`.
   - `ImageTensor.Fill`: NCHW, **B, G, R** planes, `byte * (1 / (255 * std)) - mean / std`.
   - Model → a probability map (`[1, 1, H, W]`).
   - `FindBoxes`, in pixel order: an unvisited pixel `> thresh` starts a region (until
     `max_candidates`); 8-connected flood fill (stack order) collecting the pixels
     with a 4-neighbour outside the text; monotone-chain convex hull; minimum-area
     rectangle by rotating calipers over the hull edges (corners ordered as PaddleOCR:
     the two leftmost are the left side, the higher of each pair the top); dropped if
     its short side `< 3` or its **box score** (mean probability of the pixels whose
     centres are inside, counting those within half a pixel of an edge) `<
     box_thresh`; **unclipped** by `area * unclip_ratio / perimeter` on every side
     (the offset-and-re-fit of PaddleOCR collapses to this for a rectangle); dropped if
     the grown short side `< 5`.
   - Each box scaled by `image / map` per axis, clamped to the image, and dropped
     unless both its top and left edges are `> 3` px.
3. **Classification**: none. Instead, a box at least **1.5×** taller than wide is
   vertical text and is turned a quarter **counter-clockwise** (top-right corner as
   origin). Upside-down text and text reading bottom-to-top aren't handled
   (`rotated_90` below).
4. **Recognition** (`TextRecognizer.Recognize`):
   - Each box's line image: width = the longer of its top/bottom edges, height = the
     longer of its sides (both ≥ 1); aspect = width / height.
   - Lines **sorted by aspect** (stable), in batches of **6**. A batch's width =
     `(int)(48 × max(320/48, widest aspect))`, at most **3200**. Each line is
     `ceil(48 × aspect)` wide (1..batch width), drawn upright by `ImageTensor.Crop`
     (an affine image shader from the box's origin and edges, clamped edges, the same
     Catmull-Rom/mipmap choice: shrink when an edge is > 1.01× the target), filled with
     mean = std = 0.5, the rest of the batch row left 0 (PaddleOCR's padding).
   - Greedy **CTC**: per step the first class with the highest probability; blanks
     (0) and repeats dropped; class `i` is `["", dictionary..., " "][i]`; score = mean
     probability of the kept characters (float sum).
5. **Post-processing**: text `Trim()`ed; kept if non-empty and `score >= minScore`
   (0.5, "usually an icon or noise read as characters").
6. **Lines and paragraphs** (`OCRResult`):
   - Regions sorted by their bounds' `MidY` (stable); a region joins the previous line
     when it overlaps the line's vertical extent by ≥ 50% of the shorter height. A
     line is its regions left to right joined with a space; its height is the median
     region height.
   - Paragraphs: the line gap (gap / smaller height) of neighbouring lines that could
     share a paragraph (heights within 1.6×, overlapping horizontally) taken at the
     **25th percentile** (`gaps[(n-1)/4]`, 0.35 with fewer than 2). A line starts a
     paragraph when its gap is > `max(2 × gap, gap + 0.35)`, the heights differ ≥ 1.6×,
     or it doesn't overlap the line above horizontally.
   - `GetText(singleLine, separateParagraphs)`: lines joined with `Environment.NewLine`
     (CRLF on Windows) and an extra one before paragraphs, or everything joined with
     spaces.

**Languages**: whatever the dictionary has: 15 908 CJK ideographs (Simplified
and Traditional Chinese, kanji), kana, 550 Latin letters (accents included), 62 Greek,
symbols. No Hangul, Cyrillic, Arabic, Hebrew, Thai or Devanagari: Korean comes out
empty, Russian as its punctuation.

## Options and callers

`OCROptions` (`TaskSettings.CaptureSettings.OCROptions`; JSON in ShareXrs's
`sharex-settings` until the window is ported): `SingleLine`, `SeparateParagraphs`
(true), `Silent`, `AutoCopy`, `ServiceLinks` (Google Translate/Search/Images, Bing,
DuckDuckGo, DeepL, with `{0}` for the text), `CloseWindowAfterOpeningServiceLink`,
`SelectedServiceLink`, `WindowState`, `ResultPanelWidth`. The task settings' OCR
page edits Silent, AutoCopy and CloseWindowAfterOpeningServiceLink (ported).
`ApplicationConfig.HistorySaveOCRText` (Application settings › History) is ported as
a check box.

| Caller | ShareX | What it does |
|---|---|---|
| Tools › OCR (main/tray menu, hotkey `OCR`) | `TaskHelpers.OCRImage(taskSettings)` | region capture (main window hidden 250 ms), then `OCRImage(bmp)` |
| Hotkey `OCR` on a file / task list › OCR image | `OCRImage(filePath)` / `UploadInfoManager.OCRImage` | the file through `SkiaImageHelpers.LoadImage`, `OCRImage(bmp, filePath)` |
| After capture › Recognize text (OCR) | `WorkerTask.DoOCR` → `OCRImage(Image, TaskSettings)` | synchronous, after the file jobs; no file path |
| `OCRImage(bmp, filePath)` | | **Silent**: `AsyncOCRImage`: the text (SingleLine, SeparateParagraphs) to the clipboard, or the clipboard **cleared** when empty; `<file>.txt` (UTF-8 with BOM) when there's a file path; the action-completed sound. **Not silent**: the OCR window (PNG round trip of the image); its result also goes to `<file>.txt` |
| History | `WorkerTask.StartHistoryOCR` → `RecognizeTextForHistoryAsync(Image.Copy())`; `TaskManager.AppendHistoryItemAsync` | with `HistorySaveTasks && HistorySaveOCRText`, for image tasks: single-line text, started after the after capture jobs, awaited before the item is appended; saved as the `OCR` tag unless blank; a failure is logged and the item added without it |

## ShareXrs

`crates/sharex-ocr` maps ShareX.OCR file for file:

| ShareX | ShareXrs |
|---|---|
| `PaddleOCREngine.cs` | `engine.rs`: `PaddleOcrEngine` (`new`, `shared`, `recognize`, `recognize_traced` for the intermediates), `find_model_folder`, `default_models_folder` |
| `TextDetector.cs` | `detector.rs`: `input_size`, `boxes_from_map` (`FindBoxes` + scaling), `DbParams` |
| `TextRecognizer.cs` | `recognizer.rs`: `line_crops` (sorting, batching, crops), CTC decoding |
| `ImageTensor.cs` | `image_tensor.rs` on `sharex-skia` (`Canvas`, image shaders) |
| `Geometry.cs` | `geometry.rs` (with .NET's `Math.Min/Max` semantics for floats) |
| `ModelConfig.cs` | `config.rs` |
| `OCRResult.cs` | `result.rs` |
| `OCRHelper.cs` (ShareX.Tools) | `sharex_ocr::recognize` / `ocr` |
| ONNX Runtime | `runtime.rs` |

`sharex-skia` gained what the OCR draws with: `Sampling::LinearMipmapLinear`
(`raster/mipmap.rs`: `SkMipmap::Build`'s 8888 box/triangle filters for odd and even
sizes, `ComputeLevel` with its -0.5 bias, `SkMipmapAccessor`'s level pair and weight,
`decomposeScale`; in the canvas: the upper level through `Scale(level/image) × inv`,
the generic `bilinear_*`/`accumulate` stages instead of `bilerp_clamp_8888`, the
lower level at `x × (lower/upper)`, `mipmap_linear_finish`'s `lerp`, never the legacy
blitter).

**Runtime**: the `ort` crate (2.0.0-rc.15) with `load-dynamic`: nothing is downloaded
or linked at build time; the library is loaded on first use from `ORT_DYLIB_PATH`, else
beside the executable (`onnxruntime.dll`, `libonnxruntime.dylib`,
`libonnxruntime.so[.1]`; on macOS also `../Frameworks`). Built **without** `ort`'s
`api-22`+ features on purpose: with them, `ort` sets an automatic EP selection policy
(`MaxEfficiency`) on every session, which ShareX doesn't and which could pick DirectML
or an NPU. Sessions get `GraphOptimizationLevel::All` (`ORT_ENABLE_ALL`; `ort`'s
`Level3` is `ORT_ENABLE_LAYOUT`) and nothing else, like ShareX's.

**Models**: looked for as ShareX does, in `Models/` beside the executable (macOS also
`../Resources/Models`); `SHAREXRS_OCR_MODELS` overrides the folder for development
builds. No download, as ShareX.

**Packaging (not done yet)**: a release needs ShareX's two model folders (31 MB,
Apache-2.0) and ONNX Runtime 1.24.x beside the executable. Neither is in this repo.
Copy them from a ShareX build (`ShareX-build/Models`, `ShareX-build/onnxruntime.dll`)
for a local run. Without them OCR returns an error that the callers log (as ShareX
logs its exceptions).

### In the app

- **After capture › Recognize text (OCR)** (`upload/worker.rs` `do_ocr` →
  `ocr.rs` `ocr_image`): the silent path is ported (copy or clear the clipboard,
  action-completed sound). With `Silent` off ShareX opens the OCR window; ShareXrs
  logs that the window isn't ported yet.
- **HistorySaveOCRText** (`start_history_ocr` → `recognize_text_for_history` on a
  thread; `history::task_completed` waits for it and sets the `OCR` tag): ported.
- The capture task now hands back its final image (`TaskOutcome.image`, ShareX's
  `WorkerTask.Image`) for both.

## Validation

`tools/ocr-harness` (README there) generates 38 synthetic images with SkiaSharp and
runs **ShareX's own `ShareX.OCR.dll`** on them (the reference build's ONNX Runtime and
models, through `PaddleOCREngine`'s public API, plus its internals by reflection for
the intermediates). `cargo run --release -p sharex-ocr --example ocr_validate` runs
ShareXrs on the same PNGs with **the same `onnxruntime.dll`** (`ORT_DYLIB_PATH`) and
compares, bit for bit: the detection input image, the probability map, the boxes (as
float bits), every line crop, the raw texts and scores, the regions, the lines
(bounds, heights, paragraph flags) and `GetText` three ways.

**Result: 38/38 cases identical at every stage** (Windows 11, x64, the reference
build's ONNX Runtime 1.24.4 CPU provider). Sensitivity check: with the downscale
sampling switched to plain bilinear, 4 cases diverge at the detection input,
probabilities, boxes and scores, and one changes its text.

| case | what | size → detection input | boxes | ShareX's text (⏎ = new line) | ShareXrs |
|---|---|---|---|---|---|
| en_segoe_16 | Segoe UI 16, black on white | 420×40 → 672×64 | 1 | `The quick brown fox jumps over the lazy dog.` | ✅ |
| en_arial_11 | Arial 11 (small text) | 300×30 → 640×64 | 1 | `The quick brown fox jumps over the lazy dog.` | ✅ |
| en_times_28 | Times New Roman 28, two lines | 640×110 → 640×96 | 2 | `It was the best of times, ⏎ it was the worst of times.` | ✅ |
| en_consolas_code | Consolas 14, code lines | 520×120 → 512×128 | 4 | `fn main() { ⏎ let total = 42 * 7; ⏎ println!("total = {}", total); …` | ✅ |
| en_georgia_bold_italic | Georgia 22 bold italic | 560×50 → 704×64 | 1 | `Bold and italic headlines read well` | ✅ |
| white_on_dark | Segoe UI 18, white on #1E1E1E | 460×44 → 672×64 | 1 | `Dark mode text is common in apps` | ✅ |
| red_on_yellow | Verdana 20, red on yellow | 420×46 → 576×64 | 1 | `Warning: wet floor` | ✅ |
| low_contrast | Segoe UI 16, #8A8A8A on #B0B0B0 | 400×40 → 640×64 | 1 | `Low contrast placeholder text` | ✅ |
| gradient_bg | Segoe UI 24, white on a gradient | 520×70 → 512×64 | 1 | `Gradients behind white text` | ✅ |
| large_72 | Segoe UI 72 bold (crops shrink: two mipmap levels) | 900×120 → 896×128 | 1 | `Big Headline` | ✅ |
| huge_image | 4600 px wide (shrinks, above the mipmap threshold) | 4600×400 → 4000×352 | 1 | `A very wide banner with large lettering that keeps going across the…` | ✅ |
| very_wide_odd | detection input at 0.44: levels blended, odd sizes | 9001×501 → 4000×224 | 1 | `An enormous odd sized poster whose detection input is shrunk by mor…` | ✅ |
| very_wide_short | the same shrink, 41 px tall | 9001×41 → 4000×32 | 1 | (empty: the line is squeezed into 3200 px) | ✅ |
| giant_text | Arial 200 (crops shrink past the first level) | 1500×280 → 1504×288 | 1 | `Giant words` | ✅ |
| long_line | one 3800 px line (recognition width capped at 3200) | 3800×40 → 4000×32 | 1 | `a lon in  ma t   ng n io  mal et …` (squeezed) | ✅ |
| tiny_image | Segoe UI 12 (detection input grows) | 44×22 → 128×64 | 1 | `OK` | ✅ |
| paragraphs | heading, two paragraphs | 560×300 → 576×288 | 6 | `Release notes ⏎  ⏎ This version adds text recognition that runs ⏎ e…` | ✅ |
| columns | two columns | 700×200 → 704×192 | 6 | `Left column starts here Right column has its ⏎ and continues on thi…` | ✅ |
| rotated_5 | rotated 5° | 560×260 → 576×256 | 1 | `Rotated text is still readable` | ✅ |
| rotated_15 | rotated 15° | 560×260 → 576×256 | 1 | `Rotated text is still readable` | ✅ |
| rotated_30 | rotated 30° | 560×260 → 576×256 | 1 | `Rotated text is still readable` | ✅ |
| rotated_90 | rotated 90° (reads bottom to top) | 80×420 → 64×416 | 1 | `Sidetl l sidet` (turned the wrong way, as ShareX) | ✅ |
| skewed | skewed (faux italic) | 560×80 → 576×64 | 1 | `Skewed letters lean forward` | ✅ |
| vertical_cjk | Chinese top to bottom | 70×360 → 64×352 | 1 | `竖排中文文字测试` | ✅ |
| lang_fr | French | 620×44 → 896×64 | 1 | `Ça a été très agréable, à bientôt au château !` | ✅ |
| lang_de | German | 560×44 → 800×64 | 1 | `Größe, Übung, Straße und schöne Mädchen.` | ✅ |
| lang_es | Spanish | 560×44 → 800×64 | 1 | `¿Dónde está la niña? Mañana será otro día!` | ✅ |
| lang_zh_hans | Simplified Chinese | 520×50 → 672×64 | 1 | `今天天气很好，我们去公园散步吧。` | ✅ |
| lang_zh_hant | Traditional Chinese | 440×50 → 576×64 | 1 | `繁體中文測試，臺灣與香港。` | ✅ |
| lang_ja | Japanese | 620×50 → 800×64 | 1 | `日本語の文字認識テストです。カタカナとひらがな。` | ✅ |
| lang_el | Greek | 560×44 → 800×64 | 1 | `αλα κσε, αυτ εναι α ι.` (the model's reading) | ✅ |
| lang_ko | Korean (no Hangul in the dictionary) | 360×50 → 448×64 | 1 | (empty) | ✅ |
| lang_ru | Russian (no Cyrillic in the dictionary) | 380×44 → 544×64 | 1 | `, !  ?` | ✅ |
| numbers_symbols | numbers, currency, dates | 720×44 → 1056×64 | 1 | `Invoice #2026-0042 Total: $1,234.56 Due 17/11/2026(ID: A7-x9)` | ✅ |
| ui_screenshot | synthetic settings window | 820×520 → 832×512 | 33 | `Settings - Example App □ X ⏎  ⏎ General Capture ⏎ Capture Choose wh…` | ✅ |
| transparent | text on a transparent background | 420×44 → 608×64 | 1 | `Transparent background text` | ✅ |
| shapes_and_text | icons and shapes beside text | 520×160 → 512×160 | 1 | `Toolbar with icons` | ✅ |
| empty | a blank image | 200×100 → 192×96 | 0 | (empty) | ✅ |

**CI** (`crates/sharex-ocr/tests/sharex.rs`, 263 KB of fixtures from
`make_fixtures.py`) checks what runs without ONNX Runtime and the models, on all
three platforms: detection input sizes (38 cases), detection input pixels and line
crops (8 cases, 14 images: Catmull-Rom, both mipmap paths, rotated, vertical,
skewed), boxes from ShareX's probability maps (2), and lines, paragraphs and the
three texts from ShareX's regions (38). With `ORT_DYLIB_PATH` and
`SHAREXRS_OCR_MODELS` set, `engine_matches_sharex` runs the whole engine too (Windows
x64 only).

## Platforms

- **Windows x64**: validated as above with ShareX's own `onnxruntime.dll` (the
  DirectML package's build, CPU provider). The plain CPU package of the same version
  (what a ShareXrs release would ship) is expected to run the same CPU kernels but
  wasn't compared: there's no copy here, and only the model files could be fetched.
- **macOS / Linux**: the crate builds there (clippy for aarch64-apple-darwin is
  clean; Linux through CI) and the CI tests run there; the engine itself is untested.
  ONNX Runtime picks CPU kernels by instruction set (AVX2/AVX-512 on x64, NEON on
  Apple silicon), so probabilities can differ in the last bits on another CPU; expect
  the same text, with scores that may differ in the last bits. ShareX itself has no
  OCR there: `Microsoft.ML.OnnxRuntime.DirectML` 1.24.4 ships native libraries for
  `win-x64` and `win-arm64` only, so `new InferenceSession` fails on macOS and Linux
  (an exception the callers show or log; not tried, by the package contents). ShareXrs works wherever an ONNX Runtime library is
  supplied.
- Determinism: same ORT version, CPU provider, default thread pool, same CPU →
  identical outputs run to run (the validation ran several times, identical each time).

## Not ported yet

### The OCR window (needs the desktop)

`ShareX.Tools/Tools/OCR/OCRWindow.axaml` (1100×680, min 820×520), `OCRViewModel`,
`Controls/OCRPreview`. Plan:

- Left: `OCRPreview`: the source image fitted with padding and a border, each
  region's box drawn as a rotated polygon in the highlight brush, hover highlight,
  click copies that region's text with a "Copied to clipboard" badge; a scan-line
  animation while busy. Buttons under it: Select region (hides the window, region
  capture, `ReplaceSourceAsync`), Select image (file dialog).
- Right (resizable, `ResultPanelWidth`): "Result" header, the editable text box (or
  "No text found" / the error), check boxes Single line, Separate paragraphs (only
  when not single line), Copy recognized text automatically; the service combo with
  Open service (`{0}` replaced by the URL-encoded text, `CloseWindowAfterOpeningServiceLink`)
  and Edit services (an inline editor: New, Remove, Reset, Name, URL, "Close after
  opening", Close); Copy all.
- Behaviour: `RecognizeAsync` with a version counter (late results dropped);
  `SingleLine`/`SeparateParagraphs` reformat the last result without re-running; each
  option change saves `OCROptions`; the result text goes to `<file>.txt` when the
  window was opened for a file; the window state is remembered.
- In ShareXrs: an egui window like the other tools', `sharex-ocr` on a worker thread,
  the preview's polygons from `OcrTextRegion::points`. Validate with
  `winshot.ps1` + `--render ocr` against ShareX as every window.

### Entry points (need the desktop or the window)

- **Tools › OCR** (main and tray menus, `HotkeyType.OCR`): region capture, then the
  window or the silent path; the menu entry exists (inert).
- **Task list › OCR image** (`task_view.rs` already shows the item, enabled for image
  files): `OCRImage(filePath)`; silent: text to the clipboard and `<file>.txt`.
- Both are small once the window exists. The silent path could be wired now through
  `crate::ocr`, but the menu item's default (not silent) needs the window.

### Other gaps

- Model and ONNX Runtime packaging for releases (above).
- `.txt` beside the file for the file-based callers (only reached from the entry
  points above).
- macOS/Linux engine runs (untested), the plain CPU ONNX Runtime build (not compared).
