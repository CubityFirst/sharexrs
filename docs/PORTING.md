# Porting ShareX to Rust

ShareXrs is a port of [ShareX](https://github.com/ShareX/ShareX) 22.4 (the
Avalonia rewrite; commit in `REFERENCE_COMMIT`) to Rust and egui, with the
same UI and behaviour, running on Windows, macOS and Linux. This file is the
plan and the tracker; [MACOS-LEARNINGS.md](MACOS-LEARNINGS.md) is what snapr
taught us about shipping on macOS, and [PLATFORMS.md](PLATFORMS.md) is what
ShareX itself does on macOS and Linux, feature by feature, against ShareXrs.

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
   instead of egui galley sizes (which round down). A wrapping
   `TextBlock` is its lines at the exact line height, rounded up once (two
   Segoe UI 13 lines are 35 px, not 36). A `Grid` of only `Auto` rows that
   is stretched taller than it needs gets a pixel more per non-empty row
   (`SetFinalSize`'s rounding pass; the video thumbnailer's panels), and a
   hidden row keeps the `RowSpacing` around it.
   Check fonts by measuring text widths, not by eye: a control's plain string
   content (a `MenuItem` header, a `Button`'s text) is drawn by its
   `ContentPresenter` in Avalonia's default family, which ShareX's
   `.WithInterFont()` makes Inter 3.019 (`fonts::inter`), not Segoe UI; and
   ShareX's global SemiBold `TextBlock` style doesn't reach `TextBlock`s in
   popups (menus, the tool grid), which are Segoe UI Regular.
   Draw a bordered or rounded shape with `sharex_ui::border` (not egui's
   `rect_filled` + `rect_stroke`): Avalonia fills a `Border` to the middle
   of its border and strokes that path centred, and ShareX on Windows
   renders through Skia's GPU backend, whose rounded-rect and path edges
   are antialiased by distance (half a pixel minus the signed distance to
   the edge), not by area. An Avalonia.Headless probe renders on Skia's
   CPU backend (area coverage): trust its colours and state resolution,
   but compare edges with screenshots of the real app. Show keyboard
   focus with `sharex_ui::focus` (Tab/arrows make it visible, a click
   hides it); `--render controls-gallery` draws every shared control in
   every state.
5. **Settings stay compatible.** ShareXrs reads and writes ShareX's JSON
   (`ApplicationConfig.json`, `HotkeysConfig.json`, `UploadersConfig.json`)
   with the same property names, so a user can move between the two. But
   ShareXrs keeps them in its own folder (`Documents/ShareXrs`, portable
   `ShareXrs/`, registry policy `SOFTWARE\ShareXrs`): it never reads,
   writes or migrates a real ShareX profile on the same machine.

Known, accepted differences (the full list, with what's blocked, is
[Accepted differences / blocked](#accepted-differences--blocked) at the end):
- Text anti-aliasing: Avalonia uses ClearType (subpixel) on Windows, egui
  grayscale. Shapes and positions match; edges of glyphs don't. Matching it
  (a Skia-like LCD glyph rasterizer and RGB-coverage text in egui-wgpu) is
  planned as its own project once the major tools are ported (decided
  2026-11-17); until then it's accepted, so don't "fix" text by moving it.
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
                    drawing, offscreen-renderable); copying images; scrolling
                    capture's loop and stitching (docs/SCROLLING-CAPTURE.md)
  sharex-helpers/   ShareX.HelpersLib's shared parts: NameParser, file names,
                    culture-aware (NLS) string comparisons
  sharex-image-editor/ ShareX.ImageEditor: the annotation model (ShareX's
                    JSON), ImageEditorOptions, rendering annotations as the
                    editor exports them (docs/IMAGE-EDITOR.md)
  sharex-image-effects/ ShareX.ImageEffectsLib: every effect, presets in
                    ShareX's JSON, .sxie (docs/IMAGE-EFFECTS.md)
  sharex-skia/      Skia m119's CPU rasteriser as ShareX's SkiaSharp runs it:
                    SkPath, analytic AA and supersampling, edge clipping,
                    SkStroke, legacy blitters, blur, resampling, the raster
                    pipeline maths; shared by the editor and the effects
  sharex-settings/  ShareX's settings files (ApplicationConfig, HotkeysConfig),
                    read and written as ShareX does; where they live
  sharex-strings/   ShareX's .resx (29 languages) -> Rust, generated at build
  sharex-uploaders/ ShareX.UploadersLib: HTTP layer, custom uploaders (.sxcu),
                    built-in destinations, UploadersConfig (docs/UPLOADERS.md)
  sharex-screen-recording/ ShareX's FFmpeg recorder: command lines, running
                    FFmpeg, pause by segments, two-pass/GIF, FFmpeg discovery,
                    devices, macOS/Linux inputs (docs/SCREEN-RECORDING.md)
  sharex-native-recorder/ ShareX.ScreenRecordingLib: the native recorder's
                    worker, Media Foundation MP4 writer, D3D11 video processor,
                    camera overlay, adaptive GIF writer, audio mixer; capture
                    behind traits (docs/SCREEN-RECORDING.md, section 10)
  sharex-history/   ShareX.HistoryLib as data: History.db (ShareX's schema and
                    values), the legacy JSON/XML files and their migration,
                    the history windows' queries and actions, the stats
                    (docs/HISTORY.md)
  sharex-tools/     ShareX.Tools' engines without their windows: the image
                    tools, hash checker, QR codes, index folder, metadata,
                    the video tools' FFmpeg command lines, monitor test
                    (docs/TOOLS.md)
  sharex-ocr/       ShareX.OCR: PP-OCRv6 detection and recognition on ONNX
                    Runtime (loaded at run time), Skia preprocessing, lines and
                    paragraphs (docs/OCR.md)
  sharex-ui/        theme, density, fonts, icon packs, shared widgets,
                    wgpu device, offscreen renderer, tiled textures for
                    images past the GPU's limit
  sharexrs/         the app: winit event loop, one egui context per OS window
                    (window.rs), the main window, `--render`
```

The app follows snapr's architecture (raw winit 0.30 + egui-winit +
egui-wgpu, one shared wgpu device, windows created and destroyed on demand),
which already copes with tray-only operation, per-monitor capture overlays
and macOS's quirks. New ShareX projects become crates as they're ported
(`sharex-capture` for ShareX.ScreenCaptureLib, `sharex-uploaders`,
`sharex-history`, `sharex-image-effects`, `sharex-tools`, `sharex-ocr`, ...).

## Tracker

Status: ⬜ not started · 🟨 in progress · ✅ matches ShareX (validated) · ⏸ deferred

**Deferred (user decision, 2026-11-17):** the Photo editor (ShareX.PhotoEditor)
and Pixel editor (ShareX.PixelEditor) wait until everything else is ported.
No agents are scheduled for them; their Tools menu entries stay inert.

### Phase 0: foundation
| Item | ShareX source | Status |
|---|---|---|
| Theme colours, density, fonts, icons, strings | ShareX.Avalonia/Theming, ShareX.Icons, */Localization | ✅ |
| Main window (empty state, sidebar, title bar) | Presentation/MainWindow | 🟨 render <1% pixel diff in both themes; sidebar menus done (below); task area (thumbnails, list) in Phase 1 below; the drop overlay (files and text) in the Upload row. Open: other DPIs (the 125% fixes are in progress separately) |
| Window chrome: borderless + resize + DWM rounded corners/shadow | MainWindow.axaml | 🟨 the title bar, caption buttons, resize borders, DWM corners and shadow are in (the main window's renders and live shots match ShareX's). Open: no side-by-side check of the frame's behaviour (resizing from each edge, snapping, maximize to the screen edge) is recorded; other DPIs |
| Settings files (read/write ShareX JSON) | ShareX/ApplicationConfig.cs, TaskSettings.cs, HotkeysConfig.cs, SettingManager.cs, HelpersLib/Settings, Infrastructure/AppPaths.cs, PersonalPathManager.cs | ✅ `sharex-settings`: ApplicationConfig, TaskSettings, HotkeysConfig typed; the reference build's profile and a fresh ShareX profile round-trip byte for byte, and ShareXrs's new files equal ShareX's fresh ones (HotkeysConfig byte-identical; ApplicationConfig differs only in FirstTimeRunDate, MainFormPosition and the start screen's two flags); unknown properties/enum values kept; backups (`Backup/<file>`, weekly) and load fallbacks as SettingsBase; portable mode, PersonalPath.cfg, registry policy. Loaded at startup, saved on exit and on theme toggle. Language from settings (`LanguageHelper.ChangeLanguage` at startup). UploadersConfig loaded in the background at startup and saved with the others (`upload::UploadersConfigStore`). Classes ShareXrs doesn't type (some tools' options) are kept as ShareX's JSON (`defaults.json`) and round-trip byte for byte |
| Tray icon and tray menu | TrayIconService, WindowsTrayIcon, MainWindow.ShowTrayMenu, MainMenuBuilder.BuildTrayMenu, TaskManager.UpdateTrayIcon | 🟨 Windows (`tray/windows.rs`, `tray_menu.rs`): the notification area icon (ShareXrs's icon at `SM_CXSMICON`, ShareX's progress icon at 0% while a task works, tooltip as ShareX sets it), every mouse button with the tray click actions (left after the double click time, double, middle; defaults run region capture, open main window, clipboard upload), and ShareX's own compact tray menu in a topmost popup window clipped to the menu, opened at the pointer against ShareX's 32x39 anchor window, kept on the work area, closed on deactivation, Escape or a command, settings saved on close; `TrayAutoExpandCaptureMenu`. `--render tray-menu` and the live popup match ShareX's popups at the same screen positions: root 0.47%, Capture submenu (flipped left, 407,133) 0.46%, Tools tool grid submenu 0.84% (text anti-aliasing only); hover, submenus, Escape, light dismiss, Show ShareX, Exit, close-to-tray, tray double/middle/left click checked live (`scripts/tray-icon.ps1` finds an icon through UI Automation, opening the overflow flyout; `input.ps1 -RightClick`; `winshot.ps1 -Index` for popups). Closing the main window hides it to the tray with `ShowTray` (Escape included). macOS (menu bar item, native menu built as ShareX's `BuildNativeMenu`, Dock icon only while the main window is shown, Accessory policy) type-checks for aarch64-apple-darwin, untested; Linux (ksni StatusNotifierItem, native menu) untested and not type-checked here. Outside Windows the glyph and progress icons are 16 px and take the OS's light/dark mode, as `TrayIconImage` and `GetProgressIconBytes` do there (PLATFORMS.md). The "minimized to tray" tip and Recent items (`BuildRecentItemsMenu`: Copy / Open per recent task, most recent first as set) are in (App windows row). Every tray entry runs (the Photo/Pixel editor tools stay inert ⏸; Restart as administrator closes and starts ShareXrs again with `runas`, not run live). Open: the light theme compared with ShareX's and other DPIs (the 125% fixes are in progress separately); macOS/Linux on a real desktop (hardware) |
| Hotkeys (registration, conflicts, Hotkey settings window) | HotkeyManager, HelpersLib/Input, Presentation/HotkeySettings | ✅ registration with status, the "Hotkey registration failed" message box (`--render hotkey-failed`, live: 1.23% = text anti-aliasing only), main window hotkey panel from the real hotkeys (`--render main --unregistered 0,3`: 0.90%), repeat limit, Disable hotkeys (job), `-NoHotkeys`, validated live on Windows (`RegisterHotKey` on its own thread, presses posted to winit). macOS/Linux use livesplit-hotkey: builds, untested (beyond ShareX, whose `WindowsHotkeyHost` throws outside Windows: PLATFORMS.md). Hotkeys run `App::run_job`: the capture jobs (fullscreen, active window/monitor, region, transparent region, last region, custom region/window), open main window, toggle tray menu, exit (a fullscreen hotkey checked live). Hotkey settings window: see Phase 4. The Workflows menu lists the real hotkeys (App windows row). Windows checked live (and in [LIVE-CHECKS.md](LIVE-CHECKS.md)); blocked: macOS/Linux key codes on a real desktop (hardware). Beyond ShareX and not done: a Wayland global shortcuts portal (ShareX has none) Hotkey settings' custom-settings and status tooltips open only over the icons' strokes, as Avalonia hit-tests a background-less `IconPresenter`/`Grid` (HOT3-02). |
| macOS and Linux code paths | ShareX's `OperatingSystem` checks, `LinuxDesktopWallpaperService`, `MacOSDesktopWallpaperService`, Avalonia/.NET behaviour | 🟨 inventory and status per feature in docs/PLATFORMS.md: wallpaper services, clipboard file copies, folder picker, recycle bin, tray glyphs, colour picker, Unix `Ping`, Linux openers ported as ShareX does them (unit-tested on every OS, macOS type-checked here, Linux on CI); live checks for Mac/Linux users listed there. Linux (PLATFORMS.md "Linux: display, capture and GPU"): monitors from winit, every xcap call guarded (libwayshot's panic killed startup on WSLg), Wayland capture through wlr-screencopy else the screenshot portal else a failed capture, WSLg windows through XWayland (its Weston crashes on winit's client-side decorations), hardware GL when Vulkan is software-only (WSL's D3D12); checked live in WSLg on Ubuntu 26.04 and Arch. Open: the clipboard viewer's full format list and the extra `image/png` clipboard item on macOS; blocked: every check on a real Mac or Linux desktop (hardware) |
| Packaging: Windows exe icon, macOS .app/.dmg signed, Linux .deb; CI | ShareX.Setup, build.yml (and snapr) | ✅ docs/RELEASING.md: `release.yml` (tags, dry runs) and `package.yml` (also from CI on master and packaging PRs) build the Windows x64/ARM64 installer (ShareX's Inno Setup script under ShareXrs's names and ProgIDs) and portable zip, the macOS .dmg/.app.zip (self-signed, tags fail without the certificate) and the Linux .deb/tarball, each with FFmpeg, ONNX Runtime 1.24.4 and the OCR models pinned by SHA256, licences and FFmpeg's source offer; SHA256SUMS and notes. Windows packages built and the installer round-tripped locally; macOS/Linux only on CI. The update check: see Phase 4's row. The installer's tasks are checked as ShareX's (the Explorer context menu and Send To pass the files, which a second launch forwards), and the app registers `.sxcu`/`.sxie` at start (Command line row). Blocked: Windows code signing and the macOS Developer ID (certificates); the macOS/Linux packages installed on a real machine (hardware) |
| Context menus / flyouts (`MainMenuEntry`, compact menu style) | MainMenuBuilder, ControlStyles | 🟨 sidebar menus: all 8 (`sharex_ui::menu`, `main_menu.rs`), the tool grid, submenus, hover, dark + light, validated against the source build with `scripts/compare-menus.sh` (every state ≤ the main window's own ~0.9% text anti-aliasing baseline); click, hover-to-open (400ms), Escape, arrows/Enter and light dismiss checked live. Capture › Window lists the visible windows with their icons (`WindowsList.GetVisibleWindowsList`); the tray menu is a popup window of its own (Tray row). The tool grid's edit mode (App windows row). Every menu level (root, submenus, the tool grid) is a popup placed by the port of Avalonia's `ManagedPopupPositioner` (`sharex_ui::popup::position`: the screen under the anchor, device pixels, truncated position) and, on Windows, a level past the window's edge is its own OS popup window (Popups row); `--render main-capture-menu --open 12 --popup-host … --client-origin 467,31` puts Capture at 673,76 and its flipped Screenshot delay at 439,396 (237x215), as ShareX's `edge-submenu` popups (tested); menus that fit render byte-identically to before. Live check pending: the levels past the edge as OS windows (hover-to-open, click, Escape, arrows/Enter, light dismiss over them). The task context menus are in Phase 1's task list row |
| Popups past the window's edge (menu levels, dropdowns, flyouts) | Avalonia `Popup`, `ManagedPopupPositioner` | 🟨 `sharex_ui::popup` + `popup_window.rs`: placed on the screen's work area (flip, slide, resize); one that overflows its window is an owned, never-activated OS popup window, input routed to the owner (Windows; elsewhere popups stay in the window). Menus (the sidebar's, their submenus and tool grid, the task list's and Hotkey settings' context menus, every `sharex_ui::menu::Flyout`) place each level with `popup::position`, a port of `ManagedPopupPositioner.Calculate`/`Update` (Avalonia 12.1.6): anchor point and gravity per `PlacementMode` (`Pointer` a 1x1 anchor, `RightEdgeAlignedTop` with Fluent's -4 submenu offset, `BottomEdgeAlignedLeft`, `AnchorAndGravity`), FlipX/SlideX/ResizeX then FlipY/SlideY/ResizeY against the work area of the screen under the anchor (the host passes every screen, `set_host_screens`), in device pixels with the position truncated (`MoveAndResize`), unit-tested at the screen's right and bottom edges, near them, on a monitor at negative coordinates and at 125%. The main window keeps its pointer while it's over one of its popup windows (no false light dismiss). Geometry checked with `--render --popup-host [--client-origin X,Y]` (prints each level's place and whether it leaves the window) against the shared-control sidecars (VID-02, `edge-submenu`); Escape goes to the innermost popup first (SET-05a). Live check pending ([LIVE-CHECKS.md](LIVE-CHECKS.md#fixes-for-the-fails-branch-fixlive-check-failures)), menus included Combo box dropdowns: centred under the box (Fluent's `PART_Popup` takes `Popup`'s default `Bottom`), opening with only the selected item scrolled into view, items layout-rounded (SET2-03: the time zone list at ShareX's place and size, 0.94%); re-check pending ([LIVE-CHECKS.md](LIVE-CHECKS.md#fixes-for-pass-2-branch-fixlive-checks-2)). |

### Phase 1: capture
| Item | ShareX source | Status |
|---|---|---|
| Fullscreen / monitor / window capture | ScreenCaptureLib/Screenshot.cs, ShareX/CaptureHelpers | ✅ `capture.rs`: Capture › Fullscreen, Window (a window), Monitor, and the active window, active monitor, custom region and custom window jobs, with `CaptureBase`: the screenshot delay ('just once' resets it), hiding the main window for 250 ms from its menus (not from the tray or hotkeys, nor when capturing the main window itself), `CreateMetadata` (the foreground window names the file), restoring and activating a window before capturing it. Checked side by side over `--test-pattern` (`scripts/capture-session.ps1`): fullscreen, monitor and window (a test window) give identical images, byte-identical CF_DIB and the same clipboard PNG pixels, in the same folder with the same name pattern; a 1s delay waits as ShareX's does. Which window is foreground after the main window hides varies between runs in both apps, so `%pn_` appears in either. `TaskHelpers.GetScreenshot`'s other options (`screenshot.rs`, `transparent.rs`), checked side by side on `--test-pattern --framed` (a decorated test window) with `scripts/window-capture-session.ps1` (Custom window job, or a region capture clicking the window): client area only gives identical images (400x237); `CaptureTransparent` (`Screenshot_Transparent`: a layered helper window under the target made white, black, white; `CreateTransparentImage`, `CopyOpaquePixels`, `AutoCropImage`, `TrimShadow`, with ShareX's premultiply round trips) gives the same size and identical alpha without the shadow (402x332, 1536 transparent corner pixels) and with it (`CaptureShadow`, offset 20: 442x367, 30286 non-opaque pixels), through the region capture of a whole window (300 ms after the overlay); colours differ only in the title bar, whose active/inactive tint at the moment of the grab depends on which app the forwarded command activated. The plain window capture path waits ShareX's 100 ms after activating the window, which the Windows 11 title bar fade outlasts in ShareXrs (its first grab comes 54 ms later, sooner than ShareX's), so there the two white grabs can differ and it falls back as ShareX does ("Images not equal"). `CaptureAutoHideTaskbar`: the taskbar is hidden while a window over it is grabbed and shown again (a drop guard); watched through the API (`IsWindowVisible(Shell_TrayWnd)` every 2 ms), both apps hide it for 50-80 ms and restore it. `CaptureAutoHideDesktopIcons` (`DesktopIconManager`, a drop guard that shows them whatever the capture does): the desktop icons are already hidden on the reference machine, so both apps leave them alone (checked through the API); the hiding itself isn't exercised live. The pointer (`CursorData`): drawn with `DrawIconEx` onto screenshots and with Skia's `DrawBitmap` in the region and transparent captures, as ShareX does; the pointer isn't drawn in this remote session, so it's checked headlessly against ShareX's own `CursorData` (`tools/tools-harness --cursors` and the test `standard_cursors_into_the_pattern`): 6 cursors (an XOR one included) both ways, 12/12 pixel-identical. HDR correction (`HDRScreenCapture`, `sharex_capture::hdr` and `platform::windows_hdr`): with `HDRScreenshotColorCorrection`, every GDI capture (and the transparent capture's grabs) is corrected from a DXGI desktop duplication of each HDR output it touches, through Windows' WIC tone mapper and the SDR reference, or ShareX's own tone curve when WIC has none, the output turned and read back as `SkiaPixelBuffer` does, then the pointer drawn as `CursorData.DrawCursor(SKBitmap)`; `GetSdrWhiteScale` is shared with the native recorder (`sharex_helpers::display_config`). Against ShareX's own `HDRScreenCapture` on synthetic FP16 and HDR10 frames (`tools/hdr-harness`, 19 cases): 168/168 outputs identical (tone curve, reference, WIC, rotations, corrected captures through WIC and the fallback); the fixtures run in `cargo test`. Blocked: HDR checked live (the reference machine has no HDR output, where ShareX's correction does nothing either; hardware). As ShareX: transparency, the taskbar, the desktop icons and window activation are Win32 only (PLATFORMS.md) |
| Region capture overlay (incl. transparent, last region) | ScreenCaptureLib/Presentation | 🟨 Capture › Region and the Capture region hotkey (`RectangleRegion`), default mode: one overlay window per monitor; `--render region-rest/-drag/-hover` and the live overlay over `--test-pattern` match ShareX's to ≤0.1% (only the labels' text anti-aliasing differs); behaviour checked side by side (drag, window snap, Escape, right click) with identical saved files and clipboard. Annotation tools (docs/IMAGE-EDITOR.md, "The editor UI"): every tool of the toolbar switches to the image editor's workspace, shapes draw live with the real renderer, select, move, resize and rotate with ShareX's handles, text is typed in place (existing text in its own box; speech balloons open their editor and default tail on selection), undo/redo, delete, copy/cut/paste/duplicate of annotations, z-order, flatten, the capture actions, the five image tools (Image and its picker, Clone, Cursor, Smart eraser, Emoji and its picker), the tool options row with every dropdown and colour popup and the shadow options popup (dark 0.01-0.44%, light 0.01-0.40%; the cursor type list 1.2/1.7%: ShareX's shot hovers an item); every tool and state over the pattern 0.00-0.09% from ShareX's, and the capture renders through `render::render_region_cached` (ShareX's `GetSnapshot(Rect)`, identical on 11 headless cases). Region (Transparent) (`transparent_region.rs`, `RegionCaptureTransparentWindow`: a per-pixel-alpha window per monitor at alpha 1 with the white/black dashed outline, Escape and right click as ShareX's, captured 50 ms after it closes) and Last region (the remembered region recaptured, or the overlay when there's none) give images and clipboard identical to ShareX's over the pattern. The shown pointer is an unselected image annotation over the frozen screen (`InsertWorkspaceImageAnnotation`), the workspace in host mode (no auto-expand). The toolbar follows `RegionCaptureOptions.ToolbarItems`; the colour popups' screen colour picker and the Emoji tool's picker (checked live over the pattern). Region shapes (`region/shape.rs`, `chrome.rs`: `CaptureRegion`, `RegionSelectionOverlay.Render`): the RegionEllipse/RegionFreehand toolbar items select an ellipse or a freehand region (Shift for a straight edge), a hovered window becomes an ellipse with the ellipse tool, multi region mode keeps several regions (press one to edit it, Delete removes the selection then the last one, Enter captures them all), and the dimming goes round them as ShareX's does (`GetUncoveredRectangles`, or the geometry exclusion drawn with Skia's renderer); captures with shapes or several regions are masked (`ApplyRegionMask`) and unpremultiplied as ShareX's PNG encoder does. The screen crosshair (`ShowScreenCrosshair`), custom info text (`UseCustomInfoText`, through `CodeMenuEntryPixelInfo.Parse`) and toolbar tooltips (`region/tooltip.rs`: FluentTheme's `ToolTip`, resolved by `tools/editor-harness --tooltip-theme`, after 400 ms at the pointer + 20, Segoe UI 13 Regular). Live over the pattern (`scripts/annotate-session.ps1 -Job RectangleRegion`), ShareXrs vs ShareX: at rest with the crosshair and custom info text 0.06%, ellipse tool 0.07%, ellipse mid-drag 0.08% (light 0.04%), freehand tool 0.08%, freehand mid-drag 0.08%, two regions 0.07%, regions and an ellipse 0.07%, tooltips dark 0.05% (region button, Rectangle), light 0.06% (text anti-aliasing only); `--render region-annotate --options` draws the same states. Captured outputs side by side: ellipse 251x201 pixel-identical (after the headless check `region_capture_over_the_pattern_to_a_file` found the encoder's tie rounding), two rectangles and an ellipse in multi region mode 801x351 pixel-identical, freehand identical but for its last free edge (the pointer's moves are sampled differently live; its straight and Shift edges match). Every options row dropdown opens its popup. Active monitor mode locks the pointer to the overlay and to the transparent region window while they're active (`LockCursorToWindow`, Windows only as in ShareX), and the overlay opens only on the monitor it covers; with one monitor here the clip equals the free pointer, so that isn't observable live. Speed ([Region capture performance](#region-capture-performance)): arguments received to overlay visible 116-117 ms (ShareX 145, ShareXrs before 146-148), the overlay windows kept between captures and warmed up as ShareX's (`WarmUpRegionCapture`), the saved files and the overlay's frames unchanged. Open: other DPIs (the 125% fixes are in progress separately). Blocked: several monitors (one here) and the transparent overlay outside Windows (egui, untested; hardware) Upstream 30d1682: the "Capture selected region" mouse action (`CaptureSelection`, ShareX #9277; X1 and X2 also run their actions with an annotation tool active), unit-tested; ShareX checked live (X1 over a hovered window captured it, 600x360 at 700,450). The overlays keep `WS_EX_TOOLWINDOW` and, when layered, `WS_EX_LAYERED` through winit's style rewrite on show (ShareX #9242): the transparent region drew opaque here before, now the pattern shows through (ex-style 0xC0198). The frozen screen and the shape chrome are drawn as tiles when a monitor is larger than the GPU's largest texture ([Large images and small GPUs](#large-images-and-small-gpus)). Upstream ca3f01f: the rectangle region is a toolbar item (`RegionRectangle`, Tab and NumPad0, ShareX #9326) that can be moved, hidden and rebound, its keys working while hidden; the toolbar at rest vs ShareX ca3f01f at 125% 0.16% (100% 0.03%), its tooltip 0.18%, hidden 0.16%; Tab and NumPad0 with it hidden checked live in ShareX and rendered in ShareXrs (0.16%, the region tool in both). |
| After capture tasks: save (name pattern), clipboard, annotate | ShareX/Workflow, NameParser (HelpersLib) | 🟨 add image effects (`TaskHelpers.ApplyImageEffects`: selected or random preset, `ImageEffectOnlyRegionCapture`; validated headlessly, see the Image effects row), copy to clipboard (PNG + 24 bpp CF_DIB, byte-identical to ShareX's), save to file (`%pn_%ra{10}` / `%ra{10}` in `Screenshots/%y-%mo`) in the task's image format (`TaskHelpers.PrepareImage`, `sharex_tools::capture_image`: PNG with its bit depth, level and filter and `PNGStripColorSpaceInformation`, JPEG on white with quality and subsampling, GIF with each quantizer, BMP with its depth, WebP lossless or lossy, the switch to JPEG over `ImageAutoUseJPEGSize` with `ImageAutoJPEGQuality`'s search; against ShareX's own `PrepareImage` headlessly, `tools/tools-harness`, 112 cases, all in the same format as ShareX's: JPEG 37/37 (13 from the switch), GIF 10/10, BMP 7/7, WebP 18/18 byte-identical, PNG 40/40 pixel-identical with the same colour type, its size within a few percent of Skia's zlib (−1 to −5% at the default level), so a PNG that close to the limit can switch differently), beautify image (the Image beautifier, the task waits), the capture, task completed and error sounds; NameParser ported whole; Recognize text (OCR): silently (text to the clipboard, or the clipboard cleared) or in the OCR window, which the task waits for (docs/OCR.md). The file exists window and the task notification toast (App windows row); the history (History row). Since, every other image task of `DoAfterCaptureJobs`: Send image to printer (`PrintImage`: the Print window, or the printer with `DontShowPrintSettingsDialog`), Save image to file as... (the save dialog with ShareX's types and all files, the last save folder, another format re-encoded and used for the upload, asked again when the file can't be written), Save thumbnail image to file (`CreateThumbnail`: `Resize` on white, JPEG 90, `<name><ThumbnailName>.jpg`, into `$thumbnailfilename`); the image is encoded for OCR, Analyze image and the dialog too, and saved for Analyze image, as ShareX does; tested with a fake task UI. Open Photo editor ⏸. Open: the dialog and printer checked live (no print dialog is ever opened here), the thumbnail's bytes compared with ShareX's |
| Task list + thumbnail view in main window | MainWindow (`ThumbnailPanel`, `BuildTaskContextMenu`), ThumbnailItemViewModel, TaskListColumnLayout, TaskTile, WorkerTask, TaskInfo, TaskManager, RecentTaskManager, UploadInfoStatus/Manager | 🟨 `task_view.rs`, `tasks.rs`, `thumbnail.rs`, `task_actions.rs`: a region capture runs as a task (shown at once, preparing; refreshed when done), newest first; recent tasks kept and restored as ShareX (`RecentTasksSave`). Thumbnail view (cards, title above/below, hover/selected, placeholder, play icon, failed badge) and list view (header, 6 resizable columns saved to `TaskListViewColumnWidths`, status icons, image preview pane with its resizer, side or bottom). Thumbnails decoded as Avalonia/Skia does (`DecodeToFit` with Skia's mip levels and LOD bias, bilinear upscaling, whole-pixel centring, rounded clip): identical to ShareX's within 3 levels, live and rendered. `--render main-tasks` against ShareX's shots of the same files: thumbnails one 0.44%, several 0.60%, hover 0.59%, selected 0.59%, context menu 0.79%, Open › 0.81%, Copy › 0.83%; list selected 0.58% (preview pane within 2 levels), several 0.70%, hover 0.69%, menu 0.73% (text anti-aliasing only); `--render file-delete-confirm` 0.60% (live too). Checked live side by side: click/Ctrl/Shift selection, arrows, empty-area click, double click opens the file (list), right-click menus, Open › File/Folder (folder with the file selected), Copy › File (FileDrop), Image (PNG and CF_DIB byte-identical to ShareX's), File path/name/folder, Ctrl+C, Ctrl+Shift+C, Shift+C, Delete, Shift+Delete with ShareX's confirmation (No, Yes to the recycle bin, "Don't remind me again" saved), Clear, switch view (saved), resizers. The image viewer opens from a thumbnail's click and the preview (App windows row). The long tail (built from source, no ShareX shot of these states; `--render main-tasks --combine S,T[,h|v]`, `--hover` over a title): a card or row's file dragged out past half the tap size (`drag_out.rs`: OLE's `DoDragDrop` with a shell data object on Windows, behind `DragSource` so tests use a mock; the window redraws from the drop source's feedback, so another image card shows the `IsCombineTarget` drop zones, the window shows no-drop elsewhere, and a drop on Horizontal/Vertical runs `TaskHelpers.CombineImages(source, target)`; the release after a drag clicks nothing; the window ignores its own drag's drop events), GIF/WebP playback (`AnimatedImagePreview` in `sharex-tools::animated_preview`: Skia's GIF composition, libwebp's animation decoder, Mitchell scaling, `PlayAsync`'s clock on a decoder thread in `animation.rs`) over a hovered thumbnail (at twice its size) and in the list's preview pane (at most 1024), the thumbnail from the task's own image while it works (`ImageReady` → `RefreshFromOwnedImage`, `CreatePreview`'s Mitchell resize), video thumbnails from the shell (`IShellItemImageFactory` through `CreatePreview`; none outside Windows, as ShareX), Fluent tooltips (`sharex_ui::tooltip`: ToolTipService's 400/100 ms, pointer placement, wrapped at 320) on a title (`Details`), the list's file name and URL cells, the theme toggle and the hotkey descriptions. The context menu's entries all run (Photo/Pixel editor ⏸), with Ctrl+V (clipboard upload), Ctrl+U, Ctrl+D, the Open URL click action and title click, Delete remotely (one finished Amazon S3 file upload with a valid S3 config: the question, `AmazonS3.TryGetObjectKey` and `DeleteObjectAsync` on a thread, the tip; tested on 127.0.0.1) and `FileHelpers.OpenFile`/`OpenFolderWithFile`'s "File does not exist" message box. The context menu and its submenus are popups placed by Avalonia's positioner on the screen (a submenu past the window's edge, as ShareX's Copy › in `tasks-thumb-menu-copy`, is its own OS popup window on Windows; live check pending); a menu flipped at the pointer ends one pixel right of it (Avalonia's 1x1 pointer anchor), one pixel right of before. Open: the drag outside Windows (ShareX has Avalonia's macOS/X11 drag sources; not written), Fluent's scroll bar (a thin indicator) Upstream 93145cc: the thumbnail view's columns follow `UpdateThumbnailColumns` (2523ded; card and margins rounded to device pixels, +0.001): at 125% four 200px cards fit from a 1372px window in both apps (three at 1371), checked against ShareX's shots of 17 tasks (1.8-2.9%: titles' anti-aliasing and the downscaled test-pattern thumbnails, as before); keyboard selection scrolls the task's row into view. |
| Screen recording (FFmpeg, GIF) + recorder bar | ScreenRecordingLib, ScreenCaptureLib/ScreenRecording, ScreenRecordManager, FFmpegCLIManager, Presentation/ScreenRecording, Presentation/FFmpegOptions | 🟨 map: [SCREEN-RECORDING.md](SCREEN-RECORDING.md). FFmpeg recorder core (`sharex-screen-recording`): 172/172 command-line cases identical to ShareX's DLLs, 11/11 runner checks. In the app (`recording/`): Capture › Screen recording (GIF), the recorder hotkeys (start/stop toggle, stop, pause, abort), the region overlay in select-only mode, the recorder bar (FFmpeg variant, Options panel, start delay menu), the recording frame (border, toolbar, timer, pause/resume, restart, abort and its confirmation) with its own tray icon, the FFmpeg worker, the result as a file task; the FFmpeg options window (Task settings › Screen recorder); "FFmpeg is missing" offers ShareX's unwired GitHub downloader. Side by side over the test pattern with ShareX's FFmpeg: identical command lines (MP4, GIF palettegen, pause/resume concat) and outputs (container, codec, size, fps, duration); bar, frame and panels 0.00–0.12% of the screen in both themes; FFmpeg options pages 0.08–0.33% (Advanced ~0.9%: Consolas anti-aliasing). Native recorder (`sharex-native-recorder`, SCREEN-RECORDING.md §10): the worker, MF H.264/HEVC + AAC writer, D3D11 NV12 conversion, camera overlay, adaptive GIF writer, WASAPI `Drain`, the device lists and the HDR tone mapper against ShareX's own DLL headlessly (`tools/native-recorder-harness`): 65/65 outputs match (GIF, PCM, NV12, composited camera frames, drain PCM, HDR canvases, device lists byte-identical; MP4 identical but container times/chunk interleaving). Windows Graphics Capture (regions across monitors, the active window, cursor, borderless, retarget, HDR FP16), WASAPI loopback and microphone (event-driven MMCSS thread, hidden session, `IAudioClient3`), the Media Foundation camera (mode scoring, decoded fallbacks, DXGI samples) are ported and used on Windows when `ScreenRecordUseNative` (no FFmpeg needed); the native recorder bar (System audio, Microphone, Camera with their device/volume/camera panels, bitrate, validation) and Task settings' device lists; the mouse highlighter runs while recording. Not checked live yet (SCREEN-RECORDING.md §10 "Live checks"). The after capture window and FFmpeg's output box (App windows row). "FFmpeg is missing" downloads FFmpeg in the `DownloaderWindow` (`FFmpegGitHubDownloader`: `ffmpeg.zip`, `InstallType.Event` extracting `ffmpeg.exe`; tested on 127.0.0.1, `--render update-downloader --content ffmpeg-<state>`); the bar's Mouse highlighter panel (`MouseHighlighterSettingsControl` on the bar's draft, committed with it; `--render recorder-bar --content highlighter[-circle]`, no ShareX shot). Open: the native recorder checked live (blocked where it needs audio devices or a camera: [LIVE-CHECKS.md](LIVE-CHECKS.md) REC-04/05); REC-02 fixed (the recording thread waits on the FFmpeg error box as ShareX's does), live re-check pending ([LIVE-CHECKS.md](LIVE-CHECKS.md#fixes-for-the-fails-branch-fixlive-check-failures)). REC2-03a is fixed, with a live re-check pending: a forwarded `-ScreenRecorderActiveWindow` with the main window visible recorded the main window. The cause was winit's `focus_window`, which forces the foreground with a synthesized Alt press. ShareX uses Avalonia's plain `SetForegroundWindow`, which the foreground lock refuses, so the active window stayed in front. ShareXrs now uses the plain call too, which also fixes the GIF variant and `-ActiveWindow` (SCREEN-RECORDING.md §10 "Live checks"). Blocked: macOS/Linux (hardware) Upstream 30d1682: system audio that doesn't open (no output device) is left out and the recording goes on (ShareX #9288, unit-tested); the recording frame keeps its tool window style (ShareX #9242). FFmpeg options closes on Escape after its reset confirmation (#9304, checked live in both). Upstream 93145cc: the Media Foundation writer negotiates fallback media types when an encoder rejects one (#9317; the attempt list, padding and letterboxing unit-tested, a padded and a letterboxed frame on this GPU's video processor, a 1001x601 clip encoded; the fallbacks themselves need an encoder that rejects the first attempts). |
| Scrolling capture, auto capture | ScreenCaptureLib (ScrollingCaptureManager, ScrollingCaptureOptions, ScrollingCaptureService, Presentation/ScrollingCapture), Presentation/AutoCapture, TaskHelpers | 🟨 map: [SCROLLING-CAPTURE.md](SCROLLING-CAPTURE.md). Engine (`sharex_capture::scrolling`): the capture loop, frame comparison and `CombineImages` against ShareX's own DLL over 30 synthetic frame sequences (`tools/scrolling-harness`): 30/30 identical (status, frames, scrolls, every step, the image); the auto capture window's timer and status formulas identical to .NET (22 002 second texts, percentages, decimal repeat times). Windows (`scrolling_capture/`, `auto_capture/`): Capture menu entries and the four jobs; the scrolling window (toolbar, panned preview, status, options overlay), window selection through the select-only overlay, the loop on a thread with the lime region outline, stop on activation/close/hotkey, Upload / Save, Copy; the auto capture window (custom region/fullscreen, Select region..., repeat time, minimize to tray with a clock tray icon, wait for tasks, Start/Stop, status and progress) running `RunImageTask` without annotate, sounds or toasts. `--render scrolling-capture`/`auto-capture` vs ShareX, dark/light: cancelled 0.12/0.09%, options 0.25/0.25%, result 0.12/0.11%; auto capture 0.60/0.73%, fullscreen 0.60/0.74%, running 0.57% (text anti-aliasing). Side by side over `--test-pattern --scroll` (a generated scrollable page): both stitch the whole page (420x2400), copied PNG pixel-identical and CF_DIB byte-identical, equal to the page; auto capture of the same region every 2 s: same region, count and first capture, images identical but for each app's own window shadow. Open: the other scroll methods side by side, the tray icon clicked live, other DPIs (SCR-02, the preview at 125%, fixed: image pixels as DIPs; live re-check pending). As ShareX: no input to other apps outside Windows |

### Phase 2: upload
| Item | ShareX source | Status |
|---|---|---|
| Uploader framework + custom uploaders (.sxcu) | UploadersLib, Presentation/CustomUploaderSettings | 🟨 `sharex-uploaders` (map: [UPLOADERS.md](UPLOADERS.md)): HTTP layer sending requests byte for byte as ShareX's `HttpClient` (System.Uri escaping probed and ported), `Uploader` base and kinds, `UploadersConfig` typed (DPAPI secrets; fresh and ShareX-written configs round-trip byte for byte), custom uploaders in full (both JSON shapes, migrations, all 13 syntax functions, request building, response parsing). Validated with `tools/uploaders-harness` (ShareX's own DLL vs ShareXrs against local mocks: 217/217 cases identical). Custom uploader settings window (`dialogs/custom_uploaders.rs`): library (search, new, import, duplicate/export/remove, export all, clear all), editor sections (overview, request, body, response, test), key/value grids, token menus, syntax test, tests against the uploader (response window), `.sxcu` drag and drop; `--render custom-uploader --content overview\|request\|body\|response\|test` vs ShareX dark 0.34–0.52%, overview light 0.41%. The dialog functions open their windows: `{inputbox}` (`InputBoxWindow`), `{outputbox}` (`OutputBoxWindow`) and `{select}` (`ParserSelectWindow`, `dialogs/parser_select.rs`: a button per value, ten to a column, as wide as the widest, the first when closed), asked of the event loop by the parsing thread, which waits (`upload::ParserDialogs`, tested). `-CustomUploader <file>`: Command line row. Open: `ParserSelectWindow` compared with a ShareX shot (`--render parser-select`, built from source) Upstream 30d1682: Escape closes Custom uploader settings, after the syntax test (#9304, checked live in both). Upstream ca3f01f: requests use the system proxy unless the proxy is None (ShareX #9319; `proxy`: the environment, WinHTTP's Internet settings with PAC/WPAD, CFNetwork), unit-tested and end to end through a mock proxy; upload errors end with the error's text. |
| Built-in destinations (Imgur, S3, FTP/SFTP, ...) | UploadersLib, FluentFTP, SSH.NET, System.Net.Mail | ✅ every destination ShareX has is ported (`factory::tests::every_destination_is_ported`) and identical to ShareX's own code in the harness: Imgur, ImageShack, Flickr, Photobucket, Chevereto, vgy.me; Pastebin, Paste2, Slexy, Paste.ee, GitHub Gist, uPaste, Hastebin, OneTimeSecret, Pastie, PrivateBin (paste decrypted on both sides); Dropbox, FTP/FTPS (FluentFTP's command sequence on a mock server), SFTP (SSH.NET's SFTP requests; SSH-2 on RustCrypto, no C), OneDrive, Google Drive, Google Cloud Storage, YouTube, puush, Box, MEGA (chunks decrypted and MACs checked), Amazon S3, Azure Storage, Backblaze B2, ownCloud/Nextcloud, Immich, MediaFire, Pushbullet, SendSpace, Hostr, Lambda, img.fish, Pomf, Uguu, Seafile, Streamable, s-ul, LobFile, transfer.sh, Plik, shared folder, email (SMTP dialogue and MIME on a mock server); all 13 URL shorteners; Email and Pushbullet sharing (the share-page services unit tested). OAuth 2 (code paste, PKCE, Google's loopback listener), OAuth 1.0a and the logins (MEGA, Pastebin, puush, LobFile, ImageShack, Seafile) ported in `signin`/`oauth1`/`oauth2` and 18 sign-in flows identical; FTP/SFTP/SMTP cases also replayed in-process on every platform (`tests/servers.rs`). FTP: FluentFTP's listings (LIST and MLSD; Unix, IIS, VMS, NonStop, OS/400, z/OS parsers with its auto-detection), path sanitizer, server handlers, downloads, renames, recursive deletes; FTPS client certificate files as ShareX loads them. SFTP: SSH.NET's key exchanges (ECDH nistp256/384/521, DH groups 1/14/16/18, group exchange), CBC/3DES ciphers, HMAC-SHA1 and encrypt-then-MAC, every key file it reads (encrypted PEM, PKCS#8, PuTTY v2/v3, SSH.COM, OpenSSH incl. P-521), its async listing/download/upload calls. Email subjects folded as MailMessage does. 289/289 harness cases identical. No real network traffic: everything ran against 127.0.0.1 mocks. Their windows: the Destination settings row (sign-ins, pickers, the email and YouTube options windows, the remote storage browser). Blocked: uploads, sign-ins and browser redirects against the real services (accounts, network) |
| Upload file/folder/clipboard/text/URL, drag & drop, shorten URL | UploadManager, WorkerTask, TaskManager, Presentation/* | 🟨 `crates/sharexrs/src/upload` ([UPLOADERS.md](UPLOADERS.md#upload-tasks)): uploads run as ShareX's tasks (after capture "Upload image to host", file, folder, clipboard content, text, download and upload, shorten URL, share URL; destinations, uploader filters, progress into the task list's columns and the tray icon, Stop, `MaxUploadFailRetry`, `UploadLimit` queue, invalid settings, refreshed OAuth tokens written back); hotkeys, the tray and the Upload/Debug menus start them. Validated headlessly against the mock server (18 flows). The windows (`crates/sharexrs/src/dialogs`): file/folder pickers (rfd), text, URL and shorten URL windows, clipboard content viewer (reads the clipboard: text, image, files), drag and drop window, multi upload confirmation, large file warning, before and after upload windows, QR code window (ZXing's matrix, screen scan), response and errors windows; the task menu's upload entries (Open/Copy URL and formats, Upload, Download, Shorten, Share, search image, QR code, Show response/errors), the thumbnail card's progress bar, the progress in the main window title. `--render` vs ShareX, dark: 0.23–0.55% for every window and state (QR modules identical to ZXing's), light: text 0.31%, URL 0.35%, shorten 0.33%, clipboard text 0.26%. Live side by side against the mock (each app's own custom uploader, the same file and text): the same request (method, target, headers in order, body byte for byte; only the user agent, the multipart boundary and header name case differ, and a text upload's random file name), the same URL on the clipboard, the before/after upload windows (live dark 0.36/0.57%, light 0.50/0.45%), the multi upload confirmation (11 files, 0.61%) and the large file warning (0.62%), each cancelled with nothing sent. Known: the before/after upload header title is 1px higher than ShareX's. `DoFileJobs`: see the Actions and watch folders row. `ProcessImagesDuringFileUpload` (an image file through the after capture tasks), the custom browser (`BrowserPath`) for opened URLs. Drag and drop onto the main window (`drop_target.rs`): an OLE drop target of our own (winit's reports files only) reads the drag's data object with the clipboard viewer's Avalonia-style reader, accepts files or non-empty text with Copy and shows `DropOverlay` with its files or text strings (`--render main --drop-overlay[-text]`), and the drop runs `UploadManager.DragDropUpload` (files, else a bitmap as an image task, else `UploadText`); the window's own drag is ignored; behind `DropTarget`, tested with a fake; elsewhere winit's file drops stand in. Open: the drop checked live (from Explorer and a browser) and the overlay compared with a ShareX shot. Blocked: uploads to real services (accounts) |
| Actions (`DoFileJobs`) and watch folders | HelpersLib/ExternalProgram, NameParser/CodeMenuEntryActions, WorkerTask.DoFileJobs/LoadFileStream, TaskHelpers.AddDefaultExternalPrograms, RegistryHelpers.SearchProgramPath, WatchFolder, WatchFolderSettings, WatchFolderManager, MainWindow.BuildExternalActionsMenu | 🟨 `sharex_settings::external_program` (`ExternalProgram` typed in ShareX's JSON, `CodeMenuEntryActions`, `Run`: the program started as .NET's `Process` with `UseShellExecute=false` (Windows: the arguments to `CreateProcess` as written, `CREATE_NO_WINDOW` for a hidden window; elsewhere split as .NET's `Process.Unix` splits them), waited for, the output file or the input, `DeleteInputFile`) and `sharex_settings::watch_folder` (`WatchFolderSettings`, `FileSystemWatcher`'s filter matching). Against ShareX's own code headlessly (`tools/actions-harness`, Windows): 8/8 `Parse`, 18/18 `CheckExtension`, 4/4 JSON, 29/29 `Run` cases identical (the `CreateProcess` command line byte for byte, the probe's parsed arguments, the result, the files left). `DoFileJobs` in the upload worker (`upload::worker`): after capture, for file jobs and file uploads (`UseAfterCaptureTasksDuringFileUpload`): the active actions in turn (output replaces the file, the name takes its extension, the file reloaded for upload; a missing result stops the task), then copy file / file path / folder path, show in Explorer, Scan QR code (the QR code window scans the file); headless tests with script actions, and a file upload whose action output is what reaches the mock server. The task list's Run action submenu. Watch folders (`watch_folder.rs`, `notify`): ShareX's wait (1 s, then 250 ms checks until the file is unlocked with the same size above 0 four times, 5 s timeout), duplicate events within 1 s, the filter and subfolders, folders ignored; `WatchFolderManager` started at startup and refreshed when Task settings or Hotkey settings change or close; a ready file is moved to the screenshots folder if asked, then uploaded with the task settings (`GetSafeTaskSettings`, `UploadManager.UploadFile`). Against ShareX's own `WatchFolder` (`tools/actions-harness --watch`): 10/10 cases trigger the same files, each within 400 ms of ShareX's time (2050 ms for a ready file). Task settings' Actions list and editor, Watch folders editor: see the Task settings row. Open: checked live side by side (both apps' actions on the same capture, a watch folder on the same folder). Blocked: the registry search finding a program (none of the five is installed here, and installers aren't run), macOS/Linux watchers beyond CI's unit tests (FSEvents may repeat a creation; hardware) |
| After upload tasks | WorkerTask.DoAfterUploadJobs, UploadInfoParser, Presentation | ✅ after upload tasks: URL regex replace, force HTTPS, shorten, share, copy URL (clipboard content format), open URL, QR code, the after upload window (App windows row), notifications; `UploadInfoParser` and the regex replace identical to ShareX's own code on 33 cases |
| History, image history, stats | HistoryLib, SettingManager (history), TaskManager.Task_TaskCompleted, TaskInfo.GetHistoryItem | 🟨 map: [HISTORY.md](HISTORY.md). Data side (`sharex-history`, no UI): History.db read and written with ShareX's schema text and values (dates as `ToString("o")` read back as local time, .NET `Dictionary` tag order, Json.NET's tag JSON, nulls vs empty strings), `BEGIN IMMEDIATE` and ShareX's busy waits, History.json/History.xml readers and the startup migration (History.json into History.db, then into `Backup/`), both windows' filters and lists (NLS comparisons on Windows), paging, context menu rules, copy formats, import, column layout and settings, `HistoryStats` and `OutputStats` with the Stats window's formats. Against ShareX's own DLL headlessly (`tools/history-harness`): 44/44 checks identical (reads both ways, stored values and schema byte for byte, migrations, 26 filters, 8 image filters, 5 stats, formats, concurrent appends both ways, the reference build's History.db); ShareX's files are also `cargo test` fixtures. The app opens History.db at startup (after the migration) and the upload worker hands every finished task's `GetHistoryItem` (type, `UploaderHost`, URLs, window title and process name tags, end time) to `Task_TaskCompleted`, which adds it under `HistorySaveTasks`/`HistoryCheckURL`; checked through the upload tests' mock uploads, not live. `HistorySaveOCRText` adds the image's text as the `OCR` tag (`sharex-ocr`, docs/OCR.md; not checked live). The windows (`history_ui/`, HISTORY.md "The windows in ShareXrs"): History (toolbar, column list with resizers and the column menu, preview, advanced filters, context menu with ShareX's visibility rules, keys, statistics/import/settings/edit/prompt dialogs), Image history (card grid with shell thumbnails, batches, selection, menu, settings), Stats (cards, split bar, calendar, hour chart, lists, `StatsPalette`, NLS dates via `sharex_helpers::date_format`), opened from the sidebar, the tray menu and the hotkeys. Against ShareX over the same synthetic History.db, dark/light: History 0.10–0.99% in every state (a Copy › submenu that ShareX lets run past the window 2.9%), Image history 0.07–0.94%, Stats 0.47–0.85%; live windows equal their renders. Side by side: the same clipboard text (Ctrl+C, Ctrl+Shift+C, Markdown linked image) and, after the same tag edit, favourite, delete and item edit, the same History.db rows. Since (HISTORY.md, "The windows in ShareXrs"): the selected files dragged out of both lists, URL-only images downloaded for the preview (`sharex_uploaders::http`, a plain client's GET, cancelled on another selection; tested on 127.0.0.1), both windows' `RememberWindowState` (size, place, maximized), the search boxes' process name suggestions (`AutoCompleteBox`, metrics from `tools/editor-harness --control-dump autocomplete`), the advanced filters' DatePicker flyouts (`--control-dump datepicker-presenter`), Stats' chart hover tooltips and marks, row tooltips and Share (the page rendered offscreen as an image task). Settings export/import (History.db included) is the Application settings row's. The date picker's flyout shows the hovered column's up/down `RepeatButton`s (`--control-dump datepicker-presenter-hover`; 300/100 ms repeat). Every file service runs but the Photo editor (⏸). Blocked: NLS behaviour off Windows (hardware) |

### Phase 3: tools (`MainMenuBuilder.BuildToolCategories`)
| Category | Tools | Status |
|---|---|---|
| Screen | Color picker, Screen color picker, Ruler, Classic ruler, Mouse highlighter, Pin to screen | 🟨 all six (`screen_tools/`, docs/TOOLS.md "The Screen tools in ShareXrs"), from Tools, their hotkeys, the task list and history Pin to screen, the after capture task and the command line. Overlays over `--test-pattern` vs ShareX: screen color picker 0.04%, Measure 0.01–0.04%, Ruler 0.13–0.16%, pins 0.00–0.05% (pinned pixels identical), highlighter circles identical; windows (dark/light) colour picker ~1.0% (text), pin startup ~1.0/0.92%, pin options 0.60%, highlighter 0.60/0.42%. Side by side: the same copied colour, measurements and ruler text. Outside Windows, as ShareX: the screen color picker completes with no colour and the highlighter stops with an error (PLATFORMS.md). The highlighter's colour buttons open a `Flyout` with Avalonia's `ColorView` with alpha (`MouseHighlighterSettingsControl`, shared with the recorder bar; the flyout at 125% 0.09% from ShareX's live shot), the pin options' a `Flyout` with its defaults. ShareX's tooltips with the shared Fluent tooltip (were egui's): the color picker's Pick from screen/clipboard, the pin's Copy image, Reset scale and opacity, Options and Close, the pin options' Border color and Background color (built from source). Open: the highlighter while recording checked live, the settings JSON compared live, other DPIs |
| Images | Image editor, Photo editor ⏸, Pixel editor ⏸, Image effects, Beautifier, Combiner, Comparer, Converter, Resizer, Splitter, Thumbnailer, Viewer, Watermark, Icon converter, Background remover | 🟨 Image effects engine (`sharex-image-effects`, docs/IMAGE-EFFECTS.md): all 51 effects, presets in ShareX's JSON (settings and export forms, every default and round trip equal to ShareX's), `.sxie`; against ShareX's own DLLs headlessly (`tools/effects-harness`), 117/121 cases byte-identical, the rest: Text watermark glyphs (DirectWrite), ±1 under a gradient. Image editor rendering (`sharex-image-editor` on `sharex-skia`, docs/IMAGE-EDITOR.md): annotation model and `ImageEditorOptions`, rectangle, ellipse, line, arrow, freehand, highlight, blur, pixelate, magnify (rotated too), spotlight, text, step, speech balloon, image, clone, cursor (ShareX's bundled cursors, Windows' own), smart eraser, emoji (Segoe UI Emoji's COLRv1 painted), drop shadows (steps, balloons, text and images too, rotated or not), region capture's `GetSnapshot(Rect)` (picture shader: pipeline blitters); `EditorCore` with its history and image operations (crop, cut out, rotate, flip, resize, resize canvas, auto crop, flatten, auto-expand, clone); against ShareX's editor headlessly (`tools/editor-harness`), 154/165 cases pixel-identical, the rest: step/balloon glyphs (DirectWrite), emoji (Direct2D), one level on one pixel of a shadow at 170°; drop shadows under a rotation filtered in Skia's unrotated layer space and drawn back resampled; step and balloon tails through `sharex_skia::pathops` (Skia's PathOps, every result path bit-identical to SkiaSharp's on 264 cases, `tools/pathops-harness`). Editor workspace (`sharex_image_editor::editor`) and region capture hosting it: see the Region capture row. Image effects window (`image_effects/`, docs/IMAGE-EFFECTS.md): Tool, Presets and Editor modes, presets and effects editing, the catalog's Add menu, the options panel with an editor per property kind (incl. the gradient flyout), the threaded preview, the confirmation and packager windows, `.sxie` drop import; from Tools, its hotkey, the task list, Task settings › Effects and the after capture task (the task waits); vs ShareX dark/light: tool 0.29/0.24%, Text watermark options 0.44/0.33%, presets on the sample image 0.24/0.21%; the same preset edits give byte-identical `ImageEffectPresets`. Image editor window (`image_editor.rs`, docs/IMAGE-EDITOR.md "The editor window"): `EditorWindow` with the built-in toolbars (File menu, every tool, crop, cut out; zoom, Continue/Cancel, copy, save, save as, pin, upload), popups, context menu, exit confirmation, zoom and pan; from Tools, its hotkey, the after capture Annotate image task (the task waits), the task list's and history's Edit image and `-ImageEditor`; vs ShareX on the same image dark 0.10%, light 0.09%, context menu 0.68%; the same edits in both editors export a pixel-identical image (CF_DIB byte-identical). Its dialogs (`image_editor/`, docs/IMAGE-EDITOR.md "The dialogs and the rest"): start screen, New image, Insert image, emoji picker, exit confirmation, Canvas size, the editor's notifications, recent files, New from screenshot, the colour popups' screen colour picker, drop, auto-copy, auto-expand, window state, auto-close; vs ShareX dark/light 0.05-0.22% (confirmation 0.56/0.57%: tool state), File menu as a `MenuFlyout` 0.21/0.35% of the window; an Insert below in both editors copies a byte-identical CF_DIB. Background panel (sliders, smart padding, ratios, gradient/colour/image/wallpaper; its export against ShareX's own code in the editor harness: 33/33 identical, annotations drawn in the frame through its anti-aliased clip as ShareX draws them; panel 0.06-0.16% dark, 0.12-0.13% light), toolbar customisation (Task settings › Image editor and Region capture: ShareX #9278's drag-and-drop mock toolbar, see Upstream 93145cc below; the editor and region capture follow the saved layout, spacers included, saved as ShareX saves it), Print (Print options dark 0.64%, light 0.78%, `RenderImagePage` identical to ShareX's, printed to PDF end to end), `AutoSizeWindow` (window rectangles identical to ShareX's for a normal and a maximized size), the live text canvas growth (docs/IMAGE-EDITOR.md, "The Background panel" and after). Image effects panel (`image_editor/effects.rs`, docs/IMAGE-EDITOR.md "Image effects"): the effect browser (search as `EffectSearch`, Recent and Favorites with right clicks, the favourites menu) and every effect's dialog in the right sidebar with its live preview, applied with undo; vs ShareX dark/light: browser 0.41/0.24%, search 0.13/0.11%, nine dialogs covering every parameter kind and category 0.05-0.08% (ASCII art 1.5%: its preview's glyphs), favourites menu 1.59% of the menu; 20 effects applied side by side give 20/20 pixel-identical copies and byte-identical CF_DIBs. Region capture has no image effects in ShareX. The editor's own image effect catalogue (`sharex_image_editor::effects`, docs/IMAGE-EDITOR.md "The image effect catalogue"): all 230 effects of `Core/ImageEffects` with their parameters, defaults and browser order, preview and apply with undo through `EditorCore`; against ShareX's editor headlessly (`tools/editor-harness --effects`), 1174/1202 cases pixel-identical, 223/230 effects in every case, the rest: `DrawText` glyphs (DirectWrite) and Fake hair (seeded from the clock in ShareX). `-ImageEffect <file.sxie>` unpacks the package and imports its preset into the Presets window singleton (`OpenImageEffectsSingleton`), then asks to turn on Add image effects; the `.sxie` and `.sxcu` associations are registered when missing at startup (`FileTypeRegistration`) behind `integration::Registry` (tests use an in-memory registry, never the real one); `-ImageEffects <file>` opens the window on the file. Tool windows (`image_tools/`, docs/TOOLS.md "The Images windows in ShareXrs"): combiner, splitter, thumbnailer, resizer, converter, watermark, comparer (with `ImageComparisonSlider`), icon converter, from Tools, their hotkeys and the task list's Resize image / Combine images; every state vs ShareX 0.10–0.44% dark, 0.10–0.33% light (text anti-aliasing); run side by side on the same inputs their outputs are byte- or pixel-identical (watermark glyphs aside) and `ImageCombinerOptions` is saved the same. Tool engines (`sharex-tools`, docs/TOOLS.md): combiner, splitter, thumbnailer, resizer, converter, watermark, icon converter, comparer, with ShareX's image I/O (libjpeg-turbo's encoder and decoder ported, Skia's BMP/GIF/PNG pixel paths, its GIF quantizers, WebP through the very libwebp SkiaSharp compiles in: `libwebp-sys` 0.14.4, all 112 source files matching libSkiaSharp's PDB, driven as Skia's `SkWebpEncoder`); against ShareX's own code headlessly (`tools/tools-harness`): 611/616 cases byte- or pixel-identical (all 68 WebP files byte for byte, lossy and lossless), 5 close (text watermark glyphs). Background remover engine (`sharex_tools::background_remover`, ONNX Runtime as the OCR loads it; models are the user's, none bundled): against ShareX's own service on the CPU over four synthetic models, 12/12 results byte-identical; Auto runs on the CPU (no DirectML), GPU fails with its error (logged, as ShareX's `ReportWarning`). Its window (`image_tools/background_remover.rs`, docs/TOOLS.md): the models folder's list with the saved model, Refresh, Open models folder, the pulsing guide button while there's none, the device, Remove Background on a thread, the comparison slider, Save (over a PNG/WebP source) / Save as (PNG, WebP), the toast, drop, options saved into the task's `ToolsSettingsReference`; from Tools, its hotkey and `-BackgroundRemover`; vs ShareX on the synthetic models dark/light: empty 0.08/0.06%, image without model 0.07/0.05%, ready 0.08/0.06%, compared 0.26/0.24% (the toast caught mid-animation in ShareX's shot), model menu 0.79/0.25% inside its edge, empty model menu 0.00% (light popup edges come out over black in ShareX's capture); busy, saved, divider held and device menu built from source, no shot. Beautifier (the image editor's Background panel, docs/IMAGE-EDITOR.md): 33/33 exports pixel-identical; the Image beautifier (`TaskHelpers.OpenImageBeautifier`: the editor with its Background panel open, the result through `RunImageTask`) from Tools, its hotkey, `-ImageBeautifier <file>`, the task list's Beautify image and the Beautify image after capture task (task mode, the task waits); vs ShareX dark/light: opened 0.16/0.12%, 16:9 picked 0.15/0.12%. The editor's and Image effects window's saves are `SkiaImageHelpers.SaveImage` / `SaveImageFileDialog` (the five types, byte for byte as ShareX encodes them). The previews of the converter, resizer, watermark and Image effects windows open the image viewer. ShareX's tooltips with the shared Fluent tooltip (were egui's or missing): the background remover's model, Refresh models, Open models folder, Open guide page and device (5), the image converter's WebP effort, PNG compression level and JPEG subsampling hints on each label and its control (6) (built from source). Open: the background remover's busy, saved, divider held and device menu states and the Image effects window's preview click compared with ShareX shots (built from source). Photo/Pixel editor ⏸; the Viewer is in the App windows row Upstream 30d1682: the editor's top toolbar has ShareX's drag handle, moves clamped inside the view, remembers `TopToolbarOffsetX/Y` and resets on a double click (#9296), and the bottom bar lost its New from screenshot buttons (45fe7f2): 125% renders vs the new ShareX 0.41% docked, 0.33% moved; at 125% the toolbar is 2 px narrower than ShareX's (per-element round-up, the open 125% work). Image effects: a check box per flag (Sides, #9287; options panel 0.35%), Escape closes the window and the packager (#9304, checked live in both). Upstream 93145cc: Customize toolbar is ShareX #9278's mock toolbar (drag items and spacers along it, onto it and off it, resize a spacer by its grip, Spacer width, Show spacer line, Move left/right); 125% renders of eight states vs ShareX's 0.73-1.07% of the dialog (text anti-aliasing, icons in the shrunk row); the same drags in both apps live give the same dialog (0.89-1.02%) and the saved spacers draw in the editor's toolbar; hidden tools keep their shortcuts (60a85ea, checked live in ShareXrs). Upstream ca3f01f: the item panel shows only the hotkeys (#9324); the dialog vs ShareX ca3f01f 1.57-1.79% (the tile labels' weight and text anti-aliasing). Upstream 099bcb6: a toolbar spacer is selected by a click, a resize or a drop and gets its own panel with a Spacer width box (#9327); the dialog vs ShareX 099bcb6 0.65-0.92%, the same steps live give the same dialog and saved `ToolbarItems`. |
| Video | Animated GIF maker, GIF trimmer, Video converter, Video player, Video thumbnailer, Video trimmer | 🟨 engines (`sharex-tools::video`, docs/TOOLS.md): the converter's and trimmer's FFmpeg command lines (498 cases identical) and output info (10/10), the thumbnailer (4 runs on ShareX's FFmpeg: 2 identical, 2 close by text glyphs), the GIF maker (24/24 byte-identical), the GIF trimmer's lossless cuts, re-encodes and timing; the GIF export and video trim are now cancellable. Windows (`video_tools/`, docs/TOOLS.md "The Video windows in ShareXrs"): all six, from Tools, the four hotkeys, `-VideoConverter`/`-VideoTrimmer`/`-AnimatedGifTrimmer <file>` and the task list's and history windows' Trim video / Trim animated GIF; `CheckFFmpeg` where ShareX checks. Playback (ShareX: Media Foundation, Windows only) on FFmpeg pipes and cpal (Apache-2.0) on every platform. Against the `tools-video` shots, dark/light: every UI state 0.09–0.85% (text anti-aliasing), the video picture and Media Foundation's error texts differing where video shows (player paused 6.4%, trimmer 1.5–4.6%); thumbnailer dropdowns 0.9–4.2% (the popup capture's dark edge). Real runs headlessly: a conversion, a contact sheet, a lossless trim, GIF exports. Live side by side ([LIVE-CHECKS.md](LIVE-CHECKS.md) VID-01..11): converter, GIF trimmer, trimmer and GIF maker outputs byte-identical, the player's pause/seek/fullscreen; the thumbnailer within glyph differences. ShareX's tooltips with the shared Fluent tooltip (were missing): the video trimmer's file path, previous/next frame, Play/Pause, Set start/end, Reset, Lossless/Precise hints, Show output (10); the GIF trimmer's path, transport, Set start/end, Reset, Reset crop, Show output (9); the player's Open, transport, Mute/Unmute, Volume and Full screen/Exit full screen (7) (built from source). Open: the states the reference batch didn't capture (TOOLS.md), the player's sound, live re-checks of VID-02 (dropdowns past the window's edge, now OS popups) and VID-04 (FFmpeg's failure mapped to Media Foundation's text and code), both fixed. Accepted: the contact sheet's text (VID-11: DirectWrite glyphs). Blocked: macOS/Linux playback (hardware) |
| Data | Analyze image, Hash checker, Index folder, Metadata, OCR, QR code, Remote storage browser | 🟨 engines (`sharex-tools`, docs/TOOLS.md), all identical to ShareX's: hash checker (6), QR generate/SVG (60) and scan (21), index folder in all four outputs (75), metadata read, copy text and strip (17 formats). OCR engine (`sharex-ocr`, docs/OCR.md): ShareX.OCR's whole pipeline on the same PP-OCRv6 models and ONNX Runtime CPU provider, with Skia's Catmull-Rom and linear-mipmap resampling ported into `sharex-skia`; against ShareX's own DLL headlessly (`tools/ocr-harness`, 38 synthetic cases: fonts, colours, layouts, rotations, 9 languages, a UI screenshot) 38/38 identical at every stage (detection input, probability map, boxes, crops, raw texts and scores, lines, text); wired into the after capture OCR task and `HistorySaveOCRText`. OCR window (`ocr_window.rs`): image with region boxes (hover, click to copy), scan line, Select region/Select image, result text, options saved to `OCROptions`, services and their editor, Copy all, `<file>.txt`; from Tools › OCR, the OCR hotkey, task list › OCR image and the after capture task; vs ShareX on the same image dark/light: 1.17/1.14% (live 1.17%), services 0.79/0.62%, no text 0.28%; Copy all text and `<file>.txt` byte-identical. Analyze image engine (`sharex-tools::analyze_image`, docs/TOOLS.md): AIOptions, the four providers' requests and answers, test connection and the model list; against ShareX's own `AnalyzeImageService` on a mock (`tools/tools-harness --analyze`) 107/141 identical, 34 identical but for the PNG bytes, ShareX's leak of the OpenAI key to other hosts not ported. Windows (`data_tools/`, `dialogs::qr_code`, TOOLS.md "The Data and System windows in ShareXrs"): Hash checker, Index folder (Save as, Upload; the Index folder hotkey and clipboard auto-index upload an index), Metadata (search, links, Copy all, Strip behind its confirmation; StripMetadata job), Analyze image and its options (analysis, Load models and Test connection off the UI thread), the QR code window now on `sharex_tools::qr` (ZXing's encoder: the reference text's mask differed with the `qrcode` crate) with region scan and the scan hotkeys, OCR's wheel zoom, drag to pan, double click to fit and the canvas/sheet splitter; Tools menu, hotkeys, `-HashCheck`/`-Metadata`/`-StripMetadata <file>`, `-AnalyzeImage`, `-QRCode [file]`, `-IndexFolder`, the task list's/history windows' Analyze image and the after capture task. `--render` vs the `tools-data` shots, dark/light: hash checker 0.16–0.62/0.11–0.41%, index folder 0.34–0.35/0.28%, metadata 0.21–0.44/0.16–0.36%, Analyze image 0.08–0.47/0.23–0.38%, its options 0.33–0.42/0.26–0.33%, the combo popups 0.83–1.36/0.09–0.82%, QR code 0.19/0.14–0.21% (a 4000-character paste 6.18/5.67%: wrapped lines at egui's whole-pixel line height); the blocked states (Analyze result/API error/analyzing, loaded models, connection result, index output, stripped, unsupported) rendered from synthetic data, no shot. No provider is ever called by tests or renders (the analyzer is injectable). Live ([LIVE-CHECKS.md](LIVE-CHECKS.md) DAT-01..06): the same hash and stripped bytes. Fluent tooltips (`sharex_ui::tooltip`: ToolTipService's 400/100 ms, pointer placement, wrapped at 320; none on a disabled control) where ShareX's `.axaml` and code-behind set them, all from the `.resx`: Analyze image options' API key page (Get API key), OCR's Open service and Edit services, its regions (the region's text at once under the pointer, Copied to clipboard after a click: `OCRPreview.SetHoveredRegion`/`ShowCopied`), the Remote storage browser's Up, Refresh and search box; ShareX sets none in Hash checker, Index folder, Metadata, Analyze image or QR code. `--render ... --hover X,Y --tip` holds the pointer past the delay (analyze-options, ocr, remote-storage: built from source, no ShareX shot of a tooltip in these windows; the existing states unchanged, OCR region hover still 1.16/1.12%). Open: live re-checks of DAT-02 (`{0:N0}` formatted as .NET does) and DAT-05 (the 125% column rounding: 417 px), both fixed; Index folder's title rechecked live (DAT-04a: the picked folder now keeps Avalonia's trailing separator, so the HTML title's folder name is empty as ShareX's). Blocked: an index uploaded, Analyze against a provider (network). Remote storage browser (`remote_storage_browser.rs`, UPLOADERS.md): the window on `sharex_uploaders::remote_storage` (25 provider cases identical), every call on a worker thread, from Tools › Data and Destination settings; not configured: the message box, 0.99/0.75% dark/light (text anti-aliasing); the populated window is built from source (`--render remote-storage --state ...`), no shot; tested on an in-memory provider and the FTP mock Upstream 30d1682: the OCR window takes Escape only without modifiers (#9276; not checked live: it needs a region capture). |
| System | Borderless window, Clipboard viewer, Inspect window, Monitor test, Network monitor | 🟨 Monitor test patterns (`sharex-tools::monitor_test`): 33/33 pixel-identical to ShareX's control rendered by Avalonia.Headless. Data side of the other four (`sharex_tools::system`, docs/TOOLS.md): clipboard previews (28/28 identical to `ClipboardViewerData`, DIBs through `ImageFromClipboardDib` and Skia's BMP path), Inspect window details and `WindowStyles` texts as .NET formats them (50/50), Network monitor's probe answers, transitions, samples, texts and log, Borderless window's settings and toggle (unit-tested behind traits). Windows (`system_tools/`, TOOLS.md "The Data and System windows in ShareXrs"): Clipboard viewer (the clipboard's OLE data object read as Avalonia's Win32 backend names and reads its formats), Inspect window (window list, Pick window/control through a translucent overlay per screen, details, top-most and opacity, Copy all), Monitor test (borderless/topmost on the pointer's screen, five modes, moving bars, keys/clicks/wheel), Network monitor (probing from open with ShareX's confirmation, cards, chart, events, `Logs/NetworkMonitor.log`; `IcmpSendEcho`; on macOS/Linux .NET's Unix `Ping`: a raw or datagram ICMP socket, else the system `ping` with .NET's arguments, `sharex_tools::system::unix_ping`), Borderless window (Toggle through the session's manager), the Make active window borderless/top most hotkeys; Tools menu, hotkeys and `-ClipboardViewer`/`-InspectWindow`/`-MonitorTest`/`-NetworkMonitor`/`-BorderlessWindow`. The native side sits behind `ClipboardSource`, `WindowSource`, `Pinger`, `WindowOps`: tests and renders use fakes and synthetic data, never the real clipboard, other windows or the network. `--render` vs the `tools-system` shots, dark/light: clipboard viewer 0.16–0.38/0.11–0.28% (image 0.81/0.71%: .NET's serialised bitmap bytes stood in for), inspect window 0.16/0.11%, monitor test 0.24–0.61/0.20–0.58% (motion 1.94/1.90%: a fractional bar offset), its popups 1.35–1.71/0.56–0.97%, borderless window 0.42/0.33%; Network monitor (never captured: it pings on open), the inspect window's selections and picker, borderless toggles rendered from synthetic data, no shot. Live ([LIVE-CHECKS.md](LIVE-CHECKS.md) SYS-01/03): the clipboard viewer's Unicode text and the monitor test's start. The monitor test's colour buttons open `ColorView` (320x320) in a `Flyout` (Shared controls row). Fluent tooltips (`sharex_ui::tooltip`: ToolTipService's 400/100 ms, pointer placement, wrapped at 320; none on a disabled control): Monitor test's Close, the only one ShareX sets in these five windows (`--render monitor-test --hover 837,52 --tip`, built from source). Clipboard viewer outside Windows (`sharex_tools::system::clipboard_formats`, PLATFORMS.md): every format Avalonia 12.1.6 reports, in its order, named and read as Avalonia.Native (`[NSPasteboard types]` through `ConvertPasteboardTypes`, read from `libAvaloniaNative.dylib`) and Avalonia.X11 (`TARGETS`, the text targets folded into one, `Bitmap` added for PNG/JPEG) do, behind `Pasteboard`/`Selection` traits tested with fakes (10 tests); the X11 reader (`system_tools/x11_selection.rs`, x11rb) is type-checked on Windows by the tests. Open: checked live (a picked window's details against ShareX's, opacity/top-most and a borderless toggle on a test window; the clipboard viewer on macOS and Linux), the colour flyout against a ShareX shot. Blocked: real pings (network). As ShareX: window lists, picking and borderless are Win32 only |

### Phase 4: settings windows and the rest
| Item | ShareX source | Status |
|---|---|---|
| Shared form controls (`sharex_ui::controls`) and live theme options | ShareX.Avalonia/Theming (ControlStyles, AccentPalette, ThemeManager), Avalonia.Themes.Fluent 12.1.6 templates | 🟨 Button (+`.compact`, `.primary` with `SystemAccentColorLight1/Dark1`), CheckBox, ToggleSwitch, ComboBox with its dropdown (items sized to the selected one, accent selection, light dismiss, wheel), TextBox (placeholder as ShareX's `TextBlock`), NumericUpDown (spinner repeat, `DecimalUpDown` for `Increment` 0.1 and the like, spin buttons disabled at the limits), Slider, `ColorPickerDropdown`'s button, Avalonia's `ColorPicker` button, `ListBoxItem` states with ShareX's accent selection, the indeterminate `ProgressBar`, Normal-weight combo items and Fluent's 64px combo `MinWidth`, `DecimalUpDown::show_with` (`FormatString`, no spinner; a press and release within one frame steps once), cards; rest/hover/pressed/disabled from the templates (disabled check boxes as ShareX styles them; text box and number box text at Fluent's 14px `ControlContentThemeFontSize`, which also brought Application settings' upload page to 0.23% and proxy to 0.20%). Menus: `MenuStyle::Standard` for plain `ContextMenu` items (the task pickers), popups without egui's fade-in. `AccentPalette` ported (reproduces ShareXTheme.axaml's default accent colours exactly, unit-tested); accent, spacing (per area) and icon pack are app-wide and apply live to every window; `UseSystemTheme` follows the OS (winit `ThemeChanged`), `UseSystemAccentColor` reads Windows' accent (`Explorer\Accent\AccentPalette`). Legacy icons that use `currentColor` are now drawn in their foreground. `MenuIcon::Accent`/`Plain` sizes for icons Fluent's 16x16 icon `Viewbox` scales down. Dropdowns follow Fluent's for long lists: `MaxDropDownHeight` 504, the selection scrolled into view on open, as wide as the items in view (the time zone dropdown 1.29/1.85%); menu items can carry a tooltip (`MenuEntry::tooltip`); Avalonia's `ColorView` (`sharex_ui::color_view`, ported from `Avalonia.Controls.ColorPicker` 12.1.6's Fluent theme and code: the tab strip, the spectrum tab's `ColorSpectrum` box (`HueSaturation`) between its value and alpha `ColorSlider`s, the `FluentColorPalette` tab, the components tab (RGB/HSV toggles, the hex box with the alpha trailing, a label, number box and slider per component), the `ColorPreviewer` with its accents; the colour maths with .NET's rounding, unit-tested; Avalonia's layout rounding at any scale, sizes up and offsets half to even) in ShareX's three hosts: `color_popup` (`ColorPickerDropdown`'s `ColorPickerPanel`: the palette tab, the screen colour picker button, the recent colours), `color_popup_with_alpha` (a `Flyout`), `color_popup_styled` (`ColorPopup::Flyout`, with the Monitor test's 320x320; `ColorPopup::Picker`, Avalonia's `ColorPicker` flyout above its button); `--render color-view` vs ShareX: the mouse highlighter's flyout at 125% 0.09% (the stand-in's live shot was 84.88%: the tab icons' anti-aliasing is what's left), the editor's colour dropdowns dark 0.05-0.07%, light 0.09-0.10% (were 0.06-0.09%, 0.09-0.15%). Keyboard focus is shown with `sharex_ui::focus` (Tab/arrows show it, a click hides it; `--render controls-gallery`). Open: `ColorView`'s components tab, the image tools' `ColorPicker` flyout and the Monitor test's 320x320 view against ShareX shots (none captured yet), its palette tooltips (`ColorToDisplayNameConverter`) and keyboard steps, the screen colour picker button of the Application settings, start screen and Task settings dropdowns (it closes the popup, `color_view::take_screen_pick`; those windows have no picker route yet), Fluent's expanding scroll bar (a thin indicator), dropdowns past the window's edge (in progress separately) ColorView (`color_view`): every tab compared with Avalonia's own headless renders (`tools/editor-harness --control-dump colorview`): spectrum 0.11%/0.13%, palette 0.10%/0.68%, components 1.35-1.57% after giving its number and hex boxes ShareX's `MinHeight` 40 (HIL2-02). Task message boxes wait on the task's thread (`TaskUi::message_box`), helpers without a window use `message_box::show` (APP2-02b/c, DROP3-03). |
| Start screen (welcome) | Presentation/StartScreen | 🟨 shown before startup while `ShowStartScreen` (not `-sandbox`), capped to the work area as `FitToWorkingArea`; every choice applies live (theme tiles, accent presets, colour picker, follow system accent, icon pack, spacing, language at once, run at startup); closing it clears `ShowStartScreen`/`ShowIconPackPrompt`, saves and goes on. `--render start-screen` vs ShareX's fresh profile: dark top 1.86%, light+indigo+Legacy 2.35%, light bottom 2.83% (the logo is ShareXrs's icon, ~0.5%; a few labels within 1px; content 765 vs 767 tall). ShareX opens it scrolled 31px (focus brought into view); ShareXrs opens at the top. Checked live side by side: Light, #6366F1, Legacy, Get started, then Compact in Application settings give the same `ThemeOptions`, `ShowStartScreen` and `ShowIconPackPrompt` in both apps' ApplicationConfig.json. The icon pack prompt is ported (App windows row): a fresh profile never shows it (the Start screen clears it) |
| Command line, single instance, shell integration, browser extension host | HelpersLib/CLI (CLIManager), ShareXCLIManager, Infrastructure/ApplicationCommandLine, SingleInstanceCommandRouter, HelpersLib/SingleInstanceManager (MutexManager), TaskHelpers (ExecuteJob with a file, ImportCustomUploader, ImportImageEffect, HandleNativeMessagingInput), IntegrationHelpers, FileTypeRegistration, ShareX.NativeMessagingHost (HelpersLib/NativeMessagingHost, host-manifest-*.json) | 🟨 `sharex_helpers::cli` reads the command line as ShareX's `CLIManager` (only `-`, the next argument as a parameter, ordinal case, an empty argument stops it; `ParseCLI`): 38/38 cases identical to ShareX's own DLL (`tools/cli-harness`). `cli.rs` runs every option as `ShareXCLIManager` does: `-CustomUploader` (ShareX's add/activate question), `-ImageEffect` (the package unpacked, its preset imported into the Image effects window, Images row), `-<HotkeyType> [file]` with the default task (`-OCR` file opens the OCR window, `-QRCode` file scans it, `-FileUpload` file uploads it, `-ExitShareX`; `-ImageEditor`, `-PinToScreen`, `-ImageViewer`, the video and data tools with a file open on it; only `-PhotoEditor`/`-PixelEditor` log, ⏸), `-workflow`, `-task`, `-NativeMessagingInput`, files/folders/URLs to upload; startup options `-silent`/`-s`, `-portable`/`-p`, `-sandbox`, `-multi`/`-m`, `-NoHotkeys`, `-AutoClose` (exit once no task is busy). `-uninstall` does nothing, as in ShareX's non-Steam builds. `sharex_helpers::single_instance`: ShareX's mutex and named pipe (its `BinaryWriter` message, byte-identical; current-user ACL; `AllowSetForegroundWindow`), a lock file and Unix socket on macOS/Linux (beyond ShareX, whose pipe server throws there: PLATFORMS.md); later launches' command lines run on the event loop once startup is done (5 s limit). The installed copy's mutex is the installer's AppId (its `AppMutex`); a portable copy's mutex and pipe are keyed by its folder (ShareX's aren't). Checked by `tests/single_instance.rs` on a portable scratch copy (second launch forwards and exits, the host's message reaches the first copy, `-ExitShareX`). `integration.rs`: `IntegrationHelpers` under ShareXrs's names (docs/RELEASING.md), `.sxcu`/`.sxie` registered at start unless portable, without taking ShareX's over; Application settings › Integration's check boxes work; tested in memory and on the real registry under `HKCU\Software\ShareXrsTest`. `sharexrs-native-messaging` (ShareX's host: framing, echo, temporary `.json`, app started out of the browser's job), its input read as ShareX's Json.NET reads it (72/72 identical). Open: the file type icon (the program's own). Blocked: the browsers themselves (no extension asks for ShareXrs's host name), macOS/Linux single instance on a real desktop (hardware) |
| Run at startup | ShareX/StartupManager.cs | ✅ Windows as ShareX: `ShareXrs.lnk` in the Startup folder (exe, `-silent`, its folder), disabled by user via `StartupApproved\StartupFolder`; checked live against ShareX's `ShareX.lnk`. macOS (LaunchAgent `com.sharexrs.app.plist`) and Linux (XDG autostart entry) written (beyond ShareX, whose Startup-folder shortcut fails there: PLATFORMS.md). `-silent` starts in the tray. Blocked: the macOS and Linux entries on a real desktop (hardware) |
| Application settings | Presentation/ApplicationSettings, ShareX.Avalonia/Controls/SettingsNavigation, SettingsSearch | 🟨 window (density width, live resize with the spacing), navigation, search (pages, cards and combo options as `SettingsSearch`), restart banner, all 12 pages with every control bound to ApplicationConfig (Print and Proxy through their JSON), the clipboard format list and editor, the spacing preview; opened from the sidebar and the tray menu; applied and saved on close (`ApplicationSettingsRuntime.Apply`: tray, hotkey repeat limit, task view, menus). `--render settings-<page>` vs ShareX, dark/light: general 0.56/0.45%, theme 0.37/0.28%, integration 0.35/0.25%, paths 1.21/1.00% (its folder previews show other paths), settings 0.38/0.27%, main window 0.36%(light), clipboard formats 0.22/0.16%, upload 0.42/0.37%, history 0.40/0.34%, print 0.20/0.15%, proxy 0.39/0.34%, advanced 0.48/0.39%; compact + indigo + Legacy theme page 0.51%; Theme combo dropdown 0.71%. Live: sidebar opens it, pages, compact mode, startup. [SETTINGS-WINDOWS.md](SETTINGS-WINDOWS.md): export, import and reset of the settings (`SettingManager`'s `.sxb` zip through `sharex_helpers::zip_manager`, checked against ShareX's own `ZipManager` by `tools/settings-harness`: 9/9 extractions and the export's layout identical) with the busy banner and the reset question (export options 0.32/0.24%, none selected 0.30/0.21%, import busy 0.32/0.42%, reset confirmation 0.39/0.20%), the sub folder pattern and clipboard format variable menus (0.24–0.50%), the white tray icon, the Explorer/browser check boxes behind `IntegrationBackend` (mocked in tests); the scrolled parts compared: theme 0.22–0.30%, advanced 0.42–0.51% (other pages fit or had no scrolled shot). The clipboard format menu's items have their descriptions as tooltips. Open: live re-checks of SET-08a (the white icon in every window), SET-04a (a quick click no longer closes the variable menu it opened) and SET-05a (the right-click registers; Escape closes the menu first), all fixed ([LIVE-CHECKS.md](LIVE-CHECKS.md#fixes-for-the-fails-branch-fixlive-check-failures)). Blocked: the dev build installer (installers aren't run here) Upstream 30d1682: Escape closes the window after the clipboard format editor (#9304), and Save needs a description and a format (#9305: `--render settings-clipboard-formats --state add,description-empty` vs ShareX at 125% 0.90%, the error row 0.24%); checked live in both. |
| Hotkey settings | Presentation/HotkeySettings (window, view model, models, HotkeySettingsAvaloniaService) | 🟨 `hotkey_settings.rs` over the real `HotkeysConfig` and `HotkeyManager`: toolbar (Add, Remove, Edit task, Duplicate, move up/down wrapping, Reset with its confirmation), the list with the shared hotkey column, task picker (`MenuStyle::Standard`, `None` then a submenu per `HotkeyType` category, radio items, `FindMenuIcon`), the row's Task settings button, the "custom settings" mark, recording a hotkey (cleared on start, modifiers shown while held, Escape clears, Print Screen on key up, modifiers only dropped, a click elsewhere stops), every change re-registered at once with failed hotkeys retried, statuses, the disabled-hotkeys banner and Enable hotkeys, the empty state; opened from the sidebar and the Workflows menu; closing saves `HotkeysConfig.json` and the main window's hotkey panel follows. `--render hotkeys` vs ShareX (dark): rest 0.47%, selected 0.55%, recording 0.55%, reset confirmation 0.56%, task picker root 0.44% / hovered 0.54% / Upload submenu 0.90%; light selected 0.43%, light rest 0.25%. Live, side by side: row 2 set to Open main window with Ctrl+Alt+F9 in both apps gives a byte-identical `HotkeysConfig.json` (with the Task settings checks below); the new hotkey fires in ShareXrs (main window hidden, hotkey pressed, main window back). Tooltips (move up/down, the custom settings mark, Edit task settings, the status) and `FitMinWidthToToolbar` (the minimum width grows to fit translated toolbar labels; tested). The task picker's levels are popups placed by Avalonia's positioner on the screen: a submenu past the window's edge is its own OS popup window on Windows instead of flipping or sliding to stay in the window (`--render hotkeys --click 200,112 --open 2 --popup-host … --client-origin …`; in-window renders unchanged; live check pending). Open: Remove/Duplicate/Move, the disabled banner and the Win key checked live (unit tests and renders only). Blocked: macOS/Linux key codes (hardware) Upstream 30d1682: Escape closes the window after the reset confirmation; a hotkey being recorded still takes it (#9304, checked live in both). |
| Task settings | Presentation/TaskSettings (window, view model, TaskSettingsPageBuilder), ShareX.Tools MouseHighlighterSettingsControl | 🟨 `task_settings_window.rs` on `settings_form` (Application settings' page layout, now shared): the default task settings (opens on Notifications; General and Upload select their first child) or a hotkey's (`SetDefaultSettings` first, opens on Task, "Override ..." cards, `EnabledCard`s disabled until overridden); the navigation tree (nested `TreeViewItem` margins, Fluent's expander, bring-into-view, collapse) and search; all 20 pages in ShareX's order bound to the edited `TaskSettings` (JSON-backed sections such as OCR, the tools' options, actions and watch folders edited in place), so capture, after capture tasks, file naming and region capture options apply to the next task; Task page with the task picker and the after capture/upload task pickers (compact check items, last-chosen icon); closing saves `ApplicationConfig.json` (defaults, and the menus refresh) or `HotkeysConfig.json` (a hotkey's). `--render task-settings-<page>` (`--hotkey`) vs ShareX, dark/light: notifications 0.74/0.65%, image 0.53/0.42%, effects 0.37/0.26%, thumbnail 0.37/0.28%, capture 0.70/0.55%, region capture 0.63/0.52%, screen recorder 0.72/0.58%, OCR 0.31/0.21%, file naming 0.61/0.44%, clipboard upload 0.38/0.25%, uploader filters 0.37/1.30% (ShareX lists its uploader services in that combo), tools 0.31%, image editor 0.60/0.51%, mouse highlighter 0.68/0.54%, actions 0.32/0.21%, watch folders 0.27/0.19%, advanced 0.57/0.40%; x, capture 0.61%, region capture 0.45%, advanced 0.37%. Live, side by side: a hotkey's Override capture settings + Show cursor off, and the defaults' Capture client area on, give the same `HotkeysConfig.json` (byte-identical) and `DefaultTaskSettings` values in both apps. The Actions page's list (`action-list`: active check box, name, "— path" trimmed) and the action editor (`ActionEditorOverlay`: name, file path with Browse, arguments with the `$input`/`$output` context menu inserting at the caret, output extension, extension filter, hidden window, delete input only with an output extension, the name/path checks), the Watch folders list and editor (`WatchFolderEditorOverlay`), with Add/Edit/Duplicate/Remove as ShareX's; opening the window adds the installed image editors (`AddDefaultExternalPrograms`). `--render task-settings-actions`/`-watch-folders` `--content sample|action-editor|action-editor-edit|action-editor-error|watch-folder-editor|watch-folder-editor-edit` draw them with the shared controls (compared with ShareX's shots below). Customize toolbar... for the image editor and region capture (docs/IMAGE-EDITOR.md, "Toolbar customisation"). [SETTINGS-WINDOWS.md](SETTINGS-WINDOWS.md): the notification buttons editor (0.29/0.26%; its Add... menu and empty state built from source), Select region... (the region capture select-only, 0.04/0.04%; the values 0.65/0.56%), the action and watch folder editors against ShareX's shots (0.27/0.22%, 0.34/0.26%), the Task page's destinations menu (0.74/0.51%), Advanced's multiline custom text, `InputWithButton` rows at their 40px height; scrolled parts compared on six pages (0.37–0.66%) and four hotkey pages (0.23–0.62%). The last gaps ([SETTINGS-WINDOWS.md](SETTINGS-WINDOWS.md), "The last Task settings gaps"): the name pattern, pixel info and upload info variable menus (`variable_menu.rs`; built from source, ShareX's `task-variables` shots were skipped), Uploader accounts from `UploadersConfig` (0.43/0.40%, was 0.67/1.03%), the destinations menu's commands on the edited task, uploader filter services with Add/Update/Remove (0.31/0.23%, was 0.37/1.30%), the sound file picker, the Mouse wheel tooltip, the mouse highlighter's colour flyouts, and the system time zone list with ShareX's `TimeZoneInfo` JSON and .NET 11's conversion in `NameParser` (`sharex_helpers::time_zone`; `tools/settings-harness --time-zones`: 144/144 zones' list, JSON and offsets 1970-2040 identical, the reload as `SettingsBase` does it on a Tokyo and a California machine 144/144 each; the dropdown 1.29/1.85%). Open: the time zone list on macOS and Linux (ShareX lists IANA zones there; empty in ShareXrs) Upstream 30d1682: Escape closes the window after its four editors (#9304, checked live in both); Mouse 4/5 click list "Capture selected region" (#9277). |

| Destination settings | UploadersLib/Presentation/DestinationSettings (window, view model, DestinationSettingsPageBuilder, DestinationSettingsAccounts) | 🟨 `dialogs/destination_settings.rs`: the category navigation with search, all 48 pages from ShareX's own page builder (`destination_pages.json`, written by `UploadersHarness --destination-pages`) plus the hand-built cards (accounts, Imgur/Photobucket albums, OneDrive/Box/MEGA folders, Amazon S3 with its URL preview, img.fish, the FTP and shared folder account lists with the FTP URL preview), bound to UploadersConfig through its JSON (`[JsonEncrypt]` members shown decrypted, written DPAPI-encrypted as ShareX does), the selections ShareX clamps while building clamped on open; opened from the sidebar, the tray and a task missing its settings (on the service's page); saved on close. `--render destination-<page>` vs ShareX, dark: imgur 0.35%, ftp 0.23%, amazon-s3 0.35%, email 0.30%, google-drive 0.23%, dropbox 0.27%, pastebin 0.24%, hastebin 0.24%, pomf 0.20%, bitly 0.21%, zero-width 0.17%; light: imgur 0.31%, ftp 0.19%, amazon-s3 0.30%, email 0.24%. Live: the sidebar opens it, a check box and an encrypted key are saved (the key readable by .NET's `ProtectedData`). Wired (UPLOADERS.md, "The Destination settings window's services"): every sign-in card on a worker thread (code-paste OAuth open/complete/refresh/disconnect; Google's loopback with `OAuthListenerWindow` and Cancel; Flickr/Photobucket OAuth 1; ImageShack, Pastebin, puush, LobFile, Paste.ee; MEGA with the two-factor `InputBoxWindow`), the pickers (Imgur albums, OneDrive/Box folders, Pushbullet devices, MEGA's folder tree), the remote storage browser button, the email and YouTube options windows as the uploaders' hooks; pages open through `UrlOpener` (tests record them); tests drive every button against the 127.0.0.1 mock. `--render destination-<page> --state <state>` vs ShareX's reference shots (dark/light; whole window, or the page column where ShareX's shot came through the search with the navigation unscrolled): connected-display imgur 0.42/0.39%, gist 0.25/0.21%, dropbox 0.27/0.26%, onedrive 0.21/0.20%*, box 0.22/0.16%*, bitly 0.07/0.07%*, google-drive 0.08/0.10%*, google-cloud-storage 0.13/0.13%*, youtube 0.07/0.06%*; disconnected gist 0.25/0.21%, dropbox –/0.25%, onedrive 0.21/0.19%*, box 0.22/0.16%*, bitly –/0.06%*, google-drive –/0.09%*, google-cloud-storage 0.13/0.12%*, youtube 0.07/0.06%*, flickr 0.29/0.25%, photobucket 0.28/0.23%; account forms imageshack 0.26/0.22%, paste-ee 0.19/0.17%, pastebin –/0.21%, lobfile 0.10/0.08%*, mega 0.34/0.30%*, pushbullet 0.05/0.05%*, puush 0.10/0.07%*; imgur albums 0.35/0.31%, seafile 0.18/0.17%*, google-folder 0.08%/–* (* page column; – no shot). The other states (signing in, lists loaded, errors) and the email, YouTube options, OAuth listener and input box windows are built from source with made-up data, no shot. Not in ShareX 22.4's window, so not here: a Google Drive folder/drive picker, Seafile's library buttons. Live checks still to do: real sign-ins against each service, the browser's redirect, a populated remote storage browser. Tooltips use egui's own Upstream 30d1682: Escape closes the window (#9304, checked live in both). |

| Update check | ShareXUpdateManager, HelpersLib/UpdateChecker (GitHubUpdateManager, GitHubUpdateChecker, UpdateChecker, FileDownloader), Presentation/UpdateChecker (UpdateMessageWindow, DownloaderWindow) | 🟨 `sharex-update` (docs/RELEASING.md, "The update check"): ShareXrs's own releases, ShareX's asset rules, version comparison, channels, the hourly check with `AutoCheckUpdate`/`DisableUpdateCheck` (applied at startup and when Application settings closes), portable (browser) and installed (downloader, signature question, `/UPDATE /SILENT`) flows; macOS/Linux flows as additions; `SHA256SUMS` checks. Against ShareX's own code on a mock (`tools/update-harness`): 72 cases identical, 2 additions. The update message and downloader windows (`dialogs/update.rs`, `--render update-message`, `update-downloader`) aren't compared with ShareX's live windows yet. The About window runs the check and shows its panel (App windows row). Accepted: no dev build download (ShareXrs has no dev builds repository). Blocked: the windows compared with ShareX's live (the About window's check needs the network, [LIVE-CHECKS.md](LIVE-CHECKS.md) APP-03), the macOS swap and Linux flows on a real install (hardware) |
| App windows | Presentation/About, DebugLog, DisplayDiagnostics, FileExist, IconPackPrompt, AfterCapture, Notification, ActionsToolbar, QuickTaskMenuEditor, QuickTaskMenu.cs, MainMenuBuilder (Workflows, Recent items, tool grid edit), HelpersLib/Presentation/ImageViewer, OutputBox, HelpersLib/Logger.cs, TaskHelpers (HandleExistsFile, ShowNotificationTip, AnnotateImageFromFile, OpenScreenshotsFolder) | 🟨 [APP-WINDOWS.md](APP-WINDOWS.md). Every window and where ShareX opens it: the file exists window (the tasks' threads wait for it), the icon pack prompt (once, with the main window), About (with its update check), the debug log (the app's stderr kept as ShareX's logger, `Logs/ShareX-Log-yyyy-MM.txt`), display diagnostics, the task notification toast and the tips (minimized to tray, hotkeys toggled, quick colour picker, upload errors), the after capture window (captures and recordings), the image viewer (thumbnails, previews, history, the upload windows, Tools, its hotkey), the Workflows menu from the hotkeys, the tray's Recent items, the tool grid's edit mode, the quick task menu and its editor, the Actions toolbar with its title menu and editor, the FFmpeg output box, the missing-file message, the sidebar's screenshots folder and links. `--render` vs ShareX's shots (dark/light): file exists 0.80/0.74% (new name 0.86/0.80%, empty 0.74/0.74%), About 5.11/6.47% (3.1% without the logo: ShareXrs's icon), debug log 7.12/7.60% (Cascadia Mono's 13.94px line pitch vs egui's 14px), Workflows menu light 0.50%, image toast 0.38/1.27%, tray tip 1.06/1.28%, after capture 0.99/0.99%, quick task menu 0.57/0.37%, its editor 0.10–0.78% in four states, Actions toolbar 0.20/0.11%, its title menu 0.30/0.24%, its editor 0.41/0.32%, add menu 0.48/0.42%, tool grid edit mode 0.71/0.54% (text anti-aliasing). Not compared with a ShareX shot: the image viewer, display diagnostics, the missing-file message, the update windows (the icon pack prompt and the FFmpeg output box were checked live, [LIVE-CHECKS.md](LIVE-CHECKS.md) APP-02/14). Since: tooltips in popup windows of their own over the toast's buttons and the Actions toolbar (title and actions), the toast's file dragged out (Copy | Move) and its `SpecificMonitor` by device name, GIF/WebP playback in the viewer, display diagnostics' Windows probes (`display_native.rs`, with the capture windows' log), ShareX's clicker in About (`--render about --state clicker|clicker-store`); none of these states has a ShareX shot. Live ([LIVE-CHECKS.md](LIVE-CHECKS.md) APP-01..29): the file exists window, the icon pack prompt, toasts in all nine placements, the tray tip, the after capture window, the tool grid's edit mode, the quick task menu and the FFmpeg output box behave as ShareX's. Open: live re-checks of APP-08a (the opened path normalised as `Path.GetFullPath`) and APP-13a (the drag anchored at the press), both fixed; the drag outside Windows; the geometry log's DPI changes and overlay checks. Blocked: About's update check (network), Restart as administrator (UAC), macOS/Linux (hardware) Upstream 30d1682: Escape closes About, Debug log (after its upload confirmation), Display diagnostics and Stats (#9304, checked live in both); the title bar's sun icon is ShareX's pixel-aligned one (#9274: 0 of 400 pixels differ from the new ShareX's at 125%). |


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

`scripts/annotate-session.ps1 -Job RectangleRegion` starts the capture with
the command line job sent to the running app (and waits for the overlay
while the pattern still covers the screen), not its menu. A ShareXrs main
window left topmost (by `raise.ps1`) sits over the pattern: clear that
before a session, or the overlay freezes it. Window captures target
`--test-pattern --framed X,Y,W,H`, a decorated window of ShareXrs's own:
`scripts/window-capture-session.ps1` captures it with the Custom window job
(`CaptureCustomWindow` set to its title; a forwarded `-ActiveWindow`
brings the app's own window to the front first) or a region capture
clicking its title bar, watches the taskbar and desktop icons through the
API and leaves them as they were. `pattern-check.py --opaque` judges
masked captures by their opaque pixels and `--search R` finds a shadowed
window capture's offset.

### Region capture performance

`scripts/bench-region-capture.py` (its header says what it times), at 125%
on one 2560x1600 monitor over `--test-pattern`, release builds, ShareXrs
before (master at 5ef08ef) and after, two portable copies alternating:
8 starts each, the first capture after the start (the warm-up done) and
3 repeats, medians in ms. ShareX's from the benchmark's first run.

| | before | after | ShareX |
|---|---:|---:|---:|
| first capture: trigger to overlay visible | 163 | 133 | |
| first capture: arguments received to visible | 146 | 117 | |
| repeat: trigger to overlay visible | 165 | 133 | |
| repeat: arguments received to visible | 148 | 116 | 145 |
| repeat: frozen screen painted by (PrintWindow bound) | 307 | 228 | 472 |
| release to file saved | 17 | 10 | 76 |
| app start to main window visible | 729 | 736 | |

Where a repeat capture's time goes now (`SHAREXRS_TRACE_CAPTURE=1` logs
each step): 3 ms to the grab, then `BitBlt` 94, `GetDIBits` 1.8, BGRA to
opaque RGBA 1.5, the workspace 1.6, the editor composite with the pointer
1.0, the frozen screen's colours 0.9, the first frame's UI 5.5, its
tessellation 0.3, its upload, draw and present 2.9, raise and show 6:
118 ms. Before: `BitBlt` into the DIB section 99, the conversions 4.7,
the workspace 3.7, the window 4.3, egui's context 2.5, the surface 7.4,
the first frame's UI 25 (the frozen screen cropped and converted, the
pointer annotation's composite through whole-image conversions), present
2.8, show 4.4: 156 ms.

- **The grab is the compositor's.** `BitBlt` from the screen waits on DWM:
  even a 100x100 blit takes 60-70 ms here, the whole screen 75-110, about
  the same without `CAPTUREBLT`, and a blit right after another is no
  quicker. The test pattern is a full-screen flip-model window, which DWM
  may be scanning out directly, so an ordinary desktop may wait less (not
  measurable: the desktop is never captured). Blitting into a bitmap
  compatible with the screen and reading it with `GetDIBits` saves the
  DIB section's ~20 ms, with the same pixels; DXGI duplication isn't
  used (identity with layered windows and the pointer rules unproven).
  The grab runs on another thread while the event loop lists the windows
  and readies the overlays (Windows; xcap stays on the event loop).
- **Kept overlays.** On Windows the overlay windows outlive the capture,
  hidden, with their swap chain, renderer, fonts and a frozen screen
  texture of the monitor's size; `EguiWindow::reset_for_reuse` starts the
  rest over as a new window would, and a kept window is put back on top
  of the topmost windows before it's shown. `MainWindow.WarmUpRegionCapture`
  makes them 2 s after startup once idle (~100 ms of the event loop,
  logged as ShareX does). A second after they're hidden their swap chain
  goes down to 1x1 and the frozen screen texture is freed; both are made
  again while the next capture grabs the screen (see "Memory" below).
  `SHAREXRS_NO_OVERLAY_REUSE=1` closes them after each capture as before. macOS and Linux still make and close
  them per capture: hiding and showing them again is untested there.
- **Memory** (MB, `Get-Process` working set / private / peak working set,
  GPU dedicated from `GPU Process Memory`; ShareX's read from the running
  reference copy, idle):

  | | before | after | ShareX |
  |---|---|---|---|
  | idle, main window shown (5 s) | 354 / 416 / 354, GPU 204 | 368 / 430 / 383, GPU 253 | 231 / 189 / 232, GPU 59 |
  | idle, closed to the tray | 355 / 416 / 355, GPU 204 | 368 / 430 / 383, GPU 253 | |
  | after the first capture | 383 / 427 / 486, GPU 204 | 396 / 440 / 456, GPU 253 | |
  | after 15 repeat captures | 388 / 432 / 492, GPU 204 | 404 / 448 / 466, GPU 253 | |

  The kept overlay is the +49 MB of GPU memory and +14 MB of the
  process's; the peak is lower, with fewer screen-sized copies at a time.
  (These were before the memory work: see "Memory" below.)
- **The first frame only draws.** The frozen screen is uploaded before it
  (into the kept texture) straight from the screenshot or the composite;
  image annotations and drop shadows convert and blur only what they
  cover instead of the whole image, and screen-sized conversions are
  split over threads (`sharex_skia::par`). Present mode is egui_wgpu's
  (FIFO): the first present doesn't wait (2-3 ms with the upload).
  `ShowWindow`, which activates the overlay, takes the 5-6 ms it did.
- **Nothing visible changed.** 64 of 64 saved region files (first and
  repeat captures of both builds) are byte-identical; the overlay's
  frames grabbed live are identical, kept windows included, with the same
  focus, topmost state and pointer; `--render region-*` gives the same
  bytes (10 cases); the new conversions, the bounded shadow and image
  drawing have tests against the code they replace (hundreds of random
  cases each).

### Memory

Windows, 125% on one 2560x1600 monitor (NVIDIA RTX 4080 driving it, an
AMD iGPU beside it), release builds, portable copies saving to a folder
only, `ShowIconPackPrompt` off (the reference ShareX answered it long
ago). MB: `Get-Process` working set / private bytes / peak working set,
and `GPU Process Memory` dedicated. Each figure is the mean of two runs;
after a capture the app waits 10 s first (the driver frees a destroyed
screen-sized resource some seconds later).

| | before (master at 9153502) | after | ShareX |
|---|---|---|---|
| idle, main window shown | 354 / 416 / 369, GPU 249 | 281 / 200 / 310, GPU 56 | 231 / 189 / 232, GPU 59 |
| idle, closed to the tray | 354 / 416 / 369, GPU 249 | 280 / 200 / 310, GPU 56 | |
| after the first capture | 365 / 425 / 441, GPU 249 | 290 / 208 / 396, GPU 56 | |
| after 15 captures | 370 / 430 / 448, GPU 249 | 296 / 214 / 417, GPU 56 | |
| after 50 Application settings + image editor open/close cycles (its own start) | 379 / 439 / 428, GPU 249 | 295 / 214 / 362, GPU 56 | |

Over the cycles (sampled every 10) the private bytes go 432, 430, 439,
439, 439 before and 214, 213, 213, 213, 214 after: nothing grows with
windows opened and closed.

The same builds renamed (`sharexrs-b.exe`, `sharexrs-n.exe`), so NVIDIA's
shader disk cache for them is new (see below), as on a new install:

| | before | after |
|---|---|---|
| idle, main window shown | 240 / 397 / 254, GPU 249 | 169 / 183 / 198, GPU 56 |
| after the first capture | 251 / 406 / 327, GPU 249 | 177 / 190 / 284, GPU 56 |
| after 15 captures | 255 / 410 / 333, GPU 249 | 182 / 196 / 304, GPU 56 |

Renamed, ShareXrs is below ShareX in every column at idle; as
`sharexrs.exe` on this machine only the private bytes aren't (200 to 189),
for the shader cache's index.

**What held it** (before, idle, as `sharexrs.exe`):

- **wgpu's allocator blocks: 192 MB of GPU memory and private bytes.**
  wgpu's default `MemoryHints::Performance` makes gpu-allocator reserve
  128 MB blocks of video memory and 64 MB of upload memory (DX12 and
  Vulkan), all mapped into the process (write-combined), for the ~50 MB
  of textures ShareXrs had.
- **The shader-visible descriptor heap: ~30 MB of each.** On DX12
  `max_non_sampler_bindings` sizes the one CBV/SRV/UAV heap wgpu makes up
  front: 1,000,000 descriptors from `adapter.limits()`.
- **The kept overlay: ~48 MB of each.** Its swap chain (two 2560x1600
  buffers) and the frozen screen texture, kept while hidden.
- **Fonts: ~20-30 MB of the Rust heap.** Every window's egui context read
  each system font (Segoe UI x3, Cascadia Mono twice, Consolas) into its
  own copy, and egui copies an owned font into its context again.
- **A leftover 16 MB upload buffer**: wgpu frees a frame's upload buffers
  when the device is next polled, i.e. at the next frame, so a window's
  last frame before idling (the warm-up's frozen screen texture) kept it.
- **NVIDIA's driver**, in any D3D12 process (the test pattern too):
  ~62 MB of write-combined memory in two rings and a ~24 MB heap of
  private bytes, and about half of the 56 MB of GPU memory left (wgpu
  has ~12 MB of textures and buffers in 20 MB of blocks, the main
  window's swap chain is ~8 MB). Not ours to change.
- **NVIDIA's shader disk cache for `sharexrs.exe`**
  (`%LOCALAPPDATA%\NVIDIA\DXCache\*.nvph`): the driver maps the app's
  cache file, 256 MB on this machine after months of builds, with 96 MB
  of it resident (working set, shared and file-backed) and ~18 MB of
  private bytes for its index. A new exe name gets a 64 KB cache: the
  renamed rows above. Not ours to change; it depends on the machine's
  history, not the app's.

**What changed** (idle with the main window, each change measured on its
own, in order, with the icon pack prompt still open in those runs):

| change | working set / private / GPU |
|---|---|
| (start) | 368 / 430 / 253 |
| 4 MB allocator blocks (`MemoryHints::Manual { 4 MB..4 MB }`, gpu-allocator's least: larger resources get their own block, freed with them) | 332 / 303 / 157 |
| a 65,536-descriptor heap (2 MB; VRAM runs out long before 65,536 textures do) | 332 / 274 / 129 |
| one copy of each font file for every window, mapped read-only (`'static`, which egui borrows), the definitions built once | 309 / 252 / 129 |
| a non-blocking device poll half a second after frames are painted | 290 / 232 / 129 |
| the kept overlays slimmed a second after they're hidden: swap chain to 1x1, frozen texture freed, both made again during the next capture's grab (the selections of screen recording, scrolling and auto capture ready their overlays during their grab too now) | 286 / 211 / 64 |
| (the icon pack prompt off: the measurement setup) | 281 / 200 / 56 |

On macOS and Linux: the allocator blocks apply to Vulkan as to DX12
(Metal ignores the hint), the fonts are read once for every window (not
mapped), and the poll is everywhere; the descriptor heap is DX12's, and
overlays are only kept (so slimmed) on Windows.

**What's left and why** (after, idle, renamed: 169 / 183 / 56):

- NVIDIA's driver, as above: ~86 MB of the private bytes.
- The process heap (Rust's allocations and the C runtime's), ~34 MB: two
  egui font atlases (2 MB each, kept CPU-side by egui to update them),
  glyph and layout caches, settings, history, and blocks the heap walk
  couldn't name (a 4 MB table, a zeroed 1 MB buffer: tracing heap
  allocations to their callers needs a registry setting).
- The AMD iGPU's driver (`amdxc64.dll`, 6.4 MB of working set): wgpu
  creates a D3D12 device on every adapter to choose one, so a second GPU's
  driver loads. Avoiding it needs a change in wgpu-hal.
- Two egui contexts (main window, kept overlay): each has its own font
  atlas texture (2 MB of GPU memory) and renderer.
- After captures: the main window's task list keeps each task's
  thumbnail (188 KB at this size) queued while the window is hidden, as
  ShareX keeps its thumbnails; GDI's grab leaves a screen-sized surface
  the OS reuses; the heap keeps some of what it committed. A
  `PrintWindow(PW_RENDERFULLCONTENT)` of the overlay by another process
  (the benchmarks' paint detection) leaves 15-30 MB of screen-sized
  allocations in ShareXrs: `scripts/memory-session.py` doesn't grab it.

**Speed.** `scripts/bench-region-capture.py`, 8 starts each, medians (ms),
before / after: first capture trigger to visible 139 / 135, painted by
234 / 228; repeat captures trigger to visible 133 / 106, painted by
226 / 205; release to file saved 10 / 11; app start to main window
visible 723 / 715. `scripts/bench-window-open.py`, 10 starts each, 2 opens
of each window per start, medians (ms) before / after: start to main
window visible 686 / 667 (painted by 770 / 750); Application settings
visible 120 / 117 (painted by 194 / 196); Task settings 136 / 133
(194 / 194); image editor 45 / 42, image shown by 151 / 143. Release to
file saved is within its noise (p90 15 / 15 on first captures).

**Nothing visible changed.** Saved captures: 204 from the build before
and 348 from the builds after (each change), all 552 byte-identical.
Renders: the ten `scripts/*compare*.sh` run with both
builds (outputs and inputs in temp folders) give 680 PNGs, 677
byte-identical; the other 3 (`analyze-image-analyzing`,
`index-folder-generated`, `metadata-stripped`) differ between two runs
of the same build too.

**Tools.** A `memory-profile` build (`cargo build --release --features
memory-profile`) started with `SHAREXRS_MEMPROF=<file>` appends a JSON
line a second: every heap (`HeapSummary`; with `SHAREXRS_MEMPROF_WALK`
their large blocks and first bytes, which name fonts and images) and
wgpu's allocator report (blocks, largest allocations by label).
`scripts/memory.ps1` reads a process's counters, `scripts/vmmap.ps1` its
committed memory by kind (mapped files named; `-Csv` to diff),
`scripts/wsmap.ps1` its working set by module and file, and
`scripts/memory-session.py` takes the table's points (`--phase cycles`
for the open/close cycles, without the test pattern). `wsmap.ps1
-PagesOf sharexrs.exe -PagesOut <file>` lists the exe's resident pages
and `scripts/image-pages.py` sorts them by PE section and by the
translations they hold, language by language. A counting
`#[global_allocator]` in the binary made rustc 1.96 crash
(`STATUS_HEAP_CORRUPTION`) building it with thin LTO, hence the heap
APIs.

**Translations, by language** (2026-11-19). `sharex-strings` had one row
per key: the key and 29 `Option<&str>`, one per language (480 bytes),
2 MB of pointer tables in `.rdata` with every language's strings
interleaved over ~3 MB more, so reading an English string touched pages
holding the other 28 languages' too. Now each language is one blob of
its strings in key order plus a table of `u32` ends (bit 31: not
translated), generated into `OUT_DIR` and included whole; an accessor
reads the current language's blob, then English's. The photo and pixel
editors' modules, which nothing uses yet, aren't generated. ShareX's
`.resx` have every key in every language, so English is never actually
read in another language. No string changed: `every_string_unchanged`
pins a dump of every key through `lookup` and the accessors in all 29
languages (its length and FNV-1a hash taken from the old layout), and
the two layouts' 9 MB dumps are byte-identical.

Release builds, renamed portable copies, idle (MB, the exe image
resident in KB):

| | before | after |
|---|---|---|
| main window, English: working set | 165.7, 165.7, 166.0 | 169.4, 169.3, 165.3 |
| main window, English: exe image | 12,116 (`.rdata` 3,008) | 11,980 (`.rdata` 2,884) |
| main window, German: working set | 166.1 | 165.8, 165.6 |
| main window, German: exe image | 12,136 (`.rdata` 3,024) | 12,000 (`.rdata` 2,896) |
| main window, English: Japanese text resident | 80 KB | 0 |
| tray only, English or German: exe image | 10,788 | 10,748 |

So it's ~130 KB: the app reads a few hundred strings, and only the pages
it reads are resident (232 KB of the old translation pages and 104 KB of
its pointer tables with the main window; now 40 KB of blobs). The
working set moves more than that between runs (the 169s have no more of
the exe or the heap than the 165s), and a copy's first start ever has
~13 MB more of its `.text` resident (in either layout). The exe is
61,231,104 bytes before, 59,915,264 after (-1.3 MB: the pointer tables,
their relocations, the two editors' strings). A release rebuild after
touching the strings: the crate 19.4-20.2 s before, 2.3-3.9 s after
(`strings.rs` 7.6 MB to 0.9 MB); with the exe 103-107 s, 88-92 s.
`bench-window-open.py`, 14 starts each, medians: visible 626 / 634 ms,
painted 709 / 712 ms, the log's startup time 623 / 627 ms (the same
minimums; the new build's p90 is two slow starts, 830 and 1,165 ms, in
a row while the machine was shared).

### Large images and small GPUs

A texture can't be larger than the GPU's `max_texture_dimension_2d`: 16384
on this RTX 4080 (D3D12), but 2048 on some VMs, remote sessions, old iGPUs
and GL drivers, and any GPU has a limit a large enough monitor, scrolling
capture or file passes. Over it, wgpu fails validation (a panic). Startup
logs the adapter: `Graphics adapter: NVIDIA GeForce RTX 4080 (Dx12,
DiscreteGpu, driver 32.0.16.1088), max texture size: 16384`.

- **egui is told the real limit before anything is uploaded**
  (`Gpu::configure_context`, for every window and offscreen render). egui
  assumes 2048 until a frame's input says otherwise, and `load_texture`
  asserts against it: a debug build's region capture on the 2560x1600
  screen panicked ("Texture "frozen screen" has size 2560x1600, but the
  maximum texture side is 2048") whenever the overlay was made at capture
  time (not the warmed-up one, or `SHAREXRS_NO_OVERLAY_REUSE`), because
  the frozen screen is uploaded before the overlay's first frame. Debug
  `--render`s of large images and editors panicked the same way. The
  adapter was never the 2048: it's the same RTX 4080 as before.
- **`sharex_ui::tiled::TiledTexture`** holds an image of any size: one
  texture when it fits (the same `load_texture`, `set_partial` and
  `painter.image` calls as before, so nothing changes there), otherwise a
  grid of textures of at most the limit, drawn as one image (`paint`,
  `paint_uv` for a part). Each interior tile edge carries a one-pixel apron
  copied from its neighbour, so bilinear filtering at a seam reads the
  same texels one texture would; adjacent tiles meet at the same computed
  coordinate, so no pixel is drawn twice or missed. Used by every surface
  that can show a large picture: the region overlay's frozen screen (kept
  between captures as before, tiles included) and its ellipse/freehand
  chrome, the image editor's canvas, the image viewer (and its animation
  frames), Pin to screen, the scrolling capture preview (its own fixed
  2048 px tiles, which had no aprons, are gone), the main window's list
  preview pane, the history preview, after capture, before/after upload and
  clipboard upload previews, OCR, the image tools' and effects' pictures,
  the video player and trimmer frames, the classic ruler's screen, the
  monitor test, the test pattern, the print preview, the QR code, the
  recording border's strips and `sharex_ui::border`'s rasterised shapes (a
  window-wide card). The rest are bounded far below any limit (icons,
  thumbnails at `ThumbnailSize`, magnifiers, swatches, font previews,
  window icons, timeline cells, the 1024 px animated previews).
