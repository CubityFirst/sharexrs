#!/usr/bin/env bash
# Runs ShareX's ZipManager over the settings backup cases (tools/settings-harness),
# then ShareXrs's comparisons. Windows only.
set -euo pipefail
cd "$(dirname "$0")/../.."
root=$(cygpath -w "$PWD" 2>/dev/null || pwd)
fixtures="$root\crates\sharex-helpers\tests\fixtures\zip"

python -I tools/settings-harness/make_cases.py
# ShareXrs's export of the same entries, for ShareX to read.
SHAREXRS_WRITE_FIXTURES=1 cargo test -p sharex-helpers --test zip write_sharexrs_export
dotnet build tools/settings-harness -c Release -o target/settings-harness ${SHAREX_BUILD:+-p:ShareXBuild="$SHAREX_BUILD"}
rm -rf target/settings-harness-scratch
mkdir -p target/settings-harness-scratch

target/settings-harness/SettingsHarness.exe --extract \
  "$fixtures\cases.json" "$fixtures" \
  "$root\target\settings-harness-scratch\extract" \
  "$fixtures\results-sharex.json"
target/settings-harness/SettingsHarness.exe --compress \
  "$root\target\settings-harness-scratch\compress" \
  "$fixtures\export-sharex.json" \
  "$fixtures\sharexrs-export.sxb"

cargo test -p sharex-helpers --test zip
