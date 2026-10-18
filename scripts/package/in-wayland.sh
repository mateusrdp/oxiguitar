#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Runs a command as a client of a private, headless Weston (software
# rendering, nothing on screen): for testing the Wayland build of OxiGuitar
# where no desktop runs, as xvfb-run does for the X11 build. Needs weston
# (Debian/Ubuntu/Fedora: the package weston).
#
# Usage: scripts/package/in-wayland.sh COMMAND [ARGS…]
# Exits with the command's status.

set -euo pipefail
[[ $# -gt 0 ]] || { echo "usage: in-wayland.sh COMMAND [ARGS…]" >&2; exit 2; }

runtime="$(mktemp -d)"
chmod 700 "$runtime"
XDG_RUNTIME_DIR="$runtime" weston --backend=headless --renderer=pixman \
    --socket=wayland-test --width=1280 --height=800 --idle-time=0 \
    > "$runtime/weston.log" 2>&1 &
weston=$!
trap 'kill "$weston" 2> /dev/null || true; wait "$weston" 2> /dev/null || true; rm -rf "$runtime"' EXIT
for _ in $(seq 100); do
    [[ -S "$runtime/wayland-test" ]] && break
    kill -0 "$weston" 2> /dev/null || { cat "$runtime/weston.log" >&2; exit 1; }
    sleep 0.1
done
[[ -S "$runtime/wayland-test" ]] || { echo "in-wayland: weston did not start" >&2; cat "$runtime/weston.log" >&2; exit 1; }

status=0
env -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=wayland-test "$@" || status=$?
exit "$status"
