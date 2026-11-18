#!/usr/bin/env bash
# Renders the scrolling capture and auto capture windows (`sharexrs
# --render`) and compares each with ShareX's shot of the same state
# (scripts/scrolling-session.ps1, uia.ps1 and winshot.ps1 took them into
# G:/scripts/sharexrs-ref/shots/sharex).
#
#   scripts/scrolling-compare.sh [screen-filter]
set -u
cd "$(dirname "$0")/.."
ref=G:/scripts/sharexrs-ref/shots/sharex
out="${TMPDIR:-${TEMP:-/tmp}}/scrolling-compare"
mkdir -p "$out"
exe=target/debug/sharexrs.exe
# name | render args | ShareX shot
cases=(
  "scrolling-cancelled|scrolling-capture --state cancelled|scrolling-cancelled-dark"
  "scrolling-cancelled-light|scrolling-capture --state cancelled --theme light|scrolling-cancelled-light"
  "scrolling-options|scrolling-capture --state options|scrolling-options-dark"
  "scrolling-options-light|scrolling-capture --state result-options --theme light|scrolling-options-light"
  "scrolling-result|scrolling-capture --state result|scrolling-result-dark"
  "scrolling-result-light|scrolling-capture --state result --theme light|scrolling-result-light"
  "auto-capture|auto-capture|auto-capture-dark"
  "auto-capture-light|auto-capture --theme light|auto-capture-light"
  "auto-capture-fullscreen|auto-capture --state fullscreen|auto-capture-fullscreen-dark"
  "auto-capture-fullscreen-light|auto-capture --state fullscreen --theme light|auto-capture-fullscreen-light"
  "auto-capture-running|auto-capture --state running|auto-capture-running-dark"
)
for c in "${cases[@]}"; do
  IFS='|' read -r name args shot <<<"$c"
  [[ -n "${1:-}" && "$name" != *"$1"* ]] && continue
  [[ -f "$ref/$shot.png" ]] || { echo "$name: no ShareX shot ($shot.png)"; continue; }
  # shellcheck disable=SC2086
  "$exe" --render $args --out "$out/$name.png" >/dev/null || { echo "$name: render failed"; continue; }
  printf '%-30s ' "$name"
  python scripts/compare.py "$ref/$shot.png" "$out/$name.png" "$out/$name" | sed -n 3p
done
