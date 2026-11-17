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
   with the same property names, so a user can move between the two. But
   ShareXrs keeps them in its own folder (`Documents/ShareXrs`, portable
   `ShareXrs/`, registry policy `SOFTWARE\ShareXrs`): it never reads,
   writes or migrates a real ShareX profile on the same machine.

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
  sharex-capture/   ShareX.ScreenCaptureLib: grabbing the screen, windows and
                    the pointer; the region capture overlay (state, input,
                    drawing, offscreen-renderable); copying images
  sharex-helpers/   ShareX.HelpersLib's shared parts: NameParser, file names
  sharex-image-effects/ ShareX.ImageEffectsLib: every effect, presets in
                    ShareX's JSON, .sxie, and the Skia raster maths they need
                    (docs/IMAGE-EFFECTS.md)
  sharex-settings/  ShareX's settings files (ApplicationConfig, HotkeysConfig),
                    read and written as ShareX does; where they live
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
| Main window (empty state, sidebar, title bar) | Presentation/MainWindow | 🟨 render <1% pixel diff in both themes; sidebar menus done (below); task area (thumbnails, list) in Phase 1 below |
| Window chrome: borderless + resize + DWM rounded corners/shadow | MainWindow.axaml | 🟨 |
| Settings files (read/write ShareX JSON) | ShareX/ApplicationConfig.cs, TaskSettings.cs, HotkeysConfig.cs, SettingManager.cs, HelpersLib/Settings, Infrastructure/AppPaths.cs, PersonalPathManager.cs | ✅ `sharex-settings`: ApplicationConfig, TaskSettings, HotkeysConfig typed; the reference build's profile and a fresh ShareX profile round-trip byte for byte, and ShareXrs's new files equal ShareX's fresh ones (HotkeysConfig byte-identical; ApplicationConfig differs only in FirstTimeRunDate, MainFormPosition and the start screen's two flags); unknown properties/enum values kept; backups (`Backup/<file>`, weekly) and load fallbacks as SettingsBase; portable mode, PersonalPath.cfg, registry policy. Loaded at startup, saved on exit and on theme toggle. Language from settings (`LanguageHelper.ChangeLanguage` at startup). Not yet: options of unported tools stay JSON (`defaults.json`), UploadersConfig untyped and not loaded |
| Tray icon and tray menu | TrayIconService, WindowsTrayIcon, MainWindow.ShowTrayMenu, MainMenuBuilder.BuildTrayMenu, TaskManager.UpdateTrayIcon | 🟨 Windows (`tray/windows.rs`, `tray_menu.rs`): the notification area icon (ShareXrs's icon at `SM_CXSMICON`, ShareX's progress icon at 0% while a task works, tooltip as ShareX sets it), every mouse button with the tray click actions (left after the double click time, double, middle; defaults run region capture, open main window, clipboard upload), and ShareX's own compact tray menu in a topmost popup window clipped to the menu, opened at the pointer against ShareX's 32x39 anchor window, kept on the work area, closed on deactivation, Escape or a command, settings saved on close; `TrayAutoExpandCaptureMenu`. `--render tray-menu` and the live popup match ShareX's popups at the same screen positions: root 0.47%, Capture submenu (flipped left, 407,133) 0.46%, Tools tool grid submenu 0.84% (text anti-aliasing only); hover, submenus, Escape, light dismiss, Show ShareX, Exit, close-to-tray, tray double/middle/left click checked live (`scripts/tray-icon.ps1` finds an icon through UI Automation, opening the overflow flyout; `input.ps1 -RightClick`; `winshot.ps1 -Index` for popups). Closing the main window hides it to the tray with `ShowTray` (Escape included). macOS (menu bar item, native menu built as ShareX's `BuildNativeMenu`, Dock icon only while the main window is shown, Accessory policy) type-checks for aarch64-apple-darwin, untested; Linux (ksni StatusNotifierItem, native menu) untested and not type-checked here. Not yet: the "minimized to tray" notification (logged), Recent items (no history), the tray entries of unported tools/windows, light theme checked against ShareX, other DPIs |
| Hotkeys (registration, conflicts, Hotkey settings window) | HotkeyManager, HelpersLib/Input, Presentation/HotkeySettings | 🟨 registration with status, the "Hotkey registration failed" message box (`--render hotkey-failed`, live: 1.23% = text anti-aliasing only), main window hotkey panel from the real hotkeys (`--render main --unregistered 0,3`: 0.90%), repeat limit, Disable hotkeys (job), `-NoHotkeys`, validated live on Windows (`RegisterHotKey` on its own thread, presses posted to winit). macOS/Linux use livesplit-hotkey: builds, untested. Hotkeys run `App::run_job`: the capture jobs (fullscreen, active window/monitor, region, transparent region, last region, custom region/window), open main window, toggle tray menu, exit (a fullscreen hotkey checked live). Not yet: Hotkey settings window, Workflows menu from the real hotkeys, Wayland portal (snapr has one) |
| Packaging: Windows exe icon, macOS .app/.dmg signed, Linux .deb; CI | (snapr) | 🟨 |
| Context menus / flyouts (`MainMenuEntry`, compact menu style) | MainMenuBuilder, ControlStyles | ✅ sidebar menus: all 8 (`sharex_ui::menu`, `main_menu.rs`), the tool grid, submenus, hover, dark + light, validated against the source build with `scripts/compare-menus.sh` (every state ≤ the main window's own ~0.9% text anti-aliasing baseline); click, hover-to-open (400ms), Escape, arrows/Enter and light dismiss checked live. Capture › Window lists the visible windows with their icons (`WindowsList.GetVisibleWindowsList`); the tray menu is a popup window of its own (Tray row). Not yet: the main window's menus as their own OS windows (drawn inside the window, so they can't extend past it), tool grid edit mode. The task context menus are in Phase 1's task list row |

### Phase 1: capture
| Item | ShareX source | Status |
|---|---|---|
| Fullscreen / monitor / window capture | ScreenCaptureLib/Screenshot.cs, ShareX/CaptureHelpers | 🟨 `capture.rs`: Capture › Fullscreen, Window (a window), Monitor, and the active window, active monitor, custom region and custom window jobs, with `CaptureBase`: the screenshot delay ('just once' resets it), hiding the main window for 250 ms from its menus (not from the tray or hotkeys, nor when capturing the main window itself), `CreateMetadata` (the foreground window names the file), restoring and activating a window before capturing it. Checked side by side over `--test-pattern` (`scripts/capture-session.ps1`): fullscreen, monitor and window (a test window) give identical images, byte-identical CF_DIB and the same clipboard PNG pixels, in the same folder with the same name pattern; a 1s delay waits as ShareX's does. Which window is foreground after the main window hides varies between runs in both apps, so `%pn_` appears in either. Not yet: client area only, window transparency and shadow (`CaptureTransparent`), auto hide taskbar/desktop icons, HDR correction; the pointer isn't drawn in this remote session in either app, so cursor drawing wasn't compared here; macOS/Linux window activation |
| Region capture overlay (incl. transparent, last region) | ScreenCaptureLib/Presentation | 🟨 Capture › Region and the Capture region hotkey (`RectangleRegion`), default mode: one overlay window per monitor; `--render region-rest/-drag/-hover` and the live overlay over `--test-pattern` match ShareX's to ≤0.1% (only the labels' text anti-aliasing differs); behaviour checked side by side (drag, window snap, Escape, right click) with identical saved files and clipboard. Region (Transparent) (`transparent_region.rs`, `RegionCaptureTransparentWindow`: a per-pixel-alpha window per monitor at alpha 1 with the white/black dashed outline, Escape and right click as ShareX's, captured 50 ms after it closes) and Last region (the remembered region recaptured, or the overlay when there's none) give images and clipboard identical to ShareX's over the pattern. Not yet: annotation tools (drawn, inert), ellipse/freehand regions, multi region mode, screen crosshair, custom info text, toolbar tooltips; the transparent selection's cursor lock in active monitor mode; the transparent overlay outside Windows (egui, untested). Checked on one 100% monitor only |
| After capture tasks: save (name pattern), clipboard, annotate | ShareX/Workflow, NameParser (HelpersLib) | 🟨 add image effects (`TaskHelpers.ApplyImageEffects`: selected or random preset, `ImageEffectOnlyRegionCapture`; validated headlessly, see the Image effects row), copy to clipboard (PNG + 24 bpp CF_DIB, byte-identical to ShareX's), save to file (`%pn_%ra{10}` / `%ra{10}` in `Screenshots/%y-%mo`, RGBA PNG or JPEG over 2048 KB), the capture, task completed and error sounds; NameParser ported whole. Not yet: the other tasks, the file exists dialog (a unique name is used), the task notification toast, history |
| Task list + thumbnail view in main window | MainWindow (`ThumbnailPanel`, `BuildTaskContextMenu`), ThumbnailItemViewModel, TaskListColumnLayout, TaskTile, WorkerTask, TaskInfo, TaskManager, RecentTaskManager, UploadInfoStatus/Manager | 🟨 `task_view.rs`, `tasks.rs`, `thumbnail.rs`, `task_actions.rs`: a region capture runs as a task (shown at once, preparing; refreshed when done), newest first; recent tasks kept and restored as ShareX (`RecentTasksSave`). Thumbnail view (cards, title above/below, hover/selected, placeholder, play icon, failed badge) and list view (header, 6 resizable columns saved to `TaskListViewColumnWidths`, status icons, image preview pane with its resizer, side or bottom). Thumbnails decoded as Avalonia/Skia does (`DecodeToFit` with Skia's mip levels and LOD bias, bilinear upscaling, whole-pixel centring, rounded clip): identical to ShareX's within 3 levels, live and rendered. `--render main-tasks` against ShareX's shots of the same files: thumbnails one 0.44%, several 0.60%, hover 0.59%, selected 0.59%, context menu 0.79%, Open › 0.81%, Copy › 0.83%; list selected 0.58% (preview pane within 2 levels), several 0.70%, hover 0.69%, menu 0.73% (text anti-aliasing only); `--render file-delete-confirm` 0.60% (live too). Checked live side by side: click/Ctrl/Shift selection, arrows, empty-area click, double click opens the file (list), right-click menus, Open › File/Folder (folder with the file selected), Copy › File (FileDrop), Image (PNG and CF_DIB byte-identical to ShareX's), File path/name/folder, Ctrl+C, Ctrl+Shift+C, Shift+C, Delete, Shift+Delete with ShareX's confirmation (No, Yes to the recycle bin, "Don't remind me again" saved), Clear, switch view (saved), resizers. Not yet: the image viewer (a thumbnail's default click and the preview click only log), drag out and the combine-images drop zones, GIF playback on hover, the preparing preview from the in-memory image, video thumbnails, upload/URL entries and tools in the menu (shown as ShareX shows them, inert), tooltips, Fluent's scroll bar (a thin indicator), menus past the window's edge, the file-not-found message box (logged) |
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
| Images | Image editor, Photo editor, Pixel editor, Image effects, Beautifier, Combiner, Comparer, Converter, Resizer, Splitter, Thumbnailer, Viewer, Watermark, Icon converter, Background remover | 🟨 Image effects engine (`sharex-image-effects`, docs/IMAGE-EFFECTS.md): all 51 effects, presets in ShareX's JSON (settings and export forms, every default and round trip equal to ShareX's), `.sxie`; against ShareX's own DLLs headlessly (`tools/effects-harness`), 107/121 cases byte-identical, the rest differ only on anti-aliased edges of simple paths (Skia's analytic AA not ported) and Text watermark glyphs (DirectWrite). Not yet: the Image effects window and packager, `.sxie` import; the other tools ⬜ |
| Video | Animated GIF maker, GIF trimmer, Video converter, Video player, Video thumbnailer, Video trimmer | ⬜ |
| Data | Analyze image, Hash checker, Index folder, Metadata, OCR, QR code, Remote storage browser | ⬜ |
| System | Borderless window, Clipboard viewer, Inspect window, Monitor test, Network monitor | ⬜ |

