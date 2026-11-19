#!/usr/bin/env bash
# Renders every Video tool window state with `sharexrs --render` at the size
# of ShareX's capture (the `tools-video` shots of docs/REFERENCE-SHOTS.md,
# made from the generated inputs in phase2-32616/inputs) and compares them.
#   scripts/video-tools-compare.sh [suffix]   # suffix "-light" for the light shots
# The renders that run FFmpeg take it from SHAREXRS_FFMPEG or the PATH.
set -euo pipefail
SUFFIX="${1:--dark}"
THEME=dark; [ "$SUFFIX" = "-light" ] && THEME=light
REF=G:/scripts/sharexrs-ref/shots/sharex/tools-video
OUT=G:/scripts/sharexrs-ref/tools-video
IN='G:\scripts\sharexrs-ref\phase2-32616\inputs'
FRAMES='G:\Scripts\sharexrs-ref\phase2-32616\inputs\frame-1.png,G:\Scripts\sharexrs-ref\phase2-32616\inputs\frame-2.png,G:\Scripts\sharexrs-ref\phase2-32616\inputs\frame-3.png'
BIN=./target/debug/sharexrs.exe
# ShareX's own FFmpeg, so a conversion gives the same file.
SXFF='G:/scripts/sharexrs-ref/ShareX-22.4.0-portable/ffmpeg.exe'
[ -f "$SXFF" ] && export SHAREXRS_FFMPEG="$SXFF"
mkdir -p "$OUT/rs" "$OUT/cmp"
run() { # name screen args...
  local name=$1 screen=$2; shift 2
  local ref="$REF/$name$SUFFIX.png"
  [ -f "$ref" ] || { echo "$name$SUFFIX: awaiting shot"; return; }
  local size; size=$(python -c "from PIL import Image; im=Image.open(r'$ref'); print(im.size[0], im.size[1])")
  set -- "$@" --width "${size% *}" --height "${size#* }" --theme "$THEME"
  "$BIN" --render "$screen" "$@" --out "$OUT/rs/$name$SUFFIX.png" > /dev/null 2>&1
  printf '%-40s ' "$name$SUFFIX"
  python scripts/compare.py "$ref" "$OUT/rs/$name$SUFFIX.png" "$OUT/cmp/$name$SUFFIX" | sed -n 3p
}
run gif-maker-empty gif-maker
run gif-maker-frames gif-maker --files "$FRAMES"
run gif-maker-selected gif-maker --files "$FRAMES" --select 1
run gif-maker-repeat-count gif-maker --files "$FRAMES" --state repeat-count --count 3
run gif-maker-created gif-maker --files "$FRAMES" --state created --text "$OUT\\created.gif"
run gif-trimmer-empty gif-trimmer
run gif-trimmer-loaded gif-trimmer --image "$IN\\sequence.gif"
run gif-trimmer-trimmed gif-trimmer --image "$IN\\sequence.gif" --state trimmed
run gif-trimmer-crop gif-trimmer --image "$IN\\sequence.gif" --state crop
run gif-trimmer-resize-reencode gif-trimmer --image "$IN\\sequence.gif" --state resize-reencode
run gif-trimmer-saved gif-trimmer --image "$IN\\sequence.gif" --state saved --text "$OUT\\sequence-trimmed.gif"
run gif-trimmer-invalid gif-trimmer --image "$IN\\invalid.gif"
# The light shots came after the dark ones in the same profile, whose
# VideoConverterOptions kept the input.
if [ "$THEME" = light ]; then
  run video-converter-empty video-converter --image "$IN\sample.mp4"
else
  run video-converter-empty video-converter
fi
run video-converter-input video-converter --image "$IN\\sample.mp4"
run video-converter-bitrate video-converter --image "$IN\\sample.mp4" --state bitrate
run video-converter-codec-no-rate-control video-converter --image "$IN\\sample.mp4" --state codec-no-rate-control
run video-converter-encoding video-converter --image "$IN\\sample.mp4" --state encoding --count 40
run video-converter-result video-converter --image "$IN\\sample.mp4" --state result
run video-player-empty video-player --state controls-hidden
# ShareX's controls are a popup window of their own: its client capture
# leaves them out. The paused shot is at 0.75 s.
run video-player-paused video-player --image "$IN\\sample.mp4" --state controls-hidden --count 750
run video-player-controls-hidden video-player --image "$IN\\sample.mp4" --state controls-hidden --count 750
run video-player-muted video-player --image "$IN\\sample.mp4" --state muted
run video-player-invalid video-player --image "$IN\\invalid.mp4" --state invalid
run video-thumbnailer-empty video-thumbnailer
run video-thumbnailer-input video-thumbnailer --image "$IN\\sample.mp4"
run video-thumbnailer-custom-output video-thumbnailer --image "$IN\\sample.mp4" --state custom-output --text "$OUT"
run video-thumbnailer-progress video-thumbnailer --image "$IN\\sample.mp4" --state progress --count 4
run video-trimmer-empty video-trimmer
run video-trimmer-paused video-trimmer --image "$IN\\sample.mp4" --state paused
run video-trimmer-trimmed-copy video-trimmer --image "$IN\\sample.mp4" --state trimmed-copy --count 249
run video-trimmer-reencode video-trimmer --image "$IN\\sample.mp4" --state reencode --count 249
run video-trimmer-exporting video-trimmer --image "$IN\\sample.mp4" --state exporting --count 249
run video-trimmer-invalid video-trimmer --image "$IN\\invalid.mp4"
