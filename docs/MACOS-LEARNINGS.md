# macOS (and packaging) learnings from snapr

Distilled from `G:/scripts/snapr` (Rust, egui 0.36 + egui-wgpu + winit 0.30,
xcap 0.9, objc2 0.6 / objc2-app-kit 0.3). Every item names the snapr commit
that fixed it. Read with `git -C G:/scripts/snapr show <sha>`.

Key snapr files: `scripts/macos-bundle.sh`, `.github/workflows/release.yml`,
`.cargo/config.toml`, `build.rs`, `Cargo.toml`, `src/macos.rs`,
`src/tray/macos.rs`, `src/hotkey/mod.rs` (macOS `platform` module),
`src/update.rs`, `src/theme.rs`, `src/capture.rs`, `src/overlay.rs`,
`src/session.rs`, `src/record.rs`.

---

## 1. Bundling and signing

### 1.1 Ship a real `.app`, never a bare binary (d44d1b8)

The first macOS releases were a bare `snapr-macos-aarch64` executable. TCC
(the permission database), the Dock icon, `LSUIElement` and usage strings all
need a bundle. snapr now builds `snapr.app` in `scripts/macos-bundle.sh`:

```
snapr.app/
  Contents/
    Info.plist
    MacOS/snapr            (chmod 755)
    Resources/snapr.icns   (iconutil from assets/snapr.iconset)
```

`scripts/macos-bundle.sh <binary> <version> <arch> <out>` writes
`snapr-macos-<arch>.dmg` and `snapr-macos-<arch>.app.zip`.

### 1.2 Info.plist, every key and why

```xml
<key>CFBundleIdentifier</key>        <string>st.cubityfir.snapr</string>
<key>CFBundleName</key>              <string>snapr</string>
<key>CFBundleDisplayName</key>       <string>snapr</string>
<key>CFBundleExecutable</key>        <string>snapr</string>
<key>CFBundleIconFile</key>          <string>snapr</string>   <!-- Resources/snapr.icns, b1bd6f9 -->
<key>CFBundlePackageType</key>       <string>APPL</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key>           <string>$version</string>
<key>LSMinimumSystemVersion</key>    <string>11.0</string>
<key>LSUIElement</key>               <true/>
<key>NSHighResolutionCapable</key>   <true/>
<key>NSMicrophoneUsageDescription</key>
<string>snapr records your microphone with the screen when you turn that on in Settings.</string>
<key>NSAudioCaptureUsageDescription</key>
<string>snapr records what's playing with the screen when you turn that on in Settings.</string>
```

- `CFBundleIdentifier`: TCC files grants under the bundle id **and** the
  signature's designated requirement. Never change it once shipped.
- `LSUIElement=true`: no Dock icon / app-switcher entry from launch (menu bar
  app). The Dock icon is toggled at runtime instead (section 3.5).
- `NSHighResolutionCapable=true`: otherwise Retina renders blurry at 1x.
- `NS*UsageDescription`: without them macOS refuses mic / system-audio access
  outright (no prompt). Screen Recording and Accessibility have no plist key.
- The script runs `plutil -lint "$app/Contents/Info.plist"` after writing it.
- Version comes from `Cargo.toml` (`sed -n 's/^version = "\(.*\)"/\1/p'`).

### 1.3 Signing: why ad hoc lost permissions on every update (6d6bbe7)

Original: `codesign --force --sign - "$app"` (ad hoc). Apple silicon refuses
unsigned code, so some signature is mandatory, but **an ad hoc signature's
designated requirement (DR) is `cdhash H"..."`, the hash of that exact
build**. Every update was a "new app" to TCC, so Screen Recording and
Accessibility grants were silently lost after each update.

Fix: sign every release with the **same self-signed certificate**. The DR
then names the bundle id and the certificate (typically `identifier "<bundle
id>" and certificate root = H"<cert sha1>"`), not the build hash, so grants
survive updates. Replacing the cert resets everyone's
permissions once. The first release signed this way still asked once.

Current signing block in `scripts/macos-bundle.sh`:

