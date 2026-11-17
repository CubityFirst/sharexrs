#!/usr/bin/env bash
# Runs both harnesses against the mock server and compares them.
#   tools/uploaders-harness/run.sh [port]
# Needs: python, dotnet (SDK 11), cargo, and the reference ShareX build
# (SHAREX_BUILD, default G:/scripts/sharexrs-ref/ShareX-build).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
port="${1:-18765}"
out="$root/target/uploaders-harness"
bin="${HARNESS_BIN:-G:/scripts/sharexrs-ref/uploaders-harness/bin}"
mkdir -p "$out"

python "$here/make_cases.py"
dotnet build "$here/dotnet" -o "$bin" -nologo -v q
cargo build -q -p sharex-uploaders --example uploaders_harness

python "$here/mock_server.py" "$port" &
mock=$!
trap 'kill $mock 2>/dev/null || true' EXIT
sleep 1

"$bin/UploadersHarness" "$here/cases.json" "$out/results-sharex.json" "$port" --fixture "$out/UploadersConfig.harness.json" > "$out/sharex.log"
"$root/target/debug/examples/uploaders_harness" "$here/cases.json" "$out/results-sharexrs.json" "$port" > "$out/sharexrs.log"
python "$here/compare.py" "$out/results-sharex.json" "$out/results-sharexrs.json" --report "$out/report.md"
