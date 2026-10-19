#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Lintian, Debian's package checker, on .deb, .dsc or .changes files: every
# finding is printed, and any error or warning not allowed by name fails.
# Under GitHub Actions each failing finding is also an annotation on the
# check run (pass GITHUB_ACTIONS into the container).
#
# The tags are allowed by filtering lintian's output rather than with
# --suppress-tags, which stops lintian at a name it doesn't know, and tag
# names change between lintian versions.
#
# Usage: scripts/package/lintian.sh [--allow TAG]... FILE...

set -euo pipefail

allowed=()
while [[ $# -gt 0 && "$1" == --allow ]]; do
    allowed+=("$2")
    shift 2
done
[[ $# -gt 0 ]] || { echo "usage: lintian.sh [--allow TAG]... FILE..." >&2; exit 2; }

lintian --version
out="$(mktemp)"
status=0
# --fail-on none: lintian's exit status then only says whether it ran.
lintian --allow-root --display-info --pedantic --tag-display-limit 0 --fail-on none "$@" > "$out" || status=$?
cat "$out"
if [[ "$status" -ne 0 ]]; then
    echo "lintian.sh: lintian failed (exit status $status)" >&2
    exit "$status"
fi

pattern='^$'
if [[ ${#allowed[@]} -gt 0 ]]; then
    pattern="^[EW]: [^:]+: ($(IFS='|'; echo "${allowed[*]}"))( |$)"
fi
failing="$(grep -E '^[EW]: ' "$out" | grep -Ev "$pattern" || true)"
rm -f "$out"
if [[ -n "$failing" ]]; then
    echo
    echo "lintian.sh: errors and warnings not allowed:"
    echo "$failing"
    if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
        sed 's/^/::error title=lintian::/' <<< "$failing"
    fi
    exit 1
fi
echo "lintian.sh: no error or warning beyond the allowed tags (${allowed[*]:-none})"
