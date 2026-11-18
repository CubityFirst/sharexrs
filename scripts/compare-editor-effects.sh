#!/usr/bin/env bash
# Renders the image editor's effect browser and effect dialogs
# (`--render image-editor --state effects...|effect:<id>`) and compares them
# with ShareX's shots in G:/scripts/sharexrs-ref/shots/sharex/editor-effects
# (taken with scripts/editor-effects.ps1 in the maximized editor on
# target/edtest/test-640x400.png, from scripts/editor-dialogs-inputs.py).
# ShareX's light shots were taken after the dark ones, so its Recent
# section lists the dialogs opened by then (RECENT).
#   scripts/compare-editor-effects.sh [dark|light ...]
set -euo pipefail
cd "$(dirname "$0")/.."
REF=G:/scripts/sharexrs-ref/shots/sharex/editor-effects
OUT=target/shots/editor-effects
EXE=target/debug/sharexrs.exe
IMG=target/edtest/test-640x400.png
RECENT=gaussian_blur,border,draw_background_image,ascii_art,channel_swap,colorize,rotate_custom_angle,resize_image
mkdir -p "$OUT"
themes=("$@")
[ ${#themes[@]} -eq 0 ] && themes=(dark light)
for theme in "${themes[@]}"; do
  while read -r name state; do
    [ -f "$REF/$name-$theme.png" ] || { echo "$name-$theme: no ShareX shot"; continue; }
    if [ "$theme" = light ] && [[ "$state" == effects* ]]; then state="$state@recent=$RECENT"; fi
    "$EXE" --render image-editor --theme "$theme" --width 1028 --height 720 --image "$IMG" --state "$state" --out "$OUT/$name-$theme.png" >/dev/null 2>&1
    pct=$(python scripts/compare.py "$REF/$name-$theme.png" "$OUT/$name-$theme.png" "$OUT/cmp-$name-$theme" | grep -o '([0-9.]*%)')
    echo "$name $theme $pct"
  done <<LIST
browser effects
search-blur effects:blur
resize-image effect:resize_image
rotate effect:rotate_custom_angle
colorize effect:colorize
channel-swap effect:channel_swap
ascii-art effect:ascii_art
background-image effect:draw_background_image
border effect:border
gaussian-blur effect:gaussian_blur
LIST
done
