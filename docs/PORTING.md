# Porting ShareX to Rust

ShareXrs is a port of [ShareX](https://github.com/ShareX/ShareX) 22.4 (the
Avalonia rewrite; commit in `REFERENCE_COMMIT`) to Rust and egui, with the
same UI and behaviour, running on Windows, macOS and Linux. This file is the
plan and the tracker; [MACOS-LEARNINGS.md](MACOS-LEARNINGS.md) is what snapr
taught us about shipping on macOS.

## Ground rules

1. **ShareX is the spec.** Every screen ports a specific `.axaml` (layout)
   and `.axaml.cs`/view-model (behaviour). Sizes, colours, spacing and text
   come from ShareX's files, never by eye: theme keys are ported in
   `sharex-ui::theme` (named after their `ShareX.Color.*` /
   `ShareX.Density.*` keys), strings come from ShareX's own `.resx` through
   `sharex-strings` (same key names), icons are ShareX's SVGs through
   `sharex-ui::icons` (same keys, `sharex_ui::keys::app_icons::Camera`).
   Leave a comment pointing at the ShareX file a port came from.
2. **Integrate tool by tool.** One tool (or window) at a time, end to end:
   UI, behaviour, settings, wired into the main window's menus. A tool is done
   when it passes the validation below, not when it compiles.
