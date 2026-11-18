#!/usr/bin/env bash
# Renders the Screen tools' windows (`sharexrs --render`) and compares each
# with ShareX's shot of the same state (scripts/screen-tools-session.ps1 and
# winshot.ps1 took them into G:/scripts/sharexrs-ref/shots/sharex).
#
#   scripts/screen-tools-compare.sh [screen-filter]
set -u
cd "$(dirname "$0")/.."
ref=G:/scripts/sharexrs-ref/shots/sharex
out="${TMPDIR:-${TEMP:-/tmp}}/screen-tools-compare"
mkdir -p "$out"
exe=target/debug/sharexrs.exe
# name | render args | ShareX shot
cases=(
  "color-picker|color-picker|color-picker-dark"
  "color-picker-standard|color-picker --state standard|color-picker-standard-dark"
  "color-picker-swatch|color-picker --state swatch|color-picker-swatch-dark"
  "color-picker-standard-light|color-picker --state standard --theme light|color-picker-light"
  "pin-startup|pin-startup|pin-startup-dark"
  "pin-startup-error|pin-startup --state error|pin-startup-error-dark"
  "pin-startup-light|pin-startup --theme light|pin-startup-light"
  "pin-options|pin-options|pin-options-dark"
  "mouse-highlighter|mouse-highlighter|mouse-highlighter-dark"
  "mouse-highlighter-circle|mouse-highlighter --state circle|mouse-highlighter-circle-dark"
  "mouse-highlighter-circle-light|mouse-highlighter --state circle --theme light|mouse-highlighter-circle-light"
)
for c in "${cases[@]}"; do
  IFS='|' read -r name args shot <<<"$c"
  [[ -n "${1:-}" && "$name" != *"$1"* ]] && continue
  [[ -f "$ref/$shot.png" ]] || { echo "$name: no ShareX shot ($shot.png)"; continue; }
  # shellcheck disable=SC2086
  "$exe" --render $args --out "$out/$name.png" >/dev/null || { echo "$name: render failed"; continue; }
  printf '%-28s ' "$name"
  python scripts/compare.py "$ref/$shot.png" "$out/$name.png" "$out/$name" | sed -n 3p
done
