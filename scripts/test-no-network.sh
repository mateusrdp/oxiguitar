#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# NET-5 on this machine, as CI's "No network" job runs it: builds the
# command-line program and both Linux builds of the app, fetches the
# released Guitar Pro plugins (before the session: the session itself then
# runs with no network under strace), and runs scripts/no-network.sh.
# scripts/test-all.py runs it as its "no network" stage.
#
# Usage: scripts/test-no-network.sh
# Needs strace, weston and Xvfb (Debian/Ubuntu: apt install strace weston
# xvfb), and curl. OXIGUITAR_IGNORE_RUST_VERSION (set by test-all.py's
# --ignore-rust-version) passes --ignore-rust-version to cargo.
set -euo pipefail
cd "$(dirname "$0")/.."
flags=(--locked)
[[ -n "${OXIGUITAR_IGNORE_RUST_VERSION:-}" ]] && flags+=(--ignore-rust-version)
cargo build "${flags[@]}" -p oxiguitar-app --no-default-features --features x11 --target-dir target/x11
cargo build "${flags[@]}" --workspace
cp target/x11/debug/oxiguitar target/debug/oxiguitar-x11
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
scripts/released-plugins.sh "$work/released" guitarpro
mkdir -p target/lib/oxiguitar/plugins
cp "$work/released/files/"*.oxp target/lib/oxiguitar/plugins/
status=0
scripts/no-network.sh target/debug/oxiguitar-cli --wayland target/debug/oxiguitar --x11 target/debug/oxiguitar-x11 || status=$?
# target/lib is the plugins' place only for this check.
rm -rf target/lib
exit "$status"
