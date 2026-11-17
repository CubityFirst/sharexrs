# Releasing ShareXrs

This file covers what ShareX ships and how, and how ShareXrs packages the same for
Windows, macOS and Linux. Sources: ShareX at `REFERENCE_COMMIT` (`ShareX.Setup/`,
`.github/workflows/build.yml`, `Directory.build.props`, `ShareX/IntegrationHelpers.cs`,
`ShareX/ShareXUpdateManager.cs`, `ShareX.HelpersLib/UpdateChecker/`), and snapr's
release workflow ([MACOS-LEARNINGS.md](MACOS-LEARNINGS.md) sections 1, 2 and 4).

## 1. What ShareX ships

### Builds and artifacts

`build.yml` runs on every push and on `v<x.y.z>` tags. It builds a matrix of
configurations (`Release`, `Debug`, `Steam`, `MicrosoftStore`, `MicrosoftStoreDebug`)
by platform (`x64`, `ARM64`, with no Steam ARM64). Each build is a self-contained
.NET publish (ReadyToRun).

- The version is `Directory.build.props`'s `<Version>` (22.4.0). Develop builds
  append the run number.
- The `Dev` flag is turned off on master and tags.
- Service API keys come from the `API_KEYS` secret, written over `APIKeysLocal.cs`.
- Tagged builds are signed with Azure Trusted Signing: `ShareX*.exe/dll`,
  `ffmpeg.exe`, the Avalonia DLLs and the installer.

`ShareX.Setup.exe -job <configuration> -platform <p>` then packs each build into
`Output/`:

| Job | Output |
|---|---|
| Release | `ShareX-<ver>-setup-<x64\|arm64>.exe` (Inno Setup) and `ShareX-<ver>-portable-<x64\|arm64>.zip` |
| Debug | `ShareX-<ver>-debug-<p>.zip`: the app folder plus `*.pdb` |
| Steam | `ShareX-<ver>-Steam-<p>.zip`: `ShareX_Launcher.exe`, `steam_appid.txt`, `installscript.vdf`, the launcher's DLLs, and the app folder as `Updates/` |
| MicrosoftStore(Debug) | `ShareX-<ver>-MicrosoftStore[-debug]-<p>.appx`: `makeappx pack` of the app folder plus the `MicrosoftStore/` assets and `AppxManifest.xml` (with `{PLATFORM}` and `{VERSION}` filled in) |

A tag publishes a **draft pre-release** with `setup-x64.exe` and `portable-x64.zip`
only (the ARM64 lines are commented out). Its body is a changelog link plus a table
of each file's SHA256. Develop builds go to the `ShareX/DevBuilds` repository.
`CreateChecksumFile` (a `.sha256` per file) exists, but no job turns it on.

### The app folder (`ShareX.Setup` `CreateFolder`)

```
ShareX.exe, *.dll, *.json            the self-contained .NET app (onnxruntime.dll, DirectML.dll among them)
ffmpeg.exe                           added by CI
Models/PP-OCRv6_small_{det,rec}/     inference.onnx + inference.yml (ShareX.OCR.csproj copies Models/**)
Licenses/*.txt                       the repo's Licenses folder (13 files: ShareX's GPL, Avalonia,
                                     CommunityToolkit.Mvvm, FFmpeg, FluentFTP, Microsoft.Data.Sqlite,
                                     Microsoft.ML.OnnxRuntime.DirectML, Newtonsoft.Json, PaddleOCR,
                                     SSH.NET, SkiaSharp, Vortice, ZXing.Net)
ShareX_File_Icon.ico                 the .sxcu/.sxie file icon
Languages/<culture>/*.resources.dll  satellite assemblies, moved from <culture>/
Portable                             portable zip only: an empty file
```

### The bundled pieces

