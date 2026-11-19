#!/usr/bin/env bash
# Renders every Data tool window state with `sharexrs --render` at the size
# of ShareX's capture (the `tools-data` shots of docs/REFERENCE-SHOTS.md,
# made from the generated inputs in phase2-32616/inputs) and compares them.
#   scripts/data-tools-compare.sh [suffix]   # suffix "-light" for the light shots
# Everything is written under target/compare/tools-data; the reference
# folder is only read (inputs are read in place, never written: the
# metadata `stripped` state strips a temp copy).
set -euo pipefail
SUFFIX="${1:--dark}"
THEME=dark; [ "$SUFFIX" = "-light" ] && THEME=light
REF=G:/scripts/sharexrs-ref/shots/sharex/tools-data
OUT=target/compare/tools-data
IN='G:\scripts\sharexrs-ref\phase2-32616\inputs'
BIN=./target/debug/sharexrs.exe
mkdir -p "$OUT/rs" "$OUT/cmp"
run() { # name screen args...
  local name=$1 screen=$2; shift 2
  local ref="$REF/$name$SUFFIX.png"
  local size
  if [ -f "$ref" ]; then
    size=$(python -c "from PIL import Image; im=Image.open(r'$ref'); print(im.size[0], im.size[1])")
    set -- "$@" --width "${size% *}" --height "${size#* }"
  fi
  "$BIN" --render "$screen" "$@" --theme "$THEME" --out "$OUT/rs/$name$SUFFIX.png" > /dev/null 2>&1 || { echo "$name$SUFFIX: render failed"; return; }
  printf '%-44s ' "$name$SUFFIX"
  if [ -f "$ref" ]; then
    python scripts/compare.py "$ref" "$OUT/rs/$name$SUFFIX.png" "$OUT/cmp/$name$SUFFIX" | sed -n 3p
  else
    echo "built from source, no shot"
  fi
}
HELLO="$IN\index-fixture\hello.txt"
# The match shot's second file is spelled with a capital S (as typed).
DUP='G:\Scripts\sharexrs-ref\phase2-32616\inputs\index-fixture\duplicate.txt'
DIFF='G:\Scripts\sharexrs-ref\phase2-32616\inputs\index-fixture\nested\different.txt'
run hash-checker-empty hash-checker
run hash-checker-single-result hash-checker --files "$HELLO"
run hash-checker-compare-empty hash-checker --files "$HELLO" --state compare-empty
run hash-checker-match hash-checker --files "$HELLO,$DUP"
run hash-checker-mismatch hash-checker --files "$HELLO,$DIFF"
run hash-checker-running hash-checker --files "$HELLO" --state running
run hash-checker-algorithm-menu hash-checker --state algorithm-menu
# The light shots came after the dark ones in the same profile, whose
# IndexerSettings kept JSON.
if [ "$THEME" = light ]; then FIRST=json; else FIRST=txt; fi
run index-folder-empty index-folder --content $FIRST
run index-folder-html index-folder --content $FIRST
run index-folder-text index-folder --content txt
run index-folder-xml index-folder --content xml
run index-folder-json index-folder --content json
# The dark menu was opened with HTML chosen.
if [ "$THEME" = light ]; then MENU=json; else MENU=html; fi
run index-folder-format-menu index-folder --content $MENU --state format-menu
run index-folder-generated index-folder --content txt --image "$IN\index-fixture"
run index-folder-generated-html index-folder --content html --image "$IN\index-fixture"
run index-folder-custom-css index-folder --state custom-css --text "$IN\index-fixture\style.css"
MD="$IN\metadata.jpg"
run metadata-empty metadata
run metadata-grouped metadata --image "$MD"
run metadata-filtered metadata --image "$MD" --text "$(python -c "import json;d=json.load(open(r'$REF/metadata-filtered-dark.json',encoding='utf-8-sig'));print(next((c.get('value') or '') for c in d['controls'] if c['type']=='Edit'))")"
run metadata-no-match metadata --image "$MD" --text "$(python -c "import json;d=json.load(open(r'$REF/metadata-no-match-dark.json',encoding='utf-8-sig'));print(next((c.get('value') or '') for c in d['controls'] if c['type']=='Edit'))")"
run metadata-strip-confirm metadata --image "$MD" --state strip-confirm
run metadata-stripped metadata --image "$MD" --state stripped
run metadata-unsupported metadata --image "$IN\invalid.sxb"
PHOTO="$IN\photo.png"
run analyze-image-empty analyze-image
run analyze-image-loaded-unconfigured analyze-image --image "$PHOTO" --state loaded-unconfigured
run analyze-image-prompt-menu analyze-image --state prompt-menu
run analyze-image-custom-prompt analyze-image --image "$PHOTO" --state custom-prompt --text "$(python -c "import json;d=json.load(open(r'$REF/analyze-image-custom-prompt-dark.json',encoding='utf-8-sig'));print(next((c.get('value') or '') for c in d['controls'] if c['type']=='Edit'))")"
run analyze-image-analyzing analyze-image --image "$PHOTO" --state analyzing
run analyze-image-result analyze-image --image "$PHOTO" --state result
run analyze-image-api-error analyze-image --image "$PHOTO" --state api-error
run analyze-options-openai analyze-options --content openai
run analyze-options-gemini analyze-options --content gemini
run analyze-options-openrouter analyze-options --content openrouter
run analyze-options-openai-legacy analyze-options --content openai-legacy
run analyze-options-provider-menu analyze-options --state provider-menu
run analyze-options-model-menu analyze-options --state model-menu
run analyze-options-reasoning-menu analyze-options --state reasoning-menu
run analyze-options-verbosity-menu analyze-options --state verbosity-menu
run analyze-options-loaded-models analyze-options --state loaded-models
run analyze-options-connection-result analyze-options --state connection-result
run analyze-options-auto-start-options analyze-options --state auto-start-options
QR="$IN\index-fixture\nested\ShareX reference QR fixture.png"
run qr-code-empty qr-code
run qr-code-generated qr-code --text "$(python -c "import json;d=json.load(open(r'$REF/qr-code-generated-light.json',encoding='utf-8-sig'));print(next((c.get('value') or '') for c in d['controls'] if c['type']=='Edit'))")"
run qr-code-no-code qr-code --state scan --image "$PHOTO"
run qr-code-decoded qr-code --state scan --image "$QR"
run qr-code-oversize qr-code --text "$(python -c "print('X'*4000)")"
run qr-code-custom-size qr-code --text "https://example.invalid/sharexrs" --count 256
