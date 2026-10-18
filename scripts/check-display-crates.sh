#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# The two Linux builds (docs/design/plugins.md): the Wayland build must hold
# no X11 crate and the X11 build no Wayland crate, on every Linux target we
# ship. Reads `cargo tree`; builds nothing.
#
# One exception, by necessity: the clipboard crate Slint's winit backend
# uses (arboard) depends on x11rb on every Linux build. On Wayland it is the
# clipboard's fallback through XWayland, where the compositor lacks the
# data-control protocol (GNOME), so it is live code, not dead code; x11rb is
# pure Rust and links no X11 library. It is allowed only through arboard.
#
# Usage: scripts/check-display-crates.sh

set -euo pipefail
cd "$(dirname "$0")/.."

x11='^(x11|x11-dl|x11rb|x11rb-protocol|x11-clipboard|xcb|tiny-xlib|as-raw-xcb-connection|glutin_glx_sys|xkbcommon-x11|winit-x11)$'
wayland='^(wayland-.*|smithay-client-toolkit|smithay-clipboard|sctk-adwaita|calloop-wayland-source|wl-clipboard-rs)$'
allowed_in_wayland='^(x11rb|x11rb-protocol)$'

failed=0
crates() {
    # The crates a build of the app compiles for a target, one name a line.
    cargo tree --locked -q -p oxiguitar-app -e normal,build --target "$1" --prefix none $2 \
        | sed -E 's/ v[0-9].*$//' | sort -u
}

for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
    wl_build="$(crates "$target" "")"
    x11_build="$(crates "$target" "--no-default-features --features x11")"

    bad="$(grep -E "$x11" <<< "$wl_build" | grep -Ev "$allowed_in_wayland" || true)"
    if [[ -n "$bad" ]]; then
        echo "::error title=Two Linux builds::the Wayland build for $target holds X11 crates: ${bad//$'\n'/, }"
        failed=1
    fi
    # The allowed X11 crates only through the clipboard.
    if grep -qx x11rb <<< "$wl_build"; then
        parents="$(cargo tree --locked -q -p oxiguitar-app -e normal --target "$target" -i x11rb --depth 1 --prefix none \
            | sed -E 's/ v[0-9].*$//; s/ \(\*\)$//' | grep -vx x11rb | sort -u)"
        if [[ "$parents" != "arboard" ]]; then
            echo "::error title=Two Linux builds::in the Wayland build for $target, x11rb comes from more than the clipboard: ${parents//$'\n'/, }"
            failed=1
        fi
    fi
    grep -Eq "$wayland" <<< "$wl_build" || {
        echo "::error title=Two Linux builds::the Wayland build for $target holds no Wayland crate"
        failed=1
    }

    bad="$(grep -E "$wayland" <<< "$x11_build" || true)"
    if [[ -n "$bad" ]]; then
        echo "::error title=Two Linux builds::the X11 build for $target holds Wayland crates: ${bad//$'\n'/, }"
        failed=1
    fi
    grep -Eq '^(x11-dl|x11rb)$' <<< "$x11_build" || {
        echo "::error title=Two Linux builds::the X11 build for $target holds no X11 crate"
        failed=1
    }
    [[ "$failed" -eq 0 ]] && echo "check-display-crates: $target: the Wayland and X11 builds keep apart"
done

exit "$failed"
