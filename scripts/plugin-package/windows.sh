#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the Windows installer of one plugin (windows.iss: per user, no
# administrator). Runs on Windows from Git Bash, with Inno Setup 6, which
# it installs with Chocolatey when the runner lacks it. Used by
# .github/workflows/plugin-release.yml (#165).
#
# Usage: scripts/plugin-package/windows.sh NAME VERSION ARCH OUT-DIR SRC[:DEST]...
#   ARCH  x86_64 or arm64
#   SRC   a built .oxp file; DEST the name it is installed under (default
#         NAME.oxp). A repository with several plugins, such as
#         oxiguitar-guitarpro, gives one SRC:DEST for each.
# Reads PLUGIN_TITLE and PLUGIN_HOMEPAGE from the environment.
# Writes: OUT-DIR/oxiguitar-NAME-VERSION-windows-ARCH-setup.exe
# (x64 for x86_64.)

set -euo pipefail
trap 'echo "::error::windows.sh line $LINENO: $BASH_COMMAND"' ERR

name="${1:?usage: windows.sh NAME VERSION ARCH OUT-DIR SRC[:DEST]...}"
version="${2:?}"
arch="${3:?}"
out_dir="${4:?}"
shift 4
[[ $# -gt 0 ]] || { echo "windows.sh: no plugin file given" >&2; exit 2; }
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
win() { cygpath -aw "$1"; }
mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd)"

args=(
    "/DName=$name"
    "/DTitle=${PLUGIN_TITLE:-$name}"
    "/DVersion=$plain"
    "/DArch=$inno_arch"
    "/DOutDir=$(win "$out_dir")"
    "/DHomepage=${PLUGIN_HOMEPAGE:-https://github.com/mateusrdp/oxiguitar}"
)
# The script, with a [Files] section of one line for each plugin file.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$here/windows.iss" "$work/plugin.iss"
printf '\n[Files]\n' >> "$work/plugin.iss"
for spec in "$@"; do
    src="${spec%%:*}"
    dest="$name.oxp"
    [[ "$spec" == *:* ]] && dest="${spec#*:}"
    [[ -f "$src" ]] || { echo "windows.sh: $src isn't a file" >&2; exit 2; }
    printf 'Source: "%s"; DestDir: "{app}"; DestName: "%s"; Flags: ignoreversion\n' \
        "$(win "$src")" "$dest" >> "$work/plugin.iss"
done
# Git Bash would turn the /D options into paths; every path is already
# a Windows one.
MSYS2_ARG_CONV_EXCL="*" "$iscc" "${args[@]}" "$(win "$work/plugin.iss")"

built="$out_dir/oxiguitar-$name-$plain-windows-$inno_arch-setup.exe"
[[ -f "$built" ]] || { echo "windows.sh: $built wasn't made" >&2; exit 1; }
# The file keeps the full version in its name, as the other downloads do.
final="$out_dir/oxiguitar-$name-$version-windows-$arch-setup.exe"
[[ "$built" == "$final" ]] || mv "$built" "$final"
echo "$final"
