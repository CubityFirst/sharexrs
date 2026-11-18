# Scrolling capture and auto capture harness

Checks ShareXrs's scrolling capture engine (`sharex_capture::scrolling`) and
the auto capture window's formulas (`crates/sharexrs/src/auto_capture`)
against ShareX's own code from the reference build. No window opens, nothing
is captured and no input is sent: the frames are generated.

```sh
tools/scrolling-harness/run.sh
```

Needs the .NET 11 SDK, Python, cargo, and the reference build in
`G:/scripts/sharexrs-ref/ShareX-build` (`SHAREX_BUILD` to change it). Windows
only (ShareX's build is).

- `make_cases.py` writes `crates/sharex-capture/tests/fixtures/scrolling`:
  30 cases, each a folder of PNG frames cut from a generated page (word
  blocks, a blank page, a repeating pattern, noise) at a list of scroll
  offsets, with sticky headers and footers (fixed or changing), a moving
  scroll bar, a frame inverted whole (no row matches: ShareX's best guess),
  an animated block, frames that change width or height, scrolling up, steps
  of 7 to 200 px and frames from 40x30 to 400x600.
- `--scrolling <folder>`: each case through `ScrollingCaptureManager`'s
  private `CombineImages` and state (reflection) in the order and with the
  checks of `StartCapture`'s loop (`SkiaImageHelpers.CompareImages` for two
  frames in a row, stop once the frames run out). Writes
  `<case>/result-sharex.png` and `results-sharex.json` (status, frames
  grabbed, scroll steps, the result's size and every step's outcome).
  `cargo test -p sharex-capture --test scrolling` runs ShareXrs's loop
  (`run_capture`) and `Stitcher` over the same frames and compares.
- `--auto <out.json>`: `AutoCaptureWindow`'s `(timeLeft / 1000f).ToString("0.0")`
  (0 to 20 s by the millisecond and samples to 24 h), the percentage, the
  repeat time's `(int)(decimal * 1000)`, and `Strings.AutoCaptureWindow_Status`
  / `_Region` formatted, with en-US conventions. `cargo test -p sharexrs
  auto_capture` compares.

Last run: 30/30 scrolling cases identical (status, frames, scrolls, every
step, final image pixel for pixel); auto capture formulas identical (22 001
second texts, 1 407 percentages, 12 repeat times, the status and region
texts).