- **Testing on a strong GPU**: `SHAREXRS_MAX_TEXTURE=N` caps the side
  ShareXrs uploads and tells egui (at least 1024: egui's font atlas
  asserts that much); the log says `capped to N`. It doesn't cap the
  device: a window's swap chain has the same limit, and a window larger
  than the device's (only possible on a 2048 GL adapter with a larger
  monitor) still can't be shown.

Checked (release and debug, RTX 4080, 125%, one 2560x1600 monitor):

- Renders at `SHAREXRS_MAX_TEXTURE=1024` are byte-identical to uncapped
  ones, and uncapped ones to master's (release): `region-rest` (2560x1600
  at 125% and 100%, 1024x768), `region-drag`, `region-hover`,
  `region-annotate` (a rectangle annotation, so the composite is written
  into the tiles) and the region ellipse mid-drag, `image-editor` on
  2560x1600 and 3001x2213 images, `image-viewer` on 2560x1600, 3001x2213
  and 900x4500 images, `main-tasks --list` with the preview pane,
  `scrolling-capture --state result` at 100% and 125%, `main`,
  `settings-general`, `hotkeys`, `controls-gallery`: 19 of 19.
- `tiled::tests`: the tile grid (single tile, edge tiles, odd sizes, three
  tiles with aprons on both sides, 0 to 65,537 px against limits 3 to
  4096), the crops, the drawn parts meeting at the seams, partial UVs, and
  on the GPU an image drawn as one texture and as 64 px tiles: identical
  when texels land on whole device pixels (100%, 125% with whole pixels,
  300%); otherwise within 3 levels filtered, and nearest filtering differs
  only where a sample falls exactly on a texel edge (150%: a tie the float
  rounding decides either way), showing the neighbouring texel. No seams.
