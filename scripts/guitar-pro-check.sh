#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# scripts/guitar-pro-check.sh [OUT]
#
# Writes the files for checking OxiGuitar's .gp and .gp5 writers in Guitar Pro or any
# other program: every reference song (fixtures/reference/) and the test
# song in every format (fixtures/gp/), each converted to .gp and to .gp5 by this
# build's Guitar Pro plugins, with what each conversion lost in
# OUT/conversions.txt. OUT is fixtures/guitar-pro-check/ by default, the
# copy kept in the repository (see its README.md and CONTRIBUTING.md); the
# test builds attach the same files as "guitar-pro-check".
set -eu
out=${1:-fixtures/guitar-pro-check}
cargo build --release --locked -p oxiguitar-cli
scripts/build-plugins.sh >/dev/null
mkdir -p "$out"
rm -f "$out"/*.gp "$out"/*.gp5
{
  for f in fixtures/reference/*.gp; do
    target/release/oxiguitar-cli convert "$f" "$out/$(basename "$f")"
    target/release/oxiguitar-cli convert "$f" "$out/$(basename "$f" .gp).gp5"
  done
  for f in fixtures/gp/oxiguitar-test-song.*; do
    target/release/oxiguitar-cli convert "$f" "$out/test-song-from-${f##*.}.gp"
    target/release/oxiguitar-cli convert "$f" "$out/test-song-from-${f##*.}.gp5"
  done
} | sed "s|$(pwd)/||g" > "$out/conversions.txt"
echo "$(ls "$out"/*.gp "$out"/*.gp5 | wc -l) files in $out"