```bash
identity=${SNAPR_SIGN_IDENTITY:--}
codesign --force --sign "$identity" ${SNAPR_SIGN_KEYCHAIN:+--keychain "$SNAPR_SIGN_KEYCHAIN"} "$app"
codesign --verify --strict --verbose=2 "$app"
requirement=$(codesign --display --requirements - "$app" 2>&1)
echo "$requirement"
if [ "$identity" != - ] && grep -q cdhash <<<"$requirement"; then
  echo "error: the designated requirement is the build's hash, not the certificate" >&2
  exit 1
fi
```

- The **cdhash guard** fails the build if a real identity was requested but
  the DR still pins the hash (e.g. the cert was not found/trusted).
- **Ad hoc fallback**: no `SNAPR_SIGN_IDENTITY` -> `-`. Fine locally and on
  manual CI runs; tagged releases refuse (2.3). Cert CN = `snapr self-signed`.
- Not notarised. Users must "Open Anyway" under System Settings > Privacy &
  Security on first launch (docs/GUIDE.md).

Creating the cert is **not** documented in snapr. Our suggestion (untested
there): Keychain Access > Certificate Assistant > Create a Certificate, Self
Signed Root, type Code Signing, export .p12, `base64 -i cert.p12` into the
secret. Or with openssl:

```bash
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=sharexrs self-signed" \
  -addext "keyUsage=critical,digitalSignature" -addext "extendedKeyUsage=critical,codeSigning" \
  -keyout key.pem -out cert.pem
openssl pkcs12 -export -legacy -inkey key.pem -in cert.pem -out cert.p12 -passout pass:SECRET
```

(`-legacy`: `security import` has rejected OpenSSL 3's default PKCS#12
encryption.) Losing the .p12 = permission reset for every user.

### 1.4 Zip and dmg layout (d44d1b8)

```bash
ditto -c -k --keepParent "$app" "$out/snapr-macos-$arch.app.zip"   # for the updater

stage="$work/dmg"; mkdir "$stage"
ditto "$app" "$stage/snapr.app"
ln -s /Applications "$stage/Applications"                           # drag-to-install
hdiutil create -volname snapr -srcfolder "$stage" -fs HFS+ -format UDZO -ov \
  "$out/snapr-macos-$arch.dmg"
```

Use `ditto`, not `zip`/`unzip`: it preserves the signature, xattrs and
symlinks that `unzip` can drop (the updater also unpacks with `ditto -x -k`).

### 1.5 Icon: iconset -> icns (b1bd6f9)

- `assets/snapr.iconset/icon_{16,32,128,256,512}x{same}[ @2x].png` (10
  files), generated by an ignored test from the app's own icon renderer:
  `cargo test write_iconset -- --ignored` (`src/tray/mod.rs`).
- Bundle step: `iconutil -c icns assets/snapr.iconset -o $app/Contents/Resources/snapr.icns`.
- The same iconset feeds the Windows `.ico` (section 4.3) and Linux hicolor
  icons in `[package.metadata.deb]`. One source of truth.

### 1.6 Self-update of a bundle, translocation (d44d1b8, c1fdfc3)

`src/update.rs`:
- Detect the bundle from the exe path: `X.app/Contents/MacOS/X` ->
  `X.app` (`app_of()`, checks the `MacOS`, `Contents` names and `.app` ext).
- Bundled installs download `snapr-macos-<arch>.app.zip` and swap the
  **whole .app** (keeps the signature seal intact; replacing only the binary
  would break it). Bare installs replace the binary.
- `replace_app`: `ditto -x -k zip app.unpacked`; verify
  `unpacked/snapr.app/Contents/MacOS/snapr` exists; `rename(app, app.old)`;
  `rename(new, app)` (roll back on failure). The running process keeps its
  open files.
- **App Management** (macOS 13+) can refuse deleting files inside another
  app bundle, but moving the bundle is allowed: the old app is renamed into
  `std::env::temp_dir()/snapr-old-<rand>.app` instead of `remove_dir_all`.
