#!/usr/bin/env bash
# Runs ShareX's and ShareXrs's update check code against the mock server and
# compares them.
#   tools/update-harness/run.sh [port]
# Needs: python, dotnet (SDK 11), cargo, and the reference ShareX build
# (SHAREX_BUILD, default G:/scripts/sharexrs-ref/ShareX-build).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
port="${1:-18766}"
out="$root/target/update-harness"
bin="${HARNESS_BIN:-G:/scripts/sharexrs-ref/update-harness/bin}"
mkdir -p "$out"

python "$here/make_cases.py" "$here/cases.json"
dotnet build "$here/dotnet" -o "$bin" -nologo -v q
cargo build -q -p sharex-update --example update_harness

python "$root/tools/uploaders-harness/mock_server.py" "$port" &
mock=$!
trap 'kill $mock 2>/dev/null || true' EXIT
sleep 1

"$bin/UpdateHarness" "$here/cases.json" "$out/results-sharex.json" "$port" "$out/work" > "$out/sharex.log"
"$root/target/debug/examples/update_harness" "$here/cases.json" "$out/results-sharexrs.json" "$port" "$out/work" > "$out/sharexrs.log"
python "$here/compare.py" "$here/cases.json" "$out/results-sharex.json" "$out/results-sharexrs.json" --report "$out/report.md"
