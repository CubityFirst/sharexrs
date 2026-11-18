# Update harness

Checks ShareXrs's update check (`crates/sharex-update`) against ShareX's own
code: `dotnet/` (`UpdateHarness`) runs ShareX.HelpersLib's `GitHubUpdateChecker`,
`FileDownloader`, `System.Version` and `Helpers.CompareVersion` from the reference
build, and `crates/sharex-update/examples/update_harness.rs` runs ShareXrs's. Both
use the same mock GitHub API, `tools/uploaders-harness/mock_server.py`.

**Nothing leaves the machine.** Both harnesses rewrite every URL to
`http://127.0.0.1:<port>/__/<scheme>/<host><path>` before sending it (ShareX: the
`HttpClientFactory` clients' handler is swapped by reflection, as in the uploaders
harness; ShareXrs: `http::set_url_rewriter`). The releases and assets are made up.

```sh
tools/update-harness/run.sh            # port 18766
```

Needs Python 3, the .NET 11 SDK, cargo, and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (`SHAREX_BUILD` to change it).
`make_cases.py` writes `cases.json` (not committed; about 1 MB):

- `check` cases: one `CheckUpdateAsync` against a canned `/releases/latest` or
  `/releases` answer, with the portable flag, the pre-release channel,
  `IgnoreRevision`, `IsDev` and the current version. The results are the status,
  versions, download URL, file name, `IsPreRelease`, and the requests as they
  arrived (without the user agent).
- `download` cases: `FileDownloader.StartDownload` with an Accept header. The results
  are the return value or exception, sizes, the file's SHA-256, and the requests.
- `version`: `new Version(s)` for odd strings, and `CompareVersion` pairs.

A case marked `addition` is where ShareXrs reads more than ShareX on purpose
(SemVer tags). `compare.py` writes `target/update-harness/report.md`.

After a run, the check and version cases and ShareX's results become the crate's CI
fixtures (`crates/sharex-update/tests/fixtures`: `cases.json.gz` and
`results-sharex.json` without the requests). `tests/sharex.rs` replays them through
an in-process mock.
