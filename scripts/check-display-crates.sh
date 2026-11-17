#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# The two Linux builds (docs/design/plugins.md; ADR 0011): each turns on its
# own windowing backend in Slint and winit, and not the other one, on every
# Linux target we ship. This checks our choice of features only: what the
# libraries carry inside is theirs (ADR 0011). Reads `cargo tree`; builds
# nothing.
#
# Usage: scripts/check-display-crates.sh

set -euo pipefail
cd "$(dirname "$0")/.."

failed=0
# The features a build of the app turns on in Slint's winit backend and in
# winit, one `crate feature "name"` a line. `$2` is the build's cargo
# options, split into words on purpose.
features() {
    # shellcheck disable=SC2086
    cargo tree --locked -q -p oxiguitar-app -e normal --target "$1" --prefix none --format '{p}|{f}' $2 \
        | awk -F'|' '$1 ~ /^(i-slint-backend-winit|winit) v/ {
            split($1, p, " "); sub(/ \(\*\)$/, "", $2); n = split($2, f, ",")
            for (i = 1; i <= n; i++) printf "%s feature \"%s\"\n", p[1], f[i]
        }' | sort -u
}
# Fails unless the build turns on `own` and not `other` in both crates.
check() {
    local target="$1" build="$2" own="$3" other="$4" found
    found="$(features "$target" "$5")"
    for crate in i-slint-backend-winit winit; do
        if ! grep -qxF "$crate feature \"$own\"" <<< "$found"; then
            echo "::error title=Two Linux builds::the $build build for $target doesn't turn on $crate's $own backend"
            failed=1
        fi
        if grep -qxF "$crate feature \"$other\"" <<< "$found"; then
            echo "::error title=Two Linux builds::the $build build for $target turns on $crate's $other backend"
            failed=1
        fi
    done
}

for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
    check "$target" Wayland wayland x11 ""
    check "$target" X11 x11 wayland "--no-default-features --features x11"
    [[ "$failed" -eq 0 ]] && echo "check-display-crates: $target: each build turns on its own backend only"
done

exit "$failed"
