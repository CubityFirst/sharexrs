#!/usr/bin/env bash
# Renders ShareXrs's icon (crates/sharexrs/src/app_icon.rs) into
# assets/ShareXrs.iconset, at the sizes macOS's iconutil wants. The Windows
# .exe icon (build.rs) and Linux icons come from the same files.
#
#   scripts/make-icons.sh [path to a built sharexrs]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
bin=${1:-$here/../target/debug/sharexrs}
set=$here/../assets/ShareXrs.iconset
mkdir -p "$set"
for size in 16 32 128 256 512; do
  "$bin" --render app-icon --width $size --out "$set/icon_${size}x${size}.png"
  "$bin" --render app-icon --width $((size * 2)) --out "$set/icon_${size}x${size}@2x.png"
done
