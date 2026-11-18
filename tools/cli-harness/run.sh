#!/usr/bin/env bash
# Runs ShareX's CLIManager, single instance message and native messaging input
# over the cases (tools/cli-harness), then ShareXrs's comparisons. Windows only.
set -euo pipefail
cd "$(dirname "$0")/../.."
root=$(cygpath -w "$PWD" 2>/dev/null || pwd)

python -I tools/cli-harness/make_cases.py
dotnet build tools/cli-harness -c Release -o target/cli-harness ${SHAREX_BUILD:+-p:ShareXBuild="$SHAREX_BUILD"}
mkdir -p target/cli-harness-scratch

target/cli-harness/CliHarness.exe --cli \
  "$root\crates\sharex-helpers\tests\fixtures\cli\cases.json" \
  "$root\crates\sharex-helpers\tests\fixtures\cli\results-sharex.json"
target/cli-harness/CliHarness.exe --native \
  "$root\crates\sharexrs-native-messaging\tests\fixtures\cases.json" \
  "$root\target\cli-harness-scratch" \
  "$root\crates\sharexrs-native-messaging\tests\fixtures\results-sharex.json"

cargo test -p sharex-helpers --test cli
cargo test -p sharexrs-native-messaging --test sharex
