# ShareX's history

How ShareX 22.4 (`REFERENCE_COMMIT`) keeps and shows its history, and what
ShareXrs has of it. The source is `ShareX.HistoryLib` (~8.6k lines with its
29 `.resx`), plus the places in `ShareX/` that open, write and migrate it.
ShareXrs's port is `crates/sharex-history` (data, no UI yet) and
`crates/sharexrs/src/history.rs` (the app's connection and the task hook);
it's checked against ShareX's own DLL by `tools/history-harness`.

## Where history lives

| What | ShareX | ShareXrs |
|---|---|---|
| History.db | `AppPaths.HistoryFilePath`: `<personal folder>/History.db`, none with `-sandbox` | `AppPaths::history_file()` (ShareXrs's own profile, never ShareX's) |
| Old History.json | `AppPaths.HistoryFilePathOld` | `HISTORY_FILE_NAME_OLD` |
| The open connection | `ApplicationState.HistoryManager` (`HistoryManagerSQLite`), opened by `SettingManager.HistoryConnect` at the end of `LoadApplicationConfig`, closed by `HistoryClose` on exit (`ApplicationLifecycle.CloseSequence`, before the settings are saved) and around settings export/import | `history::connect` after the settings load, `history::close` in `exiting` |
| Settings | `ApplicationConfig`: `HistorySaveTasks` (true), `HistoryCheckURL` (false), `HistorySaveOCRText` (false), `HistorySettings`, `ImageHistorySettings`; Application settings › History has the three check boxes | already in `sharex-settings`; typed window settings in `sharex_history::settings` |

## Storage: History.db

`HistoryManagerSQLite` over Microsoft.Data.Sqlite with the bundled
`e_sqlite3` (SQLite 3.53.4 in the reference build; ShareXrs bundles 3.54.1
through rusqlite, the system's libsqlite3 on macOS).

**Schema.** One table, created on every open with `CREATE TABLE IF NOT
EXISTS` (the text is ShareX's, CRLF line ends included, because SQLite keeps
it verbatim in `sqlite_master`):

```sql
CREATE TABLE IF NOT EXISTS History (
    Id INTEGER PRIMARY KEY AUTOINCREMENT,
    FileName TEXT, FilePath TEXT, DateTime TEXT, Type TEXT, Host TEXT,
    URL TEXT, ThumbnailURL TEXT, DeletionURL TEXT, ShortenedURL TEXT,
    Tags TEXT
);
```

There are no indices and no schema versions or migrations inside the
database. `AUTOINCREMENT` adds `sqlite_sequence`, so a deleted last id is
never reused. Page size, encoding (UTF-8), journal mode (`delete`) and the
rest are SQLite's defaults.

**Values.** Every column is text or `NULL`:

- a null .NET string is `NULL`, an empty one `''`;
- `DateTime` is `DateTime.ToString("o")`: `2026-07-01T10:00:00.1234567+01:00`
  for local times (the local offset at that time, so DST shows), `…Z` for UTC,
  no suffix for unspecified. Tasks write `DateTime.Now` (local);
- `Tags` is `JsonConvert.SerializeObject(Dictionary<string,string>)`, compact
  (`{"WindowTitle":"…","ProcessName":"chrome","Favorite":null}`), in the
  dictionary's enumeration order (insertion order, but a removed key's slot
  is reused by the next added key), Json.NET's escaping (control characters
  and U+0085/U+2028/U+2029 as lowercase `\uXXXX`, everything else as is);
  `NULL` when the item has no dictionary, `{}` for an empty one.

**Reading** (`Load`): `SELECT * FROM History;` (table order, oldest id
first), each column by name through `reader[..].ToString()`, so a `NULL`
string comes back as `""` (not null) and a number as its text. The date goes
through `DateTime.Parse`: a stored offset or `Z` is converted to *local* time
(`Kind = Local`, with .NET's hidden "first occurrence of the repeated hour"
bit when it lands there), no suffix stays `Unspecified`, the fraction is
summed as a double and rounded half to even. A `NULL` or unparseable date, or
`Tags` that aren't JSON, fail the whole load (ShareX then shows "Error
occurred while reading history file" and lists nothing). `Tags` `NULL`, `''`
or `null` read as no dictionary; a repeated JSON key keeps its first place
and last value; a boolean value becomes `true`/`false`, a number its text.
Reading then editing an item writes the `""`s back, so old `NULL`s become
`''` once an item has been edited.

