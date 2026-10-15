#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# NET-5: run an OxiGuitar session under strace and fail if any process tries
# to open an internet connection (IPv4 or IPv6). Local sockets (AF_UNIX) are
# allowed: desktops use them for windows, audio and D-Bus.
#
# Usage: scripts/no-network.sh <path to oxiguitar-cli>
# Needs strace (Debian/Ubuntu: apt install strace).

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

exit "$failed"
