# Uploaders: ShareX.UploadersLib in ShareXrs

The map of ShareX's upload pipeline (`ShareX.UploadersLib`, ~28k lines,
plus `ShareX/UploadManager.cs`, the upload half of `WorkerTask` and the
after-upload tasks) and what `crates/sharex-uploaders` ports of it.
Validation: [tools/uploaders-harness](../tools/uploaders-harness/README.md)
runs ShareX's own DLL and ShareXrs against a local mock server and diffs
requests byte for byte and results; every case listed below as validated
is identical (97 of 97 cases). Uploads as tasks (`crates/sharexrs/src/upload`)
are under [Upload tasks](#upload-tasks).

## Layers

```
WorkerTask.DoUploadJob ─ UploadData(service, stream, fileName)
   │  CheckConfig → CreateUploader(config, TaskReferenceHelper) → UploadAsync
   ▼
UploaderFactory ── *UploaderService (one per destination enum value)
   ▼
Uploader ─ GenericUploader ─ FileUploader ─ ImageUploader      (image/file destinations)
         │                 └ TextUploader                      (text destinations)
         ├ URLShortener                                         (URL shorteners)
         └ URLSharer                                            (URL sharing services)
   ▼
Uploader.SendRequest*/GetResponseAsync ─ HttpClientFactory (HelpersLib) ─ HttpClient
```

| ShareX | ShareXrs |
|---|---|
| `BaseUploaders/Uploader.cs` (request helpers, `ProcessError`, `RunOperationAsync`, `StopUpload`, progress events, `LastResponseInfo`, `ReturnResponseOnError`) | `uploader::Uploader` (same helper names in snake case: `send_request`, `send_request_text`, `send_request_stream`, `send_request_url_encoded`, `send_request_multipart`, `send_request_file`, `send_request_file_range`, `get_response`, `process_error`), `run_operation`, `stop_handle()` |
| `GenericUploader`, `FileUploader`, `ImageUploader`, `TextUploader`, `URLShortener`, `URLSharer` | traits `FileUploader`, `TextUploader`, `UrlShortener`, `UrlSharer`; `GenericUploader` enum (file/text); `upload_text`, `shorten_url`, `share_url` |
| `BaseServices/*UploaderService` + `UploaderFactory` | `factory::{check_*, create_*}` per kind, `factory::Ported`, `factory::service_name` |
| `UploadResult`, `UploaderErrorManager`/`UploaderErrorInfo` | `result::{UploadResult, UploaderErrorManager, UploaderErrorInfo}` |
| `Helpers/ResponseInfo`, `ProgressManager`, `ProgressStreamContent`, `SharedStreamSegment`, `RequestHelpers` | `http::{ResponseInfo, ProgressManager, StreamPart (+on_read), Content, CONTENT_TYPE_*, create_boundary}`, `uploader::create_authentication_header` |
| `HelpersLib/HttpClientFactory`, `ShareXResources.UserAgent`, `HelpersOptions.CurrentProxy` | `http::{send, set_proxy, set_user_agent}` |
| `HelpersLib/URLHelpers`, `HttpUtility` | `url_helpers` |
| `HelpersLib/MimeTypes` | `sharex_helpers::mime_types` |
| `UploadersConfig.cs` | `config::UploadersConfig` |
| `APIKeys/APIKeys.cs` | `api_keys` (empty unless `SHAREXRS_KEY_*` set at build, as ShareX's source) |
| `OAuth/*` | `oauth` (stored shapes and token checks only) |
| `CustomUploader/*` | `custom` |

## The HTTP layer

How ShareX's requests are built, all reproduced in `http` and `uploader`
(and checked byte for byte by the harness):

- **Client**: one shared `HttpClient` per redirect setting
  (`HttpClientFactory.Create(allowAutoRedirect, infiniteTimeout: true)`),
  `SocketsHttpHandler` with a 30 s connect timeout, no cookie container,
  the proxy from settings; redirects followed (.NET: up to 50; a redirect
  turns POST into GET). ShareXrs: ureq 3 on rustls with the OS certificate
  verifier, same limits, one agent per redirect setting.
- **Timeouts**: none overall (`Uploader.RequestTimeout` infinite) unless a
  destination sets one; a timeout is a `TimeoutException` ("The HTTP request
  timed out." / "The request timed out after hh:mm:ss.").
- **Headers on every request**, in this order: `Host`, the destination's
  headers (in `NameValueCollection` order, repeated names joined with `, `,
  `Cookie` taken out and appended as `Cookie: a; b` after them), then
  `User-Agent: ShareX/22.4.0` and `Cache-Control: no-cache` (HttpClient's
  defaults), then content headers: `Content-Type`, other `Content-*`,
  `Content-Length`. A destination's `Content-*` header goes on the content
  (an empty body is created for it if there's none); a second
  `Content-Type` is appended to the first (`application/json,
  application/vnd.x+json`). POST and PUT without a body send
  `Content-Length: 0`; GET/DELETE without a body send no length. Bodies
  always have a `Content-Length` (never chunked). No `Accept`,
  `Accept-Encoding` or `Connection` headers.
- **User agent**: ShareXrs identifies as itself, `ShareXrs/<version>`
  (`http::default_user_agent`), where ShareX sends `ShareX/<version>`; the
  harness normalises that one difference. A `.sxcu` can set its own.
- **URLs**: `System.Uri` rewrites them before sending; ported exactly in
  `http::dotnet_absolute_uri` (probed byte by byte, see the harness README).
- **Query strings**: `URLHelpers.CreateQueryString` with
  `HttpUtility.UrlEncode` (space as `+`, lower-case `%xx`, `!*()` kept);
  an empty value leaves just the key. Custom uploaders encode `{filename}`
  and `{input}` in URLs with ShareX's own `URLEncode` (upper-case `%XX`,
  `-._~` kept), which is different.
- **Multipart** (`MultipartFormDataContent`): boundary `-` × 20 + a GUID's
  32 hex digits, header `multipart/form-data; boundary="…"` (quoted);
  `--b` CRLF, part headers, CRLF, content; parts separated by CRLF `--b`
  CRLF; ends with CRLF `--b--` CRLF. Arguments first (`Content-Disposition:
  form-data; name="k"` only), then the file (`Content-Type: <MIME of the
  name>` then `Content-Disposition: form-data; name="f"; filename="n"`).
  Names are written as raw UTF-8, quoted with `\` and `"` escaped and
  CR/LF as `%0D`/`%0A`. Empty-named arguments are skipped.
- **Bodies**: text as UTF-8 (`ByteArrayContent`, content type only if given:
  `application/json`, no charset); URL-encoded forms; streams sent from a
  position for a length (`ProgressStreamContent`) with progress.
- **Progress**: `ProgressManager(length, position)`; reported when the
  upload ends or 250 ms passed since the last report; speed is the average
  of the last 10 samples. Only stream content reports progress.
- **Stopping**: `StopUpload()` cancels the operation; the request returns at
  once (ShareXrs runs it on a worker thread and stops reading the body) and
  `RunOperationAsync` gives `null`.
- **Responses**: `ResponseInfo` (status, reason, URL — the URL as given, or
  where redirects ended —, headers: response then content headers with
  repeats joined by `, `, the text decoded by charset, else BOM, else
  UTF-8, and the bytes). Non-2xx is an error unless the caller allows it;
  `ReturnResponseOnError` keeps the error response's text as the result's.
- **Errors** (`ProcessError`): "Error message:", the message
  (`Response status code does not indicate success: 404 (Not Found).`),
  "Request URL:", then for a response its status, response URL (if
  different), headers and text, then "Stack trace:" (empty in ShareXrs:
  there's no .NET stack). Each uploader's errors are titled by
  `WorkerTask` with the service's name.

## Destination enums (in order, as stored in settings)

- `ImageDestination`: Imgur, ImageShack, Flickr, Photobucket, Chevereto,
  Vgyme, CustomImageUploader, FileUploader
- `TextDestination`: Pastebin, Paste2, Slexy, Paste_ee, Gist, Upaste,
  Hastebin, OneTimeSecret, Pastie, PrivateBin, CustomTextUploader,
  FileUploader
- `FileDestination`: Dropbox, FTP, OneDrive, GoogleDrive, Puush, Box, Mega,
  AmazonS3, GoogleCloudStorage, AzureStorage, BackblazeB2, OwnCloud, Immich,
  MediaFire, Pushbullet, SendSpace, Localhostr, Lambda, ImgFish, Pomf, Uguu,
  Seafile, Streamable, Sul, Lithiio, Transfersh, Plik, YouTube,
  SharedFolder, Email, CustomFileUploader
- `UrlShortenerType`: BITLY, ISGD, VGD, TINYURL, TURL, YOURLS, QRnet, VURL,
  TwoGP, Polr, FirebaseDynamicLinks, Kutt, ZeroWidthShortener,
  CustomURLShortener
- `URLSharingServices`: Email, Facebook, Reddit, Pinterest, Tumblr,
  LinkedIn, StumbleUpon, Delicious, VK, Pushbullet, GoogleImageSearch,
  BingVisualSearch, CustomURLSharingService

Defaults (`TaskSettingsUpload`): Imgur, Pastebin, Dropbox (file and image
file), bit.ly, Email. `*.FileUploader` means "use the file destination".
Their names (`ServiceName`) are the `[Description]` or, for ShareX's own
kinds, the localized `<Enum>_<Value>` string (`factory::service_name`).
Other enums: `HttpMethod` (GET, POST, PUT, PATCH, DELETE),
`CustomUploaderBody`, `CustomUploaderDestinationType` (flags),
`AccountType`, `FTPProtocol`, `FTPSEncryption`, `BrowserProtocol`,
`Privacy`, `LinkFormatEnum`, `OAuthLoginStatus`, `YouTubeVideoPrivacy`,
`BoxShareAccessLevel`, `ImgurThumbnailType`, `PastebinPrivacy`,
`PastebinExpiration`, `AmazonS3StorageClass` — all in `enums`.

## Custom uploaders (.sxcu) — ported in full

`CustomUploaderItem` (`custom::CustomUploaderItem`):

| Property | Notes |
|---|---|
| `Version` | `CheckBackwardCompatibility`: missing or ≤ 12.3.1 is rejected ("Unsupported custom uploader: name"); ≤ 13.7.1 is migrated from `$name$` syntax (`$` pairs become `{}`, existing braces escaped, `\x` dropped as ShareX does, `$input$`/`$filename$` in Data) and gets the current version. A query string in `RequestURL` moves into `Parameters` (keys without `=` become empty parameters). Run on import and on load (`UploadersConfigBackwardCompatibilityTasks`). |
| `Name` | written only if set and not the request URL's host; `ToString()` is name, else host, else "Name"; file name `<name>.sxcu` |
| `DestinationType` | flags: ImageUploader, TextUploader, FileUploader, URLShortener, URLSharingService |
| `RequestMethod` | always written; default POST |
| `RequestURL` | parsed with URL encoding of `{filename}`/`{input}`, `https://` added if no prefix, `Parameters` appended as a query |
| `Parameters`, `Headers`, `Arguments` | values parsed with the name parser first (`%y`, `%ra{5}`…, file reads off, `\%` and `\\` protected), then the syntax |
| `Body` | None, MultipartFormData, FormURLEncoded, JSON, XML, Binary |
| `FileFormName` | required for multipart file uploads ("File form name must be configured") |
| `Data` | JSON/XML bodies: name parser, then `{input}`/`{filename}` replaced (case-insensitive) with JSON- or XML-escaped values |
| `URL`, `ThumbnailURL`, `DeletionURL` | parsed from the response (URL empty: the response text) |
| `ErrorMessage` | parsed from a non-2xx response and shown first |

Two JSON shapes: `UploadersConfig.json` (all properties but the
`ShouldSerialize*` exclusions, nulls written) and `.sxcu` exports
(`DefaultValueHandling.Ignore` + `NullValueHandling.Ignore`, indented); both
in `CustomUploaderItem::to_value`, reading is lenient (`from_value`),
unknown properties kept.

Which requests each kind sends: image and file uploaders: multipart file
(arguments + file) or Binary (the file as the body, its MIME type);
text: None (no body), multipart (arguments only, or the text as a file if
`FileFormName` is set), form URL-encoded, JSON/XML (Data), Binary (the text);
URL shortener/sharer: None, multipart, form, JSON/XML. Others: "Unsupported
request format: X".

**Syntax** (`custom::parser`, `ShareXSyntaxParser`): `{` function `:`
parameters separated by `|` `}`, nestable, `\` escapes. An unterminated
function is still called. Functions (names and aliases case-insensitive):

| Function | Parameters | Result |
|---|---|---|
| `response` | | response text |
| `responseurl` | | the response's URL (after redirects) |
| `header` | name | response header (case-insensitive) |
| `json` | path, or input\|path | Newtonsoft `SelectToken` (`$.` added), cast to string: numbers and booleans as .NET prints them (`True`, `1E+20`), ISO dates re-formatted (`MM/dd/yyyy HH:mm:ss`, offsets to local time) because Newtonsoft reads them as dates; several matches or an object are errors |
| `xml` | xpath, or input\|xpath | `XPathNavigator.SelectSingleNode(xpath).Value` (XPath 1.0) |
| `regex` | pattern, pattern\|group, or input\|pattern\|group | .NET regex (lookaround, named groups); group by number or name, missing group = "" |
| `filename` | | file name (URL-encoded in URLs) |
| `input` | | text/URL being sent (URL-encoded in URLs) |
| `random` | 2+ values | one of them |
| `select` | values | the one the user picks (dialog) |
| `inputbox` / `prompt` | [title[\|default]] | text the user types (dialog) |
| `outputbox` | text, or title\|text | shows text (dialog), gives nothing; `{output:` in URL and an empty result means no URL is expected |
| `base64` | text | UTF-8 base 64 |

Errors: empty function name, unknown function, too few parameters. A
response that can't be parsed is reported as "The host (x) returned invalid
response content; JSON was expected." (JSON errors, with Newtonsoft's
message for non-JSON text) or "Unable to parse the response content returned
by the host (x)." (anything else). The dialogs go through
`custom::ParserUi` (none set: they give nothing).

## OAuth

| Flow | ShareX | Used by | ShareXrs |
|---|---|---|---|
| OAuth 2 with PIN (copy code) | `IOAuth2`: `GetAuthorizationURLAsync` → user pastes PIN → `GetAccessTokenAsync`; `RefreshAccessTokenAsync`, `CheckAuthorizationAsync` | Imgur, Dropbox, Box, Gist, OneDrive (older) | Imgur's token exchange, refresh and check ported (`Imgur::authorization_url`, `get_access_token`, `refresh_access_token`); the PIN window isn't |
| OAuth 2 basic (code) | `IOAuth2Basic` | bit.ly | ported (`Bitly::authorization_url`, `get_access_token`), no window |
| OAuth 2 loopback (PKCE, local listener) | `IOAuth2Loopback`, `OAuthListener`, `OAuth2ProofKey`, `GoogleOAuth2` | Google Drive, Google Cloud Storage, YouTube, OneDrive | not ported |
| OAuth 1.0a (HMAC-SHA1 signing) | `OAuthManager`, `IOAuth` | Flickr, Photobucket | not ported |
| API key / user key | settings | most simple services | ported per destination |

Tokens live in `UploadersConfig` (`OAuth2Info`: `Client_ID`, `Client_Secret`,
`Token` {`access_token`, `refresh_token`, `expires_in`, `token_type`,
`scope`, `ExpireDate`}, `Proof`), ported in `oauth`; a token ShareX saved
keeps working. Uploaders that refresh tokens hand them back with
`WritesBackConfig::write_back`.

## Built-in destinations

✅ ported and identical in the harness · 🟨 ported, partly validated ·
⬜ not ported. "Needs": what it takes besides an HTTP request.

### Image
| Destination | Needs | Status |
|---|---|---|
| Imgur (default) | app client ID (`APIKeys`, empty in source builds); user: OAuth 2 PIN | ✅ anonymous, not-direct link + thumbnail size, video (`video` form field, gifv, trailing-dot link), error string, error object, user + album, user not signed in. Token refresh on "invalid token" ported, not harness-tested; album list/images not ported |
| ImageShack | API key + login | ⬜ |
| Flickr | OAuth 1.0a | ⬜ |
| Photobucket | OAuth 1.0a | ⬜ |
| Chevereto | URL + API key | ✅ |
| vgy.me | optional user key | ✅ |

### Text
| Destination | Needs | Status |
|---|---|---|
| Pastebin (default) | dev key (`APIKeys`); optional login → user key | ✅ upload (privacy, expiration, raw URL, user key), "Bad API request" error; login ported, not harness-tested |
| Paste2, Slexy, Pastie | — (URL is where the form post ends) | ✅ |
| Paste.ee | app key or user key | ✅ (and "API key is missing") |
| GitHub Gist | OAuth 2 PIN (or anonymous) | ⬜ |
| uPaste | optional user key | ✅ success and error |
| Hastebin | — (custom domain) | ✅ extension from file name, `.txt` ignored |
| OneTimeSecret | optional basic auth | ✅ |
| PrivateBin | client-side AES-GCM + zlib + base58 | ⬜ |

### File
| Destination | Needs | Status |
|---|---|---|
| Dropbox (default) | OAuth 2 PIN, Dropbox API | ⬜ |
| FTP / FTPS / SFTP | FluentFTP / SSH.NET | ⬜ (next: `suppaftp` + `russh`/`ssh2`) |
| OneDrive | OAuth 2 loopback | ⬜ |
| Google Drive, Google Cloud Storage, YouTube | Google OAuth 2 loopback (PKCE) | ⬜ |
| puush | API key | ✅ success, authentication error, garbage response; login ported |
| Box | OAuth 2 PIN | ⬜ |
| MEGA | MEGA's own crypto (AES, RSA) | ⬜ |
| Amazon S3 (and compatible) | access key + secret, SigV4 | ✅ single PUT, signed payload + path style + storage class + region, custom domain + no extension, 403 error, multipart (parallel parts, ETag, complete), complete error + abort; signatures verified on both sides. List/download/copy (remote storage browser) not ported; delete ported |
| Azure Storage | SharedKey (HMAC-SHA256) | ⬜ (simple HTTP + HMAC) |
| Backblaze B2 | key + SHA1 per file | ⬜ (simple HTTP) |
| ownCloud / Nextcloud | basic auth, WebDAV + OCS share | ✅ direct link + file name, preview + expiring share (3.x URL form), no share, share error, PUT error |
| Immich | API key | ✅ shareable link, photo URL with metadata, no asset id (the GUID and time it sends each upload are normalised) |
| MediaFire | login + SHA1 signatures | ⬜ |
| Pushbullet | API key | ⬜ (simple HTTP; also the sharing service) |
| SendSpace | API key + login (MD5) | ⬜ |
| Hostr (Localhostr) | basic auth | ⬜ (simple HTTP) |
| Lambda | API key | ✅ success (and the old `λ.pw` URL fixed), error list |
| img.fish | API key | ✅ success, error, response that isn't JSON |
| Pomf | upload URL (+ result URL) | ✅ |
| Uguu, transfer.sh | — | ✅ |
| Seafile | token | ⬜ (simple HTTP) |
| Streamable | optional basic auth | ✅ upload + transcode polling (progress 0–100, 1 s between checks), transcode failure, no shortcode |
| s-ul | API key | ✅ success and error |
| LobFile (Lithiio) | API key | ✅ success and error; API key fetch ported |
| Plik | URL (+ API key / login) | ⬜ (simple HTTP) |
| Shared folder | local file copy | ⬜ |
| Email | SMTP | ⬜ |

### URL shorteners
bit.ly (saved OAuth 2 token), is.gd, v.gd, TinyURL, turl.ca, YOURLS
(signature or user/password), qr.net, vurl, 2.gp, Polr (v1 and v2), Firebase
Dynamic Links, Kutt, Zero Width Shortener: ✅ all, including their error
paths (is.gd "Error:", turl "ERROR:"). Custom: ✅.

### URL sharing services
Facebook, Reddit, Pinterest, Tumblr, LinkedIn, StumbleUpon, Delicious, VK,
Google Lens, Bing visual search: 🟨 ported (`SimpleURLSharer`: open the share
page with the URL encoded; the app sets `sharing::set_url_opener`), unit
tested (no HTTP to compare). Email, Pushbullet: ⬜. Custom: ✅.

## UploadersConfig

`config::UploadersConfig`: every property in ShareX's order with its
default; settings of ported destinations typed (`CheveretoUploader`,
`PastebinSettings`, `AmazonS3Settings`, `LambdaSettings`, `ImgFishSettings`,
`LobFileSettings`, `PomfUploader`, `PlikSettings`, `KuttSettings`,
`OAuth2Info`, `OAuthInfo`, `OAuthUserInfo`, the custom uploader list), the
rest kept as JSON exactly as read. `[JsonEncrypt]` strings are `Secret`s:
DPAPI (`$DPAPIEncrypted$` + base 64, current user) on Windows when saved,
plain elsewhere (as ShareX: .NET can't encrypt there); a value that didn't
change is written back as read, so files round-trip byte for byte; a value
another account encrypted reads as null. Fixtures: a fresh profile and one
written by ShareX's own `Save` (custom uploaders, OAuth tokens, settings)
both round-trip byte for byte, and a new config equals ShareX's fresh one.

Where it lives: `config::uploaders_config_file_path` (personal folder or
`CustomUploadersConfigPath`, and with `UseMachineSpecificUploadersConfig`
`UploadersConfig-<machine>.json`, copied from the shared file the first
time). `UploadersConfig::load` (ShareX's fallbacks to the temp file and the
backups, then the backward compatibility tasks) and `save` (backup and
weekly backup). The app loads it in the background at startup and tasks
wait for it (`upload::UploadersConfigStore`, ShareX's `LoadInitialSettings`
/ `WaitUploadersConfig`); it's saved through `SettingManager`'s writer with
the other files (exit, tray menu close, `SaveSettingsAfterTaskCompleted`).

## Not ported yet

- Destinations marked ⬜ above; FTP/SFTP and the OAuth-loopback services
  (Google, OneDrive) next, then the simple-HTTP ones (Plik, Azure, B2,
  Seafile, Pushbullet, Hostr).
- OAuth sign-in flows and windows; `OAuthManager` (OAuth 1.0a).
- `UploadersConfigValidator`.
- What upload tasks don't do yet: see [Upload tasks](#upload-tasks).
- History (`HistoryLib`) of uploads.
- UI: Destination settings, Custom uploader settings (editor, test,
  import/export), upload progress in the task list, response window, the
  `{inputbox}`/`{outputbox}`/`{select}` dialogs, OAuth windows.

## Upload tasks

`crates/sharexrs/src/upload` runs uploads as ShareX's `WorkerTask` and
`UploadManager` do, each task on its own thread, reporting to the event
loop (`TaskEvent`) and the task list (`tasks.rs`). Desktop actions go
through `upload::TaskUi` (clipboard, open URL, QR code), so tests record
them.

| ShareX | ShareXrs |
|---|---|
| `WorkerTask` constructors (`CreateImageUploaderTask`, `CreateFileUploaderTask`, `CreateTextUploaderTask`, `CreateURLShortenerTask`, `CreateShareURLTask`, `CreateDownloadTask`) | `upload::manager::create_*` → `UploadTask` |
| `ThreadDoWork`, `DoThreadJob` (after capture tasks, `DoTextJobs`, `DownloadFromURL`, `AutoClearClipboard`), `DoUploadJob` (`WaitUploadersConfig`, status Uploading, `MaxUploadFailRetry` retries 1 s apart), `DoUpload` (`CheckUploadFilters`, `UploadDestination`), `UploadData` (`CheckConfig`, `CreateUploader`, error title, `BufferSizePower`, progress, `EarlyCopyURL`, bidi/reserved characters, `UploadDuration`), `GetInvalidConfigResult`, the `EarlyURLCopied` clean-up, `DeleteFile`, "URL is empty." | `upload::worker` |
| `DoAfterUploadJobs` (URL regex replace with .NET's substitutions, `ResultForceHTTPS`, shorten (also `AutoShortenURLLength`), share, copy URL with `ClipboardContentFormat`, open URL with `OpenURLFormat`, QR code), `ShortenURL`, `ShareURL`, `UploadInfoParser` | `upload::after_upload` |
| `UploaderFilter`, `UploaderFactory.AllGenericUploaderServices` | `upload::filters` |
| `UploadManager` (file, folder (recursive, more than 10 files confirmed), clipboard image/text/files, `ProcessTextUpload`, text with `TextCustom`, download and upload, shorten, share) | `upload::manager`; the pickers and windows are stubs in `upload::manager::dialogs` |
| `TaskManager.Start`/`StartTasks` (`UploadLimit` queue), `Stop`/`StopAllTasks`, `Remove` (stops), the `Task_*` handlers, `UpdateProgressUI` (tray progress) | `upload::app`, `TaskManager::{add_task, startable, prepare, stop, apply_event, finish}` |
| `ThumbnailItemViewModel.RefreshListColumns` (status %, `ToSizeString`, speed, `ProperTimeSpan`) | `TaskManager::set_progress`, `task_view::list_cells` |
| `TaskHelpers.ExecuteJob` upload jobs, `MainMenuBuilder.BuildUploadMenu`, the Debug menu's test uploads | `App::run_upload_job`, `App::upload_menu_command` |

Validated headlessly (`upload::tests`, 18 flows against
`tools/uploaders-harness/mock_server.py`): capture → upload (multipart, the
saved PNG byte for byte) → copy with a clipboard format, open URL, QR code;
text (saved with a BOM, JSON body); file with progress reports and the list
columns; an uploader filter; URL shortener task; upload + custom shortener
+ custom sharer; built-in is.gd after upload; download and upload (HEAD,
GET, upload); HTTP 500 after its retry, a timeout, a refused connection,
missing settings (Destination settings asked for), an unported
destination, `MaxUploadFailRetry` 0; Stop mid-upload; a retry that
succeeds; Imgur's refreshed OAuth token written back to the config; the
config loaded and saved with backups. `UploadInfoParser.Parse` and the URL
regex replace give what ShareX's own code gives for 33 cases
(`UploadersHarness --after-upload`, fixture in `upload/fixtures`).

Differences and gaps:
- Exceptions are reported by their message (ShareX adds the .NET type and
  stack trace: `e.ToString()`).
- A destination `sharex-uploaders` doesn't have fails the task with
  "<service> isn't ported to ShareXrs yet." (ShareX would upload).
- Not ported (logged when used): the file/folder pickers, the text, URL
  and shorten URL windows, reading the clipboard, the clipboard content
  viewer, the drag and drop window, the multi upload confirmation (goes
  ahead), the large file warning and before upload window (go ahead), the
  after upload window and task notifications, Destination settings (asked
  for, logged), `DoFileJobs` (actions, copy file/path, show in Explorer),
  OCR, `ProcessImagesDuringFileUpload`, index folder, history, the upload
  entries of the task context menu (Open/Copy URL, Upload, Shorten, Share,
  Show response/errors), the thumbnail card's progress bar, the main
  window title's progress, a custom browser (`BrowserPath`).
- .NET regex patterns run on `fancy-regex` (substitutions and group
  numbering follow .NET; pattern syntax differences aren't handled).

## Plan: upload UI and task integration

Steps 1, 2 and 4 are done for what doesn't need new windows, and step 3's
list columns (see [Upload tasks](#upload-tasks)); the rest of step 3 and
steps 5–7 remain.

1. **Settings load/save**: `SettingManager` gains `uploaders:
   UploadersConfig` (load at startup in the background as ShareX's
   `LoadUploadersConfig`, `WaitUploadersConfig` before an upload,
   machine-specific file name, backward compatibility tasks, save with the
   others and after a task).
2. **Upload task** (`tasks.rs`): a `TaskJob` with upload steps as
   `WorkerTask`: after capture, if `AfterCaptureJob` has `UploadImageToHost`
   (or a file/text/URL job), encode the image, pick the service from
   `TaskSettings` (`UseDefaultDestinations`, custom uploader override), run
   `factory::check_*` (invalid → ShareX's "configuration is invalid or
   missing" error and open Destination settings), `create_*`, set the error
   title, buffer size (`BufferSizePower`), progress callback (posted to the
   UI thread, throttled as ShareX), early URL copy; run on the task's worker
   thread with the stop handle wired to the task's Stop; retry
   `MaxUploadFailRetry` times; `write_back` into the config; then
   `DoAfterUploadJobs`.
3. **Task list**: the upload columns (status "Uploading", progress %, speed,
   elapsed/remaining from `ProgressManager`, URL), the thumbnail card's
   progress bar, the context menu's upload entries (Open › URL / Shortened
   URL / Thumbnail URL / Deletion URL, Copy › URL formats from
   `UploadInfoParser`, Upload, Shorten URL, Share URL, Show response,
   Stop), the error badge and the errors window
   (`UploadInfoManager.ShowErrors`). Each ported from its `.axaml` and
   validated with `--render main-tasks-upload*` against ShareX shots taken
   with ShareX pointed at the mock server.
4. **Main window entries**: Upload menu (file, folder, clipboard, text,
   from URL, drag and drop, shorten URL, share URL), the tray's, and the
   hotkeys' upload jobs (`UploadManager`).
5. **Destination settings window** (`DestinationSettingsWindow`,
   `DestinationSettingsPageBuilder`): the destination list and one page per
   ported destination, bound to `UploadersConfig`; OAuth PIN pages for
   Imgur/bit.ly first.
6. **Custom uploader settings window**: list, editor (request, body,
   parameters/headers/arguments grids, response fields with syntax
   highlighting), import/export `.sxcu` (`to_sxcu`/`from_json`, `.sxcu`
   double-click via `ShareXCLIManager`), the test buttons (they upload a
   test image/text/URL), response window.
7. Then FTP/SFTP, the Google/OneDrive loopback OAuth and the remaining
   simple-HTTP destinations, each added to the harness.
