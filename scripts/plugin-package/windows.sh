#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the Windows installer of one plugin (windows.iss: per user, no
# administrator). Runs on Windows from Git Bash, with Inno Setup 6, which
# it installs with Chocolatey when the runner lacks it. Used by
# .github/workflows/plugin-release.yml (#165).
#
# Usage: scripts/plugin-package/windows.sh NAME VERSION ARCH OUT-DIR SRC [SRC2:DEST2]
#   ARCH  x86_64 or arm64
# Reads PLUGIN_TITLE and PLUGIN_HOMEPAGE from the environment.
# Writes: OUT-DIR/oxiguitar-NAME-VERSION-windows-ARCH-setup.exe
# (x64 for x86_64.)

set -euo pipefail
trap 'echo "::error::windows.sh line $LINENO: $BASH_COMMAND"' ERR

name="${1:?usage: windows.sh NAME VERSION ARCH OUT-DIR SRC [SRC2:DEST2]}"
version="${2:?}"
arch="${3:?}"
out_dir="${4:?}"
src="${5:?}"
extra="${6:-}"
case "$arch" in
    x86_64) inno_arch=x64 ;;
    arm64 | aarch64) inno_arch=arm64 ;;
    *) echo "windows.sh: no installer architecture for $arch" >&2; exit 2 ;;
esac

# The installer's version is numbers and dots; a pre-release suffix goes.
plain="${version%%[-~+]*}"

iscc="$(command -v iscc || command -v ISCC || true)"
if [[ -z "$iscc" ]]; then
    for candidate in "/c/Program Files (x86)/Inno Setup 6/ISCC.exe" "/c/Program Files/Inno Setup 6/ISCC.exe"; do
        [[ -x "$candidate" ]] && iscc="$candidate"
    done
fi
if [[ -z "$iscc" ]]; then
    choco install innosetup --no-progress -y
    iscc="/c/Program Files (x86)/Inno Setup 6/ISCC.exe"
fi

here="$(cd "$(dirname "$0")" && pwd)"
win() { cygpath -w "$1"; }
mkdir -p "$out_dir"

args=(
    "/DName=$name"
    "/DTitle=${PLUGIN_TITLE:-$name}"
    "/DVersion=$plain"
    "/DArch=$inno_arch"
    "/DSource=$(win "$src")"
    "/DOutDir=$(win "$out_dir")"
    "/DHomepage=${PLUGIN_HOMEPAGE:-https://github.com/mateusrdp/oxiguitar}"
)
if [[ -n "$extra" ]]; then
    args+=("/DSource2=$(win "${extra%%:*}")" "/DDest2=${extra#*:}")
fi
# Git Bash would turn the /D options into paths; every path is already
# a Windows one.
MSYS2_ARG_CONV_EXCL="*" "$iscc" "${args[@]}" "$(win "$here/windows.iss")"

built="$out_dir/oxiguitar-$name-$plain-windows-$inno_arch-setup.exe"
[[ -f "$built" ]] || { echo "windows.sh: $built wasn't made" >&2; exit 1; }
# The file keeps the full version in its name, as the other downloads do.
final="$out_dir/oxiguitar-$name-$version-windows-$arch-setup.exe"
[[ "$built" == "$final" ]] || mv "$built" "$final"
echo "$final"
