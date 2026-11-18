# Actions harness

Checks ShareXrs's actions and watch folders against ShareX's own code:
`ActionsHarness` runs ShareX.HelpersLib's `ExternalProgram` and
`CodeMenuEntryActions` and ShareX's `WatchFolder` from the reference build;
ShareXrs's tests replay the same cases and compare with what it wrote.

**Nothing outside the scratch folder is touched.** The only program the
actions run is `crates/sharex-settings/examples/sharex_settings_action_probe.rs`,
which logs the command line it got and copies, writes or appends to files in
the case's own folder. The watch cases make and move files in their own
folders.

```sh
tools/actions-harness/run.sh
```

Needs the .NET 11 SDK, cargo, and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (`SHAREX_BUILD` to change it). Windows
only (ShareX's build is).

- `--actions`: `crates/sharex-settings/tests/fixtures/actions/cases.json`:
  `CodeMenuEntryActions.Parse` patterns, `ExternalProgram.CheckExtension`
  paths and filters, the classes' Json.NET output, and `ExternalProgram.Run`
  cases (arguments and their tokens, output extensions, the extensions filter,
  deleting the input, quoted, missing, odd and Unicode paths, an environment
  variable in the program path, hidden window, waiting for exit). A run's
  result is `Run`'s return value, the files left after
  `DeletePendingInputFile`, and the probe's log (the `CreateProcess` command
  line and the arguments it parsed), with the case folder as `<dir>` and the
  probe as `<probe>`. ShareX's results go to `results-sharex.json` beside it;
  `cargo test -p sharex-settings --test actions` compares ShareXrs's.
- `--watch`: `crates/sharexrs/src/watch_folder/fixtures/cases.json`: a
  `WatchFolder` per case on its own folder, enabled, then the case's steps
  (files written, appended to, held open, deleted, moved in, renamed), and the
  files `FileWatcherTrigger` gave with the milliseconds since the steps began.
  ShareX's results go to `results-sharex.json`;
  `cargo test -p sharexrs watch_cases_as_sharex` runs ShareXrs's watcher over
  the same cases and wants the same files, each within 400 ms.

The cases run side by side, so the harness raises the thread pool's minimum:
otherwise ShareX's watcher callbacks queue behind the cases' own sleeping
threads and its timings stretch by seconds.
