# Command line harness

Checks ShareXrs's command line against ShareX's own code: `CliHarness` runs
ShareX.HelpersLib's `CLIManager`, the `BinaryWriter` message
`SingleInstanceManager` sends to the first copy, and ShareX's
`JsonHelpers.DeserializeFromFile<NativeMessagingInput>` from the reference
build; ShareXrs's tests replay the same cases and compare.

No window opens, nothing is started, and the only files written are the
native messaging cases in `target/cli-harness-scratch`.

```sh
tools/cli-harness/run.sh
```

Needs the .NET 11 SDK, Python, cargo, and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (`SHAREX_BUILD` to change it). Windows
only (ShareX's build is).

- `make_cases.py` writes the cases.
- `--cli`: `crates/sharex-helpers/tests/fixtures/cli/cases.json`. Each
  argument array goes through `new CLIManager(args).ParseCommands()`: its
  return value, the commands (name, parameter, whether it's a command),
  `IsCommandExist` for every query, `GetParameter` for every name,
  `ToString()`, and the bytes `SingleInstanceManager` would send for it.
  Each string goes through `new CLIManager(string)` (`ParseCLI`). The cases
  cover ShareX's quirks: `/x` and `--x`, a file after a command becoming its
  parameter, empty arguments stopping the parse, ordinal case folding
  (`ı`, `ſ` and `ß` stay themselves), quotes in `ParseCLI` and its dropped
  last token. Results: `results-sharex.json` beside the cases;
  `cargo test -p sharex-helpers --test cli` compares.
- `--native`: `crates/sharexrs-native-messaging/tests/fixtures/cases.json`.
  Each message is written to a file as the host writes it and read as
  `TaskHelpers.HandleNativeMessagingInput` reads it: enum names, numbers and
  flags, wrong types, Json.NET's lenient syntax (single quotes, bare names,
  comments, trailing commas), cut-off and broken messages. Results:
  `results-sharex.json`; `cargo test -p sharexrs-native-messaging --test sharex`
  compares.

Last run: 38/38 command line cases and 72/72 native messaging inputs
identical.
