#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# NET-5: run an OxiGuitar session under strace and fail if any process tries
# to open an internet connection (IPv4 or IPv6). Local sockets (AF_UNIX) are
# allowed: desktops use them for windows, audio and D-Bus.
#
# Usage: scripts/no-network.sh <path to oxiguitar-cli> [<path to oxiguitar>]
# Needs strace (Debian/Ubuntu: apt install strace). With the desktop app's
# path, it also opens each fixture in the app under Xvfb (apt install xvfb).

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
        echo "no-network: FAIL, network attempt during: oxiguitar-cli ${args[*]}" >&2
        failed=1
    else
        echo "no-network: ok   oxiguitar-cli ${args[*]}"
    fi
done

# 3. The desktop app, if given: open each fixture in a window on a private
# X server and quit after a few seconds (OXIGUITAR_EXIT_AFTER_MS). Only the
# app is traced, not the X server.
app="${2:-}"
if [[ -n "$app" ]]; then
    Xvfb :77 -nolisten tcp -screen 0 1280x800x24 > /dev/null 2>&1 &
    xvfb=$!
    trap 'rm -f "$log"; kill "$xvfb" 2> /dev/null || true' EXIT
    sleep 2
    for file in "$root"/fixtures/gp/oxiguitar-test-song.*; do
        : > "$log"
        status=0
        DISPLAY=:77 WAYLAND_DISPLAY= OXIGUITAR_EXIT_AFTER_MS=4000 \
            strace -f -qq -e trace=connect,sendto,sendmsg -o "$log" \
            timeout 60 "$app" "$file" > /dev/null 2>&1 || status=$?
        if grep -E 'sa_family=AF_INET6?\b' "$log"; then
            echo "no-network: FAIL, network attempt in the app with $(basename "$file")" >&2
            failed=1
        elif [[ "$status" -ne 0 ]]; then
            echo "no-network: FAIL, the app exited with status $status on $(basename "$file")" >&2
            failed=1
        else
            echo "no-network: ok   oxiguitar $(basename "$file")"
        fi
    done
fi

exit "$failed"
