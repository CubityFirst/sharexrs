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
| task-option-menus time-zone | 1.29% | 1.85% | `task-settings-upload-file-naming --height 1100 --click "270,434;600,471"`, cropped with `crop-match.py`; light: the popup window's 1px outline against the page is 1.1% of it |

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
| hotkey-task-scroll task override-off | 0.43% | 0.40% | `--state overrides --text "custom:Generated reference dark"` (light: `... light`), ShareX's profile's custom uploader under Uploader accounts |
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
uploader filters light 1.29%: ShareX lists its services there; now 0.31/0.23%
with the services, below).

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

## The last Task settings gaps

| Piece | ShareX source | ShareXrs |
|---|---|---|
| Name pattern, pixel info and upload info variable menus | `NamePatternText`, `PixelInfoText`, `UploadInfoText`, `CodeMenuText`, `InsertText`; `CodeMenuEntryPixelInfo` | `variable_menu.rs` (`VariableMenus`); `sharex_helpers::code_menu::PIXEL_INFO` |
| Uploader accounts | `BuildTaskPage` (`FTPAccountList`, `CustomUploadersList`, `ObjectCombo`, `BetweenOrDefault`) | `task_settings_window.rs` (`UploaderAccounts`, read when the window opens through `UploadersConfigStore::peek`, as `UploadersConfigOrNull`) |
| Destinations menu commands | `DestinationMenu`, `MainMenuBuilder.BuildDestinationsMenu(settings)`, `CreateDestinationMenuItem` | the menu built from and applied to the edited task (`menu_settings::fill`/`apply`); its items close the menu and aren't bold, as `CreateDestinationMenuItem` drops `StaysOpenOnClick` and `BoldWhenChecked` |
| Uploader filter services | `BuildUploaderFiltersPage`, `UploaderFactory.AllGenericUploaderServices.OrderBy(ServiceName)`, `UploaderFilter` | `page_uploader_filters` over `upload::filters::Service::all()` (culture-aware stable sort), Add/Update/Remove, `LoadSelection` |
| Sound file picker | `SoundPath`, `PickFileAsync(ChooseAudioFile, WaveAudio, "*.wav")` | `pick_sound_file` (rfd) |
| Device lists | `AudioDeviceCombo`, `CaptureDeviceCombo`, `AudioCaptureDevices`, `CameraCaptureDevices` | the native recorder branch's `recording::bar_sources` (`cached_devices`, `form_choices`), taken at the merge with master: this branch had its own enumeration and dropped it |
| Tooltips | `Row(label, editor, tooltip)` (Mouse wheel), `CreateClipboardTokenItem` | `row_tooltip`; `MenuEntry::tooltip` in `sharex_ui::menu`; the clipboard format menu's descriptions |
| Mouse highlighter colours | `MouseHighlighterSettingsControl.ColorRow`, `EnabledColorRow` (`Flyout` with `ColorView`) | `mh_color_row` with `controls::color_popup_with_alpha` (a `Flyout` with Avalonia's `ColorView`, `sharex_ui::color_view`; at 125% 0.09% from ShareX's live shot); writes `DrawingColor.FromArgb` as `ColorConverter` does |
| Time zones | `TimeZoneInfo.GetSystemTimeZones()`, `ObjectCombo` (`TimeZoneInfo.Equals`), `TaskSettingsUpload.CustomTimeZone`, `NameParser.CustomTimeZone`, `TaskHelpers.GetFileName` | `sharex_helpers::time_zone`: the registry list read as .NET's `TimeZoneInfo.Win32.cs` reads it, .NET's own UTC zone, the Newtonsoft JSON, .NET 11's `GetOffsetForUtcDate`/`CacheTransitionsForYear` transcribed, `SettingsBase`'s `DateTimeZoneHandling.Local` reload; `NameParser::custom_time_zone`; `time_zone_row` |
| Dropdowns | Fluent's `ComboBox` (`MaxDropDownHeight="504"`, `ScrollIntoView` on open, a `VirtualizingStackPanel`) | `controls::dropdown`: capped at 504, the selected item scrolled into view (the least scrolling, so one further down ends at the bottom), as wide as the items in view (growing as wider ones scroll in). Short lists are unchanged |

### Time zones