- Live, over `--test-pattern`, `scripts/bench-region-capture.py verify`
  with portable release copies: capped at 2048 (two tiles) the overlay
  shows the pattern (0.9683 of it, as master's) and the saved 401x301
  region is byte-identical to master's (same SHA-256); a debug build at
  1024 with `SHAREXRS_NO_OVERLAY_REUSE=1` (six tiles, the overlay made at
  capture time) too, where master's debug build panics as above. Pin to
  screen of a 2560x1600 pattern image at 1024 (3x2 tiles), grabbed with
  `PrintWindow`: the image 0.9981 the pattern, as master's, and the two
  grabs identical but for 42 pixels a level apart under the pin's
  toolbar.
- Speed (`bench-region-capture.py`, 6 starts x 3 captures, medians in ms,
  master / this): first capture trigger to visible 144 / 136, painted
  238 / 229; repeat visible 104 / 105, painted 210 / 205; release to file
  saved 11 / 11. Capped at 2048 (two tiles, 4 starts): first visible 143,
  repeat 107 (the copy into tiles).

## Upstream sync

ShareXrs follows ShareX's `develop` branch (`REFERENCE_COMMIT`); the
reference source and the reference build are moved together, then each
upstream change is ported in its own commit.

### c4f1e6f..30d1682 (2026-11-19)

27 commits, 16 non-merge. The reference build was rebuilt from 30d1682
(profile and `Portable` marker kept, checked by SHA-256); its shots are in
`shots/sharex/upstream-30d1682/` (all at 125%).

| Upstream | What | ShareXrs |
|---|---|---|
| #9277 (3d26be2) | "Capture selected region" mouse action; X1/X2 in annotation mode | `RegionCaptureAction::CaptureSelection`, `try_complete_selection` (Enter shares it), side buttons with an annotation tool |
| #9304 (41af0df, 6f27b49), #9276 (d81f415) | Escape closes Application/Task/Hotkey/Destination/Custom uploader settings, Stats, Debug log, Display diagnostics, About, Image effects, FFmpeg options, OCR; packager Close and confirmation No are `IsCancel` | every window, each backing out of its overlay first; OCR only without modifiers |
| #9296 (7a01510, 5b7f7b9, 1a943ba, 5754da8) | the editor's movable top toolbar | drag handle, clamp, `TopToolbarOffsetX/Y` (new doubles after `AutoSizeWindow`; `defaults.json` and the fresh-profile fixture follow), double click resets |
| 45fe7f2 | no New from screenshot buttons in the editor's bottom bar | removed |
| #9288 (3a3e20d) | record without system audio when no output device | `audio::open_or_skip` in `WasapiCapture` and `EndpointCapture` |
| #9242 (d4ec393) | MicaForEveryone: the recording frame and transparent region stay tool windows | the overlay subclass keeps `WS_EX_TOOLWINDOW` (and a layered overlay's `WS_EX_LAYERED`) on every `WM_STYLECHANGING`; winit rewrote both on show |
| #9287 (f13d387) | check boxes for flags (image sides) in effect options | `PropKind::Flags`, a `WrapPanel` of check boxes |
| #9305 (16ab658) | clipboard formats need a description and a format | the error under the editor, focus and select |
| #9274 (aa312d7) | pixel-aligned sun icon | `app.caption-sun.svg` |
| #9301 (67c4a2b) | Avalonia: run startup from the dispatcher (the main window shown at startup) | not applicable: ShareXrs has no Avalonia lifetime; winit creates and shows its windows itself |

Strings: the HelpersLib, ImageEditor and ShareX `.resx` files from 30d1682
add four keys in all 29 languages; `every_string_unchanged` was re-pinned
after diffing the dumps (only those 116 records differ). Photo and Pixel
editors: not changed upstream.

Found while validating: a debug build's region capture panicked on the
2560x1600 screen ("the maximum texture side is 2048"). That was egui's
default limit, not the adapter's; fixed in
[Large images and small GPUs](#large-images-and-small-gpus).


### 30d1682..93145cc (2026-11-20)

8 commits, 5 non-merge (46 files). The reference build was rebuilt from
93145cc (profile and `Portable` marker kept, all 14 files checked by
SHA-256); its shots are in `shots/sharex/upstream-93145cc/` (all at 125%,
dark).

| Upstream | What | ShareXrs |
|---|---|---|
| #9278 (3104786) | the toolbar customization dialog as a mock toolbar: items and spacers dragged along it, onto it and off it; spacers resized by a grip or Spacer width, blank spacers (Show spacer line); Move left/right; `SpacerWidth`/`HideSpacerLine` options | `toolbar_editor.rs` rewritten on the view model's `ApplyLayoutTokens` (unit-tested), drag, drop indicator and ghost, resize, the focused slot's keys; `ItemOptions` round-trips the two fields in ShareX's order; both toolbars draw `SpacerMargin`; `--render ... --content toolbar-editor --state ...`: eight states 0.73-1.07% of the dialog; live, the same drags in both apps give the same dialog and ShareXrs saves the layout ShareX's `ToOptions` writes |
| #9278 (60a85ea) | shortcuts pick tools hidden from the toolbar | the editor's and region capture's hotkeys look through every item (unit tests; live: a hidden Crop picked by C in ShareXrs's editor) |
| 2523ded | the thumbnail view virtualized in rows (`ThumbnailRowViewModel`), `UpdateThumbnailColumns`, row scrolling, one hover animation | the column formula (the boundary moves a pixel: four 200px columns from a 1372px window at 125%, as ShareX) and row scrolling; not applicable: egui keeps no controls per card (only cards on screen are painted; each task keeps one thumbnail texture, as ShareX keeps its bitmap) and already plays only the hovered card's animation |
| #9317 (bef6e0e) | Media Foundation fallback media types (`BuildAttempts`, `FitSize` with the macroblock rate, `AlignAndPad`), padded/letterboxed NV12, the negotiated frame rate | `encoder_attempts` (pure, tested on every platform), the writer's attempts on `MF_E_INVALIDMEDIATYPE`, `ComputeDestRect` with the limited-range black background and the camera mapped into it, the recorder and capture paced at the negotiated rate, the note logged |
| 7352305 | SSH.NET 2026.0.0 → 2026.0.1 | nothing to port: of its fixes, PuTTY ECDSA keys with a leading zero (#1827) and RSA signatures padded to the modulus length (#1855) are already what ShareXrs's SFTP does, `GetCanonicalPathAsync` (#1864) isn't on ShareX's path, and the rest are SSH.NET's internals (rekey race, .NET Framework MAC reset, SCP, socket buffers) |

Strings: the ImageEditor `.resx` files from 93145cc add ten
`ToolbarCustomizationDialogView_` keys and remove `Moveup` and `Movedown`
in all 29 languages; `every_string_unchanged` was re-pinned after diffing
the dumps (290 records added, 58 removed, none changed). ShareX's defaults
are unchanged (`ToolbarItems` empty), so `defaults.json` and the
fresh-profile fixtures are too. Photo and Pixel editors: not changed
upstream.

Found while validating: the Task settings page behind the dialog lays its
navigation rows out a pixel or two apart from ShareX's at 125% (outside the
dialog; part of the open 125% work).

### 93145cc..ca3f01f (2026-11-21)

10 commits, 6 non-merge (74 files, most of them `.resx`). The reference
build was rebuilt from ca3f01f (profile and `Portable` marker kept, all 14
files checked by SHA-256 before and after; the toolbar layout and picker
text the checks below saved were put back the same way); its shots are in
`shots/sharex/upstream-ca3f01f/` (all at 125%, dark).

| Upstream | What | ShareXrs |
|---|---|---|
| #9319 (7d17d32, 47f5ef5) | the system proxy again: `UseProxy` unless None (a null `Proxy` is the system's, PAC/WPAD aware), Automatic's user name and password as `DefaultProxyCredentials`, `IsValidProxy` only Manual with a host and port, no resolved host and port written back; upload errors end with `exception.ToString()` | ShareXrs never applied `ProxySettings` (every request went direct): the app now sets them at startup and when Application settings close (`ApplyCore`); `proxy` ports `ProxyInfo`, the handler's choice and .NET's system proxy (`HttpEnvironmentProxy` everywhere; `HttpWindowsProxy` through WinHTTP on Windows; `MacProxy` through CFNetwork on macOS, without PAC; Linux has only the environment, as .NET), with one agent per proxy (ureq takes one proxy per agent, so no `MultiProxy` failover) and credentials as Basic; unit tests and an end-to-end test through a mock proxy on 127.0.0.1. The settings page never wrote a resolved host back, so it and its screen are unchanged. Upload errors: the error's text after Stack trace (no .NET type names or stack frames to add; ureq's text carries the cause) |
| #9322 (8a46e17) | `$name` in the colour picker's info text: the closest web colour's proper name (`ColorHelpers.GetColorName`) instead of an exact `KnownColor` match | the formatter uses `get_color_name`; the exact-match table is gone; ShareX ca3f01f shows `#2C2CC8 Medium blue`, `#E85846 Tomato` and `#81BC46 Yellow green` over the pattern, as ShareXrs's test does |
| #9324 (85fc349) | the toolbar customization's item panel only has the hotkeys (Spacer width, Move left/right, Begin group, Show spacer line and Show/hide removed) | the panel is ShareX's `*,Auto` grid; vs ShareX's dialog (the dialog only): the editor's 1.63%, Rectangle selected 1.79% |
| #9326 (d4e7a10, 70cc23b) | the region capture's Rectangle region button becomes a toolbar host item (`RegionRectangle`, Tab, NumPad0), listed in the customization, movable and (70cc23b) hideable with its keys still working | `region_host_defs` first item; the fixed button, its hit part and its own Tab/NumPad0 handling are gone: Tab is matched first (focus navigation takes it in ShareX), NumPad0 and other keys through the toolbar's hotkeys, both `HostToolbarItemRequested`; old layouts get the item first; the toolbar vs ShareX at 125% 0.16% (tooltip 0.18%, hidden 0.16%), the region toolbar editor 1.57%, dragged off 1.79%, and dragged off and OK saves ShareX's JSON (the test fixture); live in ShareX, Tab and NumPad0 pick the hidden rectangle region, rendered the same in ShareXrs |

Strings: ImageEditor's `.resx` from ca3f01f drop six
`ToolbarCustomizationDialogView_` keys (Begingroup, MoveLeft, MoveRight,
ShowHide, ShowSpacerLine, SpacerWidth) and ScreenCaptureLib's rename
`RegionCaptureWindow_RectangleRegionTab` to `_RectangleRegion` (the text
without " (Tab)"), in all 29 languages; `every_string_unchanged` was
re-pinned after diffing the dumps (174 records removed, 29 renamed with
that text, nothing else). ShareX's defaults are unchanged (`ToolbarItems`
empty, `ProxyInfo` Manual), so `defaults.json` and the fresh-profile
fixtures are too. Photo and Pixel editors: not changed upstream.

Found while validating: at 125% ShareX lays the toolbar items after a group
separator a pixel further right and the capture toolbar a pixel shorter
than ShareXrs (Avalonia rounds each element's desired size up: the open
125% work, as in 30d1682's entry); the customization dialog's palette tiles
draw their labels SemiBold where ShareX's are Regular (before this range
too, left for the dialog's own follow-up). The Application settings proxy
page didn't change upstream and wasn't re-shot.

### ca3f01f..099bcb6 (2026-11-22)

2 commits, 1 non-merge (33 files, 29 of them `.resx`). The reference
build was rebuilt from 099bcb6 (profile and `Portable` marker kept, all 14
files checked by SHA-256 before and after; the toolbar layout the checks
below saved was put back the same way); its shots are in
`shots/sharex/upstream-099bcb6/` (all at 125%, dark).

| Upstream | What | ShareXrs |
|---|---|---|
| #9327 (6a2ce6a4) | a spacer on the customization dialog's toolbar can be selected (`SelectedSpacerItem`, the item after it; `SelectSpacer` on a click and on starting a resize; `PlaceSpacer` selects the dropped one; `RefreshSelection` drops it once moved, merged or removed), and its panel (Grid.Row 2, as the item's) shows the line or dashes in 20x20, Spacer or Blank spacer, and a 116px Spacer width `NumericUpDown` (0-96, `Increment` 4, `FormatString` 0) on the item's `SpacerWidth` | `selected_spacer` beside `selected` with ShareX's setters, `spacer_panel` on the shared spinner, the selected spacer slot drawn `.selected`; unit-tested (`spacer_selection_as_sharex`); `--state select-spacer:ID` (and `spacer:ID:...` now starts a group); the dialog vs ShareX: a spacer clicked 0.92%, widened to 8 by the spinner 0.86%, a blank spacer dropped before Arrow 0.76%, the Crop group's spacer resized by its grip to 25 0.65%; the same steps live in ShareXrs give the same percentages, and OK saves the same 25 `ToolbarItems` as ShareX (key order too) |

Strings: ImageEditor's `.resx` from 099bcb6 add
`ToolbarCustomizationDialogView_SpacerWidth` back (ca3f01f had removed it)
in all 29 languages; `every_string_unchanged` was re-pinned after diffing
the dumps (29 records added, nothing removed or changed). ShareX's defaults
are unchanged (`ToolbarItems` empty), so `defaults.json` and the
fresh-profile fixtures are too. Photo and Pixel editors: not changed
upstream.

Found while validating: Avalonia's `NumericUpDown` sets `Value` on every
keystroke in its text box (`TextBoxOnTextChanged` →
`SyncTextAndValueProperties`), so typing 40 in Spacer width resizes the
spacer before the box loses focus; ShareXrs's shared spinner
(`sharex_ui::controls::DecimalUpDown`, 54 uses) applies typed text only on
Enter or focus loss (the same 40 after Tab: 0.91%), on every page. Left
for its own change (`upstream-099bcb6/toolbar-editor-spacer-typing-dark.png`
shows ShareX mid-typing). At 125% the spinner's two dividers land a pixel
apart from ShareX's, and the shrunk toolbar row draws a blank spacer's
dashes crisp where ShareX's are anti-aliased (both before this range too:
the open 125% work).

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

## Accepted differences / blocked

What stays different from ShareX on purpose, and what can't be checked
here. A row is ✅ when only these are left.

**Accepted differences** (decided; don't "fix" them):
- **ClearType text anti-aliasing.** Avalonia draws text with ClearType
  (subpixel) on Windows, egui with grayscale coverage: glyph edges differ,
  shapes and positions match (the ~0.2-1% left in every comparison). A
  Skia-like LCD glyph rasterizer is planned as its own project (Ground
  rules).
- **DirectWrite glyph masks.** Text drawn into images (Text watermark, step
  numbers, speech balloons, the editor's `DrawText` effect, the video
  thumbnailer's labels) is rasterized by ShareXrs's Skia port, not
  DirectWrite, so those glyphs differ by a level here and there:
  [IMAGE-EFFECTS.md](IMAGE-EFFECTS.md), [IMAGE-EDITOR.md](IMAGE-EDITOR.md),
  [TOOLS.md](TOOLS.md). Emoji in the editor are Segoe UI Emoji's COLRv1
  painted by ShareXrs, not Direct2D.
- **Photo editor and Pixel editor ⏸.** Deferred by the user (2026-11-17)
  until everything else is ported; their menu entries, `-PhotoEditor`,
  `-PixelEditor`, the Open in Photo editor after capture task and the
  history service log.
- **ShareXrs's own name and icon** in titles, the tray and the title bar;
  its own settings folder, mutex, pipe, ProgIDs and registry policy (Ground
  rules 5, [RELEASING.md](RELEASING.md)); its own user agent and no
  ShareX API keys (uploads).
- **Video playback through FFmpeg and cpal** instead of Media Foundation, on
  every platform: Media Foundation's error texts and its video picture
  differ ([TOOLS.md](TOOLS.md), the Video row).
- **No dev builds**: ShareX's `DownloadDevBuild` has no ShareXrs repository
  to download from ([RELEASING.md](RELEASING.md)).
- **Analyze image** keeps the OpenAI key on its own requests (ShareX's
  shared client leaks it to other hosts) and sends ShareXrs's user agent
  ([TOOLS.md](TOOLS.md)).
- **Platform behaviour beyond ShareX** (macOS/Linux single instance, run at
  startup, update flows, xcap capture, livesplit hotkeys) where ShareX has
  none or throws: [PLATFORMS.md](PLATFORMS.md).

**Blocked** (needs something this machine or these rules don't allow):
- **Real accounts and the network**: uploads, sign-ins, OAuth redirects and
  pickers against the real services (every destination is checked against
  ShareX's own code on 127.0.0.1 mocks instead: [UPLOADERS.md](UPLOADERS.md)),
  Analyze image against a provider, the update check and downloads, the
  Network monitor's pings, the browser extensions (they ask for ShareX's
  native messaging host), Windows code signing and the macOS Developer ID.
  [LIVE-CHECKS.md](LIVE-CHECKS.md) lists the checks skipped for this.
- **Hardware**: an HDR display (HDR screenshot correction, checked against
  ShareX's code on synthetic frames instead: `tools/hdr-harness`), several
  monitors, audio devices and a camera for the native recorder, and a real
  Mac or Linux desktop for everything [PLATFORMS.md](PLATFORMS.md) lists
  as untested there.
- **Installers, UAC and print dialogs**: the installer and the dev build
  installer aren't run here, Restart as administrator needs UAC, and a
  print dialog is never opened (it hangs this remote session).
