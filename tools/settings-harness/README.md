# Settings backup harness

Checks ShareXrs's settings backups (Application settings › Settings ›
Export, Import) against ShareX's own code: `SettingsHarness` runs
ShareX.HelpersLib's `ZipManager` from the reference build the way
`SettingManager.Export` and `SettingManager.Import` call it, and
`cargo test -p sharex-helpers --test zip` replays the same cases on
`sharex_helpers::zip_manager` and compares.

`SettingManager` itself isn't called: it reads and writes the real personal
folder through `AppPaths`. Its own logic is small and ported line by line in
`crates/sharexrs/src/settings_backup.rs`: which entries an export has, the
import filter (`json`, `xml`) and limit (1,000,000,000 bytes), the reset.

No window opens, nothing is uploaded, and the only files written are under
`target/settings-harness-scratch` and the fixtures.

```sh
tools/settings-harness/run.sh
```

Needs the .NET 11 SDK, Python, cargo, and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (`SHAREX_BUILD` to change it). Windows
only (ShareX's build is).

- `make_cases.py` writes the archives and `cases.json` in
  `crates/sharex-helpers/tests/fixtures/zip/`: a ShareX backup, names that
  climb out of the folder (`../`, `sub/../../`), rooted names (`/x`, `C:/x`),
  folder entries, upper-case and other extensions, a backslash in a name,
  `./`, a trailing dot, an archive at and over the size limit, and a file
  that isn't a zip.
- `--extract`: each case through `ZipManager.Extract(archive, dest, retain,
  filter, max)` (the import's filter is `FileHelpers.CheckExtension(entry.Name,
  ["json", "xml"])`); the destination is a folder inside the case's folder,
  so a name that escapes one level shows up. Results: every file (bytes and
  last write time) and folder, or the exception's type and message, in
  `results-sharex.json`.
- `--compress`: the export's entries (three settings files from memory, the
  history file from disk, a missing file) through `ZipManager.Compress`,
  then ShareX's `ZipFile.OpenRead` lists both its archive and ShareXrs's
  (`sharexrs-export.sxb`, written first by the Rust test with
  `SHAREXRS_WRITE_FIXTURES=1`): names, order, lengths, CRCs, bytes and time
  stamps, in `export-sharex.json`. The last case of `--extract` imports
  ShareXrs's archive with ShareX's code.

Last run: 9/9 extract cases identical (files, bytes, time stamps, the
too-large message; a non-zip fails on both sides with different messages:
.NET's "End of Central Directory record could not be found." and the zip
crate's), and ShareXrs's export laid out as ShareX's (same entries in the
same order with the same bytes; the history file keeps its time, the
settings files get the export's).
