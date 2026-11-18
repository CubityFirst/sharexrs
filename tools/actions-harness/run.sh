#!/usr/bin/env bash
# Runs ShareX's actions and watch folders over the cases (tools/actions-harness),
# then ShareXrs's comparisons. Windows only.
set -euo pipefail
cd "$(dirname "$0")/../.."
root=$(cygpath -w "$PWD" 2>/dev/null || pwd)

cargo build -p sharex-settings --example sharex_settings_action_probe
dotnet build tools/actions-harness -c Release -o target/actions-harness ${SHAREX_BUILD:+-p:ShareXBuild="$SHAREX_BUILD"}
mkdir -p target/actions-harness-scratch

target/actions-harness/ActionsHarness.exe --actions \
  "$root\\crates\\sharex-settings\\tests\\fixtures\\actions\\cases.json" \
  "$root\\target\\debug\\examples\\sharex_settings_action_probe.exe" \
  "$root\\target\\actions-harness-scratch" \
  "$root\\crates\\sharex-settings\\tests\\fixtures\\actions\\results-sharex.json"
target/actions-harness/ActionsHarness.exe --watch \
  "$root\\crates\\sharexrs\\src\\watch_folder\\fixtures\\cases.json" \
  "$root\\target\\actions-harness-scratch" \
  "$root\\crates\\sharexrs\\src\\watch_folder\\fixtures\\results-sharex.json"

cargo test -p sharex-settings --test actions
cargo test -p sharexrs watch_cases_as_sharex
