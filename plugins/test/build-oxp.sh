#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Builds the test plugin's release file for the system this runs on, the way
# a plugin repository's build-oxp.sh does (docs/plugins/template), so that
# .github/workflows/plugin-release-try.yml can try plugin-release.yml on it
# (#165). Writes plugins/test/dist/test-VERSION-SYSTEM-ARCH.oxp, a universal
# file on macOS, and test-second-... beside it for the second test plugin:
# a repository with several plugins, such as oxiguitar-guitarpro, is
# packaged from several files. The test plugins are never shipped.

set -euo pipefail

cd "$(dirname "$0")"
root="$(cd ../.. && pwd)"
version="$(cargo pkgid)"
version="${version##*[#@]}"
host="$(rustc -vV | sed -n 's/^host: //p')"
mkdir -p dist

build() {
    cargo rustc --release -p "$package" --features oxp --crate-type cdylib "$@"
}

for entry in test:oxiguitar-test-plugin test-second:oxiguitar-test-second-plugin; do
    name="${entry%%:*}"
    package="${entry#*:}"
    library="${package//-/_}"
    case "$host" in
        *apple*)
            out="dist/$name-$version-macos-universal.oxp"
            for target in aarch64-apple-darwin x86_64-apple-darwin; do
                build --target "$target"
            done
            lipo -create -output "$out" \
                "$root/target/aarch64-apple-darwin/release/lib$library.dylib" \
                "$root/target/x86_64-apple-darwin/release/lib$library.dylib"
            ;;
        *windows*)
            arch="${host%%-*}"
            [[ "$arch" == aarch64 ]] && arch=arm64
            out="dist/$name-$version-windows-$arch.oxp"
            build
            cp "$root/target/release/$library.dll" "$out"
            ;;
        *)
            out="dist/$name-$version-linux-${host%%-*}.oxp"
            build
            cp "$root/target/release/lib$library.so" "$out"
            ;;
    esac
    echo "$out"
done
