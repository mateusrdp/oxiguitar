#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Lists the plugin files in DIR as the SRC:DEST arguments of deb.sh, rpm.sh,
# windows.sh and macos-pkg.sh, one per line: a repository's build script
# writes dist/FILE-VERSION-SYSTEM-ARCH.oxp for each plugin it holds, and
# FILE.oxp is the name it is installed under (docs/design/plugins.md, "One
# file per plugin id"). A build for x86-64-v3 processors,
# FILE-VERSION-linux-x86_64-v3.oxp, is installed as FILE-x86-64-v3.oxp
# beside the first (the NAM plugin). Used by plugin-release.yml (#165).
#
# Usage: scripts/plugin-package/specs.sh DIR

set -euo pipefail

dir="${1:?usage: specs.sh DIR}"
shopt -s nullglob
found=0
for file in "$dir"/*.oxp; do
    base="$(basename "$file")"
    # What comes before the version (a plugin id may have dashes in it).
    stem="$(sed -E 's/-[0-9]+\.[0-9]+\.[0-9]+.*$//' <<< "$base")"
    if [[ "$stem" == "$base" ]]; then
        echo "specs.sh: $base isn't named FILE-VERSION-SYSTEM-ARCH.oxp" >&2
        exit 2
    fi
    if [[ "$base" == *-linux-x86_64-v3.oxp ]]; then
        echo "$file:$stem-x86-64-v3.oxp"
    else
        echo "$file:$stem.oxp"
    fi
    found=1
done
[[ $found -eq 1 ]] || { echo "specs.sh: no .oxp file in $dir" >&2; exit 2; }
