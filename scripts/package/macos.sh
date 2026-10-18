#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds OxiGuitar.app and its disk image from a universal binary. Runs on
# macOS (iconutil, codesign, hdiutil). Used by .github/workflows/release.yml.
#
# Usage: scripts/package/macos.sh VERSION
# Reads:  dist/macos/oxiguitar (universal, from lipo),
#         dist/macos/PlugIns/*.oxp (our plugins, universal, from lipo),
#         dist/THIRD-PARTY-LICENSES.html
# Writes: dist/OxiGuitar-VERSION-macos-universal.dmg
#
# The app is not signed with a developer certificate, only "ad hoc" (no
# identity), which Apple Silicon needs to run it at all; Gatekeeper still
# warns on first launch (see the README).

set -euo pipefail

version="${1:?usage: macos.sh VERSION}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
dist="$root/dist"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The bundle's version must be numbers only: 0.1.0~dev... becomes 0.1.0.
plain="${version%%~*}"

app="$work/dmg/OxiGuitar.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m755 "$dist/macos/oxiguitar" "$app/Contents/MacOS/oxiguitar"
# Our plugins (universal files), until the core and the basic plugins come
# as two downloads (ROADMAP.md, M2's packaging slice).
mkdir -p "$app/Contents/PlugIns"
install -m644 "$dist"/macos/PlugIns/*.oxp "$app/Contents/PlugIns/"
sed "s/@VERSION@/$plain/g" "$root/assets/macos/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"

# The app and document icon, from the placeholder PNGs.
set_dir="$work/OxiGuitar.iconset"
mkdir -p "$set_dir"
png="$root/assets/icon/png"
for s in 16 32 128 256 512; do
    cp "$png/oxiguitar-$s.png" "$set_dir/icon_${s}x${s}.png"
    if [[ $s -lt 512 ]]; then
        cp "$png/oxiguitar-$((s * 2)).png" "$set_dir/icon_${s}x${s}@2x.png"
    fi
done
iconutil -c icns -o "$app/Contents/Resources/OxiGuitar.icns" "$set_dir"

res="$app/Contents/Resources"
cp "$root/LICENSE" "$res/LICENSE.txt"
cp "$root/CHANGELOG.md" "$root/CREDITS.md" "$dist/THIRD-PARTY-LICENSES.html" "$res/"
cp "$root/assets/fonts/bravura/OFL.txt" "$res/Bravura-OFL.txt"
cp "$root/assets/fonts/noto-sans/OFL.txt" "$res/NotoSans-OFL.txt"

# Nested code first (the plugins), then the app around it.
for plugin in "$app"/Contents/PlugIns/*.oxp; do
    codesign --force --sign - "$plugin"
done
codesign --force --deep --sign - "$app"
codesign --verify --verbose "$app"

# The disk image: the app and a link to Applications to drag it onto.
ln -s /Applications "$work/dmg/Applications"
out="$dist/OxiGuitar-$version-macos-universal.dmg"
rm -f "$out"
hdiutil create -volname "OxiGuitar" -srcfolder "$work/dmg" -fs HFS+ -format UDZO -ov "$out"
echo "Built $out"
