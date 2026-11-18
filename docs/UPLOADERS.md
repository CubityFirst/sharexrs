# Uploaders: ShareX.UploadersLib in ShareXrs

The map of ShareX's upload pipeline (`ShareX.UploadersLib`, ~28k lines,
plus `ShareX/UploadManager.cs`, the upload half of `WorkerTask` and the
after-upload tasks) and what `crates/sharex-uploaders` ports of it.
Validation: [tools/uploaders-harness](../tools/uploaders-harness/README.md)
runs ShareX's own DLL and ShareXrs against a local mock server and diffs
requests byte for byte and results (and, for FTP, SFTP and SMTP, every
command and reply on mock servers); every case listed below as validated
is identical (217 of 217 cases). Uploads as tasks (`crates/sharexrs/src/upload`)
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
| `OAuth/*` (`OAuth2Info`, `OAuthInfo`, tokens) | `oauth` |
| `OAuth/OAuthManager.cs` (OAuth 1.0a) | `oauth1` |
| `OAuth/OAuth2ProofKey`, `OAuthListener`, `GoogleOAuth2`, `IOAuth*` | `oauth2` (traits `OAuth2Client`, `OAuth2Loopback`, `OAuth1Client`) |
| `Presentation/DestinationSettings/DestinationSettingsAccounts.cs` (the sign-in logic), `UploaderOAuthClientFactory` | `signin` |
| FluentFTP (`FtpClient` as ShareX uses it), `FileUploaders/FTP.cs`, `FTPAccount.cs` | `destinations::ftp` |
| SSH.NET (`SftpClient`), `FileUploaders/SFTP.cs` | `ssh`, `sftp`, `destinations::sftp` |
| `System.Net.Mail` (`SmtpClient`, `MailMessage`), `FileUploaders/Email.cs`, `EmailSharingService.cs` | `destinations::email` |
| (test servers) | `testing::{ftp, sftp, smtp}`, the `sharex_uploaders_mock_servers` example |
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

## OAuth and other sign-ins

All of ShareX's sign-in flows are ported without their windows
(`oauth`, `oauth1`, `oauth2`, `signin`); the Destination settings window
calls [`signin`](../crates/sharex-uploaders/src/signin.rs), which does what
`DestinationSettingsAccounts.cs`'s cards do to `UploadersConfig`.

