#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the .pkg of one plugin for users without Homebrew: pkgbuild and
# productbuild, with enable_currentUserHome so that it installs into the
# user's own ~/Library/Application Support/OxiGuitar/plugins without a
# password (#165). Runs on macOS. The package is not signed or notarised
# (no Apple fees, Mateus, 1 Nov 2026); a file installed by a .pkg is not
# quarantined, so the plugin loads as it is.
#
# Usage: scripts/plugin-package/macos-pkg.sh NAME VERSION OUT-DIR SRC[:DEST]...
#   SRC   a built .oxp file; DEST the name it is installed under (default
#         NAME.oxp); one SRC:DEST for each plugin of a repository with several.
# Reads PLUGIN_TITLE from the environment.
# Writes: OUT-DIR/oxiguitar-NAME-VERSION-macos-universal.pkg

set -euo pipefail
trap 'echo "::error::macos-pkg.sh line $LINENO: $BASH_COMMAND"' ERR

name="${1:?usage: macos-pkg.sh NAME VERSION OUT-DIR SRC[:DEST]...}"
version="${2:?}"
out_dir="${3:?}"
shift 3
[[ $# -gt 0 ]] || { echo "macos-pkg.sh: no plugin file given" >&2; exit 2; }
title="${PLUGIN_TITLE:-$name}"
# A package version is numbers and dots.
plain="${version%%[-~+]*}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
root="$work/root"
mkdir -p "$root" "$out_dir"
for spec in "$@"; do
    src="${spec%%:*}"
    dest="$name.oxp"
    [[ "$spec" == *:* ]] && dest="${spec#*:}"
    [[ -f "$src" ]] || { echo "macos-pkg.sh: $src isn't a file" >&2; exit 2; }
    install -m644 "$src" "$root/$dest"
done

identifier="io.github.mateusrdp.oxiguitar.$name"
# The install location is relative to the chosen home folder, which
# enable_currentUserHome makes the current user's.
pkgbuild --root "$root" \
    --identifier "$identifier" \
    --version "$plain" \
    --install-location "Library/Application Support/OxiGuitar/plugins" \
    "$work/component.pkg"

cat > "$work/distribution.xml" <<XML
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>OxiGuitar plugin: $title</title>
    <options customize="never" require-scripts="false" hostArchitectures="x86_64,arm64"/>
    <domains enable_anywhere="false" enable_currentUserHome="true" enable_localSystem="false"/>
    <choices-outline>
        <line choice="default"/>
    </choices-outline>
    <choice id="default" title="$title">
        <pkg-ref id="$identifier"/>
    </choice>
    <pkg-ref id="$identifier" version="$plain" onConclusion="none">component.pkg</pkg-ref>
</installer-gui-script>
XML

out="$out_dir/oxiguitar-$name-$version-macos-universal.pkg"
productbuild --distribution "$work/distribution.xml" --package-path "$work" "$out"
pkgutil --payload-files "$out" || true
echo "$out"
