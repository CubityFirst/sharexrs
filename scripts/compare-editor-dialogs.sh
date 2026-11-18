#!/usr/bin/env bash
# Renders the image editor's dialogs and states (`--render image-editor
# --state ...`) and compares them with ShareX's shots in
# G:/scripts/sharexrs-ref/shots/sharex/editor-dialogs (taken with
# scripts/editor-open.ps1 on target/edtest/test-640x400.png, 640x400, made by
# scripts/editor-dialogs-inputs.py). ShareX's editor window sat with its
# client area at 8,31 (not maximized) on a 1024x768 monitor for the dialogs
# it centres on the pointer's monitor, hence SHAREXRS_RENDER_SCREEN.
#   scripts/compare-editor-dialogs.sh [dark|light ...]
set -euo pipefail
cd "$(dirname "$0")/.."
REF=G:/scripts/sharexrs-ref/shots/sharex/editor-dialogs
OUT=target/shots/editor-dialogs
EXE=target/debug/sharexrs.exe
IMG=target/edtest/test-640x400.png
mkdir -p "$OUT"
themes=("$@")
[ ${#themes[@]} -eq 0 ] && themes=(dark light)
for theme in "${themes[@]}"; do
  # name state image screen
  while read -r name state img screen; do
    [ -f "$REF/$name-$theme.png" ] || { echo "$name-$theme: no ShareX shot"; continue; }
    args=(--render image-editor --theme "$theme" --width 1028 --height 720 --out "$OUT/$name-$theme.png")
    [ "$state" != "-" ] && args+=(--state "$state")
    [ "$img" != "-" ] && args+=(--image "$img")
    if [ "$screen" != "-" ]; then
      SHAREXRS_RENDER_SCREEN="$screen" "$EXE" "${args[@]}" >/dev/null 2>&1
    else
      "$EXE" "${args[@]}" >/dev/null 2>&1
    fi
    pct=$(python scripts/compare.py "$REF/$name-$theme.png" "$OUT/$name-$theme.png" "$OUT/cmp-$name-$theme" | grep -o '([0-9.]*%)')
    echo "$name $theme $pct"
  done <<EOF
start start - -
new-image new - -
new-image-done new-done - -
insert insert $IMG -8,-31,1024,768
emoji emoji $IMG -8,-31,1024,768
canvas-size canvas-size $IMG -
confirm confirm $IMG -
EOF
done
