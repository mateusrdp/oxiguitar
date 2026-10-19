#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# NET-5: run an OxiGuitar session under strace and fail if any process tries
# to open an internet connection (IPv4 or IPv6). Local sockets (AF_UNIX) are
# allowed: desktops use them for windows, audio and D-Bus.
#
# Usage: scripts/no-network.sh <path to oxiguitar-cli> [--wayland <app>] [--x11 <app>]
# Needs strace (Debian/Ubuntu: apt install strace). With the desktop app's
# builds, it also opens each fixture in each of them: the Wayland build in a
# private headless Weston (apt install weston), the X11 build under Xvfb
# (apt install xvfb). A bare app path after the CLI means an X11 build.

set -euo pipefail

cli="$1"
root="$(cd "$(dirname "$0")/.." && pwd)"
log="$(mktemp)"
trap 'rm -f "$log"' EXIT

trace() {
    : > "$log"
    strace -f -qq -e trace=connect,sendto,sendmsg -o "$log" "$@" > /dev/null || true
    grep -E 'sa_family=AF_INET6?\b' "$log" || return 1
}

# 1. Prove the detector works: a connection to localhost must be caught.
if ! trace bash -c 'exec 3<>/dev/tcp/127.0.0.1/9' 2> /dev/null; then
    echo "no-network: self-test failed, strace did not catch a test connection" >&2
    exit 1
fi
echo "no-network: self-test caught a test connection, as it should"

# 2. The session: every command we have, on every public fixture.
failed=0
session=("version")
while IFS= read -r -d '' file; do
    session+=("info:$file")
done < <(find "$root/fixtures" -type f \( -name '*.gp*' -o -name '*.mscz' -o -name '*.musicxml' \) -print0 2> /dev/null)

for step in "${session[@]}"; do
    if [[ "$step" == info:* ]]; then
        args=(info "${step#info:}")
    else
        args=("$step")
    fi
    if trace "$cli" "${args[@]}"; then
        echo "::error title=NET-5::network attempt during: oxiguitar-cli ${args[*]}"
        failed=1
    else
        echo "no-network: ok   oxiguitar-cli ${args[*]}"
    fi
done

# 3. The desktop app's builds, if given: open each fixture in a window and
# quit after a few seconds (OXIGUITAR_EXIT_AFTER_MS). The Wayland build runs
# in a private headless Weston, the X11 build on a private X server; only
# the app is traced, not the display server.
shift
wayland_app=""
x11_app=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --wayland) wayland_app="$2"; shift 2 ;;
        --x11) x11_app="$2"; shift 2 ;;
        *) x11_app="$1"; shift ;;
    esac
done
servers=()
cleanup() {
    rm -f "$log"
    for pid in "${servers[@]}"; do kill "$pid" 2> /dev/null || true; done
    [[ -n "${runtime:-}" ]] && rm -rf "$runtime"
}
trap cleanup EXIT

run_app() { # run_app LABEL APP ENV...
    local label="$1" app="$2"
    shift 2
    for file in "$root"/fixtures/gp/oxiguitar-test-song.*; do
        : > "$log"
        status=0
        out="$(mktemp)"
        env "$@" OXIGUITAR_EXIT_AFTER_MS=4000 \
            strace -f -qq -e trace=connect,sendto,sendmsg -o "$log" \
            timeout 60 "$app" "$file" > "$out" 2>&1 || status=$?
        if grep -E 'sa_family=AF_INET6?\b' "$log"; then
            echo "::error title=NET-5::network attempt in the $label app with $(basename "$file")"
            failed=1
        elif [[ "$status" -ne 0 ]]; then
            # The app's own output, as one annotation (CI logs cannot be fetched).
            msg="${label} build: $(tail -n 20 "$out")"
            msg="${msg//'%'/'%25'}"
            msg="${msg//$'\r'/'%0D'}"
            msg="${msg//$'\n'/'%0A'}"
            echo "::error title=NET-5 app exit $status on $(basename "$file")::$msg"
            cat "$out" >&2
            failed=1
        else
            echo "no-network: ok   oxiguitar ($label) $(basename "$file")"
        fi
        rm -f "$out"
    done
}

if [[ -n "$wayland_app" ]]; then
    runtime="$(mktemp -d)"
    chmod 700 "$runtime"
    XDG_RUNTIME_DIR="$runtime" weston --backend=headless --renderer=pixman \
        --socket=wayland-77 --width=1280 --height=800 --idle-time=0 > /dev/null 2>&1 &
    servers+=("$!")
    for _ in $(seq 50); do [[ -S "$runtime/wayland-77" ]] && break; sleep 0.1; done
    run_app Wayland "$wayland_app" -u DISPLAY XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=wayland-77
fi
if [[ -n "$x11_app" ]]; then
    Xvfb :77 -nolisten tcp -screen 0 1280x800x24 > /dev/null 2>&1 &
    servers+=("$!")
    sleep 2
    run_app X11 "$x11_app" -u WAYLAND_DISPLAY DISPLAY=:77
fi

exit "$failed"