- **App Translocation**: launched from the dmg or Downloads (quarantined),
  macOS runs a read-only random copy; if the app path contains
  `/AppTranslocation/`, refuse with "drag it to Applications".

- Cache `current_exe()` in a `OnceLock` at startup: after replacement it can
  point at the moved-away copy. `restart()` spawns that cached path.
- Clean up leftovers (`.new`, `.old*`, `.unpacked`) on start (`init()`).
- Release asset names must match `update::asset_name`:
  `snapr-<OS>-<ARCH>[.exe][.app.zip]` built from `std::env::consts`.
- The bare binary asset is still published for pre-bundle installs.

---

## 2. CI / release workflow

### 2.1 Matrix (87fd4b0, da9abcf, 6bda3d2)

```yaml
on:
  push:
    tags: ["v*"]
  workflow_dispatch:          # manual run = build only, nothing published
permissions:
  contents: write
jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            asset: snapr-windows-x86_64.exe
          # Cross-compiled on x64 (image has MSVC ARM64 tools + clang for ring)
          - os: windows-latest
            target: aarch64-pc-windows-msvc
            asset: snapr-windows-aarch64.exe
          - os: macos-latest
            target: aarch64-apple-darwin
            asset: snapr-macos-aarch64
          # 24.04, not 22.04: libspa/PipeWire bindings need newer headers
          - os: ubuntu-24.04
            target: x86_64-unknown-linux-gnu
            asset: snapr-linux-x86_64
```

- The **x86_64-apple-darwin (Intel) build was dropped** (87fd4b0): Apple
  silicon only. If sharexrs wants Intel, add a second target and `lipo` a
  universal binary before bundling; snapr never did this.
- `workflow_dispatch` + `if: github.ref_type == 'tag'` on the tag check and
  release job lets you dry-run all platform builds (da9abcf). Do this before
  the first tag; the macOS build had silently broken (see 3.12).

### 2.2 Common steps

```yaml
      - uses: actions/checkout@v4
      - name: Check the tag matches Cargo.toml
        if: github.ref_type == 'tag'
        shell: bash
        run: |
          version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
          if [ "v$version" != "$GITHUB_REF_NAME" ]; then
            echo "::error::Tag $GITHUB_REF_NAME doesn't match the version in Cargo.toml ($version)"
            exit 1
          fi
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}
      - name: Build
        run: cargo build --release --locked --target ${{ matrix.target }}
```

A tag/Cargo.toml mismatch would make the build offer itself as an update
forever, hence the check.

### 2.3 macOS signing step (secrets `MACOS_SIGNING_P12`, `MACOS_SIGNING_P12_PASSWORD`)

```yaml
      - name: Load the signing certificate
        if: runner.os == 'macOS'
        env:
          P12: ${{ secrets.MACOS_SIGNING_P12 }}           # base64 of the .p12
          P12_PASSWORD: ${{ secrets.MACOS_SIGNING_P12_PASSWORD }}
        run: |
          if [ -z "$P12" ]; then
            if [ "$GITHUB_REF_TYPE" = tag ]; then
              echo "::error::The MACOS_SIGNING_P12 secret is missing"
              exit 1
            fi
            echo "::warning::No MACOS_SIGNING_P12 secret: signing ad hoc"
            exit 0
          fi
          keychain="$RUNNER_TEMP/signing.keychain-db"
          password=$(openssl rand -hex 16)
          security create-keychain -p "$password" "$keychain"
          security set-keychain-settings -lut 3600 "$keychain"
          security unlock-keychain -p "$password" "$keychain"
          base64 --decode <<<"$P12" > "$RUNNER_TEMP/signing.p12"
          security import "$RUNNER_TEMP/signing.p12" -k "$keychain" -P "$P12_PASSWORD" -T /usr/bin/codesign
          rm "$RUNNER_TEMP/signing.p12"
          security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" > /dev/null
          security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
          echo "SNAPR_SIGN_IDENTITY=snapr self-signed" >> "$GITHUB_ENV"
          echo "SNAPR_SIGN_KEYCHAIN=$keychain" >> "$GITHUB_ENV"
```