3. **Validate against the real ShareX, often.** For every screen:
   - capture ShareX: `scripts/winshot.ps1 -Process ShareX -Title <regex> -Out <png>`
     (client area via `PrintWindow`, so other windows never end up in a shot;
     drive ShareX with `scripts/uia.ps1`),
   - render ShareXrs: `sharexrs --render <screen> --out <png>` (offscreen, at
     ShareX's size) and capture the live window the same way,
   - `python scripts/compare.py <sharex.png> <sharexrs.png> <out dir>` and
     inspect `diff.png`; then a visual review by Sol 6.1 (GPT-6.1 Sol, via a
     delegated review task) for anything numbers miss,
   - exercise the behaviour in both apps side by side (same input, same output
     file, same clipboard contents, same settings JSON).
4. **Match Avalonia's layout maths.** Avalonia rounds layout to whole pixels:
   text line boxes are the font's line height rounded *up*, centring offsets
   are rounded half-to-even, `Border` padding sits inside its border. Use
   `widgets::line_height`, `widgets::center_y` and `widgets::text_width`
   instead of egui galley sizes (which round down).
   Check fonts by measuring text widths, not by eye: a control's plain string
   content (a `MenuItem` header, a `Button`'s text) is drawn by its
   `ContentPresenter` in Avalonia's default family, which ShareX's
   `.WithInterFont()` makes Inter 3.019 (`fonts::inter`), not Segoe UI; and
   ShareX's global SemiBold `TextBlock` style doesn't reach `TextBlock`s in
   popups (menus, the tool grid), which are Segoe UI Regular.
5. **Settings stay compatible.** ShareXrs reads and writes ShareX's JSON
   (`ApplicationConfig.json`, `HotkeysConfig.json`, `UploadersConfig.json`)
   with the same property names, so a user can move between the two.

Known, accepted differences:
- Text anti-aliasing: Avalonia uses ClearType (subpixel) on Windows, egui
  grayscale. Shapes and positions match; edges of glyphs don't.
- The title bar says ShareXrs and shows ShareXrs's icon.
- On macOS the window keeps the native traffic lights (ShareX doesn't ship
  for macOS; this is what its Avalonia build would show).
- Fonts: Segoe UI and Cascadia Mono come from DirectWrite on Windows, as
  Avalonia finds them; elsewhere the Inter that ShareX bundles as its
  fallback.

## Architecture

```
crates/
  sharex-strings/   ShareX's .resx (29 languages) -> Rust, generated at build
  sharex-ui/        theme, density, fonts, icon packs, shared widgets,
                    wgpu device, offscreen renderer
  sharexrs/         the app: winit event loop, one egui context per OS window
                    (window.rs), the main window, `--render`
```

The app follows snapr's architecture (raw winit 0.30 + egui-winit +
egui-wgpu, one shared wgpu device, windows created and destroyed on demand),
which already copes with tray-only operation, per-monitor capture overlays
and macOS's quirks. New ShareX projects become crates as they're ported
(`sharex-capture` for ShareX.ScreenCaptureLib, `sharex-uploaders`,
`sharex-history`, `sharex-image-effects`, ...).

## Tracker

Status: ⬜ not started · 🟨 in progress · ✅ matches ShareX (validated)

### Phase 0: foundation
| Item | ShareX source | Status |
|---|---|---|
| Theme colours, density, fonts, icons, strings | ShareX.Avalonia/Theming, ShareX.Icons, */Localization | ✅ |
| Main window (empty state, sidebar, title bar) | Presentation/MainWindow | 🟨 render <1% pixel diff in both themes; sidebar menus done (below); task list, thumbnails to do |
| Window chrome: borderless + resize + DWM rounded corners/shadow | MainWindow.axaml | 🟨 |
| Settings files (read/write ShareX JSON) | ShareX/ApplicationConfig.cs, TaskSettings.cs | ⬜ |
| Tray icon and tray menu | TrayIconService, MainMenuBuilder.BuildTrayMenu | ⬜ |
| Hotkeys (registration, conflicts, Hotkey settings window) | HotkeyManager, Presentation/HotkeySettings | ⬜ |
| Packaging: Windows exe icon, macOS .app/.dmg signed, Linux .deb; CI | (snapr) | 🟨 |
| Context menus / flyouts (`MainMenuEntry`, compact menu style) | MainMenuBuilder, ControlStyles | ✅ sidebar menus: all 8 (`sharex_ui::menu`, `main_menu.rs`), the tool grid, submenus, hover, dark + light, validated against the source build with `scripts/compare-menus.sh` (every state ≤ the main window's own ~0.9% text anti-aliasing baseline); click, hover-to-open (400ms), Escape, arrows/Enter and light dismiss checked live. Not yet: menus as their own OS windows (drawn inside the window, so they can't extend past it), tool grid edit mode, Capture › Window's window list, tray and task context menus |

### Phase 1: capture
| Item | ShareX source | Status |
|---|---|---|
| Fullscreen / monitor / window capture | ScreenCaptureLib/Screenshot.cs | ⬜ |
| Region capture overlay (incl. transparent, last region) | ScreenCaptureLib/Presentation | ⬜ |
| After capture tasks: save (name pattern), clipboard, annotate | ShareX/Workflow, NameParser (HelpersLib) | ⬜ |
| Task list + thumbnail view in main window | MainWindow, ThumbnailItemViewModel | ⬜ |
| Screen recording (FFmpeg, GIF) + recorder bar | ScreenRecordingLib, Presentation/ScreenRecording | ⬜ |
| Scrolling capture, auto capture | ScreenCaptureLib, Presentation/AutoCapture | ⬜ |

### Phase 2: upload
| Item | ShareX source | Status |
|---|---|---|
| Uploader framework + custom uploaders (.sxcu) | UploadersLib, Presentation/CustomUploaderSettings | ⬜ |
| Built-in destinations (Imgur, S3, FTP/SFTP, ...) | UploadersLib | ⬜ |
| Upload file/folder/clipboard/text/URL, drag & drop, shorten URL | UploadManager, Presentation/* | ⬜ |
| After upload tasks, history, image history, stats | HistoryLib, Presentation | ⬜ |

### Phase 3: tools (`MainMenuBuilder.BuildToolCategories`)
| Category | Tools | Status |
|---|---|---|
| Screen | Color picker, Screen color picker, Ruler, Classic ruler, Mouse highlighter, Pin to screen | ⬜ |
| Images | Image editor, Photo editor, Pixel editor, Image effects, Beautifier, Combiner, Comparer, Converter, Resizer, Splitter, Thumbnailer, Viewer, Watermark, Icon converter, Background remover | ⬜ |
| Video | Animated GIF maker, GIF trimmer, Video converter, Video player, Video thumbnailer, Video trimmer | ⬜ |
| Data | Analyze image, Hash checker, Index folder, Metadata, OCR, QR code, Remote storage browser | ⬜ |
| System | Borderless window, Clipboard viewer, Inspect window, Monitor test, Network monitor | ⬜ |

### Phase 4: settings windows and the rest
Application settings, Task settings, Destination settings, Workflows,
Actions toolbar, Quick task menu, Start screen (welcome), About, Debug log,
notifications, updates.

## Working on a tool (for agents)

1. Read the ShareX source for the tool: its `.axaml` and code-behind or
   view-model, the strings it uses, the settings it reads.
2. Capture every state of ShareX's window you'll port (open it in the
   running ShareX built from the reference commit, `G:/scripts/sharexrs-ref/ShareX-build/ShareX.exe` (portable). Not the 22.4.0 release: master is newer and is the spec
   with `scripts/uia.ps1`), into `G:/scripts/sharexrs-ref/shots/sharex/<tool>-<state>.png`.
3. Port it, add a `--render <tool>[-state]` screen, render, compare, iterate
   until only anti-aliasing differs.
4. Check behaviour side by side with ShareX (same inputs, same outputs).
5. Mark it in the tracker with what was validated and what wasn't.
