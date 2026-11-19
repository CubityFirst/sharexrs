# ShareX outside Windows, and ShareXrs

ShareX 22.4 is an Avalonia app, so its windows, menus, dialogs, pickers,
clipboard and tray icon go through Avalonia on every platform. Its own
platform code is almost all Win32: P/Invoke into `user32`, `gdi32`,
`dwmapi`, `shell32`, the registry and COM. Outside Windows those calls
throw (`DllNotFoundException`, `PlatformNotSupportedException`) or are
skipped behind `OperatingSystem.IsWindows()`.

Inventory (2026-11-19, commit in `REFERENCE_COMMIT`): every
`OperatingSystem.Is*` and `RuntimeInformation` check, every service
interface (`IDesktopWallpaperService`, `IClipboardService`,
`ITrayIconService`, `IHotkeyHost`) and each feature's native calls.
ShareX has no `#if` for macOS or Linux, no Wayland, X11, portal, D-Bus,
`osascript` (except the wallpaper), LaunchAgent or autostart code. **Its
only macOS- and Linux-specific code is the desktop wallpaper services.**
Everything else it does there is what Avalonia and .NET do on that
platform.

The rule for ShareXrs: port what ShareX does on each platform, the same
tools, commands and fallbacks; where ShareX has nothing, ShareXrs has
nothing either, noted below.

## The table

Status: ✅ ported as ShareX does it (unit-tested; untested on a real
desktop unless the live checks below say so) · ➖ ShareX has nothing, nor
does ShareXrs · ➕ ShareXrs has more than ShareX (from before this
inventory; kept, decision pending, see "Beyond ShareX") · ⬜ gap left.

