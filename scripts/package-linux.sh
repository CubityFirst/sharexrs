#!/usr/bin/env bash
# Packs a Linux build of ShareXrs as the release's two Linux files: a .deb to
# install from, and a plain tarball for other distributions. Both put FFmpeg,
# ONNX Runtime and the OCR models beside the program, where it looks for them
# (docs/RELEASING.md). Runs on Linux, with cargo-deb installed.
#
#   scripts/package-linux.sh <target triple> <version> <payload dir> <out dir>
#
# <payload dir> is scripts/fetch-bundled.py's output for linux-x64. The
# program is target/<triple>/release/sharexrs (cargo build --release). Writes:
#
#   <out>/ShareXrs-<version>-linux-x64.tar.gz   ShareXrs/{sharexrs, ffmpeg,
#                                               libonnxruntime.so, Models, Licenses}
#   <out>/ShareXrs-<version>-linux-x64.deb      /usr/lib/sharexrs/ (the same),
#                                               /usr/bin/sharexrs, menu entry, icons
#
# Set SHAREXRS_SKIP_DEB=1 to write only the tarball.

set -euo pipefail

if [ $# -ne 4 ]; then
  echo "usage: $0 <target triple> <version> <payload dir> <out dir>" >&2
  exit 2
fi
target=$1 version=$2 payload=$3 out=$4
root=$(cd "$(dirname "$0")/.." && pwd)
bin="$root/target/$target/release/sharexrs"
arch=x64

for f in "$bin" "$payload/ffmpeg" "$payload/libonnxruntime.so" \
  "$payload/Models/PP-OCRv6_small_det/inference.onnx" "$payload/Models/PP-OCRv6_small_rec/inference.onnx" \
  "$payload/Licenses/ShareXrs_license.txt" "$payload/Licenses/FFmpeg_source.txt"; do
  if [ ! -f "$f" ]; then
    echo "error: missing $f" >&2
    exit 1
  fi
done
mkdir -p "$out"

# The tarball: one ShareXrs folder, as the Windows portable zip, without the
# Portable marker (settings go in ~/Documents/ShareXrs, as when installed).
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/ShareXrs"
install -m 755 "$bin" "$work/ShareXrs/sharexrs"
cp -R "$payload/." "$work/ShareXrs/"
chmod 755 "$work/ShareXrs/ffmpeg"
chmod 644 "$work/ShareXrs/libonnxruntime.so"
tar -C "$work" --owner=0 --group=0 --numeric-owner -czf "$out/ShareXrs-$version-linux-$arch.tar.gz" ShareXrs

if [ "${SHAREXRS_SKIP_DEB:-}" = 1 ]; then
  exit 0
fi

# The .deb: crates/sharexrs/Cargo.toml's [package.metadata.deb] takes the
# bundled pieces from build/linux-package.
stage="$root/build/linux-package"
rm -rf "$stage"
mkdir -p "$stage"
cp -R "$payload/." "$stage/"
# The package's version is Cargo.toml's (cargo-deb turns a pre-release's "-"
# into "~", which Debian sorts before the release).
cargo deb --no-build --no-strip -p sharexrs --target "$target" \
  --output "$out/ShareXrs-$version-linux-$arch.deb"
