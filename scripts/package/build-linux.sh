#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds everything the AppImages hold, on the oldest Ubuntu LTS still in
# standard support (22.04, glibc 2.35), so the programs and plugins run on
# it and on everything newer. A binary links against the glibc of the system
# it is built on: built on 24.04, Rust's standard library takes three weak
# symbols from glibc 2.39 (pidfd_spawnp, pidfd_getpid), and the AppImage
# catalog (appimage.github.io), which tests on 22.04, refuses it. Same Rust,
# same crates; only the system's C headers and libraries are older.
#
# Runs inside the container, as root, with the repository at the working
# directory (.github/workflows/release.yml):
#
#   docker run --rm -v "$PWD:/w" -w /w ubuntu:22.04 scripts/package/build-linux.sh
#
# Writes: target/release/oxiguitar (the Wayland build), oxiguitar-cli,
#         target/x11/release/oxiguitar (the X11 build) and
#         target/lib/oxiguitar/plugins/*.oxp (scripts/build-plugins.sh,
#         without the PipeWire plugin),
#         then gives them back to the user who owns the working directory.
# scripts/package/check-glibc.sh checks the result.

set -euo pipefail

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
    ca-certificates curl git build-essential pkg-config \
    libfontconfig1-dev libxkbcommon-dev libwayland-dev \
    libasound2-dev libpipewire-0.3-dev libclang-dev > /dev/null

# Rust from rustup, like everywhere else we build (Mateus, 24 Oct 2026):
# stable, as dtolnay/rust-toolchain@stable gives the other jobs.
export RUSTUP_HOME=/opt/rustup CARGO_HOME="${CARGO_HOME:-/opt/cargo}"
if ! command -v cargo > /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y -q --profile minimal --default-toolchain stable --no-modify-path
fi
export PATH="$CARGO_HOME/bin:$PATH"
rustc --version
ldd --version | sed -n 1p

export CARGO_TERM_COLOR=always CARGO_INCREMENTAL=0
cargo build --release --locked -p oxiguitar-app -p oxiguitar-cli
cargo build --release --locked -p oxiguitar-app --no-default-features --features x11 --target-dir target/x11
# The PipeWire plugin doesn't build here: cpal 0.18's libspa 0.10 needs
# newer PipeWire headers than 22.04's 0.3.48. The AppImages play through
# ALSA, which PipeWire desktops route to PipeWire (Mateus, 27 Oct 2026);
# the .deb and .rpm, built on 24.04, keep the PipeWire plugin.
scripts/build-plugins.sh --skip audio-pipewire

# Files made in the container belong to root; the runner's user packages,
# tests and caches them.
owner="$(stat -c '%u:%g' .)"
chown -R "$owner" target
# -H: the registry and git folders may be links to a cache kept outside.
for kept in "$CARGO_HOME/registry" "$CARGO_HOME/git"; do
    if [[ -e "$kept" ]]; then chown -RH "$owner" "$kept"; fi
done
