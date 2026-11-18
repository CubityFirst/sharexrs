# Uploaders harness

Checks that ShareXrs's uploaders (`crates/sharex-uploaders`) send exactly the
requests ShareX's do and make the same results of the same responses, by
running ShareX's own `ShareX.UploadersLib.dll` (from the reference build)
and ShareXrs side by side against a local mock server.

**Nothing leaves the machine.** Both harnesses rewrite every destination URL
to `http://127.0.0.1:<port>/__/<scheme>/<host><path>` just before sending it
(ShareX: a `DelegatingHandler` swapped into `HttpClientFactory`'s shared
clients by reflection, in front of ShareX's own `SocketsHttpHandler`;
ShareXrs: `http::set_url_rewriter`). Every key, account and value in the
cases is made up.

| File | What |
|---|---|
| `make_cases.py` | writes `cases.json`: per case, a `.sxcu` or a destination with its `UploadersConfig` settings, the input (file bytes, text or URL) and the canned responses |
| `mock_server.py` | the server: records each request as it arrived (request line, headers in order and case, body bytes) and answers with the next canned response; it can also read bodies slowly (`read_chunk`, `read_delay_ms`) and answer late (`delay_ms`), and port 0 picks a free port (ShareXrs's upload task tests start it so) |
| `dotnet/` | the ShareX side (`UploadersHarness`), a console app referencing `ShareX.UploadersLib.dll`, `ShareX.HelpersLib.dll` and `Newtonsoft.Json.dll` from the reference build; it also writes an `UploadersConfig.json` with ShareX's own `Save` (the `sharex-uploaders` round-trip fixture) |
| `../../crates/sharex-uploaders/examples/uploaders_harness.rs` | the ShareXrs side |
| `compare.py` | diffs the two results files |
| `../../crates/sharex-uploaders/examples/sharex_uploaders_mock_servers.rs` | the FTP, FTPS (explicit, implicit), SFTP and SMTP mocks on fixed ports (18821–18825) with a control port (18820) both harnesses reset and read like the HTTP mock's |
| `dotnet/OAuthCases.cs` | the sign-in cases on the ShareX side (ShareX's own `IOAuth*` classes, MEGA's `LoginAsync`) |
| `extract_fixtures.py` | writes `crates/sharex-uploaders/tests/fixtures/servers.sharex.json` (ShareX's FTP/SFTP/SMTP logs) for `tests/servers.rs` |
| `run.sh` | all of it |
| `after_upload_cases.json` | `UploadInfoParser.Parse` and `Regex.Replace` cases: `UploadersHarness --after-upload after_upload_cases.json <out>` runs them through ShareX's own `ShareX.dll`; the output is `crates/sharexrs/src/upload/fixtures/after_upload.sharex.json`, which ShareXrs's tests compare against |

## Running

```sh
tools/uploaders-harness/run.sh            # port 18765
```

Needs Python 3, the .NET 11 SDK, cargo, and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (`SHAREX_BUILD` to change it). The
.NET harness builds to `G:/scripts/sharexrs-ref/uploaders-harness/bin`
(`HARNESS_BIN`), outside the repo; its intermediates go to `target/`.
Results and `report.md` land in `target/uploaders-harness/`.
`HARNESS_ONLY=<case id>` runs one case.

Cases must not use `{inputbox}`, `{prompt}`, `{outputbox}` or `{select}`:
ShareX's code opens Avalonia windows for them (the .NET harness refuses
such cases). They're covered by unit tests with a fake `ParserUi`.

## What's compared

- Each request: method, request target (the escaped URL), HTTP version,
  chunked or not, headers (names ignoring case; order reported on its own),
  and body bytes. Multipart boundaries (random GUIDs) are normalised to
  `BOUNDARY`.
- Results: URL, thumbnail URL, deletion URL, shortened URL, `IsSuccess`,
  `IsError`, `IsURLExpected`, the response text, the response URL and status,
  `CheckConfig`, a custom uploader's request URL and `.sxcu` export,
  exceptions, and every error text (uploader and result) up to its stack
  trace, which .NET has and ShareXrs can't.

## How System.Uri escapes URLs

`HttpClient` sends `Uri.PathAndQuery`, and `System.Uri` rewrites what it's
given. Probed byte by byte (a throwaway console app printing `new
Uri(s).PathAndQuery` for every `%XX` and raw character, in paths and queries,
with and without non-ASCII elsewhere), and ported in
`http::dotnet_absolute_uri`:

- raw space `"` `<` `>` `^` `` ` `` `{` `|` `}`, controls and non-ASCII are
  escaped (`%XX`, upper case); `\` is `/` in the path and `%5C` in the query;
  a `%` that doesn't start an escape is `%25`;
- `%XX` of unreserved characters (`A-Z a-z 0-9 - . _ ~`) is decoded; dot
  segments are removed after that (`/a/%2e%2e/b` is `/b`);
- every other `%XX` is kept as written (`%3c` stays lower case) unless the
  URL holds any non-ASCII character (raw, or escaped UTF-8 in .NET's IRI
  ranges): then valid UTF-8 sequences and ``<>^`{|}`` (and space, `"`) come
  out upper case, while controls, `\` and reserved characters stay as
  written.

## Results

217 cases, all identical (requests byte for byte, server dialogues,
results, error texts): 28 custom uploader cases, 153 built-in destination
cases (every destination ShareX has), 18 sign-in cases (OAuth 2 code and
PIN exchanges, refreshes, Google's user info, OAuth 1.0a tokens, MEGA's
login) and, against the mock servers, 15 FTP/FTPS, 11 SFTP and 5 SMTP
cases. See `docs/UPLOADERS.md` for the per-destination table.

What can't be equal is checked, then normalised:
- Azure SharedKey signatures and OAuth 1.0a HMAC-SHA1 signatures are
  verified on each side (made-up secrets), then the dates, nonces and
  signatures normalised;
- PrivateBin's paste is decrypted with the key from each side's link;
  MEGA's chunks are decrypted with the key from the link, their MAC
  recomputed, the encrypted name and key decrypted (made-up master key);
- B2's file time, MEGA's request ids, a mail's Date and boundary GUID;
- FTP data ports and PASV/EPSV/PORT/EPRT addresses, and the client's host
  name in `EHLO`, are normalised by the mock servers themselves.

Amazon S3 requests are signed with the time, so they can't be byte-equal:
`compare.py` checks each side's AWS Signature V4 against the request as it
arrived (with the made-up secret; and the body against
`x-amz-content-sha256` when the payload is signed), then normalises
`x-amz-date` and the signature before comparing.

Known differences that the comparison normalises:
- the multipart boundary (random in both);
- header name case (ureq writes names in lower case; HTTP names are
  case-insensitive);
- error texts' stack traces;
- the user agent's version only if the .NET harness isn't built with
  ShareX's version (it is: `<Version>22.4.0</Version>`).