### Phase 4: settings windows and the rest
| Item | ShareX source | Status |
|---|---|---|
| Shared form controls (`sharex_ui::controls`) and live theme options | ShareX.Avalonia/Theming (ControlStyles, AccentPalette, ThemeManager), Avalonia.Themes.Fluent 12.1.6 templates | 🟨 Button (+`.compact`, `.primary` with `SystemAccentColorLight1/Dark1`), CheckBox, ToggleSwitch, ComboBox with its dropdown (items sized to the selected one, accent selection, light dismiss, wheel), TextBox (placeholder as ShareX's `TextBlock`), NumericUpDown (spinner repeat), Slider, `ColorPickerDropdown`'s button, cards; rest/hover/pressed/disabled from the templates. `AccentPalette` ported (reproduces ShareXTheme.axaml's default accent colours exactly, unit-tested); accent, spacing (per area) and icon pack are app-wide and apply live to every window; `UseSystemTheme` follows the OS (winit `ThemeChanged`), `UseSystemAccentColor` reads Windows' accent (`Explorer\Accent\AccentPalette`). Legacy icons that use `currentColor` are now drawn in their foreground. Not yet: Avalonia's `ColorView` (the colour popup is egui's HSV picker in ShareX's popup frame), focus rings/keyboard navigation of the controls, Fluent's expanding scroll bar (a thin indicator), dropdowns past the window's edge |
| Start screen (welcome) | Presentation/StartScreen | 🟨 shown before startup while `ShowStartScreen` (not `-sandbox`), capped to the work area as `FitToWorkingArea`; every choice applies live (theme tiles, accent presets, colour picker, follow system accent, icon pack, spacing, language at once, run at startup); closing it clears `ShowStartScreen`/`ShowIconPackPrompt`, saves and goes on. `--render start-screen` vs ShareX's fresh profile: dark top 1.86%, light+indigo+Legacy 2.35%, light bottom 2.83% (the logo is ShareXrs's icon, ~0.5%; a few labels within 1px; content 765 vs 767 tall). ShareX opens it scrolled 31px (focus brought into view); ShareXrs opens at the top. Checked live side by side: Light, #6366F1, Legacy, Get started, then Compact in Application settings give the same `ThemeOptions`, `ShowStartScreen` and `ShowIconPackPrompt` in both apps' ApplicationConfig.json. The icon pack prompt (`IconPackPromptWindow`) isn't ported: a fresh profile never shows it (the Start screen clears it) |
| Run at startup | ShareX/StartupManager.cs | 🟨 Windows as ShareX: `ShareXrs.lnk` in the Startup folder (exe, `-silent`, its folder), disabled by user via `StartupApproved\StartupFolder`; checked live against ShareX's `ShareX.lnk`. macOS (LaunchAgent `com.sharexrs.app.plist`) and Linux (XDG autostart entry) written, untested. `-silent` starts in the tray |
| Application settings | Presentation/ApplicationSettings, ShareX.Avalonia/Controls/SettingsNavigation, SettingsSearch | 🟨 window (density width, live resize with the spacing), navigation, search (pages, cards and combo options as `SettingsSearch`), restart banner, all 12 pages with every control bound to ApplicationConfig (Print and Proxy through their JSON until those types are ported), the clipboard format list and editor, the spacing preview; opened from the sidebar and the tray menu; applied and saved on close (`ApplicationSettingsRuntime.Apply`: tray, hotkey repeat limit, task view, menus). `--render settings-<page>` vs ShareX, dark/light: general 0.56/0.45%, theme 0.37/0.28%, integration 0.35/0.25%, paths 1.21/1.00% (its folder previews show other paths), settings 0.38/0.27%, main window 0.36%(light), clipboard formats 0.22/0.16%, upload 0.42/0.37%, history 0.40/0.34%, print 0.20/0.15%, proxy 0.39/0.34%, advanced 0.48/0.39%; compact + indigo + Legacy theme page 0.51%; Theme combo dropdown 0.71%. Live: sidebar opens it, pages, compact mode, startup. Not yet (logged when used): export/import/reset of settings, the dev build installer, Explorer/browser integration (check boxes only), image print settings, quick task menu editor, variable menus of the path pattern and clipboard format boxes, white tray icon; only the first screen of each page was compared (scrolled parts weren't) |

Task settings, Destination settings, Workflows, Actions toolbar, Quick task
menu, About, Debug log, notifications, updates: ⬜.

### Validating capture overlays safely

The frozen screen shows whatever is on screen, so never open a capture
overlay over the desktop. `scripts/region-session.ps1` covers every monitor
with `sharexrs --test-pattern` first, opens Capture › Region in ShareX (UI
Automation) or ShareXrs (raised above the pattern and clicked), drives the
overlay (`-At`, `-DragTo`, `-Hold`, `-Window` for a window to snap to),
grabs the screen (`-Out`) and always ends with Escape. `pattern-check.py`
deletes any grab that isn't mostly the pattern. `clipboard-dump.ps1` saves
what a capture copied. Run ShareXrs portable (a `Portable` file beside the
exe) so its captures land in `target/debug/ShareXrs`.

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
