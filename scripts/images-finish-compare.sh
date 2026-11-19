#!/usr/bin/env bash
# Renders the background remover and the Image beautifier states with
# `sharexrs --render` at the size of ShareX's `tools-images` shots
# (docs/REFERENCE-SHOTS.md) and compares them. The four synthetic ONNX
# models of tools/tools-harness/make_bg_models.py that ShareX's shots used
# are copied into target/shots/images-finish/work (the reference folder is
# only read); photo.png is read in place.
#   scripts/images-finish-compare.sh [dark|light ...]
# The compared state runs a model: ONNX Runtime from ORT_DYLIB_PATH
# (ShareX's own onnxruntime.dll by default).
set -euo pipefail
cd "$(dirname "$0")/.."
REF=G:/scripts/sharexrs-ref/shots/sharex/tools-images
IN=G:/scripts/sharexrs-ref/phase2-32616/inputs
OUT=target/shots/images-finish
WORK=$OUT/work
BIN=./target/debug/sharexrs.exe
: "${ORT_DYLIB_PATH:=G:/scripts/sharexrs-ref/ShareX-build/onnxruntime.dll}"
export ORT_DYLIB_PATH
mkdir -p "$WORK/models" "$WORK/no-models" "$WORK/one-model" "$OUT/rs" "$OUT/cmp"
cp "$IN"/bgmodels/*.onnx "$WORK/models/"
cp "$IN/bgmodels/a-nchw-sigmoid.onnx" "$WORK/one-model/"
# The image the shots used, read in place (the window prints its path).
SHOWN='G:\Scripts\sharexrs-ref\phase2-32616\inputs\photo.png'
themes=("$@")
[ ${#themes[@]} -eq 0 ] && themes=(dark light)
run() { # name theme crop(x,y,w,h or -) args...
  local name=$1 theme=$2 crop=$3; shift 3
  local ref="$REF/$name-$theme.png"
  [ -f "$ref" ] || { echo "$name-$theme: no ShareX shot"; return; }
  local out="$OUT/rs/$name-$theme.png"
  "$BIN" --render "$@" --theme "$theme" --out "$out" > "$OUT/rs/$name-$theme.log" 2>&1 || { echo "$name-$theme: render failed"; cat "$OUT/rs/$name-$theme.log"; return; }
  if [ "$crop" != "-" ]; then
    python -c "import sys; from PIL import Image; x,y,w,h=map(int,sys.argv[2].split(',')); Image.open(sys.argv[1]).crop((x,y,x+w,y+h)).save(sys.argv[1])" "$out" "$crop"
  fi
  printf '%-46s ' "$name-$theme"
  local pct; pct=$(python scripts/compare.py "$ref" "$out" "$OUT/cmp/$name-$theme" --no-crop | grep -o '([0-9.]*%)' | head -1)
  if [ "$crop" != "-" ]; then
    # A popup captured as its own window: its translucent edge and corners
    # come out over black. Also the share inside its 2px edge.
    pct="$pct, inside the edge $(python - "$ref" "$out" <<'PY'
import sys
from PIL import Image, ImageChops
a, b = (Image.open(p).convert('RGB') for p in sys.argv[1:3])
w, h = a.size
a, b = a.crop((2, 2, w - 2, h - 2)), b.crop((2, 2, w - 2, h - 2))
# compare.py's measure: the difference's luma over 40.
d = ImageChops.difference(a, b).convert("L")
px = d.load()
n = sum(1 for y in range(d.height) for x in range(d.width) if px[x, y] > 40)
print(f"({100 * n / (d.width * d.height):.2f}%)")
PY
)"
  fi
  echo "$pct"
}
for t in "${themes[@]}"; do
  # The guide button pulses (0.4-1 opacity over 1.2 s): each shot caught it
  # at its own phase, estimated from the shot's fill.
  case $t in dark) e=56; n=83;; *) e=61; n=77;; esac
  run background-remover-empty "$t" - background-remover --width 1028 --height 720 --text "$WORK/no-models" --count $e
  run background-remover-input-no-model "$t" - background-remover --width 1028 --height 720 --text "$WORK/no-models" --count $n --image "$SHOWN"
  run background-remover-ready "$t" - background-remover --width 1028 --height 720 --text "$WORK/one-model" --content CPU --image "$SHOWN"
  run background-remover-compared "$t" - background-remover --width 1028 --height 720 --text "$WORK/one-model" --content CPU --image "$SHOWN" --state compared:99
  # The dark menu shot had the device on Automatic, the light one on CPU
  # (a wider model combo).
  if [ "$t" = dark ]; then
    run background-remover-model-menu "$t" 85,117,447,150 background-remover --width 1028 --height 720 --text "$WORK/models" --state model-menu
  else
    run background-remover-model-menu "$t" 85,117,469,150 background-remover --width 1028 --height 720 --text "$WORK/models" --content CPU --state model-menu
  fi
  run background-remover-model-empty-menu "$t" 85,117,447,14 background-remover --width 1028 --height 720 --text "$WORK/no-models" --state model-empty-menu
  # The Image beautifier (`-ImageBeautifier photo.png`, maximized: a
  # 1024x697 client), as it opens and with 16:9 picked.
  run beautifier-opened "$t" - image-editor --width 1024 --height 697 --image "$SHOWN" --state beautifier
  run beautifier-preset-selected "$t" - image-editor --width 1024 --height 697 --image "$SHOWN" --state beautifier:16:9
done
