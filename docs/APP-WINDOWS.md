# App windows

ShareX's remaining app windows in ShareXrs: what each ports, where the app
opens it, how to render it and how it compares with ShareX's reference
shots (`G:/scripts/sharexrs-ref/shots/sharex/app/`, listed in
`docs/REFERENCE-SHOTS.md` on `chore/reference-shots`).

## The windows

| Window | ShareX source | ShareXrs | Opened from |
|---|---|---|---|
| File exists | `Presentation/FileExist` | `dialogs/file_exist.rs` | `TaskHelpers.HandleExistsFile` with `FileExistAction` Ask: saving a capture, a text, a download, a recording, a watch folder's move. The task's thread waits (`capture_job::set_file_exists_asker`); on the event loop's own thread, or without the app, a unique name is used |
| Icon pack prompt | `Presentation/IconPackPrompt` | `dialogs/icon_pack_prompt.rs` | `ShowOnceWithMainWindow`: when the main window first opens, with `ShowIconPackPrompt` and not `-sandbox`. Picking a card applies the pack at once; closing clears the flag and saves |
| About | `Presentation/About` | `dialogs/about.rs`, `main_commands.rs` | The sidebar. Opening it checks for updates on a thread (`sharex-update`) unless `DisableUpdateCheck`; a newer version shows the link to the update message |
| Debug log | `Presentation/DebugLog`, `HelpersLib/Logger.cs` | `dialogs/debug_log.rs`, `logger.rs` | Debug › Show debug log. `logger::init` puts a pipe in place of stderr (`SetStdHandle`, `dup2`): every `eprintln!` line is kept with ShareX's `yyyy-MM-dd HH:mm:ss.fff - ` prefix, appended to `Logs/ShareX-Log-yyyy-MM.txt` (not with `DisableLogging`) and passed on to the real stderr. Loaded assemblies lists the modules loaded from the program's folder |
| Display diagnostics | `Presentation/DisplayDiagnostics`, `HelpersLib/Native/DisplayConfigHelper`, `HelpersLib/Helpers/WindowGeometryLog` | `dialogs/display_diagnostics.rs`, `display_native.rs`, `window_geometry_log.rs` | Debug › Display diagnostics. On Windows, ShareX's `Collect` over a snapshot of Windows' answers (read-only): the thread/process DPI awareness, system DPI, compatibility layers, the Windows edition and build, the GDI virtual screen, each monitor's device, `QueryDisplayConfig` target (name, connection, refresh, HDR), adapter, display mode, effective/angular/raw DPI and scale factor, the taskbars and other app bars, the adapters and drivers, the physical cursor, this window, and the region capture overlays' geometry log, with ShareX's findings; "Avalonia" reads "winit". Elsewhere `CollectAvaloniaOnly`. The report builder is tested on synthetic snapshots |
| Notification toast | `Presentation/Notification` | `dialogs/notification.rs` | `TaskManager` on a finished task (`ShowToastNotificationAfterTaskCompleted`, not on a fullscreen window with `DisableNotificationsOnFullscreen`), an upload error, and `ShowNotificationTip`: minimized to the tray (first time), hotkeys toggled, the quick colour picker. One at a time; dark whatever the theme; never activated. `SpecificMonitor` by its device name (`MONITORINFOEX.szDevice`), else the primary; the file dragged out 20 px from a left press (Copy | Move, the fade held); its buttons' tooltips in a popup window (`dialogs/tooltip_window.rs`, `Effect::Tooltip`) |
| Tooltip popup | `ToolTip` (`PopupRoot`) | `dialogs/tooltip_window.rs` | The toast's buttons and the Actions toolbar's title and actions: `PlacementMode.Top`, `VerticalOffset="-4"`, below at the monitor's top, click-through, never activated, fading in; closed with its owner |
| After capture | `Presentation/AfterCapture` | `dialogs/after_capture.rs` | `UploadManager.RunImageTask` and `ScreenRecordManager.FinishRecording` with `ShowAfterCaptureWindow`: Continue starts the task with the goal's tasks and the file name; a recording is renamed first |
| Image viewer | `HelpersLib/Presentation/ImageViewer` | `dialogs/image_viewer.rs` | A thumbnail's click and the list's preview (the task list's images), history's Show image, the after capture, before/after upload, clipboard and QR code windows' previews, Tools › Image viewer and its hotkey (a file picked first), `-ImageViewer <file>`. A GIF or animated WebP plays at full size (`PlayAnimation`, `animation.rs`), keeping the zoom |
| About's clicker | `EasterEggs/ShareXClicker` | `clicker.rs`, `dialogs/about_clicker.rs` | Clicking About's logo (or Enter/Space on it): the game, its stats under the logo, floating texts and particles, the store in place of the links at 10 lifetime logos |
| Workflows menu | `MainMenuBuilder.BuildWorkflowsMenu` | `main_menu.rs` | The sidebar and the tray, from `HotkeysConfig` (the tray's without "Add workflows…"); a click runs the hotkey's task |
| Tray Recent items | `BuildRecentItemsMenu`, `RecentTask` | `main_menu.rs`, `tasks.rs` | With `RecentTasksSave` and `RecentTasksShowInTrayMenu` |
| Tool grid edit mode | `MainWindow.PopulateGridMenuCategories` | `sharex_ui::menu` | Tools › Edit (sidebar and tray): every tool with its switch; `HiddenTools` saved at once |
| Quick task menu | `QuickTaskMenu.cs` | `quick_task.rs`, `tray_menu.rs` | `RunImageTask` with `ShowQuickTaskMenu`: a popup with Continue centred on the pointer, closed only by its items or Escape |
| Quick task menu editor | `Presentation/QuickTaskMenuEditor` | `dialogs/quick_task_editor.rs` | The quick task menu's Edit this menu…, Application settings › General |
| Actions toolbar | `Presentation/ActionsToolbar` | `dialogs/actions_toolbar.rs`, `main_commands.rs` | Tray › Actions toolbar, its hotkey, `ActionsToolbarRunAtStartup`; title drag (unless locked), middle click closes, right click opens its menu (a popup), files dropped are uploaded |
| Actions toolbar editor | `ActionsToolbarEditorWindow` | `dialogs/actions_toolbar.rs` | The toolbar's menu › Edit… |
| Output box | `HelpersLib/Presentation/OutputBox` | `dialogs/output_box.rs` | A failed FFmpeg run (`FFmpegCLIManager.ShowError`), scrolled to the end |
| Missing file | `TaskHelpers.AnnotateImageFromFile`, `FileHelpers.OpenFile`/`OpenFolderWithFile` | `image_editor/mod.rs`, `shell.rs` | Editing a task's or history item's image file that's gone; opening a task's missing file or its folder (the title's click, Open, the click actions): `Helpers_OpenFile_File_not_exist_` and the path, through `shell::set_file_not_exist_hook` from any thread |
| Update message, downloader | `HelpersLib/Presentation/UpdateChecker`, `Media/FFmpegGitHubDownloader` | `dialogs/update.rs`, `recording/ffmpeg_options.rs` | About's link opens the update message. The downloader also gets FFmpeg: "FFmpeg is missing" › Yes looks up the latest ShareX/FFmpeg release on a thread, then opens the window on `ffmpeg.zip` with `InstallType.Event`; `InstallRequested` (1 s after the download, in the background) extracts `ffmpeg.exe` where FFmpeg was looked for, and a message box says how it went |
| Delete remotely | `MainWindow.DeleteSelectedItemRemotelyAsync`, `CanDeleteAmazonS3Object`, `AmazonS3.TryGetObjectKey` | `task_actions.rs`, `task_view.rs` | The task list's menu, for one finished file upload to Amazon S3 with a valid S3 config: Yes/No, then the object deleted on a thread and the tip (`RemoteDeleteSucceeded`/`Failed`) |

## Rendering

`sharexrs --render <screen> --theme dark|light --out x.png`:

- `file-exists` (`--text <path>`, `--state empty`, `--state name=<text>`)
- `icon-pack-prompt` (`--state legacy`)
- `about` (`--state checking|up-to-date|available|failed|disabled|clicker|clicker-store`, `--text` product, `--content` brand)
- `debug-log` (`--files <log.txt>`, `--text` startup path, `--state upload-confirm`)
- `display-diagnostics` (two synthetic monitors, a synthetic Windows snapshot: `sample_snapshot`)
- `notification` (`--state tray-minimized`, or `--image`/`--text` for a task's; `--content hover`, `hover:<n>`, `tip:<n>`); borderless, on black: compare with `compare.py --no-crop`
- `after-capture` (`--image`, `--files <recording>`, `--text` file name, `--content <AfterCaptureJob>`, `--state goal:<n>`)
- `image-viewer` (`--image` or `--files a,b --count <shown>`, `--width/--height`)
- `main-workflows-menu` (`--state workflows-unbound`), `main-tools-menu --state tools-edit`
- `quick-task` (the popup, `--hover`), `quick-task-editor` (`--state add-overlay|edit-overlay|reset-confirm`)
- `actions-toolbar` (`--text` title, `--content hover:<n>`), `actions-toolbar-menu`, `actions-toolbar-editor` (`--state add-menu`)
- `output-box` (`--files <log>`), `task-missing-file` (`--text <path>`)
- `update-downloader` (`--content waiting|getting-size|downloading|completed|...`, `ffmpeg-<state>` for FFmpeg's)

ShareX's popups are their own windows: their shots are the popup alone; a
render of the whole window is cropped to it by searching for the shot.

## Validation

Against ShareX's shots (the same content where it matters), pixels
differing by more than 40, dark / light:

| Shot | Dark | Light | What differs |
|---|---|---|---|
| file-exists default | 0.80% | 0.74% | text anti-aliasing; ShareX's box isn't focused in its shot |
| file-exists new-name | 0.86% | 0.80% | |
| file-exists empty-name | 0.74% | 0.74% | |
| about top | 5.11% | 6.47% | the logo (ShareXrs's icon) and the brand; 3.1% without the logo |
| debug-log log | 7.12% | 7.60% | Cascadia Mono 12's line pitch: Avalonia 13.94px, egui whole pixels (14), lines drift by up to 1px |
| workflows sidebar-default | — | 0.50% | (dark is an older shot of the default profile) |
| task-toast image-saved (hover on Edit) | 0.38% | 1.27% | |
| tray-minimized first-close | 1.06% | 1.28% | |
| after-capture image | 0.99% | 0.99% | at ShareX's clamped 1028px width |
| quick-task default | 0.57% | 0.37% | |
| quick-task-editor default / add / edit / reset | 0.78 / 0.61 / 0.70 / 0.17% | 0.55 / 0.50 / 0.58 / 0.10% | |
| actions-toolbar default | 0.20% | 0.11% | ShareX's separators show nothing (their brush is looked up before the window has a theme): ported as a gap |
| actions-toolbar title-menu | 0.30% | 0.24% | |
| actions-toolbar-editor default | 0.41% | 0.32% | |
| actions-toolbar-editor add-menu | 0.48% | 0.42% | |
| tools-edit all-visible | 0.71% | 0.54% | |

Not compared yet (no shot when this was written): the icon pack prompt,
About's update states and clicker, the debug log's other states, display
diagnostics, the toast's other states (text only, placement, fade), the
after capture window's other states, the image viewer, the tray's Recent
items, the Workflows menu's other states, the output box, the missing-file
message, the update message and downloader (FFmpeg's too), the
file-not-found message box, the Delete remotely question and tip.

Unit tests cover the logic: file names and answers, About's panel states,
the logger's lines and the URL under the debug log's caret, the toast's
placement (ShareX's own measured positions), click and button actions and
defaults, the after capture goals (`InferGoal`, `Resolve`,
`IsExtraAvailable`), the viewer's folder and status, the quick task menu and
editor, the toolbar's add menu, the display arrangement texts.

## Live checks for the desktop agent

Side by side with ShareX, same profile settings:

1. File exists: save a capture over an existing file with Ask; Use new name, Overwrite, Use unique name, Cancel, Enter on the unchanged name (overwrites), Escape; the files written.
2. Icon pack prompt: profile with `ShowIconPackPrompt=true`, `ShowStartScreen=false`; it opens over the main window, the cards switch the pack live, the flag is saved false.
3. About: opens with the update check (keep `DisableUpdateCheck` on in a sanitized session); links open the browser; the window is single.
4. Debug log: the text grows while open, Copy all, Open log file, the upload confirmation (Cancel only), double click a URL.
5. Toast: after a region capture saved to file — placement for all nine `ToastWindowPlacement` values, active vs primary monitor, duration then fade, the pointer holds it and shows the caption and buttons, each button and left/right/middle click action, a tooltip after 400ms, and that clicking it doesn't take the focus from the active window (`WS_EX_NOACTIVATE` isn't set by winit).
6. Tray tip on the first close to the tray (and none on the second).
7. After capture: goals, extras, destination combo, file name, Continue/Cancel/copy; the task's settings and output file; with a recording (renamed file).
8. Image viewer: thumbnail click, wheel and arrow navigation, Ctrl+wheel zoom about the pointer, middle click reset, close on click/Escape/losing focus, full screen on the active monitor.
9. Workflows menu from edited hotkeys (asterisk, hotkey suffix, `WorkflowsOnlyShowEdited`), the tray's version; running one.
10. Tray Recent items: Copy and Open.
11. Tool grid edit mode in the sidebar and tray; `HiddenTools` in ApplicationConfig.json byte-identical.
12. Quick task menu: position (Continue under the pointer, near the work area's edges), presets, Edit this menu…, Escape; the editor's add/edit/remove/move/reset and the saved `QuickTaskPresets`.
13. Actions toolbar: first position (bottom right of the pointer's monitor), drag, lock, stay on top, open at startup, middle click close, file drop upload, editor changes reflected live, `ActionsToolbarPosition` saved.
14. Output box on a failing FFmpeg command line.
15. Missing file: Edit image on a task whose file was deleted.
16. Tooltip popups: hover a toast button and an Actions toolbar action and its title for 400 ms: the tooltip opens above it in its own window (below at the top of a monitor), passes clicks through, moves to the next button at once (100 ms), closes with the toast.
17. Toast drag: press the toast's image and drag 20 px into Explorer: the file is copied (Ctrl: moved), the toast doesn't fade during the drag and fades afterwards if the duration ended, no click action runs; `SpecificMonitor` with a second monitor's `\\.\DISPLAYn`.
18. Image viewer: an animated GIF and an animated WebP play at their delays (a 0/1 cs delay at 100 ms), keep playing while zoomed, stop on navigation and on close.
19. Display diagnostics on a mixed-DPI, two-monitor machine (and with HDR): compare every section with ShareX's report on the same machine; Refresh after a region capture lists the overlay(s) under "Recent capture windows" (one per monitor in ShareXrs, one spanning window in ShareX).
20. About's clicker: logo clicks (and Enter/Space after one), the sound, the stats and goal, the store at 10, buying x1/x10/Max, milestone boosts at 10/25/50/100, the particles and floating texts.
21. Task list drag (thumbnails and list): drag a task's file into Explorer and into another app (copy), Escape cancels; the button's release opens nothing; the main window shows no-drop outside the combine zones and doesn't show its upload overlay for its own drag.
22. Combine drop zones: drag an image card over another image card: "Combine images" with Horizontal/Vertical, the zone under the pointer lit in the accent; drop on each: the combiner's result as ShareX's (source first); none over itself, a non-image or the list view.
23. Task list playback and thumbnails: hover an animated GIF/WebP card (plays, stops on leave), select one in the list view (plays in the preview pane), a capture with only Upload/Copy (its thumbnail from the image while it works), an MP4/MKV task (the shell's thumbnail with the play badge).
24. Tooltips: a thumbnail title (the URL or file path, wrapped), the list's file name and URL cells, the theme toggle, a hotkey description; 400 ms, at once after another within 100 ms, closed by a press.
25. History windows: drag selected rows/cards into Explorer (the release after it doesn't open or reselect), a URL-only image row's preview (ShareX downloads it; use a local test server), close/reopen both windows after moving/resizing/maximizing (and with Remember window off), type in both search boxes (suggestions, Up/Down/Enter/Escape, a click), the date pickers' flyouts (wheel, click, accept, dismiss, Escape, a press outside; the filter follows; a column's up/down buttons on hover, one click and held), Stats hover tooltips on the split bar/calendar/hour chart and rows, Stats › Share (an image task with the whole page).
26. Task list keys and clicks: Ctrl+V (the clipboard content viewer or the
    upload), Ctrl+U on a file task, Ctrl+D on a URL task, the Open URL click
    action and a card title's click (the URL in the browser, or a custom
    `BrowserPath`), Open › File on a deleted file (the message box).
27. Delete remotely: an image uploaded to a 127.0.0.1 S3-compatible mock
    (MinIO or similar) with a valid S3 config; the entry shows for that one
    task only, Yes deletes the object, the tip says so; No does nothing.
28. FFmpeg downloader: with FFmpeg missing, Yes to the message: the window's
    progress, Cancel, and the extracted `ffmpeg.exe` (needs the internet
    and ShareX/FFmpeg's release; or a local server and the URL rewriter).
29. Tray › Restart as administrator: the UAC prompt, ShareXrs closing and
    coming back elevated in the tray (`-silent`).

## Untested elsewhere

macOS and Linux build and lint, nothing was run there. Dragging files out
(task list, history windows, toast) isn't available outside Windows yet
(logged; ShareX gets macOS's and X11's drag sources from Avalonia); the
tooltip popup's click-through, the display diagnostics' screens-only
report and the GIF playback weren't run there either. Also untested: the stderr capture
(`dup2`), the toast and toolbar's borderless topmost windows, the popup
menus (no window regions outside Windows: the whole work-area window takes
the pointer), the image viewer's full screen, rfd's pickers.
