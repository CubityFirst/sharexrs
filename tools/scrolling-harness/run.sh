#!/usr/bin/env bash
# Writes the synthetic scrolling capture frames, runs ShareX's own
# ScrollingCaptureManager over them and the auto capture window's formulas
# (tools/scrolling-harness), then ShareXrs's comparisons. Windows only.
set -euo pipefail
cd "$(dirname "$0")/../.."
root=$(cygpath -w "$PWD" 2>/dev/null || pwd)

python -I tools/scrolling-harness/make_cases.py
dotnet build tools/scrolling-harness -c Release -o target/scrolling-harness ${SHAREX_BUILD:+-p:ShareXBuild="$SHAREX_BUILD"}

target/scrolling-harness/ScrollingHarness.exe --scrolling "$root\crates\sharex-capture\tests\fixtures\scrolling"
target/scrolling-harness/ScrollingHarness.exe --auto "$root\crates\sharexrs\tests\fixtures\auto_capture\results-sharex.json"

cargo test -p sharex-capture --test scrolling
cargo test -p sharexrs auto_capture
