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
| Display diagnostics | `Presentation/DisplayDiagnostics` | `dialogs/display_diagnostics.rs` | Debug › Display diagnostics. The report has what every platform knows (ShareX's `CollectAvaloniaOnly` plus the virtual screen, monitors, `AppendArrangement`, cursor, this window, capture settings); ShareX's Windows-only probes aren't ported (DPI awareness and compatibility layers, display modes, devices and per-monitor DPI, taskbars, graphics adapters, the region capture's window log) |
| Notification toast | `Presentation/Notification` | `dialogs/notification.rs` | `TaskManager` on a finished task (`ShowToastNotificationAfterTaskCompleted`, not on a fullscreen window with `DisableNotificationsOnFullscreen`), an upload error, and `ShowNotificationTip`: minimized to the tray (first time), hotkeys toggled, the quick colour picker. One at a time; dark whatever the theme; never activated |
| After capture | `Presentation/AfterCapture` | `dialogs/after_capture.rs` | `UploadManager.RunImageTask` and `ScreenRecordManager.FinishRecording` with `ShowAfterCaptureWindow`: Continue starts the task with the goal's tasks and the file name; a recording is renamed first |
| Image viewer | `HelpersLib/Presentation/ImageViewer` | `dialogs/image_viewer.rs` | A thumbnail's click and the list's preview (the task list's images), history's Show image, the after capture, before/after upload, clipboard and QR code windows' previews, Tools › Image viewer and its hotkey (a file picked first), `-ImageViewer <file>` |
| Workflows menu | `MainMenuBuilder.BuildWorkflowsMenu` | `main_menu.rs` | The sidebar and the tray, from `HotkeysConfig` (the tray's without "Add workflows…"); a click runs the hotkey's task |
| Tray Recent items | `BuildRecentItemsMenu`, `RecentTask` | `main_menu.rs`, `tasks.rs` | With `RecentTasksSave` and `RecentTasksShowInTrayMenu` |
| Tool grid edit mode | `MainWindow.PopulateGridMenuCategories` | `sharex_ui::menu` | Tools › Edit (sidebar and tray): every tool with its switch; `HiddenTools` saved at once |
| Quick task menu | `QuickTaskMenu.cs` | `quick_task.rs`, `tray_menu.rs` | `RunImageTask` with `ShowQuickTaskMenu`: a popup with Continue centred on the pointer, closed only by its items or Escape |
| Quick task menu editor | `Presentation/QuickTaskMenuEditor` | `dialogs/quick_task_editor.rs` | The quick task menu's Edit this menu…, Application settings › General |
| Actions toolbar | `Presentation/ActionsToolbar` | `dialogs/actions_toolbar.rs`, `main_commands.rs` | Tray › Actions toolbar, its hotkey, `ActionsToolbarRunAtStartup`; title drag (unless locked), middle click closes, right click opens its menu (a popup), files dropped are uploaded |
| Actions toolbar editor | `ActionsToolbarEditorWindow` | `dialogs/actions_toolbar.rs` | The toolbar's menu › Edit… |
| Output box | `HelpersLib/Presentation/OutputBox` | `dialogs/output_box.rs` | A failed FFmpeg run (`FFmpegCLIManager.ShowError`), scrolled to the end |
| Missing file | `TaskHelpers.AnnotateImageFromFile` | `image_editor/mod.rs` | Editing a task's or history item's image file that's gone |
| Update message, downloader | `HelpersLib/Presentation/UpdateChecker` | `dialogs/update.rs` | (Ported earlier.) About's link opens the update message |

## Rendering

`sharexrs --render <screen> --theme dark|light --out x.png`:

- `file-exists` (`--text <path>`, `--state empty`, `--state name=<text>`)
- `icon-pack-prompt` (`--state legacy`)
- `about` (`--state checking|up-to-date|available|failed|disabled`, `--text` product, `--content` brand)
- `debug-log` (`--files <log.txt>`, `--text` startup path, `--state upload-confirm`)
- `display-diagnostics` (two synthetic monitors)
- `notification` (`--state tray-minimized`, or `--image`/`--text` for a task's; `--content hover`, `hover:<n>`, `tip:<n>`); borderless, on black: compare with `compare.py --no-crop`
- `after-capture` (`--image`, `--files <recording>`, `--text` file name, `--content <AfterCaptureJob>`, `--state goal:<n>`)
- `image-viewer` (`--image` or `--files a,b --count <shown>`, `--width/--height`)
- `main-workflows-menu` (`--state workflows-unbound`), `main-tools-menu --state tools-edit`
- `quick-task` (the popup, `--hover`), `quick-task-editor` (`--state add-overlay|edit-overlay|reset-confirm`)
- `actions-toolbar` (`--text` title, `--content hover:<n>`), `actions-toolbar-menu`, `actions-toolbar-editor` (`--state add-menu`)
- `output-box` (`--files <log>`), `task-missing-file` (`--text <path>`)

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
message, the update message and downloader.

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

## Untested elsewhere

macOS and Linux build and lint, nothing was run there: the stderr capture
(`dup2`), the toast and toolbar's borderless topmost windows, the popup
menus (no window regions outside Windows: the whole work-area window takes
the pointer), the image viewer's full screen, rfd's pickers.