| Feature | ShareX on macOS | ShareX on Linux | ShareXrs |
|---|---|---|---|
| Desktop wallpaper (editor background) | `MacOSDesktopWallpaperService`: `osascript` asks System Events for the picture's POSIX path; layout from `defaults read com.apple.spaces` (current space's uuid) and `defaults read com.apple.desktop Background` (that space's `default = {` block: `Placement`, else `ImageScaling`), else Fill | `LinuxDesktopWallpaperService`: the detected desktop first (`XDG_CURRENT_DESKTOP`, `KDE_FULL_SESSION`, `DESKTOP_SESSION`), then GNOME, Cinnamon, MATE (`gsettings`, the dark picture with `prefer-dark`), Xfce (`xfconf-query`, workspace 0 first), KDE (`plasma-org.kde.plasma.desktop-appletsrc`), LXQt/LXDE (pcmanfm settings); Flatpak host mirrors (`/run/host`); JPEG XL converted by ffmpeg, glycin or gdk-pixbuf into a temp cache; prewarmed when the editor opens | ✅ `sharex_image_editor::integration::wallpaper` (the macOS and Linux services, behind a `Host` trait), wired into the background panel and the editor's prewarm. The macOS bundle has `NSAppleEventsUsageDescription` so the System Events query can be allowed. Note: `defaults` pads `default =         {`, which ShareX's `StartsWith("default = {")` misses, so ShareX (and the port) likely always reports Fill on real output; kept as ShareX has it, live check below |
| Clipboard: copy image | Avalonia: a bitmap plus `image/png` bytes (no DIB); `text/html` only with `UseAlternativeClipboardCopyImage` | same (X11 targets) | ✅ arboard: `NSImage` on macOS, `image/png` on Linux. ⬜ the extra raw `image/png` item on macOS and the HTML (not ported on Windows either) |
| Clipboard: copy files | Avalonia `SetFilesAsync`: file URLs | `text/uri-list` | ✅ arboard `file_list` (was an error before) |
| Clipboard: read image, text, files | Avalonia: `image/png` first, then the bitmap | same | ✅ arboard |
| Clipboard viewer (formats list) | every format Avalonia reports, by its identifier, with raw values | same | ⬜ text, files and image only (listing every pasteboard type / X11 target would need Avalonia's exact identifiers) |
| Tray icon | Avalonia `TrayIcon` with a `NativeMenu` built from the tray menu; left click only; double-click time from the platform (500 ms default) | same (StatusNotifierItem through Avalonia's D-Bus tray) | ✅ menu bar item / ksni, native menu (`tray/`). ✅ glyph and progress icons 16 px and the glyph colour from the OS's light/dark mode (`TrayIconImage`, `GetProgressIconBytes`); was the tray's own size and always white |
| Notifications (toast) | ShareX's own Avalonia window | same | ✅ the same egui window on every OS |
| Opening files, folders, URLs | `Process.Start` with `UseShellExecute`: `/usr/bin/open` | the first of `xdg-open`, `gnome-open`, `kfmclient` on `PATH` | ✅ `open`; ✅ the three-program search on Linux (was `xdg-open` only). ShareX's `OpenFolder` appends `\` to the folder, which breaks it on Unix; not ported (ShareXrs opens the folder) |
| Open folder with the file selected | `SHOpenFolderAndSelectItems` throws: logged, nothing opens | same | ➕ `open -R` on macOS, the folder on Linux |
| Delete to the recycle bin | `FileSystem.DeleteFile(..., SendToRecycleBin)` throws: logged, the file stays | same | ✅ fails and keeps the file (was deleted outright) |
| File and folder pickers | Avalonia `StorageProvider`: NSOpenPanel | the portal / GTK dialog | ✅ rfd everywhere; ✅ the folder picker (was a stub) |
| Screen colour picker | `ScreenColorPickerWindow.OnLoaded` completes with no colour (no `GetCursorPos`) | same | ✅ completes with no colour (the overlay used to open) |
| Network monitor pings | .NET `Ping`: ICMP over a raw socket or an unprivileged ICMP datagram socket (always allowed on macOS); else `/sbin/ping -c 1 -W <ms> -s 32`, `LC_ALL=C`, `time=` rounded half to even | same, datagram sockets within `net.ipv4.ping_group_range`; else `ping -c 1 -W <s, rounded up> -s 32`; exit 1/2 is a time-out | ✅ `sharex_tools::system::unix_ping` (was always the system `ping` with other arguments) |
| Global hotkeys | `WindowsHotkeyHost` throws `PlatformNotSupportedException`: none | same | ➕ livesplit-hotkey |
| Single instance | the named `Mutex` works (.NET's shared-memory mutex); the pipe server needs `PipeSecurity`/`WindowsIdentity` and throws, so a second launch waits 5 s for the pipe and exits, its arguments lost | same | ➕ a lock file and a Unix socket that deliver the arguments |
| Run at startup | `ShortcutHelpers` (a `.lnk` in the Startup folder, which is empty on Unix) fails with an error | same | ➕ LaunchAgent `com.sharexrs.app.plist` / XDG autostart entry |
| File associations, context menu, Send To | registry only | same | ➖ no-ops |
| Screen capture (region, fullscreen, window) | GDI `BitBlt` and `user32`: nothing captures | same | ➕ xcap (ScreenCaptureKit / X11; Wayland through xcap's own paths) |
| Window lists, window under the pointer (region capture, Capture › Window) | `EnumWindows` etc.: none | same | ➕ CGWindowList / xcap |
| Window activation (after capture, scrolling capture, active window) | `SetForegroundWindow`: none | same | ➖ none |
| Inspect window, borderless window, top most / opacity | Win32 only | same | ➖ no-ops (the inspect window shows title and bounds from the window list) |
| Scrolling capture's scrolling | `SendMessage(WM_VSCROLL)`, `SendInput` wheel, `GetScrollInfo`: none | same | ➖ none |
| Mouse highlighter | `WH_MOUSE_LL` hook and layered overlays (`WindowsNativeWindow` throws): stops with an error | same | ➖ stops with an error |
| Transparent region capture overlay | Avalonia transparent window (the tool-window style and DWM transitions are Windows only) | same | ✅ a transparent egui window per monitor (`transparent_region.rs`) |
| Lock cursor to the region window, DPI anchoring, window geometry log, `EnsureWindowPosition` | Windows only | same | ➖ Windows only |
| Display diagnostics | `CollectAvaloniaOnly` | same | ✅ the Avalonia-only report |
| Printing | throws "A printing backend is required for this platform." | same | ✅ the same error |
| Sounds | `SoundPlayer` (Windows only): silent | same | ✅ silent |
| Video player (video tools) | `WindowsMediaPlayer` (Media Foundation in an HWND): no video | same | ➕ FFmpeg + cpal playback |
| Screen recording | FFmpeg with `gdigrab`/`ddagrab` and DirectShow: none | same | ➕ FFmpeg `avfoundation` / `x11grab`, PulseAudio/ALSA (docs/SCREEN-RECORDING.md) |
| Image editor: system cursors, emoji | bundled cursors only; emoji through Skia text (the Windows emoji renderer and cursor-screen lookup are Windows only) | same | ✅ bundled cursors only |
| Wayland screen capture through the portal | none (Avalonia 11 runs on XWayland; ShareX has no portal code) | none | ➖ none (snapr's portal code isn't ported) |
| Update check and install | GitHub check everywhere; the installer flow is Windows only | same | ➕ macOS swap and Linux flows (docs/UPDATE.md) |

## Beyond ShareX

The ➕ rows predate this inventory: ShareXrs set out to run on macOS and
Linux (PORTING.md's first paragraph), and these are what make it usable
there, even though ShareX itself does nothing in those places. They're
kept, not removed, pending a decision; removing them would leave ShareXrs
without capture, hotkeys or recording outside Windows, as ShareX is.

## New dependencies and CI

- `sha2` 0.10 in `sharex-image-editor` (the wallpaper cache's file names;
  already in the workspace).
- `socket2` 0.6 with `all` in `sharex-tools`, Unix only (raw and datagram
  ICMP sockets; already in the lock file through other crates).
- No system packages: `ci.yml` and `package.yml` are unchanged.

## Live checks

Nobody has run these on a real Mac or Linux desktop yet. Each says what
ShareX's code does there, so a difference is a ShareXrs bug.

### macOS

1. **Wallpaper.** Open the image editor on a capture, open the
   background panel: the mode list has Wallpaper. Choose it: the frame
   shows the desktop picture. The first time, macOS asks to let ShareXrs
   control System Events; allow it (System Settings › Privacy & Security ›
   Automation). Denied: no Wallpaper option until allowed. Note the
   layout chosen in System Settings › Wallpaper (Fill, Fit, Stretch,
   Center, Tile) and what the frame shows (expected: Fill whatever is
   set, see the table; report it either way, with the output of
   `defaults read com.apple.desktop Background`).
2. **Copy files.** History or the task list › Copy › File: paste in
   Finder copies the file.
3. **Recycle bin.** Delete a task's file with "Yes" to the recycle bin
   (or the toast's delete): the file stays and the debug log says why.
4. **Folder picker.** Application settings › Paths › Browse: the system
   folder panel opens and its answer is used.
5. **Tray glyphs.** Start a screen recording and Auto capture: their menu
   bar icons are 16 px glyphs, black in light mode and white in dark
   mode; switching mode updates new icons. Upload progress shows the
   16 px progress icon.
6. **Screen colour picker.** Tools › Screen color picker: nothing opens,
   nothing is copied.
7. **Network monitor.** Tools › Network monitor: probes succeed with
   latencies close to `ping -c 1 1.1.1.1`'s (datagram ICMP, no `ping`
   process: check with `ps` while it probes). Unplug the network: "timed
   out" for each endpoint.

### Linux

1. **Wallpaper,** on each desktop you have (GNOME, KDE, Xfce, Cinnamon,
   MATE, LXQt, LXDE): Wallpaper is offered, shows the current picture,
   and the layout matches the desktop's setting (GNOME "zoom" is Fill,
   "spanned" Span, KDE FillMode, Xfce image-style). GNOME in dark style
   shows `picture-uri-dark`. A `.jxl` wallpaper appears after a moment
   (converted into `$TMPDIR/sharex-imageeditor-wallpaper-cache`). In a
   Flatpak build, a wallpaper under `/usr/share` is read through
   `/run/host`. A desktop with none of these (sway): no Wallpaper option.
2. **Opening.** Open a file, a folder and a URL from the task list and
   History: they open with `xdg-open`; with `xdg-open` removed from
   `PATH` and `gnome-open` present, with that.
3. **Copy files:** pasting in Files/Dolphin copies the file
   (`text/uri-list`); `xclip -selection clipboard -t TARGETS -o` lists it.
4. **Recycle bin, folder picker (the portal's), tray glyphs, screen
   colour picker:** as macOS 3–6 (the tray is the StatusNotifierItem).
5. **Network monitor.** With `sysctl net.ipv4.ping_group_range` covering
   your group (most distributions): probes succeed without a `ping`
   process. With `sudo sysctl net.ipv4.ping_group_range="1 0"`: probes
   run `/usr/bin/ping -c 1 -W 1 -s 32 <ip>` (see `ps`) and still succeed;
   restore the range afterwards.