ShareX saves the chosen zone with Newtonsoft's `ISerializable` contract
(`Id`, `DisplayName`, `StandardName`, `DaylightName`, `BaseUtcOffset`,
`AdjustmentRules` with their `TransitionTime`s, `SupportsDaylightSavingTime`;
dates written in UTC, `DateTimeZoneHandling.Utc`), and loads it with
`DateTimeZoneHandling.Local`: each date goes through `DateTime.ToLocalTime`
with the machine's zone. Where the machine's zone is UTC or GMT-like on 1
January (where the rules' dates are) nothing moves; elsewhere the rules'
dates and transition times move by the local offset, so after a restart
ShareX converts names with shifted transitions and the combo no longer finds
the zone (it shows the first item and keeps the saved zone until another is
picked). ShareXrs does the same: a zone picked this session is used as
listed (`time_zone::note_chosen`), one read from a settings file as loading
left it (`time_zone::from_settings`). The file's JSON is kept as read;
ShareX would write the shifted dates back in UTC on its next save, which
reads back the same (the zone in memory is identical either way).

`tools/settings-harness --time-zones` (its README) records .NET 11's list,
JSON and offsets; `cargo test -p sharex-helpers --test time_zones` replays
them: 144/144 zones' JSON and offsets 1970-2040 identical; reloaded on a
Tokyo and a California machine (`HARNESS_LOCAL_TZ`), 144/144 each (JSON,
offsets, the item the combo selects); on this machine the registry list as
ShareXrs reads it equals .NET's (144/144, same order). The list is only
compared on the machine that recorded it (Windows build and UI language),
and is empty on macOS and Linux (ShareX's IANA list there isn't ported; the
saved zone shows alone).

### Rendering

- `task-settings-<page> --content variables:<box>`: a variable menu open
  (`capture-name`, `window-name`, `region-info`, `picker-Format`,
  `picker-FormatCtrl`, `picker-InfoText`, `clipboard-content`,
  `balloon-tip`, `open-url`).
- `task-settings-task --text "ftp:A|B;custom:C|D"`: uploader accounts.
- File naming lists this machine's zones (the registry).

### Validation

| Shot | Dark | Light | Notes |
|---|---|---|---|
| hotkey-task-scroll task override-off | 0.43% | 0.40% | Uploader accounts (was 0.67 / 1.03%) |
| tasksettings uploader-filters | 0.31% | 0.23% | the services listed (was 0.37 / 1.30%) |
| task-option-menus time-zone | 1.29% | 1.85% | the dropdown (above) |
| tasksettings screen-recorder | 0.63% | 0.57% | before the merge; master's device code draws the same closed combos |
| tasksettings region-capture | 0.55% | 0.50% | regression |
| tasksettings mouse-highlighter | 0.62% | 0.47% | regression |
| task-overrides destinations-menu | 0.74% | 0.51% | unchanged with the commands wired |

Built from source, no shot (ShareX's `task-variables` pass was skipped and
`task-option-menus` has only the time zone): the nine variable menus, the
filter services' dropdown, the colour flyouts, the Mouse wheel tooltip
(offscreen renders don't advance time, so no tooltip shows in any render).

### Live checks for the desktop agent

1. Each variable menu: opens on focus and click to the box's right, an item
   goes in at the caret, the box keeps the focus and the menu doesn't reopen;
   Escape and a click elsewhere close it; Advanced's three boxes have Upload
   result first.
2. Uploader accounts with an FTP account and two custom uploaders in
   `UploadersConfig.json`: the check boxes enable the combos; `FTPIndex` and
   `CustomUploaderIndex` saved as ShareX saves them.
3. Destinations menu: picking a service changes the hotkey's
   `ImageDestination` etc. and closes the menu.
4. Uploader filters: Add, select, Update, Remove; `UploaderFilters` JSON as
   ShareX's.
5. Sound "...": the picker's title and its "Wave audio" `*.wav` filter.
6. Mouse highlighter colour buttons: the flyout, alpha, the saved
   `PrimaryColor` etc. ("A, R, G, B").
7. Time zone: the list against ShareX's on the same machine, picking a zone,
   `CustomTimeZone` JSON byte-identical, the preview and a capture's file
   name in that zone; then both apps restarted on a machine set to a non-UTC
   zone: the selection and the names.
8. The Mouse wheel row's tooltip, the clipboard format menu's tooltips.

### Untested on macOS and Linux

The time zone list (empty there: not ported), the sound picker (rfd), the
colour flyout, the variable menus' focus behaviour.

## Not yet

- The time zone list on macOS and Linux (ShareX's .NET lists IANA zones
  from tzdata with ICU names there).
- The canvas fill colour dropdown's screen colour picker button (it closes
  the popup; the window has no picker route yet).
- Shared controls: focus rings and keyboard navigation, Fluent's expanding
  scroll bar, popups past the window's edge. ShareX's capture pass has no
  shots of those states (`shared-controls`, `menu-past-client`), so they
  weren't built.

## Upstream 30d1682

- Escape closes Application settings, Task settings and Hotkey settings
  (ShareX #9304), after what each has open: the clipboard format editor;
  the action, watch folder, toolbar and notification button editors; the
  reset confirmation. A hotkey being recorded still takes Escape. Open
  menus, dropdowns and colour popups take it first. Checked live in both
  apps; `escape_closes_the_window_once_nothing_else_takes_it`.
- The clipboard format editor's Save needs a description and a format
  (#9305): "Description can't be empty." / "Format can't be empty." under
  the variables (13px Normal, `TaskMode.Cancel.Border`), that box focused
  with its text selected. `--render settings-clipboard-formats --state
  add,no-formats,description-empty --scale 1.25 --width 1188 --height 900`
  vs `upstream-30d1682/clipboard-format-description-empty-dark.png`:
  0.90% (the nav's 125% spacing and text anti-aliasing), the error row
  0.24%. `save_needs_a_description_and_a_format`.
- Region capture's Mouse 4/5 click lists "Capture selected region"
  (#9277).
