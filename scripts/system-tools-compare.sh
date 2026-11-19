#!/usr/bin/env bash
# Renders every System tool window state with `sharexrs --render` at the
# size of ShareX's capture (the `tools-system` shots of
# docs/REFERENCE-SHOTS.md) and compares them. The renders use synthetic
# sources: no clipboard is read, no window listed or changed, no echo sent.
#   scripts/system-tools-compare.sh [suffix]   # suffix "-light" for the light shots
# Everything is written under target/compare/tools-system; the reference
# folder is only read.
set -euo pipefail
SUFFIX="${1:--dark}"
THEME=dark; [ "$SUFFIX" = "-light" ] && THEME=light
REF=G:/scripts/sharexrs-ref/shots/sharex/tools-system
OUT=target/compare/tools-system
IN='G:\scripts\sharexrs-ref\phase2-32616\inputs'
BIN=./target/debug/sharexrs.exe
mkdir -p "$OUT/rs" "$OUT/cmp"
run() { # name screen args...
  local name=$1 screen=$2; shift 2
  local crop=""
  case "$name" in monitor-test-*) crop="--no-crop";; esac
  local ref="$REF/$name$SUFFIX.png"
  local size
  if [ -f "$ref" ]; then
    size=$(python -c "from PIL import Image; im=Image.open(r'$ref'); print(im.size[0], im.size[1])")
    set -- "$@" --width "${size% *}" --height "${size#* }"
  fi
  "$BIN" --render "$screen" "$@" --theme "$THEME" --out "$OUT/rs/$name$SUFFIX.png" > /dev/null 2>&1 || { echo "$name$SUFFIX: render failed"; return; }
  printf '%-44s ' "$name$SUFFIX"
  if [ -f "$ref" ]; then
    python scripts/compare.py "$ref" "$OUT/rs/$name$SUFFIX.png" "$OUT/cmp/$name$SUFFIX" $crop | sed -n 3p
  else
    echo "built from source, no shot"
  fi
}
run clipboard-viewer-empty clipboard-viewer --content empty
run clipboard-viewer-cleared clipboard-viewer --content cleared
run clipboard-viewer-unicode clipboard-viewer --content unicode --text "ShareX reference clipboard fixture"
run clipboard-viewer-raw-format clipboard-viewer --content raw-format --text 0123456789ABCDEF
run clipboard-viewer-file-drop clipboard-viewer --content file-drop --files "$IN\photo.png,$IN\landscape.png"
run clipboard-viewer-image clipboard-viewer --content image --image "$IN\photo.png"
run clipboard-viewer-image-bitmap clipboard-viewer --content image --image "$IN\photo.png" --select 2
run inspect-window-empty inspect-window
run inspect-window-selected inspect-window --state selected
run inspect-window-control inspect-window --state control
run inspect-window-window-menu inspect-window --state window-menu
run inspect-window-pick-window inspect-picker --state window
run inspect-window-pick-control inspect-picker --state control
for s in grayscale rgb gradient pattern controls-hidden gradient-direction pattern-menu motion-direction; do
  run monitor-test-$s monitor-test --state $s
done
# The moving bars were captured 55 px (dark) and 57 px (light) along.
if [ "$THEME" = light ]; then OFF=57; else OFF=55; fi
run monitor-test-motion monitor-test --state motion --count $OFF
run network-monitor-idle network-monitor --state idle
run network-monitor-chart network-monitor --state chart
run network-monitor-outage network-monitor --state outage
run network-monitor-target-menu network-monitor --state target-menu
run network-monitor-interval-menu network-monitor --state interval-menu
run network-monitor-range-menu network-monitor --state range-menu
run borderless-window-empty borderless-window
run borderless-window-selected borderless-window --state selected
run borderless-window-window-menu borderless-window --state window-menu
run borderless-window-toggled borderless-window --state toggled
run borderless-window-not-found borderless-window --state not-found