**Writing.** `AppendHistoryItems` keeps the valid items
(`IsValidHistoryItem`: a file name, a date that isn't `MinValue`, and a URL
or a file path) and inserts them in one transaction, each getting its id from
`last_insert_rowid()`. `Edit(item)` updates every column of the row with the
item's id; `Delete(items)` deletes by id; each is its own transaction. Edits
come from the windows: Favorite (`Tags["Favorite"] = null` / removed), Edit
tag (`Tags["Tag"]`, an empty tag removes it), Edit item, Rename file.

**Concurrent access.** One connection for the app's lifetime, `PRAGMA
busy_timeout = 5000`. `BeginTransaction()` is `BEGIN IMMEDIATE` (checked: a
second writer gets `SQLITE_BUSY` at once). Microsoft.Data.Sqlite also retries
a busy statement every 150 ms until its 30 s command timeout. ShareX appends
from a background task (`AppendHistoryItemAsync`, `Task.Run`), so the one
connection is used from several threads; ShareXrs serialises it behind a
mutex. The harness has each app append while the other holds a write
transaction for 2 s: both appends wait it out and land.

**Backups.** History.db itself is never backed up: `HistoryManager` has
`BackupFolder`/`CreateBackup`/`CreateWeeklyBackup`, but only the JSON and
XML managers' `Append` calls `Backup()`, and `HistoryConnect` doesn't set
them. What does happen:

- the startup migration moves History.json into `<personal folder>/Backup`
  (`FileHelpers.MoveFile`, replacing a file of that name);
- Settings › Export with history zips History.db (`SettingManager.Export`
  closes the connection, zips, reconnects). Import extracts only `.json` and
  `.xml` entries, so it never restores a History.db.

### Legacy files and the migration

- **History.json** (`HistoryManagerJSON`, ShareX 11 to 16): Json.NET objects
  (indented, `DefaultValueHandling.Ignore`, so nulls are left out) separated
  by `,\r\n` with no brackets, so items could be appended. Read as
  `"[" + text + "]"`: property names match ignoring case, `Id` and the other
  `[JsonIgnore]` properties are skipped, dates are ISO 8601 (Json.NET:
  `Z` stays UTC, an offset becomes local time, an 8th fraction digit rounds)
  or `/Date(ms[±hhmm])/`.
- **History.xml** (`HistoryManagerXML`, older): rootless `<HistoryItem>`
  elements with `Filename`, `Filepath`, `DateTimeUtc` (`DateTime.TryParse`,
  left at `MinValue` if it fails), `Type`, `Host`, `URL`, `ThumbnailURL`,
  `DeletionURL`, `ShortenedURL` children.
- **Migration** (`SettingManager.MigrateHistoryFile`, at startup before
  `HistoryConnect`): if History.json exists, and History.db doesn't yet, a new
  History.db gets every JSON item through the protected `Append` (no validity
  filter, so an item without a date is stored as `0001-01-01T00:00:00.0000000`);
  then History.json moves to `Backup/` whether or not it was migrated. Errors
  are logged and shown. ShareX 22.4 never migrates History.xml (it can only
  read it); ShareXrs has `migrate_from_xml` for a future tool, not called at
  startup.

### HistoryItem

`Id` (not in JSON), `FileName`, `FilePath`, `DateTime`, `Type` (`EDataType`
by name: `Image`, `Text`, `File`, `URL`), `Host` (`TaskInfo.UploaderHost`:
the destination's localized name when the task uploaded, else null), `URL`,
`ThumbnailURL`, `DeletionURL`, `ShortenedURL`, `Tags`. Derived: `Favorite`
(a `Favorite` key), `Tag`, `TagsWindowTitle`, `TagsProcessName`. Tags a
task adds (`TaskInfo.GetTags`): `WindowTitle` and `ProcessName` of the
captured window, or no dictionary when there are neither; `OCR` (the
recognised text) when `HistorySaveOCRText` is on.

### Where a task writes history

`TaskManager.Task_TaskCompleted`, for every finished task (done, failed or
stopped) whose `info.ToString()` isn't empty (the shortened URL, else the
URL, else the file path): when `HistorySaveTasks`, and with
`HistoryCheckURL` only if the URL or the shortened URL is set, it makes
`TaskInfo.GetHistoryItem()` (date `TaskEndTime`, set to `DateTime.Now` when
the task ends) and adds it from a background task, after waiting for the
task's OCR when `HistorySaveOCRText` started one (`WorkerTask.StartHistoryOCR`,
images only). The item then joins the recent tasks (`RecentManager.Add`).

## The windows

All three are opened by `TaskHelpers` (`OpenHistory`, `OpenImageHistory`,
`OpenStats`) from the sidebar, the tray menu and the `OpenHistory` /
`OpenImageHistory` hotkeys, through `HistoryIntegration`. A
`HistoryWindowServices` gives them the app's actions (upload, image editor,
photo editor, video/GIF trimmer, pin to screen, analyze image, image viewer);
an entry whose service is missing hides. Each loads every item
(`GetHistoryItemsAsync`), reverses them (newest first) and filters in memory
on a background task with a 300 ms debounce (`_filterTimer`).

### History window (`HistoryWindow`, 1300 × 700, min 850 × 560)

- **Toolbar:** search box (an `AutoCompleteBox` suggesting process names,
  `HistoryHelpers.GetProcessNames`: distinct ignoring case, sorted ignoring
  case), Advanced, Favorites (toggle, saved as `HistorySettings.Favorites`),
  Stats, Import, Settings.
- **Search** (advanced panel closed): `HistoryFilter { Filename = text,
  FilterFavorites }`. The text is a wildcard pattern (`?` one character, `*`
  any run, ignoring case, anywhere in the name) matched against the file name
  and every non-empty tag value (so window titles, process names, tags and
  OCR text are searched). Favorites on shows only favourites and ignores the
  text.
- **Advanced panel:** file name (the same pattern), URL (contains, invariant
  culture ignoring case), Type (check box + combo of the distinct types: whole
  value, invariant culture ignoring case), Host (check box + editable combo of
  the distinct hosts: contains), date range (check box, from/to pickers: the
  item's day within the range). The type and host lists are distinct ignoring
  case (first spelling, newest first) and sorted by the current culture (NLS:
  `Amazon S3`, `amazons3`, `Amazon-S3`, since NLS ignores the hyphen). Reset
  filters clears them all.
- **List:** columns `HistoryColumnLayout`: an 18 px icon (star for favourites,
  else by type: image, text, file, link), then Date 105, File name 220,
  File path 250, Type 60, Host 120, URL 250, Shortened URL 180, Process name
  110, Window title 200, Tag 120 (Date, File name, URL shown by default); the
  last visible column fills the row, widths resize down to 40, at least one
  text column stays; the header's context menu toggles columns and resets
  them; saved in `HistorySettings.Columns`. No sorting (always newest first)
  and no paging (a virtualised list of every match). Rows are at least 34 px.
  The title counts the matches; the empty state says there's no history or
  nothing matches. Multi-select; dragging selected rows drags their existing
  files.
- **Preview** (right of a 10 px splitter, the list 650 wide): the selected
  item's image (the local file if it's an image, else the URL if it looks like
  one, downloaded), on a checkered border, scaled down only; "No image
  preview" otherwise; "Select an item to preview" with nothing selected.
  Clicking it opens the image viewer with up to 201 image files around the
  item. There is no field-by-field details panel: the Edit dialog shows the
  fields.
- **Context menu** (hidden with nothing selected; entries and separators
  shown by `HistoryContextMenuHelpers.UpdateVisibility`): Open › URL,
  Shortened URL, Thumbnail URL, Deletion URL, File (Ctrl+Enter), Folder
  (Shift+Enter); Copy › URL (Ctrl+C), Shortened, Thumbnail, Deletion URL,
  File (Shift+C), Image (Alt+C), Text, HTML link/image/linked image, Forum
  (BBCode) link/image/linked image, Markdown link/image/linked image, File
  path (Ctrl+Shift+C), File name, File name with extension, Folder;
  Favorite/Unfavorite; Edit tag; Edit item; Rename file; Delete from history
  (Delete); Delete file and history item (Shift+Delete); Show image preview;
  Upload file (Ctrl+U); Edit image (Ctrl+E); Edit in photo editor; Trim video;
  Trim animated GIF; Pin to screen (Ctrl+P); Analyze image.
- **Keys:** Enter opens the shortened URL, else the URL, else the file; F5
  reloads; Escape closes a dialog, else the window.
- **Dialogs** (an overlay inside the window): Stats (`HistoryHelpers.OutputStats`
  text), Import (a folder, images only, skip duplicates: every file under it,
  oldest change first, as items with the name, path, last write time and
  Image/Text/File), Settings (remember search text, remember window size and
  position), Edit item (every field; the date read-only as `ToString("G")`;
  tags as `key=value` lines sorted by key), prompts for Edit tag, Rename
  (new name, extension kept) and the delete confirmations.

### Image history window (`ImageHistoryWindow`, 1100 × 720, min 720 × 500)

- **Toolbar:** search box (process name suggestions), Favorites, Stats,
  Import, Settings.
- **Filter:** items with a file path; favourites only when on; images and
  videos only when `ImageOnly`; existing files only when
  `FilterMissingFiles`; the trimmed search text as the same wildcard pattern
  against the file name or a tag value.
- **Grid:** thumbnail cards `ThumbnailSize` (250 × 150, at least 32 × 32)
  plus 18 px chrome, as many per row as fit, the file name above, below or
  hidden (`ThumbnailHeaderPosition`), a star on favourites, a play icon on
  videos, a placeholder icon while loading or without a thumbnail.
  Thumbnails come from the shell (`GetFileThumbnail`, so videos get one) or
  are decoded to fit, at the monitor's scale, 2 to 4 at a time, cancelled
  when scrolled away.
- **Paging:** `MaxItemCount` (500) cards at a time; with
  `AutoLoadMoreItems` the next batch loads when the scroll is within a
  thumbnail's height (120 px at least) of the end, or while the view isn't
  full. No sorting.
- **Selection and keys:** click, Ctrl/Shift click, Ctrl+A, arrows by card and
  row, Home/End; Enter shows an image in the viewer, opens text and video
  files, asks before opening anything else; the same copy/open/delete/edit
  shortcuts as the History window.
- **Context menu:** Open image, Open ›, Copy ›, Toggle favorite, Edit tag, Edit
  item, Rename, Delete, Delete file, Upload, Edit image, photo editor, trims,
  Pin, Analyze (same rules).
- **Settings dialog:** thumbnail size, header position, batch size, auto load
  more, filter missing files, images only, remember search text, remember
  window.

### Stats window (`StatsWindow`, 1060 × 900, min 720 × 480)

`HistoryStats.Calculate(items, DateTime.Now, CurrentCulture.FirstDayOfWeek)`
over every item (dates are wall clock times; `MinValue` items skipped):

- each item is a recording (a video file, or any GIF), an image (`Type` Image
  or an image file) or other;
- totals, a tally per day, per hour (0–23) and per weekday;
- the past 7 days (`now - 7d < date <= now`) and the 7 before;
- today's tally; the streak (days in a row with an item up to today, or
  yesterday while today has none); the longest streak and its last day; the
  busiest day (first of the busiest);
- top apps (process names), destinations (hosts) and file types (the last
  extension, 1–6 letters or digits, in capitals): counted ignoring case
  (first spelling kept, trimmed), by count then name; past the top six, when
  there are at least eight, the rest fold into one "Other" entry.

Shown as: the total with "about N a day/week" and the image/recording/other
split bar; Today, Past 7 days (▲/▼ change against the previous week), Streak,
Busiest day cards; a contribution calendar (past year, all years or one year;
four fill levels at the quartiles of the busy days shown; active days count);
an hour chart (All/Images/Recordings, busiest hour, a 1-2-5 grid scale); the
weekday bars from the culture's first day (busiest day); the three top lists.
Refresh reloads; Share renders the whole page and runs it as an image task.

`HistoryHelpers.OutputStats` (the History and Image history windows' Stats
dialog) is a plain text report: total, counts by type with percentages, by
year, file extensions (`tar.gz` kept), hosts and process names, each by
count.

## sharex-history

| Module | ShareX | What |
|---|---|---|
| `datetime` | `System.DateTime`, `DateTime.Parse`, Json.NET dates | ticks + kind + ambiguous bit; `ToString("o")`; the three parsers; the local zone behind `LocalZone` (`SystemZone`, or `RuleZone` in tests) |
| `tags` | `Dictionary<string,string>`, Json.NET | enumeration order with slot reuse; compact JSON as Json.NET writes and reads it |
| `item` | `HistoryItem`, `HistoryItemIconConverter` | the item, `Favorite`/`Tag` setters, `IsValidHistoryItem`, the list icon |
| `sqlite` | `HistoryManagerSQLite`, `HistoryManager` | open/ensure, `load`, `append_history_items`, `append`, `edit`, `delete`, `migrate_from_json`/`xml`; busy timeout and retries, `BEGIN IMMEDIATE` |
| `legacy` | `HistoryManagerJSON`/`XML` (readers), `SettingManager.MigrateHistoryFile`, `FileHelpers.MoveFile` | the old files and the startup migration |
| `query` | `HistoryFilter`, the windows' filter/list code | both filters, distinct types/hosts, process names, paging, column count, auto load threshold, the image viewer's file window |
| `actions` | `HistoryContextMenuHelpers`, the windows' handlers | menu visibility, copy formats, open targets, tags text, rename, delete wording, import folder |
| `settings` | `HistorySettings`, `ImageHistorySettings`, `HistoryColumnLayout` | ShareX's JSON shape, column layout rules |
| `stats` | `HistoryStats`, `HistoryHelpers.OutputStats`, `StatsFormat`, the charts' maths | the stats, the text report, number formats (.NET's rounding), calendar ranges/levels, hour scale |
| `culture` | .NET comparisons under `UseNls` | `CompareStringEx`/`FindNLSStringEx` as .NET calls them on Windows; ordinal ignore case |

### The app hook

`crates/sharexrs/src/history.rs`: `connect` (migration, then open; nothing
in sandbox mode) and `close`. Every capture and upload task runs through the
upload worker (`upload/worker.rs`); when it ends, its `report()` makes
`TaskInfo.GetHistoryItem` (`history::history_item`: file name and path,
`DataType` by name, `UploaderHost` from the task's destination as ShareX
resolves it (HelpersLib's localized name, else the enum's `[Description]`;
null for jobs that don't upload), the upload result's URL, thumbnail,
deletion and shortened URLs, the capture's `WindowTitle`/`ProcessName` tags,
dated now) and hands it over in the `TaskReport`. `Task_TaskCompleted`
(`upload/app.rs`) then applies the `HistorySaveTasks` / `HistoryCheckURL`
rules and appends it on a thread. The upload tests' mock image upload checks
the item it makes.

## Validation

`tools/history-harness` (see its README) runs ShareX's own
`ShareX.HistoryLib.dll` and compares, 44 checks, all identical on Windows in
GMT Standard Time:

- ShareXrs reading what ShareX wrote: items written and then edited by
  ShareX (favourite toggles that free and reuse dictionary slots, tags,
  edits, deletes, an append after deleting the last id), odd stored values
  (dates with `+09:00`, `Z`, no zone, date only, 3 and 8 fraction digits, a
  space, `+0530`, `-05`, no seconds, both readings of the repeated hour;
  `NULL`s, numbers and blobs in text columns; tags `{}`, `null`, `''`,
  repeated keys, numbers, booleans, escapes), and the three failing loads;
- ShareXrs writing the same items and edits: every stored value with its
  SQLite type, the schema text, `sqlite_sequence` and the PRAGMAs equal
  ShareX's database, and ShareX reads it back identically (dates in summer
  and winter, UTC, unspecified, both occurrences of the repeated hour, the
  skipped hour, nulls vs empty strings, control characters, emoji, U+2028
  in tags);
- History.json written by ShareX's JSON manager (two appends) and a
  hand-written old-style one (lower-case names, `/Date()/` with and without
  offset, 9 fraction digits, repeated properties, unknown ones), read and
  migrated: the migrated databases equal ShareX's; History.xml written by
  ShareX and a hand-written one (entities, nested markup, whitespace, a bad
  date, a nested `HistoryItem`) read alike; `MigrateHistoryFile` in a scratch
  profile;
- the queries and stats over both databases: types, hosts, process names, 26
  `HistoryFilter`s, 8 Image history filters, `OutputStats` and five
  `HistoryStats` (different "now"s and first days of the week), plus
  `StatsFormat` (share, number, compact, hours), the rate text, the hour
  scale, calendar levels, the week change and the column layouts;
- concurrent appends both ways, and the reference build's own History.db (a
  copy) read alike.

Unit tests cover the pieces with other time zones (`RuleZone`: London and
Tokyo), the context menu rules and the copy formats.

## Not ported yet

- The three windows (plan below) and their dialogs; the history entries in
  the main window's and tray's menus, the hotkeys' `OpenHistory` /
  `OpenImageHistory` / Stats jobs; Recent items in the tray menu.
- `HistorySaveOCRText` (OCR isn't ported).
- Settings export/import (history zipped in the export).
- Thumbnail loading for the Image history window (Windows shell thumbnails
  for videos); preview downloads; the Stats window's Share image task.
- Culture details off Windows: on macOS/Linux the culture-aware comparisons
  (`OrderBy` of types and hosts, the URL/host/type filters, the Edit dialog's
  tag order) are simple case folding, not NLS; `char.IsLetterOrDigit` for
  non-ASCII extensions is approximate; `DateTime.Parse` handles the ISO 8601
  forms only (ShareX would also take culture formats, which it never wrote).
- The JSON and XML writers (ShareX 22.4 never writes those files; the harness
  uses ShareX's to make fixtures).

## Window plan

Each window ports its `.axaml` and code-behind over `sharex-history`; the
data side is ready, so the work is layout and interaction, validated with
`--render` against ShareX's windows over the same History.db (copy the
harness's `sharex.db` into a scratch ShareX profile for the shots).

1. **History window** (`history_window.rs`): toolbar, list with
   `HistoryColumnLayout` (virtualised rows, icon via `HistoryItem::icon_key`),
   splitter + preview, advanced panel, context menu (entries via
   `actions::menu_entry_available`, separators collapsed like
   `UpdateItems`), the overlay dialogs. Load with
   `HistoryManagerSqlite::load_newest_first` on a thread, filter with
   `HistoryFilter::apply` (300 ms debounce), lists with
   `query::distinct_types/hosts/process_names`. Writes: `edit`, `delete`,
   `append_history_items` (import). Renders: `--render history`,
   `history-advanced`, `history-menu`, `history-edit`, `history-stats`.
2. **Image history window** (`image_history_window.rs`): toolbar, thumbnail
   grid (`query::image_history_columns`, `next_batch`, `should_load_more`),
   the thumbnail loader (decode to fit at the monitor scale; shell
   thumbnails on Windows for videos), selection and keys, the same dialogs.
3. **Stats window** (`stats_window.rs`): the cards, the split bar, the
   calendar (`calendar_spans`, `calendar_range`, `calendar_steps`,
   `calendar_level`), the hour chart (`peak_hour`, `hour_chart_scale`), the
   weekday rows and top lists, with `StatsPalette`'s colours ported from
   `StatsCharts.cs`; dates in the UI culture's short long-date pattern
   (`StatsFormat.Date`), which needs the culture data ShareXrs doesn't have
   yet.
4. Wire `TaskHelpers.OpenHistory/OpenImageHistory/OpenStats` into the
   sidebar, tray menu and hotkeys, with the services that exist (image
   viewer, editor, pin, upload as they're ported).