Notes: `set-key-partition-list` stops codesign from prompting for keychain
access (would hang CI); `list-keychains -s` adds the temp keychain to the
search list while keeping the existing ones; a tagged release hard-fails
without the secret (prevents an accidental ad hoc release that would reset
users' permissions).

### 2.4 Package step

```yaml
      - name: Package snapr.app
        if: runner.os == 'macOS'
        run: |
          version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
          bash scripts/macos-bundle.sh "target/${{ matrix.target }}/release/snapr" "$version" aarch64 dist
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset }}
          path: dist/*                    # was dist/<asset>; bundle adds 2 files
          if-no-files-found: error
```

### 2.5 Release job

`needs: build`, `if: github.ref_type == 'tag'`, `ubuntu-latest`: checkout with
`fetch-depth: 0` (notes from commits via `scripts/release-notes.sh`),
`download-artifact@v4` with `merge-multiple: true`, `sha256sum snapr-* >
SHA256SUMS`, then `gh release create "$GITHUB_REF_NAME" dist/* --verify-tag
--notes-file notes.md` plus `--prerelease` when the tag contains `-`. The
updater verifies downloads against `SHA256SUMS` and ignores pre-releases.
Linux also runs `cargo deb --no-build` (taiki-e/install-action `cargo-deb`).

---

## 3. Runtime pitfalls and fixes

### 3.1 Screen Recording permission: blank captures, not errors (be43a49)

Without the permission macOS returns **blank/wallpaper-only images, no
error**. Check before every capture and prompt once at startup:

```rust
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}
pub fn screen_recording_allowed() -> bool { unsafe { CGPreflightScreenCaptureAccess() } }
pub fn ask_for_screen_recording() {
    if !screen_recording_allowed() { unsafe { CGRequestScreenCaptureAccess() }; }
}
```

- Ask in `main()` right after building the event loop; macOS prompts only the
  first time, afterwards the app is just listed (off) in System Settings.
  `capture::grab` and `Session::new` return `SCREEN_RECORDING_OFF` on failure.
- **A grant only takes effect after relaunch** -> offer a Restart button.

### 3.2 Accessibility permission for global hotkeys (02e4292, 2b6f767)

snapr uses `livesplit-hotkey` (`Hook::with_consume_preference(PreferConsume)`),
which needs Accessibility on macOS. When the hook fails, ask once per run:

```rust
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" { fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8; }
let key = NSString::from_str("AXTrustedCheckOptionPrompt"); // kAXTrustedCheckOptionPrompt
let options = NSDictionary::from_slices(&[&*key], &[&*NSNumber::new_bool(true)]);
unsafe { AXIsProcessTrustedWithOptions(Retained::as_ptr(&options).cast()) != 0 }
```

Cached in a `static OnceLock<bool>` so the system prompt is not spammed.
e991c1b: a failed hotkey registration must **not** block saving settings
(first-run setup included); show the error but save.

### 3.3 Permission UX: deep links, macOS 27 rename, restart (2b6f767, 0a54a01)

- Deep links (open with `open <url>`):
  - Screen Recording: `x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture`
  - Accessibility: `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`
- macOS 27 renamed "Accessibility" to "Device Control and Data Access" (link
  unchanged); "Screen Recording" is now "Screen & System Audio Recording".
  Text: "Device Control and Data Access (Accessibility before macOS 27)".
- egui's default font has no arrow glyphs: `→` rendered as tofu boxes. Write
  paths as `System Settings > Privacy & Security > ...` (Apple's own style),
  or load a font with arrows (3.10).
- `0a54a01`: a missing permission becomes a card (title, steps, "Open System
  Settings", "Restart snapr") instead of an unwrapped status line. Errors are
  matched by `msg.ends_with(SCREEN_RECORDING_OFF)` etc. (`missing_permission()`
  in `src/macos.rs`). Status labels must `.wrap()`; these messages are long.
- Restart = `Command::new(cached_exe).spawn()` then quit.

### 3.4 Overlay above menu bar and Dock, on all Spaces (be43a49)

winit's `WindowLevel::AlwaysOnTop` is NSFloatingWindowLevel, **below** the
menu bar and Dock. And `Fullscreen::Borderless` on macOS switches Spaces with
an animation, so snapr does **not** use fullscreen there; it opens a
borderless, monitor-sized window and raises it via AppKit:

```rust
fn ns_window(window: &winit::window::Window) -> Option<Retained<NSWindow>> {
    let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else { return None };
    let view: &NSView = unsafe { h.ns_view.cast().as_ref() };
    view.window()
}
// cover_everything():
ns_window.setLevel(1000); // NSScreenSaverWindowLevel
ns_window.setCollectionBehavior(
    NSWindowCollectionBehavior::CanJoinAllSpaces
  | NSWindowCollectionBehavior::FullScreenAuxiliary
  | NSWindowCollectionBehavior::Stationary);
```

`FullScreenAuxiliary` lets it appear over other apps' full-screen Spaces.

### 3.5 Activation policy and Dock icon toggling (be43a49)

- At launch: `LSUIElement=true` in the plist **and**
  `EventLoopBuilderExtMacOS::with_activation_policy(ActivationPolicy::Accessory)`
  on the winit builder (covers running the bare binary too).
- When the main/settings window opens: `NSApplication::sharedApplication(mtm)
  .setActivationPolicy(NSApplicationActivationPolicy::Regular)` +
  `activateIgnoringOtherApps(true)` (deprecated, `#[allow(deprecated)]`), so it
  gets a Dock icon, Cmd-Tab entry and actually comes to the front.
- On `CloseRequested` of the main window: back to `Accessory`.
- Needs `MainThreadMarker::new()`; return early off the main thread.

### 3.6 Focus restore after capture (4613b00)

Focusing the overlay activates snapr; when it closes, macOS brings snapr's
other windows (settings) forward instead of the app the user was in.

Fix: `NSWorkspace::sharedWorkspace().frontmostApplication()` (skip if its
`processIdentifier()` is our pid), later
`app.activateWithOptions(NSApplicationActivationOptions::empty())`
(deprecated API, `#[allow(deprecated)]`). Capture it **before** the first overlay opens (only when no
session is already running); call `give_focus_to` on Cancel/Done/Record, but
not when the outcome opens a snapr window (results, pin).

### 3.7 Overlay appearance: fade instead of zoom, and first-frame latency (cdafeb5, b6f2fff)

- macOS's default window "zoom in" animation scales the frozen-screen overlay
  over the real screen, which doubles up visually. Disable and fade instead:

```rust
ns_window.setAnimationBehavior(NSWindowAnimationBehavior::None);
ns_window.setAlphaValue(0.01);          // NOT 0.0, see below
ns_window.setIgnoresMouseEvents(false); // keep the cursor/clicks while faint
// after the first frame is presented:
NSAnimationContext::beginGrouping();
NSAnimationContext::currentContext().setDuration(0.15);
ns_window.animator().setAlphaValue(1.0);
NSAnimationContext::endGrouping();
```

- **Alpha 0.0 is a trap**: macOS doesn't count a fully transparent window as
  on screen, and wgpu's first frame/present waits for that. Use 0.01.
- **Fade after first present** (track a `presented` flag in `render`), not
  at `set_visible`, or the fade is spent on an empty window.
- **Pre-open overlays while capturing** (b6f2fff, PR #3, 141 ms -> ~78 ms
  hotkey-to-first-frame): macOS needs a few frames to treat a new window as on
  screen. snapr opens one overlay window per monitor with
  `.with_transparent(true)` (clear, so not in the screenshot), sets it
  visible, **then** calls `capture_all()`, then reuses the matching window
  (`Overlay::new(..., opened: Option<Arc<Window>>)`, matched by monitor
  position and inner size).
- Gather `foreground_window()` and `window_rects()` **before** any overlay is
  up (they would count as in front).

### 3.8 Retina: points vs pixels everywhere (5f7d816, e991c1b)

macOS APIs (xcap `Monitor::x/y/width/height`, `Window` frames,
`CGWindowListCopyWindowInfo` `kCGWindowBounds`) are in **points**; images are
in **pixels**.

- Monitor placement: `pos = (m.x() * scale, m.y() * scale)` (scale from
  `m.scale_factor()`), only on macOS (`src/capture.rs capture_monitors`).
- Window frames for snapping: multiply by the scale of the monitor under the
  window's centre (else one it overlaps, else primary) (5f7d816, 0c411c7).
- Region recording (e991c1b): `xcap::Monitor::capture_region` takes
  **points** and returns pixels; convert with floor/ceil and crop the
  remainder (`in_points()` / `in_pixels()` in `src/record.rs`).
- **winit converts a new window's physical position/size to points using the
  main screen's scale**, wrong for secondary monitors with other scales. On
  macOS pass logical values with that monitor's own scale (e991c1b):

```rust
#[cfg(target_os = "macos")]
{
    let scale = monitor.scale_factor();
    attrs = attrs
        .with_position(monitor.position().to_logical::<f64>(scale))
        .with_inner_size(size.to_logical::<f64>(scale));
}
```

- **Mixed-scale monitors overlap** in "points x own scale" space (a 4K@2x at
  0..3840 px and a 1080p@1x at logical x=1920 -> both claim 1920..3840).
  snapr's `capture::untangle(&[(Rect, scale)])` re-lays each axis so each
  stretch gets the pixel count of its densest monitor; `to_platform()` maps
  back to winit's space for placing windows (pins). Overlays store the
  `moved` offset and add it in `origin()`. Tests in `src/capture.rs`
  (`mixed_scales_are_untangled_*`) are worth porting as-is. Same problem on
  Wayland.

### 3.9 Window list: skip menu bar, Dock, invisible windows; read once (b3ccab9, 0c411c7)

`CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly (1<<0) |
kCGWindowListExcludeDesktopElements (1<<4), 0)` includes the menu bar, Dock and
other apps' invisible full-screen windows, so snapping hit those first. Keep
only `kCGWindowLayer == 0`, `kCGWindowAlpha > 0`, non-empty `kCGWindowName`.
The returned CFArray is an `NSArray<NSDictionary<NSString, AnyObject>>`
(toll-free bridged), owned (`Retained::from_raw`, "Copy" rule).
xcap re-copies the whole list for **every property** of every window (~55 ms);
snapr reads it once itself (`macos::titled_windows()`), down to a few ms.
Window titles (`kCGWindowName`) are only populated with Screen Recording
permission.

### 3.10 Text rendering (b25a53e, fec23c2, 2b6f767)

- Soft text on 2x screens: disable egui subpixel binning on macOS, and do it
  **after every `set_visuals`** (which resets it) on every egui context
  (overlay, pin, toast, recording bar):
  `ctx.all_styles_mut(|s| s.visuals.text_options.subpixel_binning = false);`
- egui's Ubuntu-Light looks spindly on macOS (no Windows-style thickening) and
  lacks arrows. Load the system font, highest priority for Proportional:

```rust
let data = std::fs::read("/System/Library/Fonts/SFNS.ttf").ok()?;  // since 10.15
let mut font = egui::FontData::from_static(Vec::leak(data));       // cache in OnceLock
font.tweak.coords = egui::epaint::text::VariationCoords::new([(b"wght", 400.0), (b"opsz", 0.0)]);
ctx.add_font(egui::epaint::text::FontInsert::new("system", font,
    vec![egui::epaint::text::InsertFontFamily {
        family: egui::FontFamily::Proportional,
        priority: egui::epaint::text::FontPriority::Highest }]));
```

  SFNS.ttf is a variable font: set `wght` 400 (regular) and `opsz` minimum.
  Ubuntu stays as fallback.
- User font scanning dirs: `/System/Library/Fonts`, `/Library/Fonts`,
  `~/Library/Fonts` (`src/fonts.rs`).

### 3.11 Notch (4b8ed09)

Toolbar and hints on the overlay were hidden behind the camera notch. Offset
them by the screen's safe-area top inset (points; egui works in points):

`ns_window.screen()?.safeAreaInsets().top` after checking
`screen.respondsToSelector(sel!(safeAreaInsets))` (macOS 12+). Used as `.anchor(Align2::CENTER_TOP, vec2(0.0, 14.0 + top_inset))`.

### 3.12 Build breaks only on macOS (da9abcf, fa99eed)

- xcap's macOS `VideoRecorder` holds an `AVCaptureSession`, which objc2 does
  not mark `Send`; storing recorders in a `static Mutex<HashMap<..>>` failed to
  compile on macOS only. Fix: newtype + `unsafe impl Send for Recorder {}`
  (AVFoundation allows start/stop from any thread; used by one thread at a
  time).
- Platform-only modules referenced from shared UI code
  (`crate::portal::gnome()` is Linux-only) broke Windows and macOS builds.
  Wrap with `#[cfg(target_os = "linux")]` + a `#[cfg(not(...))]` fallback.
- Lesson: CI-check `aarch64-apple-darwin` on every PR, not only at release.

### 3.13 Menu bar item (tray) (235a329, b1bd6f9, src/tray/macos.rs)

- `NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength)`
  + an objc2 `define_class!` `MainThreadOnly` target for actions. Icon: render 36x36 RGBA -> PNG -> `NSImage::initWithData`,
  `setSize(18x18)` (2x), `setTemplate(true)` so macOS tints it for light/dark
  menu bars (custom colour styles are ignored on macOS).
- Left-click default: users expected it to open the app, not show the menu
  (235a329). Implementation: keep the status item **without** a menu,
  `button.sendActionOn(LeftMouseUp | RightMouseUp)`; in the action read
  `NSApplication::currentEvent()`; on `RightMouseUp` or Control-modifier
  temporarily `item.setMenu(Some(menu))`, `button.performClick(None)`
  (blocks until closed), then `item.setMenu(None)`. In "menu" mode just keep
  the menu set.
- Main thread only; `removeStatusItem` in `Drop`.

### 3.14 Keep own windows out of captures (e991c1b)

Toast/preview windows showed up in the next screenshot. macOS:
`ns_window.setSharingType(NSWindowSharingType::None)`; Windows:
`SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)`; Linux: close and
wait ~500 ms before capturing.

### 3.15 Misc macOS behaviours

- Single instance: `File::try_lock()` on `<config>/snapr.lock`; a second
  launch connects to `<config>/snapr.sock` (`UnixListener`) so the running one
  shows its window (e991c1b). With no Dock icon, relaunching from Finder is
  the user's way back.
- `NSSound::initWithData`, kept alive in a `thread_local!` (`src/sound.rs`);
  `open <path|url>` / `open -R <path>`; `arboard` `set().file_list()` for file
  copies (ed684f8); `keyring` -> Keychain; Ctrl-click = right-click.
- Partly covered window on click-capture: re-grab it alone with
  `xcap::Window::capture_image()`, resize to its frame if scales differ (PR #2).

---

## 4. Windows / Linux build pitfalls that also apply

### 4.1 `crt-static` (bf8fcce)

"VCRUNTIME140.dll was not found" on machines without the VC++ redistributable
(first seen on ARM, then x64). `.cargo/config.toml`:

```toml
[target.'cfg(all(windows, target_env = "msvc"))']
rustflags = ["-C", "target-feature=+crt-static"]
```

Use the cfg form, not `[target.aarch64-pc-windows-msvc]`, so x64 gets it too.

### 4.2 PDB size / disk (f63162f)

Full debug info produced ~300 MB `.pdb` per debug build, outgrew the linker's
PDB size limit and filled the disk. `Cargo.toml`:

```toml
[profile.dev]
debug = "line-tables-only"
[profile.release]
lto = "thin"
strip = true
```

Plus `scripts/sweep-target.ps1` to prune stale `target/` output (cargo never
deletes old `<name>-<hash>.exe/.pdb`).

### 4.3 Embedding the exe icon (db23843)

The runtime-drawn window/tray icon doesn't reach Explorer, Start menu
shortcuts or taskbar pins; those read the icon resource in the .exe. `build.rs`
(only when `CARGO_CFG_TARGET_OS == "windows"`, so cross-checks still work):
builds an `.ico` from the iconset PNGs (16, 32, 32@2x=64, 128, 256; PNG-in-ICO,
size byte 0 = 256), writes `1 ICON "path"` to an `.rc`, and compiles it with
`embed-resource = "3"`:
`embed_resource::compile(&rc, embed_resource::NONE).manifest_optional()`.
Failures are a `cargo:warning`, not a build failure. Declare
`cargo:rerun-if-changed=assets/<app>.iconset`.

### 4.4 Other

- Windows on ARM: cross-compile `aarch64-pc-windows-msvc` on `windows-latest`
  x64 (the image has MSVC ARM64 tools and clang, which `ring` needs).
- Linux release on `ubuntu-24.04` (PipeWire/libspa headers); the .deb then
  requires Ubuntu 24.04 / Debian 13+.
- A Linux-only module referenced unconditionally breaks the other two
  targets (fa99eed); CI-check all three targets on every PR.
- `build.rs` in snapr also generates the third-party licence list with
  `cargo metadata --filter-platform $TARGET` (offline first), so the About
  page lists exactly the crates for that platform.

---

## 5. Checklist for sharexrs

1. Pick the bundle id now and never change it; one self-signed code-signing
   cert, .p12 backed up, in `MACOS_SIGNING_P12` / `_PASSWORD`; sign the very
   first public release with it; keep the `cdhash` guard; tags fail without it.
2. `scripts/macos-bundle.sh` like snapr's: plist keys from 1.2 (`LSUIElement`,
   `NSHighResolutionCapable`, usage strings), `plutil -lint`, `iconutil`,
   `ditto` zip for the updater, dmg with `/Applications` symlink.
3. Decide arm64-only vs universal (`lipo`) before the first release.
4. CI: `workflow_dispatch` dry runs, tag-vs-Cargo.toml check, `--locked`,
   `fail-fast: false`, SHA256SUMS, `-` tags = pre-release. PR CI checks
   windows-msvc, aarch64-apple-darwin and linux-gnu (non-`Send` objc types,
   cfg leaks).
5. Updater: swap the whole `.app` (`ditto -x -k`), move the old one to temp,
   refuse under `/AppTranslocation/`, cache `current_exe()`, clean leftovers.
6. Permissions: `CGPreflightScreenCaptureAccess` before every capture,
   `CGRequestScreenCaptureAccess` once at launch, `AXIsProcessTrustedWithOptions`
   once per run on hotkey failure; card with deep links + Restart; macOS 27
   names; `>` not arrows; hotkey errors never block saving settings.
7. Activation: `Accessory` at launch (plist + winit builder), `Regular` +
   activate while the main window is open, back on close.
8. Overlay: no winit fullscreen on macOS; level 1000 + CanJoinAllSpaces |
   FullScreenAuxiliary | Stationary; animation None; alpha 0.01, fade after
   first present; pre-open transparent overlays before grabbing; restore
   `frontmostApplication()` after; notch via `safeAreaInsets().top`;
   `setSharingType(None)` on toasts/previews/recording UI.
9. Coordinates: macOS frames are points; create windows with the monitor's own
   scale; port `untangle`/`to_platform` + tests; own single-pass
   `CGWindowListCopyWindowInfo` with layer 0 / alpha > 0 / titled filters.
10. Text: `subpixel_binning = false` after every `set_visuals`; SFNS.ttf
    (wght 400, opsz min) as top Proportional font.
11. Windows/Linux: `+crt-static` for all msvc targets, `debug =
    "line-tables-only"`, `strip`, thin LTO, exe icon via `embed-resource` from
    the shared iconset, Linux CI on ubuntu-24.04 if PipeWire is used.
