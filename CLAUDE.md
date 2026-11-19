# ShareXrs

A 1:1 port of ShareX 22.4 (C#/Avalonia) to Rust/egui. Read `docs/PORTING.md`
first: it has the rules, the architecture and the tracker.

- ShareX's source (the spec): `G:/scripts/sharexrs-ref/ShareX` (commit in
  `REFERENCE_COMMIT`). The running reference app:
  `G:/scripts/sharexrs-ref/ShareX-build/ShareX.exe`, built from that exact
  commit (`dotnet build ShareX/ShareX.csproj -c Release -p:Platform=x64 -o ../ShareX-build`)
  and portable (a `Portable` file beside it; its
  config is in that folder). Reference screenshots go in
  `G:/scripts/sharexrs-ref/shots/sharex/`.
- Never guess a size, colour or string: port it from ShareX's `.axaml`,
  theme (`sharex_ui::theme`), `.resx` (`sharex_strings::<project>::Key()`)
  and icons (`sharex_ui::keys::*`).
- Validate every screen against ShareX: `scripts/winshot.ps1`,
  `sharexrs --render`, `scripts/compare.py`. Only text anti-aliasing should
  differ.
- Use `widgets::line_height` / `center_y` / `text_width` for text layout,
  never raw galley sizes (Avalonia rounds line boxes up; egui rounds down).
- macOS packaging and runtime: follow `docs/MACOS-LEARNINGS.md`.
- Licence: GPL-3.0-or-later (it's a port of ShareX).
- Build: `cargo build`; render a screen: `target/debug/sharexrs --render main --out shots/x.png`.
- Before committing, also lint macOS from Windows:
  `powershell -File scripts/clippy-macos.ps1`, i.e.
  `cargo clippy --workspace --all-targets --target aarch64-apple-darwin -- -D warnings`
  with a stub C compiler for libwebp-sys's build script (no macOS SDK here;
  clippy doesn't link). `rustup target add aarch64-apple-darwin` once. Linux can't be checked on
  Windows (system libraries); CI covers it, so check CI after every push.
- Tests that compare against ShareX's Windows screenshots or depend on
  Windows fonts, folders or environment (Segoe UI layout, `Documents`,
  case-insensitive env vars) must be `#[cfg(windows)]` or skip themselves
  elsewhere: CI runs every test on macOS and Linux too.
