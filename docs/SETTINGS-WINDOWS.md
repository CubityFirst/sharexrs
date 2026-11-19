# Settings windows: the last states

What finished Application settings, Task settings and Hotkey settings
against ShareX's `settings` and `theme-scroll` reference shots
(`G:/scripts/sharexrs-ref/shots/sharex/settings/`, `.../theme-scroll/`,
listed in `docs/REFERENCE-SHOTS.md`), how to render each state, and what's
left. The windows themselves are in PORTING.md's Phase 4 rows.

## What was added

| Piece | ShareX source | ShareXrs |
|---|---|---|
| Export, import, reset of the settings | `SettingManager.Export`, `Import`, `ResetSettings`; `ApplicationSettingsViewModel.ExportAsync`, `ImportAsync`, `ResetAsync`; `ApplicationSettingsWindow.OnExportClick`, `OnImportClick` | `settings_backup.rs`; the zip format in `sharex_helpers::zip_manager` (`ZipManager`, `ZipEntryInfo`) |
| The busy banner | `IsBusy`, the status `Border` with its `ProgressBar` | `app_settings.rs` (`start_busy`, the banner) |
| Variable menus of the sub folder patterns and the clipboard format | `AttachFilenamePatternMenu`, `AttachClipboardFormatMenu`, `InsertText`, `InsertClipboardToken`; `CodeMenuEntryFilename` | `app_settings.rs` (`filename_pattern_menu`, `clipboard_format_menu`, `insert_at_caret`); `sharex_helpers::code_menu` |
| Notification buttons editor | `NotificationButtonsEditorOverlay`, `ShowNotificationButtonsEditor` and its handlers | `task_editors.rs` (`NotificationButtonsEditor`) |
| Select region... | `RegionCaptureTasks.GetRectangleRegionAsync` from the Capture page | `auto_capture/mod.rs` (`select_region_for_task`, the auto capture's selection with a target) |
| White tray icon | `UseWhiteShareXIcon`, `ShareXResources.IconBytes`, `MainForm.UpdateTrayIcon` | `tray/mod.rs` (`white_icon`, `current_icon`), `App::apply_tray_icon_setting` |
| Integration check boxes behind a trait | `RefreshIntegrations`, `IntegrationHelpers.Create*` | `integration::IntegrationBackend` (`SystemIntegration`, `NoIntegration`); `ApplicationSettings::with_integration` |
| Custom text multiline | `TaskSettingsPageBuilder` (`TextCustom`: `AcceptsReturn`, `MinHeight="80"`) | `settings_form::Form::text_area_row` |
| Destinations menu as the Task page builds it | `DestinationMenu`, `CreateDestinationMenuItem`, `CreateAccentMenuIcon` | `task_settings_window.rs` (`destination_items`), `MenuIcon::Accent` |

Fixes the comparisons found:

- `InputWithButton` rows (sound paths, the screenshots folder): the text box
  stretches to the row the button's margins make (40 tall, not 32).
- The editor overlays (actions, watch folders, notification buttons, the
  clipboard format) are on egui's `Middle` order: on `Foreground`, egui
  draws a layer that isn't an `Area` after the `Area`s, so the overlay
  covered its own menus (the action editor's arguments menu had never
  shown).
- Editor cards are centred as Avalonia rounds a half pixel (to even): the
  action editor sat 1px low.
- The Task page's destinations menu: no accent on the service names
  (`Header` alone), icons at 16 * 16/18 (an 18-wide `IconPresenter` in
  Fluent's 16x16 icon `Viewbox`).
- An import or a reset unregisters the old hotkeys before registering the
  new ones and shows "Hotkey registration failed" if some fail
  (`HotkeyManager.UpdateHotkeys`).

### Export, import, reset

- **Export** saves every settings file (`SaveAllSettings`), then on a
  thread writes the `.sxb` zip: `ApplicationConfig.json`,
  `UploadersConfig.json` (always under that name, its secrets in plain text
  as `SaveToMemoryStream(false)` writes them: `dpapi::without_encryption`)
  and `HotkeysConfig.json` with Settings, `History.db` with History (history
  closed meanwhile). The picker suggests
  `ShareXrs-<version>-<machine>-backup.sxb` (ShareX's name under
  ShareXrs's), filters "ShareX backup" `*.sxb` and "All".
- **Import** extracts only `.json` and `.xml` entries into the personal
  folder (so, as in ShareX, a backup's `History.db` isn't imported), with
  `ZipManager.Extract`'s guards: at most 1,000,000,000 bytes uncompressed,
  rooted names and names that leave the folder skipped. Then every settings
  file loads again (with the backups' fallbacks), the language, the window
  (`Reload()`: first page, clipboard formats, integrations, startup) and the
  app (`ReloadAsync`: settings applied, hotkeys, watch folders, menus, tray).
- **Reset** asks "Would you like to reset ShareX settings?" (Yes/No,
  exclamation), deletes the three files, loads them without fallbacks,
  saves, and reloads as an import does; the status says "Settings reset.".
- Errors: ShareX's message box (`SettingManager_ExportBackupError` /
  `ImportBackupError`, "ShareXrs - Error") and the status "Backup export
  failed." / "Backup import failed.". ShareX formats the exception with its
  stack trace; ShareXrs shows the error's message.

`tools/settings-harness` runs ShareX.HelpersLib's own `ZipManager` over the
same cases as `cargo test -p sharex-helpers --test zip`: 9/9 extractions
identical (files, bytes, time stamps, the too-large message; a file that
isn't a zip fails on both sides with different messages), and ShareXrs's
export laid out as ShareX's (same entries, order, bytes, CRCs; the history
file keeps its time stamp, the settings files get the export's). ShareX's
`SettingManager` isn't run itself: it works on the real personal folder.

## Rendering

`sharexrs --render <screen> --theme dark|light --out x.png`; new options:

- `settings-<page>`: `--scroll N` (`100000`: the end), `--state` a comma
  list of `none-selected`, `busy-export`, `busy-import`, `busy-phase:<s>`
  (the progress bar's animation time), `exported`, `export-failed`,
  `imported`, `import-failed`, `reset`, `add`/`edit` (the clipboard format
  editor), `no-formats`. Renders read no integration from the system
  (`NoIntegration`).
- `settings-reset-confirm`, `settings-import-error` (message boxes).
- `path-variables-menu --state capture|window`, `clipboard-format-menu`:
  the menus alone, as their popup windows.
- `task-settings-<page>`: `--scroll N`; `--content notification-buttons`,
  `notification-buttons-empty`, `notification-buttons-add-menu`;
  `--state no-actions` (no installed image editors added),
  `region:X,Y,W,H`, `overrides` (after capture, after upload and
  destinations overridden).
- `hotkeys --state custom:N` (hotkey N with custom settings).
- `region-rest --select-only` (the overlay as Select region... opens it).
- `custom-uploader-import-prompt --text <name>`.
- `tray-icon [--state white] [--width N]`.

Scripts: `scripts/scroll-offset.py <shot> <tall render>` finds a scrolled
shot's offset in an unscrolled render (`--height 4000`);
`scripts/crop-match.py <popup shot> <window render> <out>` crops a window
render to a popup shot for `compare.py --no-crop`.

## Validation

Pixels differing by more than 40, dark / light. "Built from source, no
shot" where ShareX's capture pass has none.

### Settings (`settings/`)

| Shot | Dark | Light | Render / notes |
|---|---|---|---|
| settings-export options | 0.32% | 0.24% | `settings-settings` |
| settings-export none-selected | 0.30% | 0.21% | `--state none-selected` |
| settings-import invalid-archive (busy) | 0.32% | 0.42% | `--state busy-import`; light: the indicator's animation phase |
| settings-reset confirm | 0.39% | 0.20% | `settings-reset-confirm` |
| path-variables capture-menu | 0.50% | 0.35% | `path-variables-menu --state capture` |
| path-variables window-menu | 0.44% | 0.28% | `--state window` |
| clipboard-format add | 0.41% | 0.32% | `settings-clipboard-formats --state busy-import,busy-phase:0.145,add,no-formats` (ShareX's shot kept the busy banner of the import before it) |
| clipboard-format variable-menu | 0.42% | 0.24% | `clipboard-format-menu` |
| notification-buttons default | 0.29% | 0.26% | `task-settings-general-notifications --scroll 37 --content notification-buttons` |
| task-actions-editor add | 0.27% | 0.22% | `task-settings-actions --content action-editor --state no-actions` |
| watch-folder-editor add | 0.34% | 0.26% | `task-settings-watch-folders --content watch-folder-editor` |
| task-overrides destinations-menu | 0.74% | 0.51% | `task-settings-task --hotkey --click "270,546;585,587"`, cropped; the rest is the popup's corners over the page |
| task-capture-settings region-picker | 0.04% | 0.04% | `region-rest --select-only --pointer 512,384` (light 470,350) |
| task-capture-settings region-values | 0.65% | 0.56% | `task-settings-capture --state region:205,170,266,181` |
| custom-uploader-import cli-prompt | 0.86% | 0.57% | `custom-uploader-import-prompt --text "Generated reference dark"` |
| task-option-menus time-zone | — | — | not ported: the system time zone list (below) |

Built from source, no shot: the export's busy state and every finished
status, the import error box, the notification buttons editor empty and with
its Add... menu, the reset's result, the white tray icon.

### Theme and scroll gaps (`theme-scroll/`)

Scroll offsets found with `scroll-offset.py`; each "bottom" shot also
matches ShareXrs's own end of the page (`--scroll 100000`), so the pages are
as tall as ShareX's.

| Shot | Dark | Light | Notes |
|---|---|---|---|
| app-scroll theme mid01 / bottom | 0.30 / 0.30% | 0.22 / 0.23% | scroll 132 / 347 |
| app-scroll advanced mid01 / bottom | 0.42 / 0.51% | 0.46 / 0.46% | 393 / 1035 |
| task-scroll advanced mid01 / bottom | 0.52 / 0.53% | 0.45 / 0.41% | 234 / 616 (after the multiline custom text) |
| task-scroll capture mid01 / bottom | 0.66 / 0.66% | 0.57 / 0.57% | 30 / 78, `--state region:205,170,266,181` (the profile's region) |
| task-scroll mouse-highlighter mid01 / bottom | 0.62 / 0.62% | 0.47 / 0.47% | 4 / 10 |
| task-scroll notifications mid01 / bottom | 0.60 / 0.45% | 0.53 / 0.37% | 210 / 552 (after the 40px sound path rows) |
| task-scroll region-capture mid01 / bottom | 0.63 / 0.55% | 0.56 / 0.47% | 199 / 524 |
| task-scroll screen-recorder mid01 / bottom | 0.62 / 0.47% | 0.59 / 0.48% | 319 / 840 |
| hotkey-task-scroll advanced bottom / top | 0.47% / — | 0.42 / 0.23% | `--hotkey` |
| hotkey-task-scroll capture bottom / top | 0.60% / — | 0.62 / 0.61% | |
| hotkey-task-scroll notifications bottom / top | 0.36% / — | 0.29 / 0.46% | |
| hotkey-task-scroll region-capture bottom / top | 0.45% / — | 0.33 / 0.32% | |
| hotkey-task-scroll task override-off | 0.67% | 1.03% | `--state overrides`; the rest is Uploader accounts, where ShareX's profile had a custom uploader (accounts aren't shown yet) |
| settings-light rest | — | 0.25% | `hotkeys --none 0,1,2,3,4 --state custom:0 --width 680 --height 600` |
| settings-light task-tools | — | 0.23% | `task-settings-tools` |
| app-menu-light upload / debug | — | 0.49 / 0.60% | `main-<menu>-menu`, cropped |
| app-menu-light after-upload-tasks | — | 1.65% | ShareX's profile had no default after upload task (ShareXrs's default Copy URL is bold) |
| capture-gaps region rest / hover / drag | — | 0.04 / 0.04 / 0.05% | `region-rest --pointer 470,350`, `region-hover --pointer 285,215`, `region-drag --drag 200,150,520,390` |
| main-tasks-light (4) | — | — | not comparable: the thumbnails are captures from ShareX's session, deleted at teardown |
| destination-light (3) | — | — | Destination settings: another branch's work |

Regression pass after these changes, every Application settings and Task
settings page against its first-screen shot: all equal to or below the
numbers PORTING.md had (paths 1.14/0.95%: its previews show other folders;
uploader filters light 1.29%: ShareX lists its services there).

## Live checks for the desktop agent

Side by side with ShareX, same profile, scratch folders only:

1. Export with Settings and History, with one of them, with neither (Export
   disabled); the picker's title, file name and filters; the busy banner;
   the `.sxb`'s entries (`ApplicationConfig.json`, `UploadersConfig.json`,
   `HotkeysConfig.json`, `History.db`), and that ShareX imports ShareXrs's
   backup and the other way round.
2. Import of a valid backup (settings, hotkeys, uploaders reloaded; the
   window back on General; the language; the hotkeys re-registered), of a
   file that isn't a zip (error box, "Backup import failed."), of an archive
   with a `../` entry (nothing written outside the folder).
3. Reset: No changes nothing; Yes resets all three files, the theme and
   hotkeys follow, "Settings reset.".
4. Sub folder pattern boxes: the menu opens on focus and on click, to the
   box's right, closes on a click elsewhere or Escape; an item goes in at
   the caret (keeping the selection), the menu doesn't reopen; the window
   box's menu has Window and Image.
5. Clipboard format editor: right click in Format opens the menu at the
   pointer; an item replaces the selection; Escape closes the menu first,
   then the editor; the tokens' tooltips (ShareXrs has none yet).
6. Notification buttons editor: Configure... opens it on the task's buttons;
   Add... lists the missing actions (plain icons, as ShareX's have no accent class), Remove, up and
   down, Save and Cancel, Escape and a click outside; the saved
   `ToastWindowButtons` identical in both apps' JSON.
7. Select region...: the overlay without annotations, the values in X, Y,
   Width, Height, saved in `CaptureCustomRegion`.
8. Use white ShareX icon: the tray icon and the main window's icon switch at
   once and at startup; the progress icon still shows during a task.
9. Integration check boxes: each one on and off, against ShareX's
   registration (ShareXrs's own keys, `docs/RELEASING.md`), and the Send To
   error text.

## Untested on macOS and Linux

The pickers (rfd), the white tray icon on their trays, the registry-free
integration (every box reads off and does nothing there), the zip import's
rooted-name rule (Windows treats `C:` and `\` as rooted; the comparisons
with ShareX's Windows results run on Windows only), the region selection.

## Not yet

- The time zone list (Task settings › File naming): `TimeZoneInfo.GetSystemTimeZones`
  and the `CustomTimeZone` JSON ShareX writes (its adjustment rules) need a
  port of .NET's Windows time zone data; the saved zone still shows alone.
- Task settings' other variable menus (`task-variables`: name pattern,
  pixel info, upload info), Uploader accounts (FTP accounts, custom
  uploaders), the sound file picker, uploader filter services, device lists.
- Shared controls: focus rings and keyboard navigation, Fluent's expanding
  scroll bar, popups past the window's edge. ShareX's capture pass has no
  shots of those states (`shared-controls`, `menu-past-client`), so they
  weren't built.
- Tooltips on the clipboard format menu's items.
