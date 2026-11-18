#!/usr/bin/env bash
# Renders every Images tool window state with `sharexrs --render` at the size
# ShareX's capture has (scripts/image-tools-shots.ps1) and compares them.
#   scripts/image-tools-compare.sh [suffix]   # suffix "-light" for the light shots
set -euo pipefail
SUFFIX="${1:-}"
THEME=dark; [ "$SUFFIX" = "-light" ] && THEME=light
REF=G:/scripts/sharexrs-ref/shots/sharex
OUT=G:/scripts/sharexrs-ref/tools-ui
IN='G:\Scripts\sharexrs-ref\tools-ui\in'
THREE="$IN\photo.png,$IN\landscape.png,$IN\portrait.png"
BIN=./target/debug/sharexrs.exe
mkdir -p "$OUT/rs" "$OUT/cmp"
run() { # name screen args...
  local name=$1 screen=$2; shift 2
  local ref="$REF/$name$SUFFIX.png"
  [ -f "$ref" ] || { echo "$name: no ShareX shot"; return; }
  local size; size=$(python -c "from PIL import Image; im=Image.open(r'$ref'); print(im.size[0], im.size[1])")
  set -- "$@" --width "${size% *}" --height "${size#* }" --theme "$THEME"
  "$BIN" --render "$screen" "$@" --out "$OUT/rs/$name$SUFFIX.png" > /dev/null
  printf '%-24s ' "$name$SUFFIX"
  python scripts/compare.py "$ref" "$OUT/rs/$name$SUFFIX.png" "$OUT/cmp/$name$SUFFIX" | sed -n 3p
}
run combiner-empty image-combiner
run combiner-three image-combiner --files "$THREE"
run combiner-selected image-combiner --files "$THREE" --select 1
run combiner-horizontal image-combiner --files "$THREE" --select 1 --state horizontal
run splitter-empty image-splitter
run splitter-loaded image-splitter --image "$IN\photo.png" --state 3x2
run thumbnailer-empty image-thumbnailer
run thumbnailer-three image-thumbnailer --files "$THREE"
run resizer-empty image-resizer
run resizer-three image-resizer --files "$THREE"
run resizer-jpeg image-resizer --files "$THREE" --state jpeg
run converter-empty image-converter
for f in png jpeg webp gif bmp; do run converter-$f image-converter --files "$THREE" --state $f; done
run watermark-empty image-watermark
run watermark-text image-watermark --files "$THREE"
run watermark-image image-watermark --files "$THREE" --state image --image "$IN\stamp.png"
run comparer-empty image-comparer
run comparer-one image-comparer --files "$IN\photo.png"
run comparer-slider image-comparer --files "$IN\photo.png,$IN\photo-edited.png"
run comparer-diff image-comparer --files "$IN\photo.png,$IN\photo-edited.png" --state diff
run comparer-identical image-comparer --files "$IN\photo.png,$IN\photo.png"
run icon-empty icon-converter
run icon-loaded icon-converter --image "$IN\alpha.png"
run effects-tool image-effects --image "$IN\photo.png"
run effects-text-watermark image-effects --image "$IN\photo.png" --select 1
run effects-presets image-effects