| Flow | ShareX | Used by | ShareXrs |
|---|---|---|---|
| OAuth 2, code paste | `IOAuth2Basic` / `IOAuth2`: open `GetAuthorizationURLAsync`, the user pastes the code (Imgur: the PIN) into the card, `GetAccessTokenAsync`; `RefreshAccessTokenAsync`, `CheckAuthorizationAsync` | Imgur (PIN), GitHub Gist, Dropbox, OneDrive (with a PKCE proof key), Box, bit.ly | `oauth2::OAuth2Client` (implemented by each), `signin::{open_authorization, complete_authorization, refresh_token, disconnect}` |
| OAuth 2 loopback | `IOAuth2Loopback`, `OAuthListener` (a one-request HTTP listener on `127.0.0.1:<free port>`, random 32-character state), `GoogleOAuth2`, `OAuthListenerWindow` (user info after) | Google Drive, Google Cloud Storage, YouTube | `oauth2::{OAuthListener, connect, GoogleOAuth2}`, `signin::connect_loopback` (the page opened through a callback; stoppable) |
| PKCE | `OAuth2ProofKey` (S256) | OneDrive | `oauth2::OAuth2ProofKey` (RFC 7636 vectors in its tests), stored in `OAuth2Info.Proof` |
| OAuth 1.0a | `OAuthManager` (HMAC-SHA1; parameters sorted, nonce, timestamp), `Uploader.GetAuthorizationURLAsync`/`GetAccessTokenExAsync` | Flickr, Photobucket | `oauth1` (RFC 5849's example in its tests), `signin::{open_authorization_oauth1, complete_authorization_oauth1}`. RSA-SHA1 isn't (ShareX's signs a SHA-1 hash as MD5, which .NET refuses; no service uses it) |
| User / password | per destination | Pastebin, puush, LobFile (API key fetch), ImageShack (auth token), Seafile (auth token, library checks), MEGA (session, two-factor code), Pushbullet (device list) | `Pastebin::login`, `Puush::login`, `LobFile::fetch_api_key`, `ImageShack::get_access_token`, `Seafile::{get_auth_token, check_api_url, check_auth_token, get_account_info, get_libraries, get_or_make_default_library, decrypt_library}`, `Mega::login` (`MegaError::Api(-26)`: ask the code, log in again), `Pushbullet::get_device_list` |

Tokens live in `UploadersConfig` (`OAuth2Info`: `Client_ID`,
`Client_Secret`, `Token` {`access_token`, `refresh_token`, `expires_in`,
`token_type`, `scope`, `ExpireDate`}, `Proof`; `OAuthInfo` for OAuth 1),
in ShareX's JSON; a token ShareX saved keeps working. Uploaders that
refresh tokens hand them back with `WritesBackConfig::write_back`.
Client IDs and secrets are `api_keys` (`SHAREXRS_KEY_*` at build time,
empty otherwise, as in a ShareX built from source). The redirect page
(`oauth_callback_page.html`) is ShareX's under ShareXrs's name.

Validated: 18 sign-in cases run ShareX's own sign-in code and ShareXrs's
against mock token endpoints (authorization URLs, code and PIN exchanges,
refreshes keeping the refresh token, `CheckAuthorization` with an expired
token, refused codes, Google's user info, OAuth 1.0a request and access
tokens with their HMAC-SHA1 signatures verified on both sides, MEGA's v2
login and two-factor error): identical. The loopback listener is tested
in-process (code, wrong state, stop).

## Built-in destinations

✅ ported and identical in the harness (case count). Every value of the
destination enums is ported (`factory::tests::every_destination_is_ported`).

### Image
| Destination | Needs | Status |
|---|---|---|
| Imgur (default) | app client ID (`APIKeys`, empty in source builds); user: OAuth 2 PIN | ✅ anonymous, not-direct link + thumbnail size, video (`video` form field, gifv, trailing-dot link), error string, error object, user + album, user not signed in; sign-in 2. Album list/images not ported |
| ImageShack | API key + auth token (login) | ✅ 2 (upload, error) |
| Flickr | OAuth 1.0a | ✅ 2 (upload with OAuth form fields + `flickr.photos.getSizes`, `fail` response); sign-in 2 |
| Photobucket | OAuth 1.0a | ✅ 2 (upload to the subdomain, album index out of range); sign-in 1 |
| Chevereto | URL + API key | ✅ |
| vgy.me | optional user key | ✅ |

### Text
| Destination | Needs | Status |
|---|---|---|
| Pastebin (default) | dev key (`APIKeys`); optional login → user key | ✅ upload (privacy, expiration, raw URL, user key), "Bad API request" error; login ported, not harness-tested |
| Paste2, Slexy, Pastie | — (URL is where the form post ends) | ✅ |
| Paste.ee | app key or user key | ✅ (and "API key is missing") |
| GitHub Gist | OAuth 2 | ✅ 3 (raw URL, custom API URL, 401); sign-in 1 |
| uPaste | optional user key | ✅ success and error |
| Hastebin | — (custom domain) | ✅ extension from file name, `.txt` ignored |
| OneTimeSecret | optional basic auth | ✅ |
| PrivateBin | PBKDF2-SHA256 + AES-256-CBC on this side, base58 key | ✅ 4 (password + burn + markdown, defaults, error, 401); compare.py decrypts each side's paste with the key from its link |

### File
| Destination | Needs | Status |
|---|---|---|
| Dropbox (default) | OAuth 2 | ✅ 4 (direct link, link already exists → listed, no link, API error); sign-in 2. Metadata, copy, move, delete, create folder, list links ported (remote storage browser) |
| FTP / FTPS / SFTP | FluentFTP / SSH.NET | ✅ 26 (15 FTP/FTPS, 11 SFTP): see [FTP, FTPS and SFTP](#ftp-ftps-and-sftp) |
| OneDrive | OAuth 2 + PKCE | ✅ 6 (folder + link, root + web URL + escaped name, chunk error → session deleted, expired token refreshed, refresh refused, not signed in); sign-in 1. Folder listing (`get_path_info`) ported |
| Google Drive | Google OAuth 2 loopback | ✅ 5 (public + direct link, folder in a shared drive, shared drive root, expired token refreshed, error); sign-in 1. Folder and drive listings ported |
| Google Cloud Storage | Google OAuth 2 loopback | ✅ 3 (ACL + extension removed, custom domain, error); sign-in 1 |
| YouTube | Google OAuth 2 loopback | ✅ 2 (unlisted, short link + rejected); sign-in 1. The video options window is a hook (`set_youtube_options_ui`) |
| puush | API key | ✅ success, authentication error, garbage response; login ported |
| Box | OAuth 2 | ✅ 4 (share, no share → folder link, refresh refused, not signed in); sign-in 2. Folder listing (`get_files`) ported |
| MEGA | MEGA's API and crypto | ✅ 5 (Cloud Drive root, chosen folder with a non-ASCII name, expired session, not logged in, hashcash proof of work) + login 2; compare.py decrypts each side's chunk, checks its MAC, name and key. Folder listing not ported |
| Amazon S3 (and compatible) | access key + secret, SigV4 | ✅ single PUT, signed payload + path style + storage class + region, custom domain + no extension, 403 error, multipart (parallel parts, ETag, complete), complete error + abort; signatures verified on both sides. List/download/copy (remote storage browser) not ported; delete ported |
| Azure Storage | SharedKey (HMAC-SHA256) | ✅ 4 (sub folder + cache control, `$root` + custom domain + other cloud, 403, missing account); signatures verified on both sides |
| Backblaze B2 | key + SHA1 per file | ✅ 7 (bucket lookup, key restricted to a bucket + 503 retry + custom URL, auth errors JSON and not, name prefix refused, no bucket, upload error) |
| ownCloud / Nextcloud | basic auth, WebDAV + OCS share | ✅ direct link + file name, preview + expiring share (3.x URL form), no share, share error, PUT error |
| Immich | API key | ✅ shareable link, photo URL with metadata, no asset id (the GUID and time it sends each upload are normalised) |
| MediaFire | login + SHA1/MD5 signatures | ✅ 3 (upload + poll until done with key rotation, login error, poll error) |
| Pushbullet | API key + device | ✅ 2 (file push through the S3 form, no device); sharing 1 |
| SendSpace | API key (+ login, MD5) | ✅ 4 (anonymous, anonymous refused, user: token + login + info, user again with the kept session) |
| Hostr (Localhostr) | basic auth | ✅ 2 (direct + thumbnail, href) |
| Lambda | API key | ✅ success (and the old `λ.pw` URL fixed), error list |
| img.fish | API key | ✅ success, error, response that isn't JSON |
| Pomf | upload URL (+ result URL) | ✅ |
| Uguu, transfer.sh | — | ✅ |
| Seafile | token | ✅ 3 (raw share link with password and expiry, encrypted library, upload link error) |
| Streamable | optional basic auth | ✅ upload + transcode polling (progress 0–100, 1 s between checks), transcode failure, no shortcode |
| s-ul | API key | ✅ success and error |
| LobFile (Lithiio) | API key | ✅ success and error; API key fetch ported |
| Plik | URL + API key (+ login) | ✅ 3 (secured + comment + TTL in hours, never expires, metadata error) |
| Shared folder | local file copy | ✅ 1 (the file written and the URL; both harnesses read the file back) |
| Email | SMTP | ✅ 4 + sharing 1: see [Email](#email) |

### URL shorteners
bit.ly (saved OAuth 2 token), is.gd, v.gd, TinyURL, turl.ca, YOURLS
(signature or user/password), qr.net, vurl, 2.gp, Polr (v1 and v2), Firebase
Dynamic Links, Kutt, Zero Width Shortener: ✅ all, including their error
paths (is.gd "Error:", turl "ERROR:"). Custom: ✅.

### URL sharing services
Facebook, Reddit, Pinterest, Tumblr, LinkedIn, StumbleUpon, Delicious, VK,
Google Lens, Bing visual search: 🟨 ported (`SimpleURLSharer`: open the share
page with the URL encoded; the app sets `sharing::set_url_opener`), unit
tested (no HTTP to compare). Email: ✅ (SMTP, below). Pushbullet: ✅ (a link
push). Custom: ✅.

## FTP, FTPS and SFTP

ShareX uses FluentFTP and SSH.NET; ShareXrs sends what they send for
ShareX's calls. What that is was read off a mock server
([`testing::ftp`](../crates/sharex-uploaders/src/testing/ftp.rs),
[`testing::sftp`](../crates/sharex-uploaders/src/testing/sftp.rs): virtual
file systems that answer as vsftpd and OpenSSH's `sftp-server` do and
record every command and reply) with ShareX's own uploaders talking to it.

- **FTP / FTPS** (`destinations::ftp`, its own client on `std::net` and
  native-tls): connect `[TLS] 220`, `[AUTH TLS]`, `USER`, `PASS`,
  `[PBSZ 0, PROT P]`, `FEAT`, `OPTS UTF8 ON` (if listed), `SYST`, `PWD`;
  upload `SIZE`, `TYPE I` (once), `EPSV` (passive; a refusal is an error,
  as FluentFTP's) or `EPRT` falling back to `PORT` (active), `STOR`, the
  data (over TLS with `PROT P`), the reply; on 550/553 ShareX's
  `CreateMultiDirectory`, i.e. FluentFTP's forced `CreateDirectory` for
  each level (the parent's `DirectoryExists`: `CWD`, `PWD` for a relative
  path, `CWD` back; then `MKD`), and the upload again; `QUIT`. Errors read
  as FluentFTP's (`Code: 530 Message: Login incorrect.`, `Failed to connect
  to host.`). The server's certificate is accepted when it doesn't
  validate, as ShareX does; a client certificate (`FTPSCertificateLocation`)
  isn't sent. `FTPAccount`'s URL (`GetUriPath`, its `UriBuilder` and
  `HttpHomePath` rules), FTP path and previews are ported.
- **SFTP** (`destinations::sftp` on `ssh` + `sftp`, SSH-2 and SFTP v3
  written for this on RustCrypto crates: curve25519-sha256,
  ssh-ed25519 / ecdsa-sha2-nistp256 / rsa-sha2 host keys, aes-ctr,
  hmac-sha2; russh needs ring or aws-lc, C code that doesn't cross-check
  for macOS): the `none` login then the password or the key (RSA keys as
  `ssh-rsa` then `rsa-sha2-512`, each asked first; OpenSSH keys, encrypted
  ones with the passphrase, and unencrypted PEM RSA keys), a refused login
  dropping the connection; `INIT 3`, `REALPATH .`; SSH.NET's canonical
  paths (`REALPATH` of the full path, else of its parent); `OPEN`
  (write|create|truncate), `WRITE`s of the buffer size less the request's
  other fields, `CLOSE`; for a missing directory ShareX's
  `CreateDirectory` / `CreateMultiDirectory` (`LSTAT`, `MKDIR`, permission
  denied ignored). Errors as SSH.NET's (`Permission denied (password).`,
  `No such file. Path: '…'.`, `Key file does not exist.`). Host keys
  aren't checked against a known list (ShareX doesn't either); their
  signatures are.

Cases (identical: every command and reply in order, the stored files,
results and errors): FTP passive with directories created, existing,
parent existing, `MKD` refused once, active, active without `EPRT`, no
`EPSV` (error), login refused, absolute path with name parsing and
no-extension URL, `STOR` refused, a second account with a query-style home
path, connection refused; FTPS explicit, implicit, explicit active. SFTP
password with directories created, existing (several writes), ed25519 key,
passphrase-protected RSA key, PEM RSA key, wrong password, missing key
file, no credentials, `MKDIR` refused, `OPEN` failing, absolute path with a
non-ASCII name. `tests/servers.rs` replays them in-process on every
platform against ShareX's logs (`tests/fixtures/servers.sharex.json`).

## Email

`destinations::email`: the SMTP dialogue .NET's `SmtpClient` has with
ShareX's settings (`EHLO`, `STARTTLS`, `EHLO`, `AUTH login <user>` +
password, `MAIL FROM`, `RCPT TO` per address, `DATA`, `QUIT` only on
success; a refused login goes on to `MAIL FROM`, as .NET does) and the
message `MailMessage` writes (`MIME-Version`, `From`, `To`, `Date`,
`Subject` as `=?utf-8?B?…?=` when not ASCII; the body us-ascii
quoted-printable or utf-8 base64; with a file, `multipart/mixed` with
`--boundary_<n>_<guid>` (n counting per process, as .NET's) and the file as
`application/octet-stream; name=…` in 68-column base64; parameters folded
at 76 columns). The server certificate is checked (the tests accept the
mock's). Without automatic sending, the email window is a hook
(`email::set_email_ui`); `EmailLastTo` is remembered. Cases (identical
dialogue and mail, Date and boundary GUID normalised): attachment, non-ASCII
subject/body/name, login refused, invalid sender address, URL shared by
email; replayed in-process by `tests/servers.rs`.

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

- The windows: see [What the Destination settings window still
  needs](#what-the-destination-settings-window-still-needs).
- MEGA's folder listing (`GetFoldersAsync`, node name decryption), Imgur's
  album list, FTP's directory listing (FluentFTP's `LIST` parsing), the
  S3/Azure/B2/GCS listings: the remote storage browser's data side.
- FTPS client certificates (`FTPSCertificateLocation`); SFTP: encrypted PEM
  keys (convert to the OpenSSH format), PuTTY keys, Diffie-Hellman key
  exchange (servers without curve25519, OpenSSH before 6.5), other ciphers
  than AES-CTR and other MACs than HMAC-SHA2.
- Long non-ASCII email subjects are one encoded word (.NET folds them).
- `UploadersConfigValidator`.
- What upload tasks don't do yet: see [Upload tasks](#upload-tasks).

## What the Destination settings window still needs

Everything below is logic that's ported; the window
(`crates/sharexrs/src/dialogs/destination_settings.rs`) draws the cards and
their buttons say they aren't ported. Run network calls on a worker
thread and save `UploadersConfig` after a change, as the other cards do.

- **Sign-in buttons** (`signin`, by `SignInService::from_page_id(page)`):
  - code-paste cards (imgur, gist, dropbox, onedrive, box, bitly): "Open
    authorization page" → `open_authorization` and open the URL it gives
    (`sharing::open_url`); "Complete authorization" → `complete_authorization`
    with the code box's text; "Refresh token" → `refresh_token`;
    "Disconnect" → `disconnect`; the status line from `is_connected`;
  - loopback cards (google-drive, google-cloud-storage, youtube): "Connect
    account" → `connect_loopback` (blocks until the browser comes back:
    ShareX shows `OAuthListenerWindow` with Cancel meanwhile → cancel the
    token), status "Connected as {0}" from `connected_as`;
  - OAuth 1 cards (flickr, photobucket): `open_authorization_oauth1`,
    `complete_authorization_oauth1`, `disconnect`;
  - user/password cards: Pastebin `Pastebin::login`, puush
    `Puush::login`, LobFile `LobFile::fetch_api_key`, ImageShack
    `ImageShack::get_access_token` (+ `write_back`), MEGA `Mega::login`
    (on `MegaError::Api(-26)` ask for the two-factor code with the input
    box and call it again; save `session_id`/`master_key` into
    `MegaSessionID`/`MegaMasterKey`), Seafile's token, account info and
    library buttons (`Seafile::*`), Pushbullet "Get device list"
    (`Pushbullet::get_device_list`).
- **Pickers**: Imgur albums (not ported: album list request), OneDrive
  folders (`OneDrive::get_path_info`), Box folders (`BoxUploader::get_files`),
  Google Drive folders and shared drives (`GoogleDrive::{get_folders,
  get_drives}`), MEGA folders (listing not ported), Pushbullet devices
  (above), Seafile libraries (`Seafile::get_libraries`).
- **Remote storage browser** (Tools › Remote storage browser): SFTP has
  `SftpSession::{list_directory, delete_file, delete_directory, rename,
  stat}`; Dropbox has metadata/copy/move/delete/create folder; S3 has
  delete; the FTP/S3/Azure/B2/GCS listings aren't ported.
- **Dialog hooks** the app must set: `email::set_email_ui` (the email
  window when automatic sending is off: to, subject, body; `None`
  cancels), `oauth_services::set_youtube_options_ui` (title, description,
  privacy when "show dialog" is on), and `sharing::set_url_opener` (done).

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
| `UploadManager` (file, folder (recursive, more than 10 files confirmed), clipboard image/text/files, `ProcessTextUpload`, text with `TextCustom`, download and upload, shorten, share) | `upload::manager`; the pickers and windows in `crates/sharexrs/src/dialogs` |
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
- `DoFileJobs` (actions, copy file/path/folder, show in Explorer, Scan QR
  code) is ported and tested headlessly (`upload/tests.rs`, "File jobs");
  the Analyze image window it would open isn't.
- Not ported (logged when used): task notifications, OCR,
  `ProcessImagesDuringFileUpload`, index folder, history, a custom browser
  (`BrowserPath`), drag and drop onto the main window.
- The windows (pickers, text/URL/shorten, clipboard viewer, drop window,
  confirmations, before/after upload, QR code, response, Custom uploader
  and Destination settings) and the task menu's upload entries are ported
  and were checked live against the mock beside ShareX: see
  [PORTING.md](PORTING.md) (Upload, Destination settings rows).
- .NET regex patterns run on `fancy-regex` (substitutions and group
  numbering follow .NET; pattern syntax differences aren't handled).

## Plan: upload UI and task integration

Steps 1–6 are done (the windows of steps 3, 5 and 6 are in
`crates/sharexrs/src/dialogs`); step 7 remains, with the sign-in flows.

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
7. FTP/SFTP, email, the OAuth flows and every remaining destination are
   ported and in the harness; what's left is the windows' side (above).
