#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Writes dist/THIRD-PARTY-LICENSES.html: every crate compiled into the
# desktop app and the command-line tool on the release targets, with its
# licence text, plus the bundled fonts. Every release package carries it.
#
# Needs cargo-about (cargo install cargo-about --features cli --locked); configuration in
# about.toml, layout in scripts/package/licenses.hbs.
#
# Usage: scripts/package/licenses.sh

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$root/dist"
cd "$root"
# The app's crates include every crate the command-line tool uses.
cargo about generate --locked --fail \
    --manifest-path crates/oxiguitar-app/Cargo.toml \
    --output-file dist/THIRD-PARTY-LICENSES.html \
    scripts/package/licenses.hbs
echo "Wrote dist/THIRD-PARTY-LICENSES.html"