| Piece | Where ShareX gets it | Licence shipped | Where ShareX looks for it |
|---|---|---|---|
| **FFmpeg** | CI downloads BtbN's `ffmpeg-n9.0-latest-<win64\|winarm64>-gpl-9.0.zip` from FFmpeg-Builds' rolling `latest` release (`FFMPEG_VERSION: "9.0"`), with **no hash check**. It copies `bin/ffmpeg.exe` beside `ShareX.exe` | `Licenses/FFmpeg_license.txt` (GPL v3 text). No source pointer or offer | `FFmpegOptions.FFmpegPath`: the custom path when "Use custom FFmpeg path" is on, else `ffmpeg.exe` beside the exe. If it's missing, ShareX offers to download it (`FFmpegGitHubDownloader`) |
| **ONNX Runtime** | NuGet `Microsoft.ML.OnnxRuntime.DirectML` 1.24.4 (native `win-x64` and `win-arm64` only) | `Microsoft.ML.OnnxRuntime.DirectML_license.txt` (MIT) | `onnxruntime.dll` beside the exe (.NET's native library probing) |
| **OCR models** | Committed to ShareX's repo (`ShareX.OCR/Models`) and copied to the build output | `PaddleOCR_license.txt` (Apache-2.0) | `PaddleOCREngine.DefaultModelsFolder`, which is `Models` beside the exe ([OCR.md](OCR.md)) |
| **Languages** | The `.resx` translations, compiled to satellite DLLs | (ShareX's own) | `Languages/<culture>` |

### Portable mode

Portable mode is on when a file named `Portable` sits beside `ShareX.exe`
(`AppPaths.PortableCheckFilePath`), or when ShareX is started with `-portable` or
`-p`. `PersonalPathManager` then keeps settings in `ShareX/` beside the exe instead
of `Documents/ShareX`. Portable mode also skips the run-time file type registration
(`FileTypeRegistration`) and changes how updates install (below).

### The installer (`ShareX.Setup/InnoSetup/ShareX-setup.iss`, Inno Setup 6)

- `AppId` and `AppMutex` are `82E6AC09-0FEF-4390-AD9F-0DD3F5561EFC`.
- `PrivilegesRequired=none`. An administrator elevates and installs to
  `{commonpf}\ShareX`; a non-admin gets `{userpf}\ShareX` (`InitializeWizard`).
- `MinVersion=10.0.14393`. There's a licence page (`LICENSE.txt`).
  `ArchitecturesAllowed` is `x64compatible` or `arm64`.
- Tasks are all checked by default and hidden with `/UPDATE`:
  - a desktop shortcut (unless one exists);
  - the **"Upload with ShareX" Explorer context menu**;
  - a Send To shortcut;
  - **run at startup** (`{userstartup}\ShareX.lnk` with `-silent`);
  - browser extension support (Chrome and Firefox native messaging hosts);
  - disable the Print Screen key for the Snipping Tool.
- `[Icons]` adds the Start menu entry and its uninstall entry. `[Run]` starts ShareX
  after install unless `/NORUN` is given.
- `[Registry]` (all HKCU) adds `Software\Classes\*\shell\ShareX` and
  `Directory\shell\ShareX` (`"Upload with ShareX"`, icon `ShareX.exe,0`, command
  `"ShareX.exe" "%1"`). On uninstall it also removes:
  - `.sxcu`, `ShareX.sxcu`, `.sxie` and `ShareX.sxie`;
  - `SystemFileAssociations\image\shell\ShareXImageEditor` and `ShareXPhotoEditor`;
  - the native messaging host keys;
  - `Control Panel\Keyboard\PrintScreenKeyForSnippingEnabled`.
- On uninstall, if the mutex is held, ShareX asks to close itself and runs
  `taskkill /f /im ShareX.exe`.
- `[InstallDelete]` removes language DLLs that ShareX 18 put beside the exe.

### Shell integration at run time (`IntegrationHelpers`)

The installer only creates the context menu. The app registers the rest itself:

| What | Key or place | Command |
|---|---|---|
| Upload with ShareX | `HKCU\Software\Classes\*\shell\ShareX`, `Directory\shell\ShareX` | `"ShareX.exe" "%1"` |
| Edit with ShareX (image editor) | `…\SystemFileAssociations\image\shell\ShareXImageEditor` | `-ImageEditor "%1"` |
| Photo editor | `…\image\shell\ShareXPhotoEditor` | `-PhotoEditor "%1"` |
| `.sxcu` custom uploaders | `.sxcu` → `ShareX.sxcu` ("ShareX custom uploader", `DefaultIcon` `ShareX_File_Icon.ico`) | `-CustomUploader "%1"` |
| `.sxie` image effects | `.sxie` → `ShareX.sxie` ("ShareX image effect") | `-ImageEffect "%1"` |
| Browser extensions | `HKCU\SOFTWARE\Google\Chrome\NativeMessagingHosts\com.getsharex.sharex`, `…\Mozilla\NativeMessagingHosts\ShareX` | `host-manifest-*.json` |
| Send To | `SendTo\ShareX.lnk` | |
| Run at startup | `Startup\ShareX.lnk -silent`. "Disabled by user" is read from `Explorer\StartupApproved\StartupFolder` | |

`FileTypeRegistration.RegisterMissingExtensions` registers `.sxcu` and `.sxie` again
at every start, except in portable and Store builds. `ShareX.exe -uninstall` runs
`IntegrationHelpers.Uninstall`, which turns all of the above off.

### Update check (`ShareXUpdateManager`)

- ShareX runs `GitHubUpdateChecker("ShareX", "ShareX")` every hour while
  `AutoCheckUpdate` is on and the `DisableUpdateCheck` policy is off.
- The latest release comes from `/releases/latest`; on the pre-release channel it's
  the newest of `/releases`. The tag must start with `v`, and the revision is ignored.
- The asset is chosen **by its file-name suffix**:
  - portable: `portable-arm64.zip` (on ARM64), else `portable-x64.zip`, else
    `portable.zip`;
  - installed: `setup-arm64.exe` (on ARM64), else `setup-x64.exe`, else `setup.exe`.
- Portable builds open the asset's browser download URL. Installed builds download
  the asset and run it with `/UPDATE` (plus `/SILENT` or `/VERYSILENT`). The installer
  then skips its tasks and starts ShareX again.

## 2. ShareXrs's release

### Workflows

- **`.github/workflows/package.yml`** (reusable) builds every package, with one job
  per platform. Every package bundles FFmpeg, ONNX Runtime and the OCR models.
- **`.github/workflows/release.yml`** runs on a `v*` tag. It calls `package.yml` with
  the secrets, then a release job:
  - writes the notes with `scripts/release-notes.sh`: the commits since the last tag,
    grouped by Conventional Commits type, plus a SHA256 table as in ShareX's body;
  - writes `SHA256SUMS`;
  - runs `gh release create --verify-tag`. Tags with a `-` become pre-releases.

  A **manual run** on a branch builds everything and publishes nothing; the packages
  are the run's artifacts.
- **`.github/workflows/ci.yml`** also calls `package.yml` without secrets, on master
  pushes and on pull requests that touch packaging. These packages are signed ad hoc
  on the Mac, have no API keys, and are kept for 3 days.
- A tag fails if it isn't `v<Cargo.toml version>`, or if the macOS signing secrets
  are missing.

### Artifacts

| Platform | File | Contents |
|---|---|---|
| Windows x64 and ARM64 | `ShareXrs-<ver>-setup-<x64\|arm64>.exe` | The Inno Setup installer (`packaging/windows/ShareXrs-setup.iss`) |
| | `ShareXrs-<ver>-portable-<x64\|arm64>.zip` | At the zip's root: `sharexrs.exe`, `ffmpeg.exe`, `onnxruntime.dll`, `Models/`, `Licenses/`, `Portable` |
| macOS Apple silicon | `ShareXrs-macos-aarch64.dmg` | `ShareXrs.app` and a link to `Applications` |
| | `ShareXrs-macos-aarch64.app.zip` | `ShareXrs.app` (packed with `ditto`), for an updater that replaces the whole app |
| Linux x86_64 | `ShareXrs-<ver>-linux-x64.deb` | `/usr/lib/sharexrs/{sharexrs, ffmpeg, libonnxruntime.so, Models/}`, `/usr/bin/sharexrs` (launcher), `/usr/share/applications/sharexrs.desktop`, hicolor icons, `/usr/share/doc/sharexrs/{copyright, Licenses/}` |
| | `ShareXrs-<ver>-linux-x64.tar.gz` | `ShareXrs/{sharexrs, ffmpeg, libonnxruntime.so, Models/, Licenses/}` |
| All | `SHA256SUMS` | |

`ShareXrs.app` contains:

- `Contents/MacOS/{sharexrs, ffmpeg}`;
- `Contents/Frameworks/libonnxruntime.dylib`;
- `Contents/Resources/{ShareXrs.icns, Models/, Licenses/}`;
- `Info.plist`, with the bundle ID `com.sharexrs.app` (never change it).

FFmpeg and ONNX Runtime are signed first, with the same identity, and then the app.
The `cdhash` check fails the build if a real identity still left a hash-only
designated requirement.

`Licenses/` holds:

- `ShareXrs_license.txt` (GPL-3.0);
- `FFmpeg_license.txt` (ShareX's copy, GPL v3);
- `FFmpeg_source.txt` (that build's source, generated; see section 4);
- `OnnxRuntime_license.txt` and `OnnxRuntime_ThirdPartyNotices.txt` (from the package);
- `PaddleOCR_license.txt` (ShareX's copy);
- `Rust_crates.txt`: every crate compiled into `sharexrs` for that target, with its
  licence, from `cargo metadata`.

The asset names keep ShareX's suffixes (`setup-x64.exe`, `portable-arm64.zip`, …), so
ShareX's update checker logic will find them unchanged once it's ported.

### Where the app finds the pieces

The app looks beside the executable, as ShareX does, and on macOS also in the
bundle's folders. `library_candidates_for`, `models_folder_for` and
`fallback_locations_for` take the OS name, so unit tests cover each platform's layout
on any host.

| Piece | Lookup | Code |
|---|---|---|
| FFmpeg | ShareX's rule: the custom path, else `ffmpeg[.exe]` beside the exe (`Contents/MacOS` in the app, `/usr/lib/sharexrs` in the .deb). Without a custom path, it then falls back to: macOS `Contents/Resources`, Homebrew and MacPorts; `PATH`; Linux `/usr/bin` and `/usr/local/bin` | `sharex-screen-recording/src/discovery.rs` |
| ONNX Runtime | `ORT_DYLIB_PATH`, then beside the exe, then `Contents/Frameworks` (when in `X.app/Contents/MacOS`) | `sharex-ocr/src/runtime.rs` |
| OCR models | `SHAREXRS_OCR_MODELS`, then `Models` beside the exe, then `Contents/Resources/Models` (macOS, when there's none beside the exe) | `sharex-ocr/src/engine.rs` |
| Portable marker | `Portable` beside the exe, or `-portable`/`-p`: settings go in `ShareXrs/` beside the exe | `sharex-settings/src/paths.rs` |

On Linux, `current_exe()` resolves `/proc/self/exe`. The `/usr/bin/sharexrs` launcher
runs `exec /usr/lib/sharexrs/sharexrs`, so "beside the exe" is `/usr/lib/sharexrs`.

### The Windows installer, compared with ShareX's

`packaging/windows/ShareXrs-setup.iss` is ShareX's script with ShareXrs's names:

| | ShareX | ShareXrs |
|---|---|---|
| AppId | `82E6AC09-…` | `F7492E77-AA16-4212-9FD6-53D97873DD92` (never change it) |
| Install folder | `{commonpf}\ShareX` or `{userpf}\ShareX` | `{commonpf}\ShareXrs` or `{userpf}\ShareXrs`. A `/DIR=` on the command line is kept (ShareX overrides it for non-admins) |
| Privileges, MinVersion, licence page, architectures | as listed above | the same |
| Running copy | `AppMutex`, and a `taskkill` prompt on uninstall | ShareXrs has no mutex yet. Setup uses the Restart Manager; uninstall finds `sharexrs.exe` with `tasklist`, then shows the same prompt (suppressible) and runs `taskkill` |
| Desktop, Start menu, startup | checked; `ShareX.lnk -silent` | the same, with `ShareXrs.lnk -silent`, the shortcut `startup.rs` reads and writes |
| Context menu, Send To | checked | **unchecked** until ShareXrs acts on files passed on its command line. `ApplicationCommandLine` isn't ported, so they'd only start a second ShareXrs |
| Context menu keys | `*\shell\ShareX`, `Directory\shell\ShareX` | `*\shell\ShareXrs`, `Directory\shell\ShareXrs` ("Upload with ShareXrs") |
| Browser extension support | native messaging hosts | left out: ShareXrs has no native messaging host |
| Print Screen for Snipping Tool | checked; the value is removed on uninstall | the same, but only offered when Print Screen still opens the Snipping Tool, so a value ShareX set is never removed |
| Removed on uninstall | `.sxcu`, `ShareX.sxcu`, `.sxie`, `ShareX.sxie`, the image and photo editor keys | `ShareXrs.sxcu`, `ShareXrs.sxie`, `ShareXrsImageEditor`, `ShareXrsPhotoEditor`. In the shared `.sxcu` and `.sxie` keys, only the `OpenWithProgids` value `ShareXrs.*` and a default that is `ShareXrs.*` are removed, so ShareX's registration stays |
| Old language DLLs | `[InstallDelete]` | not needed (the translations are inside `sharexrs.exe`) |
| `/UPDATE`, `/NORUN` | | the same |

When `FileTypeRegistration` and `IntegrationHelpers` are ported, they must use the
`ShareXrs.*` ProgIDs and add them to `OpenWithProgids`. They should take the default
of `.sxcu` or `.sxie` only when nothing else has it, which is what the uninstaller
expects.

### Not shipped (and why)

- **Debug zip**: release builds carry line numbers only.
- **Steam and Microsoft Store**: ShareXrs isn't in either store, and the Steam
  launcher and `AppxManifest` are ShareX's identities.
- **Windows code signing**: ShareX uses Azure Trusted Signing. ShareXrs has no
  Windows certificate, so SmartScreen warns on first download.
- **Notarisation**: the Mac app is self-signed, so the first launch needs System
  Settings › Privacy & Security › Open Anyway ([MACOS-LEARNINGS.md](MACOS-LEARNINGS.md)
  1.3).
- **`ShareX_File_Icon.ico`**: ShareXrs doesn't register `.sxcu` or `.sxie` yet.
- **Languages folder**: the translations are compiled into the program.
- **The update check**: not ported yet (see the PORTING.md tracker). The assets are
  already named for it.

## 3. The bundled pieces, pinned

Every download is pinned by URL and SHA256 in `packaging/bundled.json`.
`scripts/fetch-bundled.py` checks each one every time it's used, and CI caches the
downloads by the manifest's hash. Inno Setup 6.7.4 is pinned the same way in
`package.yml`.

To bump a piece:

1. Change its URL, SHA256 and archive member together.
2. Run the script for each platform.
3. Update this table.

| Piece | Platform | Source | SHA256 of the download |
|---|---|---|---|
| FFmpeg n9.0.2-39-g7adfbe40dc, GPL static | Windows x64 and ARM64, Linux x64 | BtbN FFmpeg-Builds `autobuild-2026-10-31-12-59`: the same builds ShareX's CI takes from the rolling `latest` (`…-<win64\|winarm64\|linux64>-gpl-9.0`). Month-end autobuilds are kept, so they can be pinned | `693f3e22…`, `8b3c3947…`, `73e29a56…` |
| FFmpeg 9.0.2, GPL static | macOS arm64 | Martin Riedl's build server, release build (BtbN has no macOS build) | `c8ed4c4e…` |
| ONNX Runtime 1.24.4, CPU | all four | NuGet `Microsoft.ML.OnnxRuntime` 1.24.4, Microsoft's CPU package, from the same channel and at the same version as ShareX's. GitHub's onnxruntime releases page returns 404. The nupkg's SHA512 matches nuget.org's catalog | `4b978d50…` |
| PP-OCRv6 small det and rec | all | ShareX's repo at `REFERENCE_COMMIT`, `ShareX.OCR/Models/*` (raw.githubusercontent) | per file in the manifest |

The NuGet CPU `onnxruntime.dll` (win-x64) with these models passes `sharex-ocr`'s
`engine_matches_sharex`. On all 8 fixtures its boxes and scores are bit-identical to
ShareX's own run, which used the DirectML package's DLL.

## 4. Licences

ShareXrs is GPL-3.0-or-later. Each piece is distributed beside it as a separate
program or library: FFmpeg runs as a separate process, and ONNX Runtime is loaded at
run time.

| Piece | Licence | With GPL-3.0-or-later | What we ship |
|---|---|---|---|
| FFmpeg (BtbN, Martin Riedl) | GPL v3 (`--enable-gpl --enable-version3`). The linked libraries include x264 and x265 (GPL) and OpenSSL 3 (Apache-2.0, which works with GPLv3), among others. No `--enable-nonfree` | Compatible: the same licence, and an aggregate anyway. GPLv3 §6 requires the Corresponding Source | The licence text, and `FFmpeg_source.txt`. That file names the exact build and where its source and build scripts are: FFmpeg's commit and BtbN's build-scripts commit (which pins every library's source), or FFmpeg 9.0.2 with Riedl's build script and `versions.txt`. It also carries a three-year **written offer** (§6(b)) to provide the source on request, through the repo's issues |
| ONNX Runtime 1.24.4 | MIT, with third-party notices in the package | Compatible | `OnnxRuntime_license.txt` and `OnnxRuntime_ThirdPartyNotices.txt` |
| PP-OCRv6 models | Apache-2.0 (PaddleOCR) | Compatible with GPLv3 (not with GPLv2) | `PaddleOCR_license.txt` |
| Rust crates | MIT, Apache-2.0, BSD, Zlib, MPL-2.0, Unicode and others (see `Rust_crates.txt`) | Compatible | The list with each crate's licence (not the full texts) |
| Inno Setup | Inno Setup licence | Only the installer is built with it, and its runtime stub may be used in any installer | — |

The release maintainer is bound by the written offer. For three years after each
release that ships an FFmpeg build, they must be ready to supply its source: the BtbN
build-scripts commit, its dependencies' sources and FFmpeg's commit, or Riedl's.
Attaching the full source to each release would make the offer unnecessary, but for
a static build with dozens of libraries it's large.

## 5. Validation

- **Fetching**: `scripts/fetch-bundled.py` ran for all four platforms (downloads,
  SHA256s, archive members). The extracted binaries' headers were checked: PE x64 and
  ARM64, Mach-O arm64, ELF x86_64.
- **Windows packages**: `scripts/package-windows.ps1` (Windows PowerShell 5.1) built
  x64 and ARM64 (ARM64 cross-compiled) into a scratch folder. The portable zip's
  entries are named with `/`. Both installers compiled with Inno Setup 6.7.4 (a
  portable copy), with no warnings.
- **Installer round trip**, using the same script with `PrivilegesRequired=lowest`.
  With ShareX's `none`, Setup asks an administrator account for elevation, which an
  unattended session can't grant, so it exits with code 2.
  - A silent install with the default tasks created the files, the Start menu entry,
    the desktop shortcut, the `ShareXrs.lnk -silent` startup shortcut and the
    uninstall entry. With `/NORUN`, nothing was launched.
  - A silent install with the context menu and Send To created the keys and commands
    as in the table.
  - A silent uninstall removed all of it, including a `ShareXrs.sxcu` value under the
    user's existing ShareX `.sxcu` key, and left `.sxcu` → `ShareX.sxcu` alone.
- **Linux packages**: a dry run of `scripts/package-linux.sh` on Windows, with
  cargo-deb 3.8 and a stand-in binary. The `.deb`'s control file and its data paths
  and modes were checked, as was the tarball's layout. `Depends` is empty there
  because `dpkg-shlibdeps` only exists on Linux; CI fills it.
- **Linting**: `shellcheck` 0.11 on the shell scripts, and `actionlint` 1.7.12 (with
  shellcheck) on the workflows. Both clean.
- **macOS**: `macos-bundle.sh` (the payload and nested signing) is only checked by
  shellcheck here. It runs on the macOS runner in CI.

## 6. One-time setup

1. **Create the macOS signing certificate.** Do this once and keep the certificate
   forever: a new one resets every user's Screen Recording and Accessibility grants.

   ```bash
   openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=ShareXrs self-signed" \
     -addext "keyUsage=critical,digitalSignature" -addext "extendedKeyUsage=critical,codeSigning" \
     -keyout key.pem -out cert.pem
   openssl pkcs12 -export -legacy -inkey key.pem -in cert.pem -out cert.p12 -passout pass:<password>
   base64 -i cert.p12 | pbcopy        # or: base64 -w0 cert.p12
   ```

   Back up `cert.p12` and its password somewhere safe. The workflow finds the identity
   by its SHA-1, so the certificate's name doesn't matter.
2. **Add the secrets** (Settings › Secrets and variables › Actions):
   `MACOS_SIGNING_P12` (the base64) and `MACOS_SIGNING_P12_PASSWORD`.
3. **Register the API keys.** Register ShareXrs's own application with each service
   that needs one; never reuse ShareX's. Add each key as a secret named after its
   variable. The names all start with `SHAREXRS_KEY`:
   `_IMGUR_CLIENT_ID`, `_IMGUR_CLIENT_SECRET`, `_IMAGE_SHACK_KEY`,
   `_FLICKR_KEY`, `_FLICKR_SECRET`, `_PHOTOBUCKET_CONSUMER_KEY`,
   `_PHOTOBUCKET_CONSUMER_SECRET`, `_PASTEBIN_KEY`, `_GITHUB_ID`, `_GITHUB_SECRET`,
   `_PASTE_EE_APPLICATION_KEY`, `_DROPBOX_CONSUMER_KEY`, `_DROPBOX_CONSUMER_SECRET`,
   `_BOX_CLIENT_ID`, `_BOX_CLIENT_SECRET`, `_SEND_SPACE_KEY`, `_MEDIA_FIRE_APP_ID`,
   `_MEDIA_FIRE_API_KEY`, `_ONE_DRIVE_CLIENT_ID`, `_ONE_DRIVE_CLIENT_SECRET`,
   `_BITLY_CLIENT_ID`, `_BITLY_CLIENT_SECRET`, `_GOOGLE_CLIENT_ID`,
   `_GOOGLE_CLIENT_SECRET`. A missing secret builds an empty key, and that service
   then behaves as in a ShareX built from source.
4. **Do a dry run.** Run Actions › Release › Run workflow on master, download the
   artifacts and try each package. On the Mac, check that the app opens and that
   `codesign -dr - ShareXrs.app` names the certificate, not a `cdhash`.
5. **Release.** Bump `[workspace.package] version`, commit, then tag `v<version>` and
   push the tag.
