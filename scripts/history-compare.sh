#!/bin/sh
# Renders a history/OCR window state and compares it with ShareX's shot.
#   scripts/history-compare.sh <shot name> <screen> [render args...]
# e.g. scripts/history-compare.sh history-advanced-dark history --state advanced
REF=${REF:-G:/scripts/sharexrs-ref/shots/sharex/history}
DB=${DB:-G:/scripts/sharexrs-ref/history-fixture/History.db}
name=$1; screen=$2; shift 2
theme=dark; case $name in *-light*) theme=light;; esac
mkdir -p shots/history
./target/debug/sharexrs --render $screen --db $DB --theme $theme --width 1000 --height 673 --out shots/history/$name.png "$@" >/dev/null &&
  python scripts/compare.py $REF/$name.png shots/history/$name.png shots/history/cmp-$name | sed -n 3p
