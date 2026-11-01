#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the test plugin's release file for the system this runs on, the way
# a plugin repository's build-oxp.sh does (docs/plugins/template), so that
# .github/workflows/plugin-release-try.yml can try plugin-release.yml on it
# (#165). Writes plugins/test/dist/test-VERSION-SYSTEM-ARCH.oxp, a universal
# file on macOS. The test plugin is never shipped.

set -euo pipefail

cd "$(dirname "$0")"
root="$(cd ../.. && pwd)"
version="$(cargo pkgid)"
version="${version##*[#@]}"
host="$(rustc -vV | sed -n 's/^host: //p')"
mkdir -p dist

build() {
    cargo rustc --release -p oxiguitar-test-plugin --features oxp --crate-type cdylib "$@"
}

case "$host" in
    *apple*)
        out="dist/test-$version-macos-universal.oxp"
        for target in aarch64-apple-darwin x86_64-apple-darwin; do
            build --target "$target"
        done
        lipo -create -output "$out" \
            "$root/target/aarch64-apple-darwin/release/liboxiguitar_test_plugin.dylib" \
            "$root/target/x86_64-apple-darwin/release/liboxiguitar_test_plugin.dylib"
        ;;
    *windows*)
        arch="${host%%-*}"
        [[ "$arch" == aarch64 ]] && arch=arm64
        out="dist/test-$version-windows-$arch.oxp"
        build
        cp "$root/target/release/oxiguitar_test_plugin.dll" "$out"
        ;;
    *)
        out="dist/test-$version-linux-${host%%-*}.oxp"
        build
        cp "$root/target/release/liboxiguitar_test_plugin.so" "$out"
        ;;
esac
echo "$out"
