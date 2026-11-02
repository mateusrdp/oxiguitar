#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Checks that a plugin's release holds every file it should, and that
# SHA256SUMS agrees with them: for each of Linux x86_64 and aarch64, Windows
# x86_64 and arm64 and macOS universal a zip, and a .deb and .rpm for each
# Linux processor, a setup .exe for each Windows one, one macOS .pkg
# (#165). A plugin for some systems only (an audio output's) says which in
# PLUGIN_SYSTEMS, such as "linux" (default: "linux windows macos"). Used by plugin-release.yml before the draft is made and by
# plugin-publish.yml before the package repositories are told.
#
# Usage: scripts/plugin-package/check-assets.sh NAME VERSION DIR

set -euo pipefail

name="${1:?usage: check-assets.sh NAME VERSION DIR}"
version="${2:?}"
dir="${3:?}"
rpm_version="${version//-/~}"

systems=" ${PLUGIN_SYSTEMS:-linux windows macos} "
want=(SHA256SUMS)
if [[ "$systems" == *" linux "* ]]; then
    want+=(
        "oxiguitar-$name-$version-linux-x86_64.zip"
        "oxiguitar-$name-$version-linux-aarch64.zip"
        "oxiguitar-${name}_${version}_amd64.deb"
        "oxiguitar-${name}_${version}_arm64.deb"
        "oxiguitar-$name-$rpm_version-1.x86_64.rpm"
        "oxiguitar-$name-$rpm_version-1.aarch64.rpm"
    )
fi
if [[ "$systems" == *" windows "* ]]; then
    want+=(
        "oxiguitar-$name-$version-windows-x86_64.zip"
        "oxiguitar-$name-$version-windows-arm64.zip"
        "oxiguitar-$name-$version-windows-x86_64-setup.exe"
        "oxiguitar-$name-$version-windows-arm64-setup.exe"
    )
fi
if [[ "$systems" == *" macos "* ]]; then
    want+=(
        "oxiguitar-$name-$version-macos-universal.zip"
        "oxiguitar-$name-$version-macos-universal.pkg"
    )
fi

missing=0
for f in "${want[@]}"; do
    if [[ ! -s "$dir/$f" ]]; then
        echo "::error::$f is missing or empty in $dir"
        missing=1
    fi
done
[[ $missing -eq 0 ]] || exit 1

# SHA256SUMS lists every other file, once, and the sums are right.
(cd "$dir" && sha256sum --check --strict SHA256SUMS)
for f in "${want[@]}"; do
    [[ "$f" == SHA256SUMS ]] && continue
    grep -qE "  \*?$f\$" "$dir/SHA256SUMS" || {
        echo "::error::SHA256SUMS doesn't list $f"
        exit 1
    }
done
echo "$name $version: ${#want[@]} files, all present and summed."
