#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Installs packages on a GitHub Actions Ubuntu runner, and does not hang on
# a mirror that stalls. A runner's mirror sometimes stops answering in the
# middle of `apt-get update` or an install: with nothing to stop it, the step
# sat until its 15-minute limit, failed CI, and with it the Release
# workflow's wait for `ci-success` (19 Nov 2026, twice). An ordinary run takes
# about 3 minutes, so each try here is cut off after 5, a stalled connection
# after 30 seconds, and the whole is tried 3 times.
#
# Usage: scripts/apt-install.sh PACKAGE...

set -euo pipefail

(( $# > 0 )) || { echo "usage: $0 PACKAGE..." >&2; exit 2; }

opts=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)

for attempt in 1 2 3; do
  if sudo timeout 300 apt-get "${opts[@]}" update \
    && sudo timeout 300 apt-get "${opts[@]}" install -y --no-install-recommends "$@"; then
    exit 0
  fi
  echo "::warning::apt did not finish (try $attempt of 3)"
  sleep 10
done
echo "::error::apt failed 3 times: $*"
exit 1
