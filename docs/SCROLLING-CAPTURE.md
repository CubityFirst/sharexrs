# Scrolling capture and auto capture

Capture › Scrolling capture and Capture › Auto capture, ported from
`ShareX.ScreenCaptureLib` (`ScrollingCaptureManager`, `ScrollingCaptureOptions`,
`ScrollingCaptureService`, `Presentation/ScrollingCapture`) and
`ShareX/Presentation/AutoCapture` (with `TaskHelpers.OpenScrollingCapture`,
`OpenAutoCapture`, `StartAutoCapture`, `StopAutoCapture` and the
`ApplicationConfig.AutoCapture*` settings).

## Map

| Where | ShareX source | What's there |
|---|---|---|
| `sharex_capture::scrolling` | `ScrollingCaptureOptions`, `ScrollingCaptureManager`, `Enums.cs` (`ScrollMethod`, `ScrollingCaptureStatus`) | the options in ShareX's JSON; `Stitcher` (`CombineImages`: the side and bottom edge offsets, the auto ignored bottom edge, the match search, best guesses, the status); `compare_images` (`SkiaImageHelpers.CompareImages`); `run_capture` (`StartCapture`'s loop: activate, start delay, scroll to top, grab, two frames alike stop it, scroll, stitch, scroll delay less the stitching time) over a `ScrollTarget`; `ScreenTarget` (the screen without the pointer, `SendInput` wheel and keys by virtual key as `InputManager`, `WM_VSCROLL`) |
| `sharex-capture` `platform` | `InputHelpers`, `NativeMethods.SendMessage` | `send_mouse_wheel`, `send_key_press`, `send_vscroll` (Windows; no-ops elsewhere) |
| `crates/sharexrs/src/scrolling_capture/window.rs` | `ScrollingCaptureWindow.axaml(.cs)` | the docked toolbar (Capture..., Upload / Save, Copy, Options..., Help), the preview (`Stretch="None"`, centred, panned by dragging, the wheel; results taller than a texture drawn in 2048 px tiles), the empty state, the status row (Ready, Capturing, Stopping, Selection cancelled, the three results with ShareX's colours, the size), the options overlay (delays, method, amount hidden for Page down, the four check boxes, Cancel/OK) |
| `crates/sharexrs/src/scrolling_capture/mod.rs` | `ScrollingCaptureWindowIntegration`, `ScrollingCaptureService`, `SelectWindowAsync`, `ScrollingCaptureRegionWindow` | one window; opening it minimizes it and selects at once, the hotkey or Capture... again starts or stops; 250 ms after minimizing, the select-only region overlay with the default region options but the task's active monitor mode; a window picked (no window: "selection cancelled"); the loop on a thread with the lime dashed outline (click-through, topmost) around the region; activating the window or closing it stops the capture (closing waits for it); the result back to the window, the action sound, `AutoUpload`; the options saved into the task's (or default task's) `ScrollingCaptureOptions` |
| `crates/sharexrs/src/auto_capture/timer.rs` | `AutoCaptureWindow`'s timers and `UpdateStatus` | the screenshot timer (1 s to the first capture, then the repeat time, 1 s retries while tasks are busy with Wait until tasks complete), the count kept across restarts, the decimal repeat time to whole ms, `(timeLeft / 1000f).ToString("0.0")` as .NET formats a float, the percentage, the status and region texts |
| `crates/sharexrs/src/auto_capture/window.rs` | `AutoCaptureWindow.axaml(.cs)` | Custom region / Fullscreen (the screen bounds, the custom region kept), the region text, Select region..., the repeat time, the two check boxes bound to `ApplicationConfig`, Start/Stop (disabled with no region), the status bar and its progress |
| `crates/sharexrs/src/auto_capture/mod.rs` | `AutoCaptureWindowIntegration`, `TakeScreenshot`, `HideToTray`/`RestoreFromTray` | one window (Show, Start unless running, Stop if running); Select region... through the select-only overlay with the task's region options; each capture of `AutoCaptureRegion` (`GetScreenshot`: the pointer per `ShowCursor`) through `RunImageTask` with Annotate and Photo editor removed and the sounds and toast off; minimize to tray on Start and when minimized, a clock tray icon that brings it back |
| `main.rs`, `dialogs/mod.rs` | `MainMenuBuilder`, `TaskHelpers.ExecuteJob` | Capture › Scrolling capture... and Auto capture... (default task settings), the `ScrollingCapture`, `AutoCapture`, `StartAutoCapture`, `StopAutoCapture` jobs (hotkeys, command line); dialogs gained `close_requested` and `activated` hooks |
| `test_pattern.rs` | | `--test-pattern --scroll X,Y,W,H`: a 2400 px generated page in a scrollable window (wheel 40 px a notch, arrows 20 px, Page Up/Down, Home/End) over the pattern; the pattern windows draw without egui's dithering |

## Validation

### Engines, headless

`tools/scrolling-harness` (README there) runs ShareX's own
`ScrollingCaptureManager.CombineImages` (reflection on the reference
build's `ShareX.ScreenCaptureLib.dll`) in `StartCapture`'s loop order over
30 synthetic frame sequences (`make_cases.py`: a generated page seen at
scroll offsets, with sticky headers and footers, a moving scroll bar, a
whole frame inverted for the best guess, an animated block, width and
height changes, scrolling up, blank/repeating/noise pages, 40x30 to
400x600 frames). `cargo test -p sharex-capture --test scrolling`:
**30/30 identical** (status, frames grabbed, scroll steps, every step's
outcome and size, the final image pixel for pixel). Outcomes cover
Successful (20), PartiallySuccessful (3) and Failed (7, the result so
far kept).

Its `--auto` mode evaluates the auto capture window's formulas on .NET with
ShareX.dll's strings; `cargo test -p sharexrs auto_capture`: identical on
22 002 second texts (0–20 s by the millisecond and samples to 24 h; .NET
rounds a float's 7 digits half up, so a naive `{:.1}` would differ),
1 407 percentages, 12 decimal repeat times (`4.35` is 4350 ms, not 4349),
the status and region texts.

### Windows

ShareX's windows were captured live (`scripts/scrolling-session.ps1`,
`uia.ps1`, `winshot.ps1`); `scripts/scrolling-compare.sh` renders and
compares. Pixels differing by more than 40:

| State | Dark | Light |
|---|---|---|
| Scrolling capture, selection cancelled | 0.12% | 0.09% |
| Scrolling capture, options overlay | 0.25% | 0.25% (over a result) |
| Scrolling capture, a result (420x2400) | 0.12% | 0.11% |
| Auto capture, no region | 0.60% | 0.73% |
| Auto capture, Fullscreen | 0.60% | 0.74% |
| Auto capture, running (56.5 s, 5%, total 1) | 0.57% | |

What's left is text anti-aliasing (ClearType against grayscale). Measured
and kept: the auto capture window's radio row is 33 px and its repeat time
box 121x41 one pixel right of its column (Avalonia's rounding there). The
live ShareXrs scrolling window with a result is 0.12% from ShareX's live
window.

### Behaviour, side by side (over `--test-pattern --scroll 300,120,420,480`)

- **Scrolling capture**, default options, the page's window clicked in each
  app's overlay: both "Capture successful", 420x2400; the copied images are
  pixel-identical PNGs and byte-identical CF_DIBs, and equal the generated
  page exactly. Grabbed 3 s into the capture, the two screens differ in
  0.05% of pixels, all on the region outline, by at most 6 levels (both
  apps had scrolled the page to the same place).
- **Auto capture**, the same steps in both (Select region... dragged
  750,20 → 1010,160, minimize to tray off, repeat time typed as 2, Start,
  7.6 s, Stop): the same region (X: 750, Y: 20, Width: 261, Height: 141),
  4 captures each, the first ~1.4 s after the Start click, then every
  2.00 s in ShareXrs and 2.02–2.03 s in ShareX (its `DispatcherTimer`
  restarts after the tick's work). The images are identical but for a
  50x91 corner where each app's own window (raised above the pattern)
  casts its shadow into the region.

## Not ported or not checked

- Scrolling: `IsScrollReachedBottom` (unused by ShareX), the Help button
  opens ShareX's docs page; the methods other than the mouse wheel weren't
  run side by side (the test page handles arrows and Page Down; it has no
  native scroll bar for `WM_VSCROLL`); stopping by activating the window
  and closing mid-capture were checked in code, not live; the options
  JSON after edits wasn't compared (ShareX saves it later).
- Input to other apps (wheel, keys, `WM_VSCROLL`) and activating the
  target window are Windows only: on macOS and Linux the loop grabs and
  stitches but doesn't scroll (untested there).
- Auto capture: the tray icon is checked only for showing and restoring
  in code (no live click on it); minimizing the window to the tray goes
  through winit's minimized state; macOS/Linux untested.
- Fluent's scroll bars are the shared thin indicator; the preview has no
  horizontal bar (dragging and Shift+wheel pan).
- One 1024x768 monitor at 100% only.
