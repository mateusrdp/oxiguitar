#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Installs packages on a GitHub Actions Ubuntu runner without letting a bad
# mirror fail the job.
#
# Two things go wrong with a runner's mirror. It stops answering in the
# middle of `apt-get update` or an install (19 Nov 2026, twice), or it
# answers at a crawl: on 24 Nov 2026 the NET-5 job got 140 kB/s for its 77 MB
# and three tries of 5 minutes each did not fit in the step's 15 minutes.
# The release then stopped on a red `ci-success`.
#
# So:
# - A stalled connection is dropped after 30 seconds (apt's own timeout).
# - Each download (the index, then the packages) is cut off after TRY
#   minutes, and tries go on until the whole has taken BUDGET minutes. What
#   a try downloaded stays in apt's cache, so the next one only fetches the
#   rest. The install itself, from the cache, is never cut off.
# - Before each new try, the mirror that was just used goes to the back of
#   the runner's mirror list (/etc/apt/apt-mirrors.txt), so a slow mirror is
#   not used again while another is listed.
# - No translations are downloaded (they were a fifth of the index).
#
# The step that runs this needs a timeout-minutes above BUDGET (25 for the
# default of 20).
#
# Usage: scripts/apt-install.sh [--recommends] PACKAGE...
#   --recommends also installs the packages' recommended packages.
# Environment: APT_INSTALL_BUDGET (minutes, default 20),
#              APT_INSTALL_TRY (minutes, default 8).

set -euo pipefail

install=(install -y --no-install-recommends)
if [[ "${1:-}" == --recommends ]]; then
  install=(install -y)
  shift
fi
(( $# > 0 )) || { echo "usage: $0 [--recommends] PACKAGE..." >&2; exit 2; }

budget=$(( ${APT_INSTALL_BUDGET:-20} * 60 ))
try=$(( ${APT_INSTALL_TRY:-8} * 60 ))
mirrors=/etc/apt/apt-mirrors.txt
opts=(
  -o Acquire::Retries=3
  -o Acquire::http::Timeout=30
  -o Acquire::https::Timeout=30
  -o Acquire::Languages=none
  -o APT::Keep-Downloaded-Packages=true
)

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

# Seconds the next command may take: what is left of BUDGET, at most TRY.
limit() {
  local left=$(( budget - (SECONDS - start) ))
  (( left > 1 )) || left=1
  (( left < try )) && echo "$left" || echo "$try"
}

start=$SECONDS
attempt=0
while :; do
  attempt=$((attempt + 1))
  # Only the downloads are cut off: a dpkg stopped half way would leave the
  # system for every later try to refuse ("dpkg was interrupted").
  if sudo timeout "$(limit)" apt-get "${opts[@]}" update \
    && sudo timeout "$(limit)" apt-get "${opts[@]}" "${install[@]}" --download-only "$@" \
    && sudo apt-get "${opts[@]}" "${install[@]}" "$@"; then
    (( attempt == 1 )) || echo "::notice::apt finished on try $attempt, after $(( (SECONDS - start) / 60 )) minutes"
    exit 0
  fi
  if (( SECONDS - start + 60 >= budget )); then
    echo "::error::apt did not finish in $(( budget / 60 )) minutes ($attempt tries): $*"
    exit 1
  fi
  echo "::warning::apt did not finish (try $attempt); trying again with the next mirror"
  rotate_mirrors || echo "::warning::could not reorder $mirrors"
  sleep 10
done
