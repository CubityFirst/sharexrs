#!/usr/bin/env bash
# Packs a macOS build of ShareXrs as ShareXrs.app, then as the release's two
# Mac files: a .dmg to install from, and a zip of the app for the updater,
# which swaps the whole app. Runs on macOS. Adapted from snapr's, which
# learned each line the hard way (docs/MACOS-LEARNINGS.md, section 1).
#
#   scripts/macos-bundle.sh <sharexrs binary> <version> <arch> <out dir> [<payload dir>]
#
# Writes <out dir>/ShareXrs-macos-<arch>.dmg and ShareXrs-macos-<arch>.app.zip.
# Signs with $SHAREXRS_SIGN_IDENTITY (in $SHAREXRS_SIGN_KEYCHAIN) when set,
# ad hoc otherwise.
#
# <payload dir> is scripts/fetch-bundled.py's output for macos-arm64; its
# pieces go where ShareXrs looks for them (docs/RELEASING.md):
#
#   Contents/MacOS/ffmpeg                    beside the program (ShareX's rule)
#   Contents/Frameworks/libonnxruntime.dylib
#   Contents/Resources/Models/...            the OCR models
#   Contents/Resources/Licenses/*.txt

set -euo pipefail

if [ $# -ne 4 ] && [ $# -ne 5 ]; then
  echo "usage: $0 <sharexrs binary> <version> <arch> <out dir> [<payload dir>]" >&2
  exit 2
fi
bin=$1 version=$2 arch=$3 out=$4 payload=${5:-}
here=$(cd "$(dirname "$0")" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
app="$work/ShareXrs.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/sharexrs"
chmod 755 "$app/Contents/MacOS/sharexrs"
# The app icon, rendered by ShareXrs itself into assets/ShareXrs.iconset
# (scripts/make-icons.sh).
iconutil -c icns "$here/../assets/ShareXrs.iconset" -o "$app/Contents/Resources/ShareXrs.icns"

# Everything else bundled; nested code is signed below, before the app.
nested=()
if [ -n "$payload" ]; then
  mkdir -p "$app/Contents/Frameworks"
  install -m 755 "$payload/ffmpeg" "$app/Contents/MacOS/ffmpeg"
  install -m 644 "$payload/libonnxruntime.dylib" "$app/Contents/Frameworks/libonnxruntime.dylib"
  cp -R "$payload/Models" "$payload/Licenses" "$app/Contents/Resources/"
  nested=("$app/Contents/Frameworks/libonnxruntime.dylib" "$app/Contents/MacOS/ffmpeg")
fi

# CFBundleIdentifier is part of what macOS files Screen Recording and
# Accessibility grants under (with the signature, below): it never changes.
# LSUIElement keeps ShareXrs out of the Dock from launch, as a tray (menu
# bar) app; it takes a Dock icon while the main window is open (the
# activation policy, at runtime). The usage descriptions are what macOS
# shows when asking for the microphone and system audio for screen
# recordings; without them it refuses outright.
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>com.sharexrs.app</string>
	<key>CFBundleName</key>
	<string>ShareXrs</string>
	<key>CFBundleDisplayName</key>
	<string>ShareXrs</string>
	<key>CFBundleExecutable</key>
	<string>sharexrs</string>
	<key>CFBundleIconFile</key>
	<string>ShareXrs</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>LSUIElement</key>
	<true/>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSMicrophoneUsageDescription</key>
	<string>ShareXrs records your microphone with the screen when you choose an audio source for screen recordings.</string>
	<key>NSAudioCaptureUsageDescription</key>
	<string>ShareXrs records what's playing with the screen when you choose system audio for screen recordings.</string>
</dict>
</plist>
EOF
plutil -lint "$app/Contents/Info.plist"

# macOS files granted permissions under the signature's designated
# requirement as well as the bundle identifier. An ad hoc signature's
# requirement is the hash of this very build, so every update would lose
# them (snapr 6d6bbe7); signing with the same certificate every release
# makes it name the certificate instead. Without one it falls back to ad
# hoc, which still seals the app (Apple silicon won't run unsigned code).
# FFmpeg and ONNX Runtime are signed first with the same identity (replacing
# their own signatures), as codesign seals nested code by its signature.
identity=${SHAREXRS_SIGN_IDENTITY:--}
for code in ${nested[@]+"${nested[@]}"} "$app"; do
  codesign --force --sign "$identity" ${SHAREXRS_SIGN_KEYCHAIN:+--keychain "$SHAREXRS_SIGN_KEYCHAIN"} "$code"
done
codesign --verify --deep --strict --verbose=2 "$app"
requirement=$(codesign --display --requirements - "$app" 2>&1)
echo "$requirement"
if [ "$identity" != - ] && grep -q cdhash <<<"$requirement"; then
  echo "error: the designated requirement is the build's hash, not the certificate" >&2
  exit 1
fi

mkdir -p "$out"
ditto -c -k --keepParent "$app" "$out/ShareXrs-macos-$arch.app.zip"

# The disk image opens to the app beside a link to Applications to drag it
# onto.
stage="$work/dmg"
mkdir "$stage"
ditto "$app" "$stage/ShareXrs.app"
ln -s /Applications "$stage/Applications"
hdiutil create -volname ShareXrs -srcfolder "$stage" -fs HFS+ -format UDZO -ov \
  "$out/ShareXrs-macos-$arch.dmg"
