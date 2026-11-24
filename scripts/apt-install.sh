#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Installs packages on a GitHub Actions Ubuntu runner without letting a bad
# mirror fail the job.
#
# A runner's mirror is sometimes slow: 140 to 185 kB/s for 75 MB on 24 Nov
# 2026, when tries cut off after a fixed 5 minutes failed CI and the
# v0.21.8 packages three times, although every try had downloaded more. And
# sometimes it stops answering altogether (19 Nov 2026).
#
# So apt is never stopped for being slow, only for being stuck:
# - While apt downloads (the index, then the packages), this watches the
#   bytes in apt's download folders. If they have not grown for STALL
#   seconds, apt is stopped and tried again; as long as they grow, it may
#   take as long as it needs.
# - What a try downloaded stays in apt's cache, so the next one only
#   fetches the rest.
# - Before each new try, the mirror that was just used goes to the back of
#   the runner's mirror list (/etc/apt/apt-mirrors.txt).
# - The install itself, from the cache, is never stopped: a dpkg stopped
#   half way leaves every later try refusing ("dpkg was interrupted").
# - No translations are downloaded (they were a fifth of the index).
#
# Give the step a generous timeout-minutes (60): it is only a last resort.
#
# The same file in every OxiGuitar repository (the core, oxiguitar-packages
# and each plugin): change it in all of them at once.
#
# Usage: scripts/apt-install.sh [--recommends] PACKAGE...
#   --recommends also installs the packages' recommended packages.
# Environment: APT_STALL_SECONDS (default 120), APT_TRIES (stuck tries
# allowed, default 5). APT_CHECK_SECONDS, APT_PAUSE_SECONDS, APT_WATCH_DIRS and
# APT_MIRRORS are for the tests (tests/test-apt-install.sh in oxiguitar-packages).

set -euo pipefail

install=(install -y --no-install-recommends)
if [[ "${1:-}" == --recommends ]]; then
  install=(install -y)
  shift
fi
(( $# > 0 )) || { echo "usage: $0 [--recommends] PACKAGE..." >&2; exit 2; }

stall="${APT_STALL_SECONDS:-120}"
tries="${APT_TRIES:-5}"
check="${APT_CHECK_SECONDS:-10}"
read -r -a watched <<< "${APT_WATCH_DIRS:-/var/lib/apt/lists /var/cache/apt/archives}"
mirrors="${APT_MIRRORS:-/etc/apt/apt-mirrors.txt}"
opts=(
  -o Acquire::Retries=3
  -o Acquire::http::Timeout=30
  -o Acquire::https::Timeout=30
  -o Acquire::Languages=none
  -o APT::Keep-Downloaded-Packages=true
)

# Bytes in apt's download folders (partial files included).
downloaded() {
  sudo du -sb "${watched[@]}" 2> /dev/null | awk '{ s += $1 } END { print s + 0 }'
}

# Runs `apt-get ARGS...` and returns its status, or 124 when its downloads
# did not grow for STALL seconds (apt is then stopped).
watched_apt() {
  sudo apt-get "${opts[@]}" "$@" &
  local apt=$! ticker finished status last now since=$SECONDS
  last="$(downloaded)"
  while :; do
    sleep "$check" &
    ticker=$!
    status=0
    wait -n -p finished "$apt" "$ticker" || status=$?
    if [[ "$finished" == "$apt" ]]; then
      kill "$ticker" 2> /dev/null || true
      wait "$ticker" 2> /dev/null || true
      return "$status"
    fi
    now="$(downloaded)"
    if [[ "$now" != "$last" ]]; then
      last="$now"
      since=$SECONDS
    elif (( SECONDS - since >= stall )); then
      echo "::warning::apt downloaded nothing for $stall seconds: stopping it"
      sudo kill -TERM "$apt" 2> /dev/null || true
      wait "$apt" 2> /dev/null || true
      return 124
    fi
  done
}

# Moves the mirror with the best priority to the back of the list and
# numbers them again. Mirrors without a priority count as last, as apt does.
rotate_mirrors() {
  [[ -f "$mirrors" ]] || return 0
  sudo python3 -I - "$mirrors" <<'EOF'
import sys
path = sys.argv[1]
entries = []
for line in open(path, encoding="utf-8"):
    fields = line.split()
    if not fields or fields[0].startswith("#"):
        continue
    rank = 1 << 30
    meta = []
    for item in fields[1:]:
        if item.startswith("priority:") and item[9:].isdigit():
            rank = int(item[9:])
        else:
            meta.append(item)
    entries.append((rank, len(entries), fields[0], meta))
if len(entries) < 2:
    sys.exit(0)
entries.sort()
entries = entries[1:] + entries[:1]
with open(path, "w", encoding="utf-8") as out:
    for number, (_, _, uri, meta) in enumerate(entries, 1):
        out.write("\t".join([uri, " ".join([f"priority:{number}"] + meta)]) + "\n")
print("apt mirrors, in the order they will be tried:")
for _, _, uri, _ in entries:
    print("  " + uri)
EOF
}

start=$SECONDS
for (( attempt = 1; ; attempt++ )); do
  if watched_apt update && watched_apt "${install[@]}" --download-only "$@"; then
    sudo apt-get "${opts[@]}" "${install[@]}" "$@"
    (( attempt == 1 )) || echo "::notice::apt finished on try $attempt, after $(( (SECONDS - start) / 60 )) minutes"
    exit 0
  fi
  if (( attempt >= tries )); then
    echo "::error::apt failed $attempt times: $*"
    exit 1
  fi
  echo "::warning::apt did not finish (try $attempt of $tries); trying again with the next mirror"
  rotate_mirrors || echo "::warning::could not reorder $mirrors"
  sleep "${APT_PAUSE_SECONDS:-10}"
done
