#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Installs OxiGuitar's desktop entry, icons and Guitar Pro file type for the
# current user, so a build from source (or an AppImage) gets its icon on
# Linux and opens .gp files on a double click. On Wayland the compositor takes
# the icon from the desktop entry whose name matches the window's app id
# (io.github.mateusrdp.OxiGuitar), never from the window itself.
#
# Usage: scripts/install-desktop-entry.sh [path to the oxiguitar binary or AppImage]
#        scripts/install-desktop-entry.sh --uninstall
# Default binary: target/release/oxiguitar. Writes only under ~/.local/share.
# The .deb and .rpm packages install the same files system-wide.

set -euo pipefail

id="io.github.mateusrdp.OxiGuitar"
root="$(cd "$(dirname "$0")/.." && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
sizes=(16 24 32 48 64 128 256 512)

if [[ "${1:-}" == "--uninstall" ]]; then
    rm -f "$data/applications/$id.desktop"
    for s in "${sizes[@]}"; do
        rm -f "$data/icons/hicolor/${s}x${s}/apps/$id.png"
    done
    rm -f "$data/icons/hicolor/scalable/apps/$id.svg"
    rm -f "$data/mime/packages/$id.xml"
    command -v update-mime-database > /dev/null && update-mime-database "$data/mime" || true
    touch "$data/icons/hicolor" 2> /dev/null || true
    echo "Removed OxiGuitar's desktop entry, icons and file type from $data"
    exit 0
fi

bin="$(realpath "${1:-$root/target/release/oxiguitar}")"
if [[ ! -x "$bin" ]]; then
    echo "No program at $bin; build it first: cargo build --release -p oxiguitar-app" >&2
    exit 1
fi

for s in "${sizes[@]}"; do
    install -Dm644 "$root/assets/icon/png/oxiguitar-$s.png" \
        "$data/icons/hicolor/${s}x${s}/apps/$id.png"
done
install -Dm644 "$root/assets/icon/oxiguitar-placeholder.svg" \
    "$data/icons/hicolor/scalable/apps/$id.svg"
mkdir -p "$data/applications"
# The entry starts this build, by its full path.
sed "s|^Exec=oxiguitar |Exec=\"$bin\" |" "$root/assets/linux/$id.desktop" \
    > "$data/applications/$id.desktop"

install -Dm644 "$root/assets/linux/$id.xml" "$data/mime/packages/$id.xml"
command -v update-mime-database > /dev/null && update-mime-database "$data/mime" || true

# Tell desktops something changed. No icon cache is written: a stale cache in
# the user's own icon folder can hide icons that other programs add later.
touch "$data/icons/hicolor"
command -v update-desktop-database > /dev/null && update-desktop-database -q "$data/applications" || true

echo "Installed OxiGuitar's desktop entry, icons and file type in $data, starting $bin"
