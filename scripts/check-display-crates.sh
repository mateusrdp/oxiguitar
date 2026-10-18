#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# The two Linux builds (docs/design/plugins.md): the Wayland build must hold
# no X11 crate and the X11 build no Wayland crate, on every Linux target we
# ship. No exceptions: Wayland comes first, and only the X11 build carries
# X11 code (Mateus, 19 Oct 2026). Reads `cargo tree`; builds nothing.
#
# Usage: scripts/check-display-crates.sh

set -euo pipefail
cd "$(dirname "$0")/.."

x11='^(x11|x11-dl|x11rb|x11rb-protocol|x11-clipboard|xcb|tiny-xlib|as-raw-xcb-connection|glutin_glx_sys|xkbcommon-x11|winit-x11|arboard)$'
wayland='^(wayland-.*|smithay-client-toolkit|smithay-clipboard|sctk-adwaita|calloop-wayland-source|wl-clipboard-rs)$'

failed=0
crates() {
    # The crates a build of the app compiles for a target, one name a line.
    cargo tree --locked -q -p oxiguitar-app -e normal,build --target "$1" --prefix none $2 \
        | sed -E 's/ v[0-9].*$//' | sort -u
}

for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
    wl_build="$(crates "$target" "")"
    x11_build="$(crates "$target" "--no-default-features --features x11")"

    bad="$(grep -E "$x11" <<< "$wl_build" || true)"
    if [[ -n "$bad" ]]; then
        echo "::error title=Two Linux builds::the Wayland build for $target holds X11 crates: ${bad//$'\n'/, }"
        failed=1
    fi
    grep -Eq "$wayland" <<< "$wl_build" || {
        echo "::error title=Two Linux builds::the Wayland build for $target holds no Wayland crate"
        failed=1
    }

    # Keyboard maps are shared; their X11 part only in the X11 build.
    if cargo tree --locked -q -p oxiguitar-app -e features --target "$target" -i xkbcommon-dl 2> /dev/null \
        | grep -q 'xkbcommon-dl feature "x11"'; then
        echo "::error title=Two Linux builds::the Wayland build for $target turns on xkbcommon-dl's X11 support"
        failed=1
    fi

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
