#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Fails if any ELF file given (or found in a folder given) needs a glibc
# newer than MAX, weak references included: the AppImage catalog's test
# (appimage.github.io) refuses an AppImage when any of its files names a
# glibc version its test system lacks, even one only referenced weakly.
# Our Linux builds run on Ubuntu 22.04 (scripts/package/build-linux.sh),
# so MAX is 2.35.
#
# Usage: scripts/package/check-glibc.sh MAX PATH...

set -euo pipefail

max="${1:?usage: check-glibc.sh MAX PATH...}"
shift
[[ $# -gt 0 ]] || { echo "usage: check-glibc.sh MAX PATH..." >&2; exit 2; }

checked=0
failed=0
while IFS= read -r -d '' f; do
    [[ "$(head -c 4 "$f" | od -An -tx1 | tr -d ' \n')" == 7f454c46 ]] || continue
    checked=$((checked + 1))
    newest="$(objdump -T "$f" 2> /dev/null | grep -oE 'GLIBC_[0-9]+(\.[0-9]+)+' \
        | sed 's/^GLIBC_//' | sort -uV | tail -n 1 || true)"
    if [[ -n "$newest" ]] && [[ "$(printf '%s\n%s\n' "$newest" "$max" | sort -V | tail -n 1)" != "$max" ]]; then
        echo "::error::$f needs glibc $newest, newer than $max:" >&2
        objdump -T "$f" | grep -F "(GLIBC_$newest)" >&2 || true
        failed=1
    else
        echo "$f: glibc ${newest:-none} at most"
    fi
done < <(find "$@" -type f -print0)

[[ "$checked" -gt 0 ]] || { echo "::error::no ELF file found in $*" >&2; exit 1; }
exit "$failed"
