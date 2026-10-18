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
mkdir -p "$corpus"/{guitar_pro_3_5,guitar_pro_6,guitar_pro_7,gpif,any_file,plugin_dispatch}

cp "$fixtures"/gp/*.gp3 "$fixtures"/gp/*.gp4 "$fixtures"/gp/*.gp5 "$corpus/guitar_pro_3_5/"
cp "$fixtures"/gp/*.gpx "$corpus/guitar_pro_6/"
cp "$fixtures"/gp/*.gp "$fixtures"/reference/*.gp "$corpus/guitar_pro_7/"
cp "$fixtures"/gp/*.gp* "$fixtures"/reference/*.gp "$corpus/any_file/"
cp "$fixtures"/gp/*.gp* "$fixtures"/reference/*.gp "$corpus/plugin_dispatch/"
# The XML inside each .gp file, for the GPIF target.
for f in "$fixtures"/gp/*.gp "$fixtures"/reference/*.gp; do
    unzip -p "$f" Content/score.gpif > "$corpus/gpif/$(basename "$f" .gp).gpif"
done
for t in "$corpus"/*/; do
    echo "$(basename "$t"): $(ls "$t" | wc -l) seed(s)"
done
