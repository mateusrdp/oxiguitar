#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Fills fuzz/corpus/<target>/ with seeds from the public fixtures only
# (fixtures/), never from the private corpus. Run from anywhere; existing
# corpus files are kept, so a corpus carried over from an earlier run grows.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
fixtures="$here/../fixtures"
corpus="$here/corpus"
mkdir -p "$corpus/plugin_tab"

cp "$fixtures"/plugin/*.answer "$corpus/plugin_tab/"
for t in "$corpus"/*/; do
    echo "$(basename "$t"): $(ls "$t" | wc -l) seed(s)"
done
