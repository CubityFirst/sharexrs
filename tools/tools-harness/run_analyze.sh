#!/usr/bin/env bash
# Analyze image: ShareX's AnalyzeImageService and ShareXrs's analyze_image
# against the same mock server, then the comparison.
#   tools/tools-harness/run_analyze.sh [port]
# Needs: python, dotnet (SDK 11), cargo, and the reference ShareX build.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
port="${1:-18767}"
out="$root/target/tools-harness-analyze"
bin="${HARNESS_BIN:-G:/scripts/sharexrs-ref/tools-harness/bin}"
mkdir -p "$out"

python "$here/make_analyze_cases.py" "$out/analyze_cases.json"
dotnet build "$here" -c Release -o "$bin" -nologo -v q
cargo build -q --release -p sharex-tools --example tools_analyze_harness

python "$root/tools/uploaders-harness/mock_server.py" "$port" &
mock=$!
trap 'kill $mock 2>/dev/null || true' EXIT
sleep 1

"$bin/ToolsHarness" --analyze "$out/analyze_cases.json" "$out/results-sharex.json" "$port" "$out/work" > "$out/sharex.log"
"$root/target/release/examples/tools_analyze_harness" "$out/analyze_cases.json" "$out/results-sharexrs.json" "$port" "$out/work" > "$out/sharexrs.log"
"$root/target/release/examples/tools_analyze_harness" --compare "$out/analyze_cases.json" "$out/results-sharex.json" "$out/results-sharexrs.json" --report "$out/report.md"
