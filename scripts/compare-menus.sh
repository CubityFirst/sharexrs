#!/usr/bin/env bash
# Renders every main-window menu state and compares it with ShareX's
# screenshot of the same state (G:/scripts/sharexrs-ref/shots/sharex/*.png,
# captured with scripts/screenshot-region.ps1, since Avalonia opens menus as
# their own popup windows).
#
#   scripts/compare-menus.sh [shots dir]
#
# The ShareX shots were taken with its window's client area at 62,5 on a
# 1024x768 screen whose work area ends at y=720, which is where Avalonia
# slides tall menus to: --work-area gives the renderer the same bounds, in
# window pixels. Hotkeys 0 and 3 failed to register in that ShareX.
#
# BIN=<sharexrs> uses that build instead of building this one (a baseline);
# MENU_ARGS adds render arguments to every state, e.g. MENU_ARGS="--popup-host
# -62,-5,962,715 --client-origin 62,5" places the menus as the live window
# does, each level on the screen (the image stays the window's), and prints
# where each level went.
set -euo pipefail
cd "$(dirname "$0")/.."
R=${1:-../sharexrs-ref/shots}
WA="-62,-5,962,715"
if [ -z "${BIN:-}" ]; then
  cargo build -q
  # `.exe` where there is one: a portable profile (`target/debug/ShareXrs/`)
  # shadows the bare name on case-insensitive file systems.
  BIN=target/debug/sharexrs
  [ -f "$BIN.exe" ] && BIN="$BIN.exe"
fi
# shellcheck disable=SC2206
EXTRA=(${MENU_ARGS:-})
run() { # name screen [args...]
  local name=$1 screen=$2; shift 2
  local log
  log=$("$BIN" --render "$screen" --out "$R/sharexrs/$name.png" --unregistered 0,3 --work-area "$WA" "${EXTRA[@]}" "$@")
  [ ${#EXTRA[@]} -gt 0 ] && grep '^popup' <<<"$log" | sed "s/^/  /" >&2
  printf '%-44s %s\n' "$name" "$(python scripts/compare.py "$R/sharex/$name.png" "$R/sharexrs/$name.png" "$R/compare/$name" | sed -n 3p)"
}
for m in capture upload workflows tools after-capture-tasks after-upload-tasks destinations debug; do
  run "main-$m-menu" "main-$m-menu"
done
for m in capture tools destinations after-capture-tasks; do
  run "main-$m-menu-light" "main-$m-menu" --theme light
done
run main-capture-menu-hover main-capture-menu --hover 300,150
run main-capture-menu-delay main-capture-menu --hover 300,379
run main-capture-menu-delay-sub main-capture-menu --open 12 --hover 638,415
run main-capture-menu-delay-sub-light main-capture-menu --open 12 --hover 638,415 --theme light
run main-capture-menu-monitor main-capture-menu --hover 300,122 --monitors 1024x768
run main-after-capture-tasks-menu-effects main-after-capture-tasks-menu --hover 300,162
run main-destinations-menu-image main-destinations-menu --hover 300,251
run main-tools-menu-hover main-tools-menu --hover 470,314
